import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { randomBytes, randomInt, randomUUID, timingSafeEqual } from 'node:crypto';
import { createReadStream, existsSync, statSync } from 'node:fs';
import { extname, join, resolve, sep } from 'node:path';
import { type Duplex } from 'node:stream';
import { TLSSocket } from 'node:tls';
import { fileURLToPath } from 'node:url';
import { WebSocketServer, WebSocket } from 'ws';
import { createBackend, type Backend, type SessionRecord } from './backend.js';

const PORT = Number(process.env.PORT || 3000);
const PAIR_TTL_MS = 5 * 60 * 1000;
const GATEWAY_TOKEN = process.env.GATEWAY_TOKEN || process.env.GATEWAY_SECRET || '';
const PUBLIC_URL = (process.env.GATEWAY_PUBLIC_URL || '').trim().replace(/\/(?:ws)?\/?$/, '');

/** `mac` or `phone:<deviceId>`. */
type Role = string;

const LOG_LEVELS = { debug: 10, info: 20, warn: 30, error: 40 } as const;
const LOG_LEVEL = LOG_LEVELS[(process.env.LOG_LEVEL || 'info') as keyof typeof LOG_LEVELS] ?? LOG_LEVELS.info;

/** One JSON object per line on stdout, so `docker logs` or any log shipper can read it. Never log tokens or pairing codes. */
function log(level: keyof typeof LOG_LEVELS, event: string, fields: Record<string, unknown> = {}): void {
  if (LOG_LEVELS[level] < LOG_LEVEL) return;
  const line = JSON.stringify({ t: new Date().toISOString(), level, event, ...fields });
  if (level === 'error' || level === 'warn') console.error(line); else console.log(line);
}

const short = (id: string | undefined): string => (id ? id.slice(0, 8) : '');
const stats = { connections: 0, sessions: 0, pairings: 0, relayedBytes: 0, errors: 0 };
type Kind = 'mac' | 'phone';
const DEVICE_ID = /^[0-9a-f]{32}$/;
const CLIENT_ID = /^[0-9a-f]{32}$/;
const MAX_PHONES = 8;

interface Sock extends WebSocket {
  client?: Client;
  alive?: boolean;
  peerIp?: string;
  gatewayUrl?: string;
  queue?: Promise<void>;
}

interface Client {
  ws: Sock;
  kind: Kind;
  /** Slot role: `mac` or `phone:<deviceId>`. */
  role: Role;
  deviceId?: string;
  sessionId: string;
  connId: string;
}

const phoneRole = (deviceId: string): Role => `phone:${deviceId}`;

/**
 * Sessions, pairing codes and the relay between desktop and phone stay in memory.
 * Clients connected to this instance are kept in `local` and reached without a round trip.
 */
const backend: Backend = createBackend(onBusMessage);
const local = new Map<string, Client>();

/** Pairing tokens are 20 characters from a 31 character alphabet without look-alikes (about 99 bits). */
const TOKEN_ALPHABET = '23456789ABCDEFGHJKMNPQRSTUVWXYZ';
const TOKEN_PATTERN = /^[2-9A-HJKMNP-Z]{20}$/;

const slot = (sessionId: string, role: Role): string => `${sessionId}:${role}`;

/** Shown to people as XXXX-XXXX-XXXX-XXXX-XXXX; QR codes carry the plain token. */
function displayCode(code: string): string {
  return code.match(/.{1,4}/g)?.join('-') ?? code;
}

function normalizeCode(value: string): string {
  return value.toUpperCase().replace(/[^A-Z0-9]/g, '');
}

function qrPayload(gatewayUrl: string, code: string): string {
  return `pairspan://pair?code=${code}&g=${encodeURIComponent(PUBLIC_URL || gatewayUrl)}`;
}

function publicScheme(req: IncomingMessage): 'ws' | 'wss' {
  const value = headerValue(req.headers['x-forwarded-proto']);
  if (value === 'https') return 'wss';
  if (value === 'http') return 'ws';
  return req.socket instanceof TLSSocket && req.socket.encrypted ? 'wss' : 'ws';
}

function headerValue(value: string | string[] | undefined): string {
  const raw = Array.isArray(value) ? value[0] : value;
  return raw?.split(',')[0]?.trim() || '';
}

function peerAddress(req: IncomingMessage): string {
  return headerValue(req.headers['x-forwarded-for']) || req.socket.remoteAddress || 'unknown';
}

function authorized(value: unknown): boolean {
  if (!GATEWAY_TOKEN) return true;
  if (typeof value !== 'string') return false;
  const expected = Buffer.from(GATEWAY_TOKEN);
  const received = Buffer.from(value);
  return expected.length === received.length && timingSafeEqual(expected, received);
}

function send(ws: WebSocket, message: object): void {
  if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(message));
}

async function newPairCode(sessionId: string): Promise<string> {
  for (;;) {
    const code = Array.from({ length: 20 }, () => TOKEN_ALPHABET[randomInt(0, TOKEN_ALPHABET.length)]).join('');
    if (await backend.claimCode(code, sessionId)) return code;
  }
}

/** Bus messages carry either a frame for the local client in a slot or an order to close a replaced one. */
function onBusMessage(sessionId: string, role: Role, raw: string): void {
  const client = local.get(slot(sessionId, role));
  if (!client) return;
  try {
    const message = JSON.parse(raw) as { t: string; msg?: object; except?: string };
    if (message.t === 'send' && message.msg) send(client.ws, message.msg);
    else if (message.t === 'close' && message.except !== client.connId) client.ws.close(4000, 'replaced');
  } catch { /* ignore malformed bus frames */ }
}

/** Delivers to whichever instance holds the peer. Returns how many receivers got it (0 means offline). */
async function deliver(sessionId: string, role: Role, msg: object): Promise<number> {
  const peer = local.get(slot(sessionId, role));
  if (peer) { send(peer.ws, msg); return 1; }
  return backend.publish(sessionId, role, JSON.stringify({ t: 'send', msg }));
}

async function attachLocal(client: Client): Promise<void> {
  const key = slot(client.sessionId, client.role);
  const first = !local.has(key);
  local.set(key, client);
  if (first) await backend.subscribe(client.sessionId, client.role);
}

async function detachLocal(client: Client): Promise<void> {
  const key = slot(client.sessionId, client.role);
  if (local.get(key) !== client) return;
  local.delete(key);
  await backend.unsubscribe(client.sessionId, client.role);
}

async function detach(client: Client): Promise<void> {
  log('info', 'client_left', { kind: client.kind, session: short(client.sessionId), device: short(client.deviceId) });
  await detachLocal(client);
  const record = await backend.getSession(client.sessionId);
  if (!record) return;
  if (client.kind === 'mac' && record.macConn === client.connId) {
    record.macConn = undefined;
    await broadcastPhones(record, { type: 'event', event: 'mac_offline', payload: {} });
  } else if (client.kind === 'phone' && client.deviceId && record.phoneConns[client.deviceId] === client.connId) {
    delete record.phoneConns[client.deviceId];
    await deliver(record.id, 'mac', { type: 'event', event: 'phone_bye', payload: {}, deviceId: client.deviceId });
  } else {
    return;
  }
  if (!record.macConn && Object.keys(record.phoneConns).length === 0) {
    record.emptySince = Date.now();
    await backend.putSession(record);
    log('info', 'session_waiting_reconnect', { session: short(record.id) });
  } else {
    await backend.putSession(record);
  }
}

async function broadcastPhones(record: SessionRecord, msg: object): Promise<void> {
  for (const deviceId of Object.keys(record.phoneConns)) await deliver(record.id, phoneRole(deviceId), msg);
}

async function rotatePairCode(record: SessionRecord): Promise<void> {
  await backend.releaseCode(record.pairCode, record.id);
  record.pairCode = await newPairCode(record.id);
  record.pairExpiresAt = Date.now() + PAIR_TTL_MS;
  await backend.putSession(record);
  await deliver(record.id, 'mac', {
    type: 'pair_code',
    pairCode: displayCode(record.pairCode),
    qrPayload: qrPayload(record.gatewayUrl, record.pairCode),
    expiresAt: record.pairExpiresAt,
  });
}

async function createMacSession(ws: Sock, requestedClientId: unknown, requestedSessionId: unknown): Promise<void> {
  const resumeId = typeof requestedSessionId === 'string' && /^[0-9a-f]{48}$/.test(requestedSessionId) ? requestedSessionId : '';
  const previous = resumeId ? await backend.getSession(resumeId) : null;
  const resumed = previous && previous.clientId === requestedClientId ? previous : null;
  const id = resumed?.id ?? randomBytes(24).toString('hex');
  const clientId = resumed?.clientId ?? (typeof requestedClientId === 'string' && CLIENT_ID.test(requestedClientId) ? requestedClientId : id.slice(0, 32));
  const connId = randomUUID();
  const record: SessionRecord = resumed ? { ...resumed, macConn: connId, emptySince: undefined, gatewayUrl: ws.gatewayUrl || resumed.gatewayUrl } : {
    id,
    pairCode: await newPairCode(id),
    pairExpiresAt: Date.now() + PAIR_TTL_MS,
    gatewayUrl: ws.gatewayUrl || '',
    clientId,
    macConn: connId,
    phoneConns: {},
  };
  const oldMac = local.get(slot(id, 'mac'));
  if (oldMac) oldMac.ws.close(4000, 'replaced');
  await backend.putSession(record);
  if (!resumed) stats.sessions++;
  log('info', resumed ? 'session_resumed' : 'session_opened', { session: short(id), client: short(clientId), peer: ws.peerIp });
  const client: Client = { ws, kind: 'mac', role: 'mac', sessionId: id, connId };
  ws.client = client;
  if (oldMac) local.delete(slot(id, 'mac'));
  await attachLocal(client);
  send(ws, {
    type: 'welcome',
    role: 'mac',
    sessionId: id,
    clientId,
    pairCode: displayCode(record.pairCode),
    qrPayload: qrPayload(record.gatewayUrl, record.pairCode),
    expiresAt: record.pairExpiresAt,
  });
  if (resumed) await broadcastPhones(record, { type: 'event', event: 'mac_online', payload: { clientId } });
}

async function attachPhone(ws: Sock, record: SessionRecord, rotate: boolean, deviceId: string): Promise<void> {
  const role = phoneRole(deviceId);
  const known = deviceId in record.phoneConns;
  if (!known && Object.keys(record.phoneConns).length >= MAX_PHONES) {
    log('warn', 'too_many_devices', { session: short(record.id), device: short(deviceId) });
    send(ws, { type: 'error', code: 'too_many_devices', message: 'Too many phones are connected to this computer.' });
    ws.close(4009, 'too many devices');
    return;
  }
  const client: Client = { ws, kind: 'phone', role, deviceId, sessionId: record.id, connId: randomUUID() };
  const previous = local.get(slot(record.id, role));
  if (previous) previous.ws.close(4000, 'replaced');
  else if (known) await backend.publish(record.id, role, JSON.stringify({ t: 'close', except: client.connId }));
  const before = record.phoneConns[deviceId];
  record.phoneConns[deviceId] = client.connId;
  record.emptySince = undefined;
  await backend.putSession(record);
  ws.client = client;
  if (previous) local.delete(slot(record.id, role));
  await attachLocal(client);

  const macReached = await deliver(record.id, 'mac', { type: 'event', event: 'phone_online', payload: { at: new Date().toISOString() }, deviceId });
  if (!macReached) {
    if (before) record.phoneConns[deviceId] = before; else delete record.phoneConns[deviceId];
    await backend.putSession(record);
    await detachLocal(client);
    send(ws, { type: 'error', code: 'code_expired', message: 'Code not found or expired.' });
    ws.close(4002, 'pair expired');
    return;
  }
  if (rotate) await rotatePairCode(record);
  stats.pairings++;
  log('info', 'phone_joined', { session: short(record.id), device: short(deviceId), resumed: !rotate, phones: Object.keys(record.phoneConns).length, peer: ws.peerIp });
  send(ws, { type: 'welcome', role: 'phone', sessionId: record.id, clientId: record.clientId, deviceId });
}

async function joinPhone(ws: Sock, pairCodeRaw: string, resumeSessionId: string | undefined, requestedDeviceId: unknown): Promise<void> {
  // Phones that predate device ids get a fresh one per connection, so they still work one at a time.
  const deviceId = typeof requestedDeviceId === 'string' && DEVICE_ID.test(requestedDeviceId) ? requestedDeviceId : randomBytes(16).toString('hex');
  if (resumeSessionId && /^[0-9a-f]{48}$/.test(resumeSessionId)) {
    const existing = await backend.getSession(resumeSessionId);
    if (existing?.macConn) {
      await attachPhone(ws, existing, false, deviceId);
      return;
    }
    if (existing) {
      send(ws, { type: 'error', code: 'mac_offline', retry: true, message: 'Computer is reconnecting.' });
      ws.close(4010, 'computer reconnecting');
      return;
    }
  }

  if (!(await backend.allowPairAttempt(ws.peerIp || 'unknown'))) {
    log('warn', 'rate_limited', { peer: ws.peerIp });
    send(ws, { type: 'error', code: 'rate_limited', message: 'Too many pairing attempts. Try again later.' });
    ws.close(4008, 'rate limited');
    return;
  }
  const code = normalizeCode(pairCodeRaw);
  if (!TOKEN_PATTERN.test(code)) {
    log('info', 'pair_rejected', { reason: 'bad_code', peer: ws.peerIp });
    send(ws, { type: 'error', code: 'bad_code', message: 'Invalid pairing code.' });
    ws.close(4001, 'bad pair code');
    return;
  }
  const sessionId = await backend.sessionIdForCode(code);
  const record = sessionId ? await backend.getSession(sessionId) : null;
  if (!record || !record.macConn || record.pairCode !== code || Date.now() > record.pairExpiresAt) {
    log('info', 'pair_rejected', { reason: 'code_expired', peer: ws.peerIp });
    send(ws, { type: 'error', code: 'code_expired', message: 'Code not found or expired.' });
    ws.close(4002, 'pair expired');
    return;
  }
  await attachPhone(ws, record, true, deviceId);
}

async function handleMessage(ws: Sock, raw: string): Promise<void> {
  let message: Record<string, unknown>;
  try { message = JSON.parse(raw) as Record<string, unknown>; }
  catch { send(ws, { type: 'error', code: 'bad_request', message: 'Invalid JSON.' }); return; }

  const type = typeof message.type === 'string' ? message.type : '';

  if (type === 'hello') {
    if (!authorized(message.token ?? message.auth)) {
      log('warn', 'unauthorized', { peer: ws.peerIp });
      send(ws, { type: 'error', code: 'unauthorized', message: 'Gateway authentication failed.' });
      ws.close(4003, 'unauthorized');
      return;
    }
    const role = message.role === 'phone' ? 'phone' : message.role === 'mac' ? 'mac' : null;
    if (!role) { send(ws, { type: 'error', code: 'bad_request', message: 'role is required.' }); return; }
    if (ws.client) return;
    if (role === 'mac') await createMacSession(ws, message.clientId, message.sessionId);
    else await joinPhone(ws, String(message.pairCode || ''), typeof message.sessionId === 'string' ? message.sessionId : undefined, message.deviceId);
    return;
  }

  const client = ws.client;
  if (!client) { send(ws, { type: 'error', code: 'bad_request', message: 'Send hello first.' }); return; }

  if (type === 'rotate_pair' && client.kind === 'mac') {
    const record = await backend.getSession(client.sessionId);
    if (!record) { send(ws, { type: 'error', code: 'no_session', message: 'No session.' }); return; }
    await rotatePairCode(record);
    return;
  }

  if (type === 'event') {
    const event = typeof message.event === 'string' ? message.event : '';
    if (!event || event.length > 64) return;
    const payload = message.payload && typeof message.payload === 'object' ? message.payload as Record<string, unknown> : {};
    if (event === 'mac_online' && client.kind === 'mac') send(ws, { type: 'heartbeat_ack' });

    if (event === 'shell_request') {
      if (client.kind !== 'mac') return;
      const id = typeof payload.id === 'string' ? payload.id : '';
      const command = typeof payload.command === 'string' ? payload.command.trim() : '';
      const sentAt = typeof payload.sentAt === 'number' ? payload.sentAt : 0;
      if (!/^[0-9a-f-]{36}$/i.test(id) || !command || command.length > 512) {
        send(ws, { type: 'error', code: 'bad_request', message: 'Invalid shell_request.' });
        return;
      }
      if (Math.abs(Date.now() - sentAt) > 30000) {
        send(ws, { type: 'error', code: 'bad_request', message: 'shell_request timed out.' });
        return;
      }
    }

    if (event === 'shell_result' && client.kind !== 'phone') return;
    if (event === 'adb_tunnel_open' && client.kind !== 'mac') return;
    if (event === 'adb_tunnel_data') {
      const data = typeof payload.data === 'string' ? payload.data : '';
      if (data.length > 200_000) {
        send(ws, { type: 'error', code: 'bad_request', message: 'adb_tunnel_data is too large.' });
        return;
      }
    }
    if (event === 'adb_tunnel_ready' && client.kind !== 'phone') return;
    if (event === 'adb_tunnel_data') stats.relayedBytes += typeof payload.data === 'string' ? payload.data.length : 0;
    if (event === 'adb_tunnel_open' || event === 'adb_tunnel_ready' || event === 'adb_tunnel_close') {
      log('info', event, { session: short(client.sessionId), from: client.kind, device: short(client.deviceId ?? (typeof message.deviceId === 'string' ? message.deviceId : undefined)), reason: typeof payload.reason === 'string' ? payload.reason.slice(0, 80) : undefined });
    }
    if (client.kind === 'phone') {
      await deliver(client.sessionId, 'mac', { type: 'event', event, payload, deviceId: client.deviceId });
      return;
    }
    const target = typeof message.deviceId === 'string' && DEVICE_ID.test(message.deviceId) ? message.deviceId : '';
    let reached = 0;
    if (target) reached = await deliver(client.sessionId, phoneRole(target), { type: 'event', event, payload });
    else {
      const record = await backend.getSession(client.sessionId);
      if (record) for (const id of Object.keys(record.phoneConns)) reached += await deliver(client.sessionId, phoneRole(id), { type: 'event', event, payload });
    }
    if (!reached && event === 'shell_request') {
      send(ws, { type: 'error', code: 'phone_offline', message: 'Phone is not connected.' });
    }
  }
}

function health(_req: IncomingMessage, res: ServerResponse): void {
  res.writeHead(200, { 'content-type': 'application/json' });
  res.end(JSON.stringify({
    ok: true,
    service: 'pairspan',
    sessions: local.size,
    store: backend.kind,
    stats,
    requestId: randomUUID(),
  }));
}

const DOCS_ROOT = resolve(process.env.DOCS_ROOT || fileURLToPath(new URL('../public/', import.meta.url)));
const DOC_TYPES: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.txt': 'text/plain; charset=utf-8',
  '.md': 'text/markdown; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.woff2': 'font/woff2',
};

function requestPath(req: IncomingMessage): string {
  const raw = req.url || '/';
  try {
    return new URL(raw, 'http://127.0.0.1').pathname.replace(/\/+$/, '') || '/';
  } catch {
    return raw.split('?')[0]?.replace(/\/+$/, '') || '/';
  }
}

function serveDocs(req: IncomingMessage, res: ServerResponse): boolean {
  if (req.method !== 'GET' && req.method !== 'HEAD') return false;
  const path = requestPath(req);
  const rel = path === '/' ? 'index.html' : path.slice(1);
  const file = resolve(DOCS_ROOT, rel);
  if (file !== DOCS_ROOT && !file.startsWith(DOCS_ROOT + sep)) return false;
  let target = file;
  if (existsSync(target) && statSync(target).isDirectory()) target = join(target, 'index.html');
  if (!existsSync(target) || !statSync(target).isFile()) return false;
  const type = DOC_TYPES[extname(target).toLowerCase()] || 'application/octet-stream';
  res.writeHead(200, { 'content-type': type, 'cache-control': 'no-cache' });
  if (req.method === 'HEAD') {
    res.end();
    return true;
  }
  createReadStream(target).pipe(res);
  return true;
}

const wss = new WebSocketServer({ noServer: true, maxPayload: 256 * 1024 });
const upgraded = new WeakSet<IncomingMessage>();

function acceptUpgrade(req: IncomingMessage, socket: Duplex, head: Buffer): void {
  if (upgraded.has(req)) return;
  const path = requestPath(req);
  if (path !== '/ws' && path !== '/') {
    socket.destroy();
    return;
  }
  upgraded.add(req);
  wss.handleUpgrade(req, socket, head, (ws) => {
    wss.emit('connection', ws, req);
  });
}

const server = createServer((req, res) => {
  if (headerValue(req.headers.upgrade).toLowerCase() === 'websocket') {
    acceptUpgrade(req, req.socket, Buffer.alloc(0));
    return;
  }
  if (requestPath(req) === '/health') return health(req, res);
  if (serveDocs(req, res)) return;
  res.writeHead(404, { 'content-type': 'text/plain' });
  res.end('Not found');
});

server.on('upgrade', (req, socket, head) => {
  acceptUpgrade(req, socket, head);
});

wss.on('connection', (socket, req) => {
  const ws = socket as Sock;
  ws.alive = true;
  ws.on('pong', () => { ws.alive = true; });
  ws.peerIp = peerAddress(req);
  stats.connections++;
  log('debug', 'socket_open', { peer: ws.peerIp });
  const host = headerValue(req.headers['x-forwarded-host']) || headerValue(req.headers.host) || `127.0.0.1:${PORT}`;
  ws.gatewayUrl = `${publicScheme(req)}://${host}`;
  ws.queue = Promise.resolve();
  // Handle one frame at a time per socket so relayed ADB data keeps its order.
  ws.on('message', data => {
    ws.queue = (ws.queue ?? Promise.resolve())
      .then(() => handleMessage(ws, String(data)))
      .catch(error => { stats.errors++; log('error', 'handler_failed', { message: error instanceof Error ? error.message : String(error) }); send(ws, { type: 'error', code: 'internal', message: 'Gateway error.' }); });
  });
  const closed = () => {
    const client = ws.client;
    if (client) (ws.queue ?? Promise.resolve()).then(() => detach(client)).catch(() => { /* ignore */ });
  };
  ws.on('close', closed);
  ws.on('error', closed);
});

// Keep idle connections alive through reverse proxies and discard half-open sockets.
setInterval(() => {
  for (const ws of wss.clients as Set<Sock>) {
    if (ws.readyState !== WebSocket.OPEN) continue;
    if (ws.alive === false) { ws.terminate(); continue; }
    ws.alive = false;
    ws.ping();
  }
}, 20_000).unref();

setInterval(async () => {
  await backend.sweep();
  const now = Date.now();
  for (const client of [...local.values()]) {
    if (client.kind !== 'mac') continue;
    try {
      const record = await backend.getSession(client.sessionId);
      if (!record) continue;
      if (now > record.pairExpiresAt) await rotatePairCode(record);
      else await backend.putSession(record);
    } catch { /* retry on the next sweep */ }
  }
}, 30_000).unref();

setInterval(() => log('info', 'stats', { ...stats, sessionsLive: local.size }), 5 * 60_000).unref();

export default server;

server.listen(PORT, '0.0.0.0', () => {
  log('info', 'listening', { port: PORT, logLevel: process.env.LOG_LEVEL || 'info' });
  console.log(`Pairspan listening on http://0.0.0.0:${PORT}  ws://0.0.0.0:${PORT}/ws`);
});
