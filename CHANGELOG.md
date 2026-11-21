# Changelog

All notable changes to Pairspan are documented here. This project uses [semantic versions](https://semver.org/).

## [1.1.0] — 2026-11-21

No server of our own any more. Pair your phones again (plug them in once).

### Changed
- Signalling runs on Supabase Realtime (presence + broadcast); data goes directly between the computer and the phone over a WebRTC data channel. The end-to-end encryption above it is unchanged.
- Pairing happens over the USB cable: plug a phone in and the desktop app installs or updates the Android app, pairs both sides and turns on wireless ADB by itself. No QR codes or typed codes.
- Desktop: an on/off switch, step-by-step USB setup, About with an update check. The gateway, automation, logs and QR panels are gone.
- Android: an on/off switch and every setup step with its state (USB debugging, paired computer, wireless ADB, notifications, battery). No QR scanner, gateway address or device id.
- Website and `install.sh` are on GitHub Pages: https://ferhatozcelik.github.io/pairspan/

### Removed
- The Rust gateway, its Docker image and the VPS deploy workflow.

## [1.0.0] — 2026-11-12

A clean start. Earlier builds (Electron desktop app, Node gateway, protocol v2) are not compatible; pair your phones again.

### Added
- Desktop: Control has a **Multi view** that shows every reachable phone live side by side, each tile controllable on its own, with an optional "Mirror input" that sends the same taps, swipes, keys and text to all of them.
- Desktop: a status card in the sidebar shows the gateway (live/offline), latency, phones online and automation at a glance. Settings can reset to the default gateway (wss://pairspan.ferhatozcelik.com/ws) and take a gateway token.
- Rust gateway: signature-authenticated clients, pairing rooms that never see the pair code, mutual-trust binary routing, backpressure, `/health` and Prometheus `/metrics`. Also serves the website; one 5.7 MB container.
- End-to-end encryption between computer and phone: P-256 handshake (ephemeral + static ECDH, pair-code PSK on first pairing), HKDF-SHA256, AES-256-GCM with replay protection. Spec in `docs/protocol.md`.
- Desktop app (Tauri) styled like the website, with menu-bar icon, QR pairing, USB first-time setup (install the Android app, enable wireless ADB) and a local automation socket.
- Control view in the desktop app: live phone screen over ADB (paired phones, USB or Wi-Fi adb) with tap, swipe, long-press, scroll, keys, typing, `adb shell` and screenshots.
- `pairspan` CLI bundled in the app: devices, pair, shell, adb, tap, click, swipe, text, key, elements, dump, screenshot, launch, stop, install, apps, current, logcat, usb.
- `pairspan mcp`: MCP server for Claude, Cursor and VS Code, with `--read-only`.
- Every paired phone appears as an adb device (`127.0.0.1:47100+`).
- Android app redesigned with the website palette, light and dark themes, edge-to-edge system bars and a new adaptive/themed icon.
- `install.sh` (release download, build from a checkout, `--link-only`, `--uninstall`); it signs the app ad hoc and clears quarantine so macOS opens it without the "can't verify" prompt and Next.js website with docs in English and Turkish.

### Removed
- Electron app, Node gateway, WebRTC screen streaming, embedded Android ADB client, accessibility/notification/device-admin stubs and privileged permissions (`DUMP`, `READ_LOGS`, `WRITE_SECURE_SETTINGS`, …).
