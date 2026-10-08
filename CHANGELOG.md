# Changelog

All notable changes to Pairspan are documented here. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project uses [semantic versions](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-10-08

First public release.

### Added

- Android app (`com.pairspan`) for gateway pairing, wireless ADB trust, and the phone side of the ADB relay.
- Desktop Electron tray app that shows a six-digit code and QR, then connects local `adb` to `127.0.0.1:44755`. macOS: `pairspan-mac-1.0.0.dmg` (universal, not notarized). Windows: `pairspan-windows-1.0.0.exe` (x64 installer, not code-signed, includes `adb.exe`).
- Node.js WebSocket gateway with `/ws`, `GET /health`, single-use pairing codes, and an optional shared token.
- One `pairspan` container serves the landing page, `/p/` pairing redirect, `/ws`, and `/health`.
- Open-source docs: architecture, protocol, component guides, security, and contributing notes.

### Changed

- Android ADB client and QR scanner are vendored in the app. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
- Screenshots and ADB screen mirroring are allowed.

### Not in this release

- No iOS app. Pairspan 1.0.0 does not include an iPhone or iPad client.

[1.0.0]: https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0
