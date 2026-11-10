---
name: pairspan
description: Work in the Pairspan repo. Use when editing the gateway, core, CLI/MCP, desktop app, Android app, website, docs, workflows or releases.
---

# Pairspan

## Layout

| Path | Role |
| --- | --- |
| `crates/gateway` | Rust relay + website server (one Docker image, `Dockerfile` at the root) |
| `crates/core` | Rust PC agent: crypto, gateway client, ADB tunnel, automation, local JSON-RPC |
| `crates/cli` | `pairspan` CLI and `pairspan mcp`; talks only to the app's local socket |
| `apps/desktop` | Tauri app; bundles the CLI and adb (`npm run desktop`) |
| `apps/android` | Android app, package `com.pairspan`, Kotlin/Compose |
| `apps/web` | Next.js static site and docs |

Public base URL `https://pairspan.ferhatozcelik.com`; clients use `wss://pairspan.ferhatozcelik.com/ws`.

## Rules

- Protocol changes: update `docs/protocol.md`, `crates/core` and the Android app together; keep `docs/test-vectors.json` passing on both sides.
- UI follows `docs/design.md` (website palette everywhere).
- Docs in English (README.tr.md mirrors README.md in Turkish).
- Version 1.0.0 everywhere; Android `versionCode` only goes up.

## GitHub Actions

| Workflow | When |
| --- | --- |
| `ci.yml` | Every push: Rust tests, Android unit tests + build, website build |
| `web-deploy.yml` | Push to `main` touching gateway/web: devops pipeline builds the root `Dockerfile` (`working_directory: .`) |
| `release.yml` | Tag `v*`: `Pairspan-X.Y.Z-macos-arm64.zip` + signed `pairspan-android-X.Y.Z.apk` |

Secrets: `ARTIFACT_TOKEN`, `DEVOPS_PAT`, `GATEWAY_PUBLIC_URL`, `GATEWAY_TOKEN`, `NODE_ENV`, `KEYSTORE_BASE64`, `KEYSTORE_PASSWORD`, `KEY_ALIAS`, `KEY_PASSWORD`.
