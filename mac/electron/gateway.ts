import { randomUUID } from 'node:crypto';
import { execFile } from 'node:child_process';
import { createConnection } from 'node:net';
import { networkInterfaces } from 'node:os';
import { AdbProxy } from './adbTunnel';
import { adbLaunch, run as runAdb } from './adb';

export interface PhoneState {
  deviceId: string;
  connected: boolean;
  name: string;
  model: string;
  sdk: number | null;
  lastSeenAt: string | null;
  adbReady: boolean;
  adbDetail: string;
  /** `adb connect` target for this phone, empty while its tunnel is closed. */
  adbProxy: string;
}

export interface CommandState {
  pending: boolean;
  deviceId: string;
  command: string;
  output: string;
  exitCode: number | null;
  error: string | null;
}

export interface GatewayState {
  enabled: boolean;
  status: 'connecting' | 'online' | 'offline';
  error: string | null;
  /** Stable id of this desktop app, shown to the phones it hosts. */
  clientId: string;
  pairCode: string;
  qrPayload: string;
  phones: PhoneState[];
  command: CommandState;
}

interface PhoneEntry {
  state: PhoneState;
  proxy: AdbProxy;
  lastMs: number;
  adbConnectPending: boolean;
  lastAdbConnectMs: number;
  adbGeneration: number;
  /** `<ip>:5555` once the phone answers directly on the local network; the cloud tunnel is stopped then. */
  direct?: string;
  probing: boolean;
  lastProbeMs: number;
}

type Listener = (state: GatewayState) => void;
const listeners = new Set<Listener>();
const emptyCommand: CommandState = { pending: false, deviceId: '', command: '', output: '', exitCode: null, error: null };
let state: GatewayState = { enabled: true, status: 'offline', error: null, clientId: '', pairCode: '', qrPayload: '', phones: [], command: emptyCommand };
export const DEFAULT_GATEWAY_URL = 'https://pairspan.ferhatozcelik.com';
const DEVICE_ID = /^[0-9a-f]{32}$/;
/** Gateways that predate multi-phone support do not tag events, and they only ever carry one phone. */
const LEGACY_DEVICE_ID = '0'.repeat(32);
let settings = { url: DEFAULT_GATEWAY_URL, token: '' };
const phones = new Map<string, PhoneEntry>();
let socket: WebSocket | undefined;
let heartbeat: NodeJS.Timeout | undefined;
let rotation: NodeJS.Timeout | undefined;
let reconnectTimer: NodeJS.Timeout | undefined;
let pendingCommandId = '';
let commandTimeout: NodeJS.Timeout | undefined;
let stopped = true;

function resetAdbConnect(entry: PhoneEntry): void {
  entry.adbGeneration++;
  entry.adbConnectPending = false;
  entry.lastAdbConnectMs = 0;
}

function connectLocalAdb(entry: PhoneEntry): void {
  const port = entry.proxy.getPort();
  const target = entry.direct ?? (port > 0 ? `127.0.0.1:${port}` : '');
  if (!target || entry.adbConnectPending || Date.now() - entry.lastAdbConnectMs < 30000) return;
  entry.adbConnectPending = true;
  entry.lastAdbConnectMs = Date.now();
  const generation = entry.adbGeneration;
  const adb = adbLaunch();
  execFile(adb.file, ['connect', target], { timeout: 15000, maxBuffer: 4096, cwd: adb.cwd, windowsHide: true }, (error, stdout, stderr) => {
    if (generation !== entry.adbGeneration) return;
    entry.adbConnectPending = false;
    const result = `${stdout}\n${stderr}`.trim();
    if (error || !/\b(?:already )?connected to\b/i.test(result)) {
      console.warn(`PAIRSPAN_ADB_CONNECT_FAILED=${result || String(error)}`);
    } else console.log(`PAIRSPAN_ADB_CONNECTED=${target}`);
  });
}

/** True when `ip` belongs to a subnet this computer is attached to, so a phone cannot make us dial arbitrary hosts. */
export function onLocalNetwork(ip: string): boolean {
  const octets = ip.split('.').map(Number);
  if (octets.length !== 4 || octets.some(n => !Number.isInteger(n) || n < 0 || n > 255)) return false;
  const toInt = (parts: number[]): number => ((parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3]) >>> 0;
  const target = toInt(octets);
  for (const list of Object.values(networkInterfaces())) {
    for (const address of list || []) {
      if (address.family !== 'IPv4' || address.internal) continue;
      const mask = toInt(address.netmask.split('.').map(Number));
      if ((toInt(address.address.split('.').map(Number)) & mask) === (target & mask)) return true;
    }
  }
  return false;
}

function reachable(ip: string, port: number): Promise<boolean> {
  return new Promise(resolve => {
    const socket = createConnection({ host: ip, port, timeout: 1500 });
    const done = (ok: boolean) => { socket.destroy(); resolve(ok); };
    socket.once('connect', () => done(true));
    socket.once('timeout', () => done(false));
    socket.once('error', () => done(false));
  });
}

/**
 * Phone and computer on the same network: talk to the phone's ADB directly. That skips the relay in the cloud,
 * which is what makes screen mirroring and file transfers fast. The cloud tunnel stays as the fallback.
 */
async function probeDirect(entry: PhoneEntry, ip: string): Promise<void> {
  if (entry.probing || Date.now() - entry.lastProbeMs < 15000) return;
  entry.probing = true;
  entry.lastProbeMs = Date.now();
  try {
    const ok = await reachable(ip, 5555);
    const target = `${ip}:5555`;
    if (ok && entry.direct !== target && phones.get(entry.state.deviceId) === entry) {
      const tunnelPort = entry.proxy.getPort();
      entry.direct = target;
      resetAdbConnect(entry);
      if (tunnelPort > 0) { void runAdb(['disconnect', `127.0.0.1:${tunnelPort}`], 8000); entry.proxy.stop(); }
      entry.state = { ...entry.state, adbProxy: `adb connect ${target}` };
      publishPhones();
      console.log(`PAIRSPAN_ADB_DIRECT=${target} device=${entry.state.deviceId}`);
      connectLocalAdb(entry);
    } else if (!ok && entry.direct) {
      console.log(`PAIRSPAN_ADB_DIRECT_LOST=${entry.direct}`);
      void runAdb(['disconnect', entry.direct], 8000);
      entry.direct = undefined;
      resetAdbConnect(entry);
      entry.state = { ...entry.state, adbProxy: '' };
      publishPhones();
    }
  } finally { entry.probing = false; }
}

function publish(update: Partial<GatewayState>): void {
  state = { ...state, ...update };
  for (const listener of listeners) listener(state);
}

function publishPhones(): void {
  publish({ phones: [...phones.values()].map(entry => entry.state) });
}

function dropPhone(deviceId: string): void {
  const entry = phones.get(deviceId);
  if (!entry) return;
  resetAdbConnect(entry);
  if (entry.direct) void runAdb(['disconnect', entry.direct], 8000);
  entry.proxy.stop();
  phones.delete(deviceId);
}

function dropAllPhones(): void {
  for (const id of [...phones.keys()]) dropPhone(id);
}

export function snapshot(): GatewayState { return state; }
export function getSettings(): { url: string; token: string } { return { ...settings }; }
export function setClientId(clientId: string): void { publish({ clientId }); }
export function setSettings(next: { url: string; token: string }): void {
  const url = normalizeBaseUrl(next.url);
  settings = { url, token: next.token };
  if (reconnectTimer) clearTimeout(reconnectTimer);
  reconnectTimer = undefined;
  socket?.close();
  socket = undefined;
  dropAllPhones();
  publish({ status: 'offline', pairCode: '', qrPayload: '', phones: [] });
  if (!stopped) connect();
}
export function subscribe(listener: Listener): () => void {
  listeners.add(listener); listener(state);
  return () => { listeners.delete(listener); };
}

function normalizeBaseUrl(raw: string): string {
  if (!raw.trim()) return '';
  let url: URL;
  try { url = new URL(raw.trim()); }
  catch { throw new Error('Enter a valid gateway base URL.'); }
  if (!['http:', 'https:', 'ws:', 'wss:'].includes(url.protocol) || !url.hostname || url.username || url.password
      || url.search || url.hash || !['/', '/ws'].includes(url.pathname)) {
    throw new Error('Enter only the gateway base URL (for example https://example.com).');
  }
  return `${url.protocol}//${url.host}`;
}

function gatewayWsUrl(): string {
  if (!settings.url) return '';
  const base = settings.url.replace(/^http:/, 'ws:').replace(/^https:/, 'wss:');
  return `${base}/ws`;
}

function pairingPayload(code: string): string {
  // The QR carries the plain token; dashes are only for reading it aloud or typing it.
  return `pairspan://pair?code=${code.replace(/[^A-Za-z0-9]/g, '')}&g=${encodeURIComponent(pairingBaseUrl())}`;
}

function pairingBaseUrl(): string {
  if (!settings.url) return '';
  const url = new URL(settings.url);
  if (!['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)) return settings.url;
  const interfaces = networkInterfaces();
  for (const name of ['en0', ...Object.keys(interfaces)]) {
    for (const address of interfaces[name] || []) {
      if (address.family !== 'IPv4' || address.internal) continue;
      url.hostname = address.address;
      return url.origin;
    }
  }
  return settings.url;
}

function send(message: object): void {
  if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
}

function acceptPhone(deviceId: string, payload: Record<string, unknown>): void {
  if (!DEVICE_ID.test(deviceId)) return;
  let entry = phones.get(deviceId);
  if (!entry) {
    entry = {
      state: { deviceId, connected: true, name: 'Android', model: '', sdk: null, lastSeenAt: null, adbReady: false, adbDetail: '', adbProxy: '' },
      proxy: new AdbProxy(deviceId, send), lastMs: 0, adbConnectPending: false, lastAdbConnectMs: 0, adbGeneration: 0, probing: false, lastProbeMs: 0,
    };
    phones.set(deviceId, entry);
  }
  const current = entry.state;
  const name = typeof payload.name === 'string' ? payload.name.slice(0, 80) : current.name;
  const model = typeof payload.model === 'string' ? payload.model.slice(0, 80) : current.model;
  const sdk = typeof payload.sdk === 'number' && Number.isInteger(payload.sdk) ? payload.sdk : current.sdk;
  const adbReady = payload.adbReady === true;
  const adbDetail = typeof payload.adbDetail === 'string' ? payload.adbDetail.slice(0, 100) : current.adbDetail;
  entry.lastMs = Date.now();
  entry.state = { ...current, connected: true, name, model, sdk, lastSeenAt: new Date(entry.lastMs).toISOString(), adbReady, adbDetail };
  const target = entry;
  const ip = typeof payload.ip === 'string' ? payload.ip : '';
  if (adbReady && payload.adbPort === 5555 && onLocalNetwork(ip)) void probeDirect(entry, ip);
  if (adbReady && entry.direct) {
    connectLocalAdb(entry);
  } else if (adbReady) {
    entry.proxy.start(() => {
      target.state = { ...target.state, adbProxy: target.proxy.getConnectHint() };
      publishPhones();
      connectLocalAdb(target);
    });
  } else if (entry.proxy.getPort() > 0 || entry.direct) {
    if (entry.direct) void runAdb(['disconnect', entry.direct], 8000);
    entry.direct = undefined;
    resetAdbConnect(entry);
    entry.proxy.stop();
    entry.state = { ...entry.state, adbProxy: '' };
  }
  publishPhones();
  send({ type: 'event', event: 'mac_ack', payload: { at: new Date().toISOString(), clientId: state.clientId }, deviceId });
}

/** Events without a usable id belong to the only phone there is, which is the case on gateways that predate device ids. */
function resolvePhone(deviceId: string): PhoneEntry | undefined {
  const exact = phones.get(deviceId);
  if (exact || deviceId !== LEGACY_DEVICE_ID) return exact;
  return phones.size === 1 ? [...phones.values()][0] : undefined;
}

function handleMessage(raw: string): void {
  let message: Record<string, unknown>;
  try { message = JSON.parse(raw) as Record<string, unknown>; }
  catch { return; }
  const type = typeof message.type === 'string' ? message.type : '';

  if (type === 'welcome') {
    const pairCode = typeof message.pairCode === 'string' ? message.pairCode : state.pairCode;
    const qrPayload = pairCode ? pairingPayload(pairCode) : '';
    publish({ status: 'online', error: null, pairCode, qrPayload });
    console.log(`PAIRSPAN_PAIR_CODE=${pairCode}`);
    console.log(`PAIRSPAN_QR_PAYLOAD=${qrPayload}`);
    send({ type: 'event', event: 'mac_online', payload: { at: new Date().toISOString(), clientId: state.clientId } });
    return;
  }

  if (type === 'pair_code') {
    const pairCode = typeof message.pairCode === 'string' ? message.pairCode : '';
    if (pairCode) {
      publish({ pairCode, qrPayload: pairingPayload(pairCode) });
      console.log(`PAIRSPAN_PAIR_CODE=${pairCode}`);
      console.log(`PAIRSPAN_QR_PAYLOAD=${pairingPayload(pairCode)}`);
    }
    return;
  }

  if (type === 'error') {
    const error = typeof message.code === 'string' ? `err.${message.code}` : typeof message.message === 'string' ? message.message : 'err.closed';
    publish({ error });
    return;
  }

  if (type === 'event') {
    const event = typeof message.event === 'string' ? message.event : '';
    const payload = (message.payload && typeof message.payload === 'object')
      ? message.payload as Record<string, unknown> : {};
    // Gateways that predate device ids pass payloads through untouched, so the phone repeats its id there.
    const fromGateway = typeof message.deviceId === 'string' ? message.deviceId : '';
    const fromPayload = typeof payload.deviceId === 'string' ? payload.deviceId : '';
    const deviceId = DEVICE_ID.test(fromGateway) ? fromGateway : DEVICE_ID.test(fromPayload) ? fromPayload : LEGACY_DEVICE_ID;
    // `phone_online` is written by the gateway itself and never names a phone on gateways that predate device ids.
    if (event === 'phone_online' && deviceId === LEGACY_DEVICE_ID) return;
    if (event === 'phone_hello' || event === 'phone_heartbeat' || event === 'phone_online') acceptPhone(deviceId, payload);
    else if (event === 'phone_bye') {
      dropPhone(deviceId);
      publishPhones();
    } else if (event === 'shell_result') {
      if (!payload || payload.id !== pendingCommandId) return;
      if (commandTimeout) clearTimeout(commandTimeout);
      pendingCommandId = '';
      publish({ command: {
        pending: false,
        deviceId: state.command.deviceId,
        command: state.command.command,
        output: typeof payload.output === 'string' ? payload.output.slice(0, 24000) : '',
        exitCode: typeof payload.exitCode === 'number' ? payload.exitCode : null,
        error: null,
      } });
    } else if (event === 'adb_tunnel_data' || event === 'adb_tunnel_close' || event === 'adb_tunnel_ready') {
      resolvePhone(deviceId)?.proxy.handleEvent(event, payload);
    }
  }
}

function scheduleReconnect(): void {
  if (stopped || reconnectTimer) return;
  reconnectTimer = setTimeout(() => {
    reconnectTimer = undefined;
    if (!stopped) connect();
  }, 2000);
}

function connect(): void {
  const url = gatewayWsUrl();
  if (!url) {
    publish({ status: 'offline', error: 'err.url' });
    return;
  }
  publish({ status: 'connecting', error: null });
  try {
    socket = new WebSocket(url);
  } catch (error) {
    publish({ status: 'offline', error: String(error) });
    scheduleReconnect();
    return;
  }
  const current = socket;
  current.addEventListener('open', () => { if (socket === current) send({ type: 'hello', role: 'mac', clientId: state.clientId, token: settings.token }); });
  current.addEventListener('message', event => { if (socket === current) handleMessage(String(event.data)); });
  current.addEventListener('close', () => {
    if (socket !== current) return;
    socket = undefined;
    dropAllPhones();
    publish({ status: 'offline', error: state.error || 'err.closed', phones: [], pairCode: '', qrPayload: '' });
    scheduleReconnect();
  });
  current.addEventListener('error', () => {
    if (socket !== current) return;
    publish({ status: 'offline', error: 'err.ws' });
  });
}

export async function runCommand(deviceId: string, command: string): Promise<void> {
  const value = command.trim();
  if (!value || value.length > 512) throw new Error('Command must be 1–512 characters.');
  const phone = phones.get(deviceId)?.state;
  if (state.status !== 'online' || !phone?.connected || !phone.adbReady) throw new Error('The phone’s ADB shell is not ready.');
  if (state.command.pending) throw new Error('Waiting for the previous command to finish.');
  const id = randomUUID();
  pendingCommandId = id;
  publish({ command: { pending: true, deviceId, command: value, output: '', exitCode: null, error: null } });
  commandTimeout = setTimeout(() => {
    if (pendingCommandId !== id) return;
    pendingCommandId = '';
    publish({ command: { pending: false, deviceId, command: value, output: '', exitCode: null, error: 'The phone did not respond (15 s).' } });
  }, 15000);
  try {
    send({ type: 'event', event: 'shell_request', deviceId, payload: { id, command: value, sentAt: Date.now() } });
  } catch (error) {
    if (commandTimeout) clearTimeout(commandTimeout);
    pendingCommandId = '';
    publish({ command: { pending: false, deviceId, command: value, output: '', exitCode: null, error: String(error) } });
  }
}

export function start(): void {
  if (!stopped && socket) return;
  stopped = false;
  publish({ enabled: true });
  connect();
  rotation = setInterval(() => send({ type: 'rotate_pair' }), 5 * 60 * 1000);
  heartbeat = setInterval(() => {
    let changed = false;
    for (const [id, entry] of phones) {
      if (entry.lastMs && Date.now() - entry.lastMs > 30000) { dropPhone(id); changed = true; }
    }
    if (changed) publishPhones();
    if (state.status === 'online') send({ type: 'event', event: 'mac_online', payload: { at: new Date().toISOString(), clientId: state.clientId } });
  }, 10000);
}

export function stop(): void {
  stopped = true;
  if (heartbeat) clearInterval(heartbeat);
  if (rotation) clearInterval(rotation);
  if (reconnectTimer) clearTimeout(reconnectTimer);
  if (commandTimeout) clearTimeout(commandTimeout);
  pendingCommandId = '';
  heartbeat = rotation = reconnectTimer = undefined;
  dropAllPhones();
  try { socket?.close(); } catch { /* ignore */ }
  socket = undefined;
  publish({ status: 'offline', phones: [], pairCode: '', qrPayload: '', command: emptyCommand, error: null });
}

/** Master switch: off closes the gateway connection and every phone tunnel until it is turned on again. */
export function setEnabled(enabled: boolean): void {
  if (enabled === state.enabled && enabled === !stopped) return;
  if (enabled) start();
  else { stop(); publish({ enabled: false }); }
}

export function adbProxyFor(deviceId: string): string {
  return phones.get(deviceId)?.state.adbProxy ?? '';
}
