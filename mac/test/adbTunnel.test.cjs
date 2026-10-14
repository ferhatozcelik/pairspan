const { test } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { AdbProxy } = require('../dist-electron/adbTunnel');
class Socket extends EventEmitter {
  destroyed = false;
  writes = [];
  setNoDelay() {}
  destroy() { this.destroyed = true; }
  write(data) { this.writes.push(data.toString()); }
}
test('duplicate connections cannot interrupt an active ADB stream', () => {
  const events = [];
  const proxy = new AdbProxy('phone', message => events.push(message));
  const first = new Socket();
  proxy.accept(first);
  const id = events[0].payload.id;
  proxy.handleEvent('adb_tunnel_ready', { id });
  const duplicate = new Socket();
  proxy.accept(duplicate);
  assert.equal(duplicate.destroyed, true);
  assert.equal(first.destroyed, false);
  assert.equal(events.length, 1);
  first.emit('data', Buffer.from('video'));
  assert.equal(events[1].payload.id, id);
  proxy.handleEvent('adb_tunnel_data', { id, data: Buffer.from('reply').toString('base64') });
  assert.deepEqual(first.writes, ['reply']);
  assert.equal(proxy.hasConnection(), true);
  proxy.stop();
});
test('a closed transport can reconnect and stale callbacks cannot affect its replacement', () => {
  const events = [];
  const proxy = new AdbProxy('phone', message => events.push(message));
  const old = new Socket();
  proxy.accept(old);
  const oldId = events[0].payload.id;
  old.destroy();
  assert.equal(proxy.hasConnection(), false);
  const next = new Socket();
  proxy.accept(next);
  const newId = events[1].payload.id;
  old.emit('close');
  old.emit('data', Buffer.from('stale'));
  proxy.handleEvent('adb_tunnel_close', { id: oldId });
  proxy.handleEvent('adb_tunnel_ready', { id: newId });
  next.emit('data', Buffer.from('new'));
  assert.equal(next.destroyed, false);
  assert.equal(events.length, 3);
  assert.equal(events[2].payload.id, newId);
  proxy.handleEvent('adb_tunnel_close', { id: newId });
  assert.equal(next.destroyed, true);
  assert.equal(proxy.hasConnection(), false);
  proxy.stop();
});
test('data waits for readiness and closing the local socket releases the tunnel', () => {
  const events = [];
  const proxy = new AdbProxy('phone', message => events.push(message));
  const socket = new Socket();
  proxy.accept(socket);
  const id = events[0].payload.id;
  socket.emit('data', Buffer.from('handshake'));
  assert.equal(events.length, 1);
  proxy.handleEvent('adb_tunnel_ready', { id: 'stale' });
  assert.equal(events.length, 1);
  proxy.handleEvent('adb_tunnel_ready', { id });
  assert.equal(Buffer.from(events[1].payload.data, 'base64').toString(), 'handshake');
  socket.destroy();
  socket.emit('close');
  assert.equal(events[2].event, 'adb_tunnel_close');
  assert.equal(events[2].payload.id, id);
  assert.equal(proxy.hasConnection(), false);
  proxy.stop();
});
