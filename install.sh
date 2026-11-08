#!/usr/bin/env bash
set -euo pipefail

# Pairspan — Android Device Center | macOS Installer
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_NAME="Pairspan.app"
APP_BUNDLE_PATH="$ROOT_DIR/target/release/bundle/macos/$APP_NAME"
INSTALL_PATH="/Applications/$APP_NAME"

echo "========================================================"
echo "  Pairspan — Android Device Center (macOS Installer)"
echo "========================================================"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Hata: Bu kurulum betiği yalnızca macOS içindir."
  exit 1
fi

echo "[1/4] Rust Gateway ve Masaüstü uygulaması derleniyor..."
cd "$ROOT_DIR"
cargo build --release -p pairspan-gateway

touch "$ROOT_DIR/desktop/src-tauri/src/main.rs"
cd "$ROOT_DIR/desktop"
npx --yes @tauri-apps/cli build --bundles app

if [[ ! -d "$APP_BUNDLE_PATH" ]]; then
  echo "Hata: $APP_BUNDLE_PATH oluşturulamadı."
  exit 1
fi

echo "[2/4] Çalışan eski Pairspan süreçleri kapatılıyor..."
pkill -f "Pairspan.app/Contents/MacOS/pairspan-desktop" 2>/dev/null || true
pkill -f "pairspan-gateway" 2>/dev/null || true
sleep 1

echo "[3/4] /Applications/$APP_NAME dizinine kuruluyor..."
rm -rf "$INSTALL_PATH"
cp -R "$APP_BUNDLE_PATH" "$INSTALL_PATH"
xattr -dr com.apple.quarantine "$INSTALL_PATH" 2>/dev/null || true
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$INSTALL_PATH" 2>/dev/null || true

echo "[4/4] Pairspan Gateway ve Pairspan — Android Device Center başlatılıyor..."
nohup "$ROOT_DIR/target/release/pairspan-gateway" >/tmp/pairspan-gateway.log 2>&1 </dev/null &
open "$INSTALL_PATH" 2>/dev/null || true
sleep 1
if ! pgrep -f "$INSTALL_PATH/Contents/MacOS/pairspan-desktop" >/dev/null 2>&1; then
  nohup "$INSTALL_PATH/Contents/MacOS/pairspan-desktop" >/tmp/pairspan-desktop.log 2>&1 </dev/null &
fi

echo ""
echo "✓ Kurulum tamamlandı!"
echo "  • Uygulama: $INSTALL_PATH"
echo "  • Menü Çubuğu (Status Bar): Üst barda Pairspan ikonuna tıklayarak bağlı cihazlar widget'ını açabilirsiniz."
