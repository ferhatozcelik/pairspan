import { app } from 'electron';
import { randomBytes } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

interface Stored { enabled: boolean; clientId: string }

const file = (): string => join(app.getPath('userData'), 'settings.json');

export function load(): Stored {
  let stored: Partial<Stored> = {};
  try { stored = JSON.parse(readFileSync(file(), 'utf8')) as Partial<Stored>; } catch { /* first run */ }
  const clientId = typeof stored.clientId === 'string' && /^[0-9a-f]{32}$/.test(stored.clientId) ? stored.clientId : randomBytes(16).toString('hex');
  const next = { enabled: stored.enabled !== false, clientId };
  if (next.clientId !== stored.clientId) save(next);
  return next;
}

export function save(value: Stored): void {
  try { writeFileSync(file(), JSON.stringify(value)); } catch { /* best effort */ }
}
