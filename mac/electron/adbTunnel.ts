import { createServer, type Server, type Socket } from 'node:net';
import { randomUUID } from 'node:crypto';

const BASE_PORT = 44755;
const MAX_PORTS = 16;

type Send = (message: object) => void;

const usedPorts = new Set<number>();

/** One local TCP listener per phone; `adb connect 127.0.0.1:<port>` reaches that phone's ADB. */
export class AdbProxy {
  private server: Server | undefined;
  private localSocket: Socket | undefined;
  private tunnelId = '';
  private tunnelReady = false;
  private pending: Buffer[] = [];
  private listeningPort = 0;
  private starting = false;

  constructor(private readonly deviceId: string, private send: Send) {}

  getPort(): number { return this.listeningPort; }
  getConnectHint(): string { return this.listeningPort > 0 ? `adb connect 127.0.0.1:${this.listeningPort}` : ''; }

  private emit(event: string, payload: object): void {
    this.send({ type: 'event', event, payload, deviceId: this.deviceId });
  }

  private flushPending(): void {
    if (!this.tunnelReady || !this.tunnelId) return;
    for (const chunk of this.pending) this.emit('adb_tunnel_data', { id: this.tunnelId, data: chunk.toString('base64') });
    this.pending = [];
  }

  start(onListening: (port: number) => void): void {
    if (this.server) {
      if (this.listeningPort > 0) onListening(this.listeningPort);
      return;
    }
    if (this.starting) return;
    this.starting = true;
    const server = createServer(socket => this.accept(socket));
    this.server = server;
    let offset = 0;
    const tryListen = (): void => {
      while (offset < MAX_PORTS && usedPorts.has(BASE_PORT + offset)) offset++;
      if (offset >= MAX_PORTS) { this.starting = false; this.server = undefined; return; }
      server.listen(BASE_PORT + offset, '127.0.0.1');
    };
    server.on('error', (error: NodeJS.ErrnoException) => {
      if (error.code === 'EADDRINUSE' && this.listeningPort === 0 && offset < MAX_PORTS - 1) { offset++; tryListen(); return; }
      this.starting = false;
      this.listeningPort = 0;
      if (this.server === server) this.server = undefined;
    });
    server.on('listening', () => {
      this.starting = false;
      this.listeningPort = BASE_PORT + offset;
      usedPorts.add(this.listeningPort);
      console.log(`PAIRSPAN_ADB_PROXY=127.0.0.1:${this.listeningPort} device=${this.deviceId}`);
      onListening(this.listeningPort);
    });
    tryListen();
  }

  private accept(socket: Socket): void {
    try { this.localSocket?.destroy(); } catch { /* ignore */ }
    this.localSocket = socket;
    this.tunnelId = randomUUID();
    this.tunnelReady = false;
    this.pending = [];
    socket.setNoDelay(true);
    this.emit('adb_tunnel_open', { id: this.tunnelId });
    socket.on('data', chunk => {
      if (!this.tunnelId) return;
      const buf = Buffer.from(chunk);
      if (!this.tunnelReady) { this.pending.push(buf); return; }
      this.emit('adb_tunnel_data', { id: this.tunnelId, data: buf.toString('base64') });
    });
    socket.on('close', () => {
      if (this.localSocket !== socket) return;
      const id = this.tunnelId;
      this.tunnelId = '';
      this.tunnelReady = false;
      this.pending = [];
      this.localSocket = undefined;
      if (id) this.emit('adb_tunnel_close', { id });
    });
    socket.on('error', () => { try { socket.destroy(); } catch { /* ignore */ } });
  }

  handleEvent(event: string, payload: Record<string, unknown>): void {
    if (event === 'adb_tunnel_ready') {
      if (!payload || payload.id !== this.tunnelId) return;
      this.tunnelReady = true;
      console.log(`PAIRSPAN_ADB_TUNNEL_READY id=${String(payload.id || '')} port=${String(payload.port || '')}`);
      this.flushPending();
      return;
    }
    if (event === 'adb_tunnel_data') {
      if (!payload || payload.id !== this.tunnelId) return;
      const data = typeof payload.data === 'string' ? payload.data : '';
      if (!data || !this.localSocket) return;
      try { this.localSocket.write(Buffer.from(data, 'base64')); } catch { /* ignore */ }
      return;
    }
    if (event === 'adb_tunnel_close') {
      if (!payload || payload.id !== this.tunnelId) return;
      console.log(`PAIRSPAN_ADB_TUNNEL_CLOSE reason=${String(payload.reason || '')}`);
      this.tunnelId = '';
      this.tunnelReady = false;
      this.pending = [];
      try { this.localSocket?.destroy(); } catch { /* ignore */ }
      this.localSocket = undefined;
    }
  }

  stop(): void {
    const id = this.tunnelId;
    this.tunnelId = '';
    this.tunnelReady = false;
    this.pending = [];
    try { this.localSocket?.destroy(); } catch { /* ignore */ }
    this.localSocket = undefined;
    if (id) this.emit('adb_tunnel_close', { id });
    try { this.server?.close(); } catch { /* ignore */ }
    this.server = undefined;
    if (this.listeningPort > 0) usedPorts.delete(this.listeningPort);
    this.listeningPort = 0;
    this.starting = false;
  }
}
