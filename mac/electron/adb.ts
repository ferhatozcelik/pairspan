import { execFile } from 'node:child_process';
import { existsSync, promises as fs } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';

export const APP_PACKAGE = 'com.pairspan';
export const SECURE_SETTINGS_PERMISSION = 'android.permission.WRITE_SECURE_SETTINGS';
export const GITHUB_RELEASES_API = 'https://api.github.com/repos/ferhatozcelik/pairspan/releases/latest';
export const FALLBACK_APK_URL = 'https://github.com/ferhatozcelik/pairspan/releases/download/v1.0.0/pairspan-android-1.0.0.apk';

export interface UsbDevice {
  serial: string;
  /** `device` is ready; `unauthorized` means the phone has not accepted this computer's USB debugging prompt. */
  state: 'device' | 'unauthorized' | 'offline';
  model: string;
  apkInstalled?: boolean;
  apkVersion?: string;
  latestVersion?: string;
  hasUpdate?: boolean;
}

export interface GrantResult { ok: boolean; message: string; version?: string }

export interface ReleaseApkInfo {
  version: string;
  tagName: string;
  downloadUrl: string;
  size: number;
}

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

export function isNewerVersion(remote: string, local: string): boolean {
  const rParts = remote.trim().replace(/^[vV]/, '').split('.').map(n => parseInt(n, 10) || 0);
  const lParts = local.trim().replace(/^[vV]/, '').split('.').map(n => parseInt(n, 10) || 0);
  const max = Math.max(rParts.length, lParts.length);
  for (let i = 0; i < max; i++) {
    const r = rParts[i] ?? 0;
    const l = lParts[i] ?? 0;
    if (r > l) return true;
    if (r < l) return false;
  }
  return false;
}

export async function getApkStatus(serial: string): Promise<{ installed: boolean; version?: string }> {
  const pathResult = await run(['-s', serial, 'shell', 'pm', 'path', APP_PACKAGE], 10000);
  if (!pathResult.ok || !pathResult.out.includes('package:')) {
    return { installed: false };
  }
  const dumpsysResult = await run(['-s', serial, 'shell', 'dumpsys', 'package', APP_PACKAGE], 10000);
  const match = /versionName=([^\s\r\n]+)/.exec(dumpsysResult.out);
  return { installed: true, version: match?.[1] };
}

let cachedReleaseInfo: { data: ReleaseApkInfo; fetchedAt: number } | null = null;

export async function fetchLatestReleaseApk(): Promise<ReleaseApkInfo> {
  const now = Date.now();
  if (cachedReleaseInfo && now - cachedReleaseInfo.fetchedAt < 60_000) {
    return cachedReleaseInfo.data;
  }
  try {
    const res = await fetch(GITHUB_RELEASES_API, {
      headers: {
        'Accept': 'application/vnd.github+json',
        'User-Agent': 'Pairspan-Desktop-Installer',
      },
      signal: AbortSignal.timeout(10000),
    });
    if (res.ok) {
      const json = await res.json() as any;
      const tagName = json.tag_name || 'v1.0.0';
      const version = String(tagName).replace(/^[vV]/, '');
      let downloadUrl = FALLBACK_APK_URL;
      let size = 0;
      if (Array.isArray(json.assets)) {
        for (const asset of json.assets) {
          if (typeof asset.name === 'string' && asset.name.toLowerCase().endsWith('.apk')) {
            downloadUrl = asset.browser_download_url;
            size = asset.size || 0;
            break;
          }
        }
      }
      const data: ReleaseApkInfo = { version, tagName, downloadUrl, size };
      cachedReleaseInfo = { data, fetchedAt: now };
      return data;
    }
  } catch (_e) {}
  return {
    version: '1.0.0',
    tagName: 'v1.0.0',
    downloadUrl: FALLBACK_APK_URL,
    size: 0,
  };
}

export async function downloadApkToCache(info: ReleaseApkInfo): Promise<string> {
  const cacheDir = join(homedir(), '.pairspan', 'cache');
  await fs.mkdir(cacheDir, { recursive: true });
  const apkPath = join(cacheDir, `pairspan-android-${info.version}.apk`);

  if (existsSync(apkPath)) {
    try {
      const stat = await fs.stat(apkPath);
      if (stat.size > 1_000_000 && (info.size <= 0 || stat.size === info.size)) {
        return apkPath;
      }
    } catch (_e) {}
  }

  const res = await fetch(info.downloadUrl, {
    headers: { 'User-Agent': 'Pairspan-Desktop-Installer' },
    signal: AbortSignal.timeout(90000),
  });
  if (!res.ok) {
    throw new Error(`Failed to download APK: HTTP ${res.status}`);
  }
  const buffer = Buffer.from(await res.arrayBuffer());
  const tempPath = `${apkPath}.tmp`;
  await fs.writeFile(tempPath, buffer);
  await fs.rename(tempPath, apkPath);
  return apkPath;
}

export async function installOrUpdateApk(
  serial: string,
  options?: { force?: boolean }
): Promise<GrantResult> {
  if (!/^[A-Za-z0-9._-]{1,64}$/.test(serial)) {
    return { ok: false, message: 'Invalid device serial.' };
  }

  const status = await getApkStatus(serial);
  const release = await fetchLatestReleaseApk();

  if (status.installed && !options?.force) {
    if (status.version && !isNewerVersion(release.version, status.version)) {
      return { ok: true, message: `Pairspan v${status.version} is up to date.`, version: status.version };
    }
  }

  let apkPath: string;
  try {
    apkPath = await downloadApkToCache(release);
  } catch (err: any) {
    return { ok: false, message: `Could not download APK: ${err.message}` };
  }

  const installResult = await run(['-s', serial, 'install', '-r', '-g', apkPath], 90000);
  const verify = await getApkStatus(serial);
  if (installResult.out.includes('Success') || verify.installed) {
    return {
      ok: true,
      message: `Pairspan v${release.version} installed successfully.`,
      version: release.version,
    };
  }
  return {
    ok: false,
    message: `ADB APK installation failed: ${installResult.out.slice(0, 300)}`,
  };
}

/** Phones attached by cable. Tunnel targets such as 127.0.0.1:44755 and Wi-Fi targets are skipped. */
export async function listUsbDevices(): Promise<UsbDevice[]> {
  const result = await run(['devices', '-l'], 20000);
  if (!result.ok) return [];
  const devices: UsbDevice[] = [];
  const latest = await fetchLatestReleaseApk();

  for (const line of result.out.split('\n').slice(1)) {
    const match = /^(\S+)\s+(device|unauthorized|offline)\b(.*)$/.exec(line.trim());
    if (!match || match[1].includes(':') || /^emulator-/.test(match[1])) continue;
    const model = /\bmodel:(\S+)/.exec(match[3])?.[1]?.replace(/_/g, ' ') ?? '';
    const serial = match[1];
    const state = match[2] as UsbDevice['state'];

    let apkInstalled = false;
    let apkVersion: string | undefined;
    let hasUpdate = false;

    if (state === 'device') {
      try {
        const apk = await getApkStatus(serial);
        apkInstalled = apk.installed;
        apkVersion = apk.version;
        if (apkInstalled && apkVersion) {
          hasUpdate = isNewerVersion(latest.version, apkVersion);
        }
      } catch (_e) {}
    }

    devices.push({
      serial,
      state,
      model,
      apkInstalled,
      apkVersion,
      latestVersion: latest.version,
      hasUpdate,
    });
  }
  return devices;
}

/**
 * Runs `adb -s <serial> shell pm grant com.pairspan android.permission.WRITE_SECURE_SETTINGS`, then
 * `adb -s <serial> tcpip 5555` so the phone's own ADB client can reach the phone without a cable.
 * The tcpip step lasts until the phone restarts; pressing the button again repeats it.
 * If Pairspan is not installed on the phone, automatically installs the latest APK from GitHub Releases first.
 */
export async function grantSecureSettings(serial: string): Promise<GrantResult> {
  if (!/^[A-Za-z0-9._-]{1,64}$/.test(serial)) return { ok: false, message: 'Invalid device.' };

  // Auto-install latest Pairspan APK if not installed
  const status = await getApkStatus(serial);
  if (!status.installed) {
    const installRes = await installOrUpdateApk(serial);
    if (!installRes.ok) {
      return { ok: false, message: `Could not auto-install Pairspan APK: ${installRes.message}` };
    }
  }

  const result = await run(['-s', serial, 'shell', 'pm', 'grant', APP_PACKAGE, SECURE_SETTINGS_PERMISSION], 20000);
  if (!result.ok || /exception|error|unknown package|not installed/i.test(result.out)) return { ok: false, message: result.out.slice(0, 400) };
  const tcpip = await run(['-s', serial, 'tcpip', '5555'], 20000);
  if (!tcpip.ok || /error|failed|closed/i.test(tcpip.out)) return { ok: false, message: tcpip.out.slice(0, 400) };
  return { ok: true, message: '' };
}
