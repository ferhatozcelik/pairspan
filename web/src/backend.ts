export interface SessionRecord {
  id: string;
  pairCode: string;
  pairExpiresAt: number;
  /** Public WebSocket base URL the desktop connected to, used for the QR payload. */
  gatewayUrl: string;
  /** Stable id of the desktop app instance, shown next to the phones it hosts. */
  clientId: string;
  macConn?: string;
  /** Connected phones by device id. */
  phoneConns: Record<string, string>;
}

/** `mac` or `phone:<deviceId>`. */
type Role = string;

export interface Backend {
  kind: 'memory';
  getSession(id: string): Promise<SessionRecord | null>;
  putSession(record: SessionRecord): Promise<void>;
  deleteSession(id: string): Promise<void>;
  /** Reserves a pairing code for a session. False when the code is already taken. */
  claimCode(code: string, sessionId: string): Promise<boolean>;
  releaseCode(code: string, sessionId: string): Promise<void>;
  sessionIdForCode(code: string): Promise<string | null>;
  allowPairAttempt(ip: string): Promise<boolean>;
  subscribe(sessionId: string, role: Role): Promise<void>;
  unsubscribe(sessionId: string, role: Role): Promise<void>;
  /** Number of subscribers that received the message. */
  publish(sessionId: string, role: Role, data: string): Promise<number>;
  sweep(): Promise<void>;
}

type BusHandler = (sessionId: string, role: Role, raw: string) => void;

const ATTEMPT_WINDOW_SECONDS = 5 * 60;
const MAX_ATTEMPTS = 20;

export function createBackend(onMessage: BusHandler): Backend {
  return new MemoryBackend(onMessage);
}

class MemoryBackend implements Backend {
  kind = 'memory' as const;
  private sessions = new Map<string, SessionRecord>();
  private codes = new Map<string, string>();
  private attempts = new Map<string, { count: number; resetsAt: number }>();
  private subscribed = new Set<string>();

  constructor(private onMessage: BusHandler) {}

  async getSession(id: string) { const record = this.sessions.get(id); return record ? { ...record } : null; }
  async putSession(record: SessionRecord) { this.sessions.set(record.id, { ...record }); }
  async deleteSession(id: string) { this.sessions.delete(id); }
  async claimCode(code: string, sessionId: string) {
    if (this.codes.has(code)) return false;
    this.codes.set(code, sessionId);
    return true;
  }
  async releaseCode(code: string, sessionId: string) { if (this.codes.get(code) === sessionId) this.codes.delete(code); }
  async sessionIdForCode(code: string) { return this.codes.get(code) ?? null; }

  async allowPairAttempt(ip: string) {
    const now = Date.now();
    const current = this.attempts.get(ip);
    if (!current || now > current.resetsAt) {
      this.attempts.set(ip, { count: 1, resetsAt: now + ATTEMPT_WINDOW_SECONDS * 1000 });
      return true;
    }
    current.count++;
    return current.count <= MAX_ATTEMPTS;
  }

  async subscribe(sessionId: string, role: Role) { this.subscribed.add(`${sessionId}:${role}`); }
  async unsubscribe(sessionId: string, role: Role) { this.subscribed.delete(`${sessionId}:${role}`); }
  async publish(sessionId: string, role: Role, data: string) {
    if (!this.subscribed.has(`${sessionId}:${role}`)) return 0;
    this.onMessage(sessionId, role, data);
    return 1;
  }

  async sweep() {
    const now = Date.now();
    for (const [ip, attempt] of this.attempts) if (now > attempt.resetsAt) this.attempts.delete(ip);
  }
}
