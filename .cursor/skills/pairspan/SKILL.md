---
name: pairspan
description: Work in the Pairspan repo. Use when editing core, CLI/MCP, desktop app, Android app, website, docs, workflows or releases.
---

# Pairspan

## Layout

| Path | Role |
| --- | --- |
| `crates/core` | Rust PC agent: crypto, Supabase signalling (`signal.rs`), WebRTC link (`rtc.rs`), USB setup (`usb.rs`), ADB tunnel, automation, local JSON-RPC |
| `crates/cli` | `pairspan` CLI and `pairspan mcp`; talks only to the app's local socket |
| `apps/desktop` | Tauri app; bundles the CLI and adb (`npm run desktop`) |
| `apps/android` | Android app, package `com.pairspan`, Kotlin/Compose, WebRTC answerer |
| `apps/web` | Static site on GitHub Pages (`/pairspan` base path) |

No server of our own: signalling is Supabase Realtime (project `jcqkznspriizdmsbjvvj`, public channels, no tables); data is peer to peer. Site: `https://ferhatozcelik.github.io/pairspan/`.

## Rules

- Protocol changes: update `docs/protocol.md`, `crates/core` and the Android app together; keep `docs/test-vectors.json` passing on both sides (it also pins the channel names).
- UI follows `docs/design.md` (website palette everywhere).
- Docs in English (README.tr.md mirrors README.md in Turkish).
- Android `versionCode` only goes up.

## GitHub Actions

| Workflow | When |
| --- | --- |
| `ci.yml` | Every push: Rust tests, Android unit tests + build, website build |
| `pages.yml` | Push to `main` touching the site or `install.sh`: GitHub Pages |
| `release.yml` | Tag `v*`: `Pairspan-X.Y.Z-macos-arm64.zip` + signed `pairspan-android-X.Y.Z.apk` |

Secrets: `KEYSTORE_BASE64`, `KEYSTORE_PASSWORD`, `KEY_ALIAS`, `KEY_PASSWORD`.
