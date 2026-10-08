import { execFile } from 'node:child_process';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';

export const APP_PACKAGE = 'com.pairspan';
export const SECURE_SETTINGS_PERMISSION = 'android.permission.WRITE_SECURE_SETTINGS';

export interface UsbDevice {
  serial: string;
  /** `device` is ready; `unauthorized` means the phone has not accepted this computer's USB debugging prompt. */
  state: 'device' | 'unauthorized' | 'offline';
  model: string;
}

export interface GrantResult { ok: boolean; message: string }

/** Prefers the adb shipped inside the app, then falls back to an installed Android SDK, then PATH. */
export function adbLaunch(): { file: string; cwd?: string } {
  const name = process.platform === 'win32' ? 'adb.exe' : 'adb';
  const platform = process.platform === 'win32' ? 'win' : 'mac';
  const bundled = [join(process.resourcesPath || '', 'adb', name), join(__dirname, '..', 'vendor', platform, 'adb', name)].find(existsSync);
  if (bundled) return { file: bundled, cwd: dirname(bundled) };
  const candidates = [process.env.ANDROID_HOME, process.env.ANDROID_SDK_ROOT,
    join(homedir(), 'Library', 'Android', 'sdk'), join(homedir(), 'AppData', 'Local', 'Android', 'Sdk')];
  for (const sdk of candidates) {
    if (!sdk) continue;
    const path = join(sdk, 'platform-tools', name);
    if (existsSync(path)) return { file: path, cwd: dirname(path) };
  }
  return { file: name };
}

export function run(args: string[], timeout: number): Promise<{ ok: boolean; out: string }> {
  const adb = adbLaunch();
  return new Promise(resolve => {
    execFile(adb.file, args, { timeout, maxBuffer: 64 * 1024, cwd: adb.cwd, windowsHide: true }, (error, stdout, stderr) => {
      resolve({ ok: !error, out: `${stdout}\n${stderr}`.trim() || (error ? String(error.message) : '') });
    });
  });
}

/** Phones attached by cable. Tunnel targets such as 127.0.0.1:44755 and Wi-Fi targets are skipped. */
export async function listUsbDevices(): Promise<UsbDevice[]> {
  const result = await run(['devices', '-l'], 20000);
  if (!result.ok) return [];
  const devices: UsbDevice[] = [];
  for (const line of result.out.split('\n').slice(1)) {
    const match = /^(\S+)\s+(device|unauthorized|offline)\b(.*)$/.exec(line.trim());
    if (!match || match[1].includes(':') || /^emulator-/.test(match[1])) continue;
    const model = /\bmodel:(\S+)/.exec(match[3])?.[1]?.replace(/_/g, ' ') ?? '';
    devices.push({ serial: match[1], state: match[2] as UsbDevice['state'], model });
  }
  return devices;
}

/**
 * Runs `adb -s <serial> shell pm grant com.pairspan android.permission.WRITE_SECURE_SETTINGS`, then
 * `adb -s <serial> tcpip 5555` so the phone's own ADB client can reach the phone without a cable.
 * The tcpip step lasts until the phone restarts; pressing the button again repeats it.
 */
export async function grantSecureSettings(serial: string): Promise<GrantResult> {
  if (!/^[A-Za-z0-9._-]{1,64}$/.test(serial)) return { ok: false, message: 'Invalid device.' };
  const result = await run(['-s', serial, 'shell', 'pm', 'grant', APP_PACKAGE, SECURE_SETTINGS_PERMISSION], 20000);
  if (!result.ok || /exception|error|unknown package|not installed/i.test(result.out)) return { ok: false, message: result.out.slice(0, 400) };
  const tcpip = await run(['-s', serial, 'tcpip', '5555'], 20000);
  if (!tcpip.ok || /error|failed|closed/i.test(tcpip.out)) return { ok: false, message: tcpip.out.slice(0, 400) };
  return { ok: true, message: '' };
}
