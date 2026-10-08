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
