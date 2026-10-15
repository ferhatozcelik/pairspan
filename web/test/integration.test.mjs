import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import WebSocket from 'ws';

async function freePort() {
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}
function next(ws, predicate = () => true) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { cleanup(); reject(new Error('WebSocket message timeout')); }, 5000);
    function cleanup() { clearTimeout(timeout); ws.off('message', receive); ws.off('error', fail); }
    function receive(raw) { const value = JSON.parse(String(raw)); if (predicate(value)) { cleanup(); resolve(value); } }
    function fail(error) { cleanup(); reject(error); }
    ws.on('message', receive); ws.on('error', fail);
  });
}
async function connect(url) {
  const ws = new WebSocket(url);
  await new Promise((resolve, reject) => { ws.once('open', resolve); ws.once('error', reject); });
  return ws;
}

test('token pairing, secret, QR payload and ADB relay', async () => {
  const port = await freePort();
  const child = spawn(process.execPath, ['--import', 'tsx', 'src/server.ts'], {
    cwd: new URL('..', import.meta.url).pathname,
    env: { ...process.env, PORT: String(port), GATEWAY_TOKEN: 'test-token', GATEWAY_SECRET: '', GATEWAY_PUBLIC_URL: `wss://example.test` },
    stdio: 'ignore'
  });
  const sockets = [];
  try {
    let ready = false;
    for (let i = 0; i < 60; i++) {
      try { ready = (await (await fetch(`http://127.0.0.1:${port}/health`)).json()).ok; if (ready) break; }
      catch { await new Promise(resolve => setTimeout(resolve, 100)); }
    }
    assert.ok(ready, 'gateway started');
    const home = await fetch(`http://127.0.0.1:${port}/`);
    assert.equal(home.status, 200);
    assert.match(await home.text(), /Pairspan/);
    assert.equal((await fetch(`http://127.0.0.1:${port}/p/`)).status, 200);
    const url = `ws://127.0.0.1:${port}/ws`;
    const denied = await connect(url); sockets.push(denied);
    const errorPromise = next(denied, msg => msg.type === 'error');
    denied.send(JSON.stringify({ type: 'hello', role: 'mac', token: 'wrong' }));
    assert.equal((await errorPromise).code, 'unauthorized');

    const mac = await connect(url); sockets.push(mac);
    const welcomePromise = next(mac, msg => msg.type === 'welcome');
    mac.send(JSON.stringify({ type: 'hello', role: 'mac', token: 'test-token' }));
    const welcome = await welcomePromise;
    assert.match(welcome.pairCode, /^[2-9A-HJKMNP-Z]{4}(-[2-9A-HJKMNP-Z]{4}){4}$/);
    assert.equal(welcome.qrPayload, `pairspan://pair?code=${welcome.pairCode.replaceAll('-', '')}&g=${encodeURIComponent('wss://example.test')}`);
    const heartbeatAck = next(mac, msg => msg.type === 'heartbeat_ack');
    mac.send(JSON.stringify({ type: 'event', event: 'mac_online', payload: {} }));
    await heartbeatAck;

    const phone = await connect(url); sockets.push(phone);
    const phoneWelcome = next(phone, msg => msg.type === 'welcome');
    const rotated = next(mac, msg => msg.type === 'pair_code');
    phone.send(JSON.stringify({ type: 'hello', role: 'phone', pairCode: welcome.pairCode, token: 'test-token' }));
    assert.equal((await phoneWelcome).sessionId, welcome.sessionId);
    assert.notEqual((await rotated).pairCode, welcome.pairCode);
    const tunnel = next(phone, msg => msg.type === 'event' && msg.event === 'adb_tunnel_open');
    mac.send(JSON.stringify({ type: 'event', event: 'adb_tunnel_open', payload: { id: 'tunnel-1' } }));
    assert.equal((await tunnel).payload.id, 'tunnel-1');
    const binary = Buffer.from([0, 1, 127, 255]).toString('base64');
    const dataAtPhone = next(phone, msg => msg.type === 'event' && msg.event === 'adb_tunnel_data');
    mac.send(JSON.stringify({ type: 'event', event: 'adb_tunnel_data', payload: { id: 'tunnel-1', data: binary } }));
    assert.equal((await dataAtPhone).payload.data, binary);
    const dataAtMac = next(mac, msg => msg.type === 'event' && msg.event === 'adb_tunnel_data');
    phone.send(JSON.stringify({ type: 'event', event: 'adb_tunnel_data', payload: { id: 'tunnel-1', data: binary } }));
    assert.equal((await dataAtMac).payload.data, binary);

    const direct = await connect(url); sockets.push(direct);
    const directWelcome = next(direct, msg => msg.type === 'welcome' || msg.type === 'error');
    direct.send(JSON.stringify({ type: 'hello', role: 'phone', direct: true, token: 'test-token' }));
    const joined = await directWelcome;
    assert.equal(joined.type, 'error');
  } finally {
    for (const ws of sockets) ws.terminate();
    child.kill();
  }
});

test('several phones share one desktop and are told apart by device id', async () => {
  const port = await freePort();
  const child = spawn(process.execPath, ['--import', 'tsx', 'src/server.ts'], {
    cwd: new URL('..', import.meta.url).pathname,
    env: { ...process.env, PORT: String(port), GATEWAY_TOKEN: '', GATEWAY_SECRET: '' },
    stdio: 'ignore'
  });
  const sockets = [];
  try {
    for (let i = 0; i < 60; i++) {
      try { if ((await (await fetch(`http://127.0.0.1:${port}/health`)).json()).ok) break; }
      catch { await new Promise(resolve => setTimeout(resolve, 100)); }
    }
    const url = `ws://127.0.0.1:${port}/ws`;
    const clientId = 'c'.repeat(32), deviceA = 'a'.repeat(32), deviceB = 'b'.repeat(32);
    const mac = await connect(url); sockets.push(mac);
    const macWelcome = next(mac, msg => msg.type === 'welcome');
    mac.send(JSON.stringify({ type: 'hello', role: 'mac', clientId }));
    const welcome = await macWelcome;
    assert.equal(welcome.clientId, clientId);

    async function join(deviceId, pairCode) {
      const phone = await connect(url); sockets.push(phone);
      const ok = next(phone, msg => msg.type === 'welcome');
      phone.send(JSON.stringify({ type: 'hello', role: 'phone', deviceId, pairCode }));
      const reply = await ok;
      assert.equal(reply.clientId, clientId);
      assert.equal(reply.deviceId, deviceId);
      return phone;
    }
    const rotatedA = next(mac, msg => msg.type === 'pair_code');
    const phoneA = await join(deviceA, welcome.pairCode);
    const second = (await rotatedA).pairCode;
    const phoneB = await join(deviceB, second);

    const fromB = next(mac, msg => msg.type === 'event' && msg.event === 'phone_heartbeat');
    phoneB.send(JSON.stringify({ type: 'event', event: 'phone_heartbeat', payload: {} }));
    assert.equal((await fromB).deviceId, deviceB);

    let leaked = false;
    phoneA.on('message', raw => { if (JSON.parse(String(raw)).event === 'shell_request') leaked = true; });
    const atB = next(phoneB, msg => msg.type === 'event' && msg.event === 'shell_request');
    mac.send(JSON.stringify({ type: 'event', event: 'shell_request', deviceId: deviceB, payload: { id: '11111111-1111-1111-1111-111111111111', command: 'id', sentAt: Date.now() } }));
    await atB;
    await new Promise(resolve => setTimeout(resolve, 100));
    assert.equal(leaked, false);

    const bye = next(mac, msg => msg.type === 'event' && msg.event === 'phone_bye');
    phoneA.close();
    assert.equal((await bye).deviceId, deviceA);
  } finally {
    for (const ws of sockets) ws.close();
    child.kill();
  }
});

test('a disconnected desktop and phone can resume the same session', async () => {
  const port = await freePort();
  const child = spawn(process.execPath, ['--import', 'tsx', 'src/server.ts'], {
    cwd: new URL('..', import.meta.url).pathname,
    env: { ...process.env, PORT: String(port), GATEWAY_TOKEN: '', GATEWAY_SECRET: '' },
    stdio: 'ignore'
  });
  const sockets = [];
  try {
    for (let i = 0; i < 60; i++) {
      try { if ((await (await fetch(`http://127.0.0.1:${port}/health`)).json()).ok) break; }
      catch { await new Promise(resolve => setTimeout(resolve, 100)); }
    }
    const url = `ws://127.0.0.1:${port}/ws`;
    const clientId = 'c'.repeat(32), deviceId = 'd'.repeat(32);
    const mac = await connect(url); sockets.push(mac);
    const macWelcome = next(mac, msg => msg.type === 'welcome');
    mac.send(JSON.stringify({ type: 'hello', role: 'mac', clientId }));
    const original = await macWelcome;
    const phone = await connect(url); sockets.push(phone);
    const phoneWelcome = next(phone, msg => msg.type === 'welcome');
    phone.send(JSON.stringify({ type: 'hello', role: 'phone', deviceId, pairCode: original.pairCode }));
    const originalPhone = await phoneWelcome;
    await new Promise(resolve => { mac.close(); mac.once('close', resolve); });
    await new Promise(resolve => { phone.close(); phone.once('close', resolve); });

    const earlyPhone = await connect(url); sockets.push(earlyPhone);
    const retry = next(earlyPhone, msg => msg.type === 'error');
    earlyPhone.send(JSON.stringify({ type: 'hello', role: 'phone', deviceId, sessionId: original.sessionId, resumeToken: originalPhone.resumeToken }));
    assert.equal((await retry).retry, true);

    const resumedMac = await connect(url); sockets.push(resumedMac);
    const resumedWelcome = next(resumedMac, msg => msg.type === 'welcome');
    resumedMac.send(JSON.stringify({ type: 'hello', role: 'mac', clientId, sessionId: original.sessionId, resumeToken: original.resumeToken }));
    assert.equal((await resumedWelcome).sessionId, original.sessionId);

    const resumedPhone = await connect(url); sockets.push(resumedPhone);
    const joined = next(resumedPhone, msg => msg.type === 'welcome');
    resumedPhone.send(JSON.stringify({ type: 'hello', role: 'phone', deviceId, sessionId: original.sessionId, resumeToken: originalPhone.resumeToken }));
    assert.equal((await joined).sessionId, original.sessionId);
    const relayed = next(resumedMac, msg => msg.type === 'event' && msg.event === 'phone_heartbeat');
    resumedPhone.send(JSON.stringify({ type: 'event', event: 'phone_heartbeat', payload: {} }));
    assert.equal((await relayed).deviceId, deviceId);
  } finally {
    for (const ws of sockets) ws.terminate();
    child.kill();
  }
});

test('WebRTC signals are scoped, UDP-only, resumable only by the paired member, and pairing is atomic', async () => {
  const port = await freePort();
  const child = spawn(process.execPath, ['--import', 'tsx', 'src/server.ts'], {
    cwd: new URL('..', import.meta.url).pathname,
    env: { ...process.env, PORT: String(port), GATEWAY_TOKEN: '', GATEWAY_SECRET: '', ALLOWED_ORIGINS: 'https://allowed.test' }, stdio: 'ignore'
  });
  const sockets = [];
  try {
    for (let i = 0; i < 60; i++) {
      try { if ((await (await fetch(`http://127.0.0.1:${port}/health`)).json()).ok) break; }
      catch { await new Promise(resolve => setTimeout(resolve, 100)); }
    }
    const url = `ws://127.0.0.1:${port}/ws`;
    const badOrigin = new WebSocket(url, { origin: 'https://evil.test' });
    await new Promise((resolve, reject) => { badOrigin.once('error', resolve); badOrigin.once('open', () => { badOrigin.terminate(); reject(new Error('Untrusted origin accepted')); }); });
    const mac = await connect(url); sockets.push(mac);
    let reply = next(mac, m => m.type === 'welcome');
    mac.send(JSON.stringify({ type: 'hello', role: 'mac', clientId: 'c'.repeat(32) }));
    const welcome = await reply;
    assert.match(welcome.resumeToken, /^[0-9a-f]{64}$/);
    const a = await connect(url), b = await connect(url); sockets.push(a, b);
    const joinedA = next(a, m => m.type === 'welcome' || m.type === 'error');
    const joinedB = next(b, m => m.type === 'welcome' || m.type === 'error');
    a.send(JSON.stringify({ type: 'hello', role: 'phone', deviceId: 'a'.repeat(32), pairCode: welcome.pairCode }));
    b.send(JSON.stringify({ type: 'hello', role: 'phone', deviceId: 'b'.repeat(32), pairCode: welcome.pairCode }));
    const [ra, rb] = await Promise.all([joinedA, joinedB]);
    assert.equal([ra, rb].filter(m => m.type === 'welcome').length, 1, 'one code cannot be consumed concurrently twice');
    const phone = ra.type === 'welcome' ? a : b, member = ra.type === 'welcome' ? ra : rb;
    const streamId = '12345678-1234-1234-1234-123456789012';
    const send = (ws, event, payload, deviceId) => ws.send(JSON.stringify({ type: 'event', event, payload: { streamId, ...payload }, deviceId }));
    reply = next(phone, m => m.event === 'stream_offer');
    send(mac, 'stream_offer', { sdp: 'v=0\r\n' }, member.deviceId);
    assert.equal((await reply).payload.sdp, 'v=0\r\n');
    reply = next(mac, m => m.event === 'stream_answer');
    send(phone, 'stream_answer', { sdp: 'v=0\r\n' });
    assert.equal((await reply).deviceId, member.deviceId);
    reply = next(mac, m => m.event === 'stream_ice');
    send(phone, 'stream_ice', { candidate: { candidate: 'candidate:1 1 udp 123 192.0.2.1 5000 typ host', sdpMid: '0', sdpMLineIndex: 0 } });
    await reply;
    for (const payload of [
      { event: 'stream_offer', data: { sdp: 'v=0' } },
      { event: 'stream_ice', data: { candidate: { candidate: 'candidate:1 1 tcp 123 192.0.2.1 5000 typ host', sdpMid: '0', sdpMLineIndex: 0 } } },
      { event: 'stream_video', data: { pixels: 'not allowed' } }
    ]) {
      reply = next(phone, m => m.type === 'error'); send(phone, payload.event, payload.data);
      assert.equal((await reply).code, 'bad_signal');
    }
    reply = next(mac, m => m.type === 'error'); send(mac, 'stream_offer', { sdp: 'v=0' });
    assert.equal((await reply).code, 'bad_signal', 'stream cannot be broadcast to every phone');
    for (const [deviceId, token] of [[member.deviceId, '0'.repeat(64)], ['f'.repeat(32), member.resumeToken]]) {
      const attacker = await connect(url); sockets.push(attacker);
      reply = next(attacker, m => m.type === 'error');
      attacker.send(JSON.stringify({ type: 'hello', role: 'phone', deviceId, sessionId: welcome.sessionId, resumeToken: token }));
      assert.equal((await reply).code, 'invalid_session');
    }
    // Replacing a connection immediately invalidates its old authority and resume token.
    const replacement = await connect(url); sockets.push(replacement);
    reply = next(replacement, m => m.type === 'welcome');
    replacement.send(JSON.stringify({ type: 'hello', role: 'phone', deviceId: member.deviceId, sessionId: welcome.sessionId, resumeToken: member.resumeToken }));
    assert.notEqual((await reply).resumeToken, member.resumeToken);
    const stale = await connect(url); sockets.push(stale);
    reply = next(stale, m => m.type === 'error');
    stale.send(JSON.stringify({ type: 'hello', role: 'phone', deviceId: member.deviceId, sessionId: welcome.sessionId, resumeToken: member.resumeToken }));
    assert.equal((await reply).code, 'invalid_session');
    const flood = await connect(url); sockets.push(flood);
    const ended = new Promise(resolve => flood.once('close', code => resolve(code)));
    for (let i = 0; i < 1100; i++) flood.send('{}');
    assert.equal(await ended, 4008, 'unbounded inbound queues are rejected');
  } finally { sockets.forEach(ws => ws.terminate()); child.kill(); }
});
