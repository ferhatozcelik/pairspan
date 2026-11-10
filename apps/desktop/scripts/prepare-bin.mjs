// Puts the files the app bundle carries into src-tauri/bin/:
//   pairspan  – the CLI / MCP server (built from crates/cli)
//   adb       – Android platform-tools adb (downloaded once, used only when the user has no adb)
// Usage: node scripts/prepare-bin.mjs [--debug]
import { execFileSync } from 'node:child_process';
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../../..');
const bin = resolve(here, '../src-tauri/bin');
const debug = process.argv.includes('--debug');
mkdirSync(bin, { recursive: true });

const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { stdio: 'inherit', cwd: root, ...opts });

// 1. CLI
run('cargo', ['build', '-p', 'pairspan-cli', ...(debug ? [] : ['--release'])]);
copyFileSync(join(root, 'target', debug ? 'debug' : 'release', 'pairspan'), join(bin, 'pairspan'));
chmodSync(join(bin, 'pairspan'), 0o755);
console.log('✓ bin/pairspan');

// 2. adb
const adb = join(bin, 'adb');
if (!existsSync(adb)) {
  const os = process.platform === 'darwin' ? 'darwin' : process.platform === 'win32' ? 'windows' : 'linux';
  const url = `https://dl.google.com/android/repository/platform-tools-latest-${os}.zip`;
  const work = mkdtempSync(join(tmpdir(), 'pairspan-adb-'));
  try {
    run('curl', ['-fsSL', url, '-o', join(work, 'pt.zip')]);
    run('unzip', ['-q', join(work, 'pt.zip'), 'platform-tools/adb', '-d', work]);
    copyFileSync(join(work, 'platform-tools', 'adb'), adb);
    chmodSync(adb, 0o755);
    console.log('✓ bin/adb (platform-tools)');
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
} else {
  console.log('✓ bin/adb (cached)');
}
