import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const dest = join(root, 'vendor', 'mac', 'adb');
const zip = join(root, 'vendor', 'mac', 'platform-tools.zip');
const url = 'https://dl.google.com/android/repository/platform-tools_r36.0.0-darwin.zip';
const needed = ['adb'];

if (needed.every(name => existsSync(join(dest, name)))) process.exit(0);

mkdirSync(dirname(zip), { recursive: true });
execFileSync('curl', ['-fsSL', '-o', zip, url], { stdio: 'inherit' });
rmSync(dest, { recursive: true, force: true });
mkdirSync(dest, { recursive: true });
execFileSync('unzip', ['-o', '-j', zip, ...needed.map(name => `platform-tools/${name}`), '-d', dest], { stdio: 'inherit' });
for (const name of needed) {
  if (!existsSync(join(dest, name))) {
    console.error(`missing ${name}`);
    process.exit(1);
  }
}
