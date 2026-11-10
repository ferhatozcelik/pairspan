# Changelog

All notable changes to Pairspan are documented here. This project uses [semantic versions](https://semver.org/).

## [1.0.0] — 2026-11-10

A clean start. Earlier builds (Electron desktop app, Node gateway, protocol v2) are not compatible; pair your phones again.

### Added
- Rust gateway: signature-authenticated clients, pairing rooms that never see the pair code, mutual-trust binary routing, backpressure, `/health` and Prometheus `/metrics`. Also serves the website; one 5.7 MB container.
- End-to-end encryption between computer and phone: P-256 handshake (ephemeral + static ECDH, pair-code PSK on first pairing), HKDF-SHA256, AES-256-GCM with replay protection. Spec in `docs/protocol.md`.
- Desktop app (Tauri) with menu-bar icon, QR pairing, USB first-time setup (install the Android app, enable wireless ADB) and a local automation socket.
- `pairspan` CLI bundled in the app: devices, pair, shell, adb, tap, click, swipe, text, key, elements, dump, screenshot, launch, stop, install, apps, current, logcat, usb.
- `pairspan mcp`: MCP server for Claude, Cursor and VS Code, with `--read-only`.
- Every paired phone appears as an adb device (`127.0.0.1:47100+`).
- Android app redesigned with the website palette, light and dark themes, edge-to-edge system bars and a new adaptive/themed icon.
- `install.sh` (release download, `--source`, `--link-only`, `--uninstall`) and Next.js website with docs in English and Turkish.

### Removed
- Electron app, Node gateway, WebRTC screen streaming, embedded Android ADB client, accessibility/notification/device-admin stubs and privileged permissions (`DUMP`, `READ_LOGS`, `WRITE_SECURE_SETTINGS`, …).
