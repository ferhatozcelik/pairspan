#!/bin/sh
# Pairspan installer — https://pairspan.ferhatozcelik.com/docs/#install
#
#   curl -fsSL https://pairspan.ferhatozcelik.com/install.sh | sh     # latest release
#   ./install.sh --source        # build from this checkout (needs Rust + Node)
#   ./install.sh --link-only     # only put the bundled `pairspan` command on PATH
#   ./install.sh --uninstall     # remove the app and the command (keeps your pairings)
#
# The CLI and MCP server ship inside the app bundle; this script links them to PATH.
set -eu

REPO="ferhatozcelik/pairspan"
APP="/Applications/Pairspan.app"
BIN_DIR="${PAIRSPAN_BIN_DIR:-/usr/local/bin}"
MODE="release"

for arg in "$@"; do
  case "$arg" in
    --source) MODE="source" ;;
    --link-only) MODE="link" ;;
    --uninstall) MODE="uninstall" ;;
    -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
    *) echo "Unknown option: $arg" >&2; exit 2 ;;
  esac
done

say() { printf '\033[1;38;5;202m›\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m✗\033[0m %s\n' "$*" >&2; exit 1; }

as_root() {
  if [ -w "$1" ] || { [ ! -e "$1" ] && [ -w "$(dirname "$1")" ]; }; then shift; "$@"
  elif command -v sudo >/dev/null 2>&1; then shift; sudo "$@"
  else die "No permission to write $1 (and no sudo)."; fi
}

link_cli() {
  src="$1"
  [ -x "$src" ] || die "pairspan command not found at $src"
  if [ ! -d "$BIN_DIR" ]; then as_root "$(dirname "$BIN_DIR")" mkdir -p "$BIN_DIR"; fi
  as_root "$BIN_DIR" ln -sf "$src" "$BIN_DIR/pairspan"
  say "pairspan → $BIN_DIR/pairspan"
  case ":$PATH:" in *":$BIN_DIR:"*) ;; *) say "Add $BIN_DIR to your PATH." ;; esac
}

stop_app() {
  pkill -f "Pairspan.app/Contents/MacOS/pairspan-desktop" 2>/dev/null || true
  sleep 1
}

install_app_bundle() {
  bundle="$1"
  stop_app
  as_root /Applications rm -rf "$APP"
  as_root /Applications cp -R "$bundle" "$APP"
  xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true
  say "Installed $APP"
}

OS="$(uname -s)"

case "$MODE" in
  uninstall)
    stop_app
    [ -L "$BIN_DIR/pairspan" ] && as_root "$BIN_DIR" rm -f "$BIN_DIR/pairspan"
    [ -d "$APP" ] && as_root /Applications rm -rf "$APP"
    say "Removed. Pairings and keys stay in ~/Library/Application Support/Pairspan."
    exit 0
    ;;
  link)
    [ "$OS" = "Darwin" ] || die "--link-only is for macOS."
    link_cli "$APP/Contents/MacOS/pairspan"
    exit 0
    ;;
esac

[ "$OS" = "Darwin" ] || die "This installer supports macOS. Linux builds are on https://github.com/$REPO/releases"

if [ "$MODE" = "source" ]; then
  ROOT="$(cd "$(dirname "$0")" && pwd)"
  [ -f "$ROOT/Cargo.toml" ] && [ -d "$ROOT/apps/desktop" ] || die "--source must run from a Pairspan checkout."
  command -v cargo >/dev/null 2>&1 || die "Rust is required (https://rustup.rs)."
  command -v npm >/dev/null 2>&1 || die "Node.js 22+ is required."
  say "Building Pairspan from source…"
  (cd "$ROOT" && npm install --no-audit --no-fund >/dev/null && npm run desktop)
  install_app_bundle "$ROOT/target/release/bundle/macos/Pairspan.app"
else
  say "Finding the latest Pairspan release…"
  api="https://api.github.com/repos/$REPO/releases/latest"
  url="$(curl -fsSL -H 'Accept: application/vnd.github+json' "$api" \
    | grep -o '"browser_download_url": *"[^"]*macos[^"]*\.zip"' | head -n1 | sed 's/.*"\(https[^"]*\)"/\1/')"
  [ -n "$url" ] || die "No macOS build in the latest release."
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  say "Downloading $(basename "$url")…"
  curl -fsSL "$url" -o "$tmp/pairspan.zip"
  ditto -x -k "$tmp/pairspan.zip" "$tmp"
  install_app_bundle "$tmp/Pairspan.app"
fi

link_cli "$APP/Contents/MacOS/pairspan"
open -g "$APP" --args --background 2>/dev/null || true
say "Done. Pairspan is in the menu bar."
say "Next: click Pair a phone (or run: pairspan pair). For AI agents: claude mcp add pairspan -- pairspan mcp"
