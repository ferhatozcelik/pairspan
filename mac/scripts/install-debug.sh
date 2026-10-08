#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
osascript -e 'quit app "Pairspan"' >/dev/null 2>&1 || true
killall Pairspan >/dev/null 2>&1 || true
pkill -f "$PWD/node_modules/electron/dist/Electron.app" >/dev/null 2>&1 || true
sleep 1
node scripts/fetch-mac-adb.mjs
npm run build
mkdir -p pack
sips -z 1024 1024 assets/icon.png --out pack/icon.png
npx electron-builder --mac dir --arm64
killall Pairspan >/dev/null 2>&1 || true
sleep 1
APP="$(find release -type d -name 'Pairspan.app' -path '*mac-arm64*' -print -quit)"
if [ -z "$APP" ]; then
  APP="$(find release -type d -name 'Pairspan.app' -print -quit)"
fi
test -n "$APP"
rm -rf /Applications/Pairspan.app
ditto "$APP" /Applications/Pairspan.app
xattr -dr com.apple.quarantine /Applications/Pairspan.app || true
open /Applications/Pairspan.app
