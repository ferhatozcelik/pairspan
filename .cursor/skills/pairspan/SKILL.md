---
name: pairspan
description: Work in the Pairspan repo. Use when editing pairspan, its README, docs, Android app, Mac or Windows tray app, web service, env files, or the 1.0.0 GitHub release.
---

# Pairspan

## Layout

| Path | Role |
| --- | --- |
| `android/` | Android app, package `com.pairspan` |
| `mac/` | Electron tray app for macOS and Windows |
| `web/` | Site, WebSocket server, landing page |

There is no iOS app. The public base URL is `https://pairspan.ferhatozcelik.com`. Clients open `wss://pairspan.ferhatozcelik.com/ws`.

## Env

Read and edit only `web/.env.example`, `mac/.env.example`, and `android/.env.example`. Do not create another env file.

## Docs

Write README and markdown docs in English. Keep the root README short: what it is, the site, the 1.0.0 download, and the three folders.

## Release 1.0.0

Tag `v1.0.0`. Assets: `pairspan-mac-1.0.0.dmg`, `pairspan-windows-1.0.0.exe`, `pairspan-android-1.0.0.apk`. Android `versionName` is `1.0.0`.

## GitHub Actions

| Workflow | When |
| --- | --- |
| `web-deploy.yml` | Push to `main`. Kut deploy. |
| `mac-release.yml` | Tag `v*` or manual. Uploads the DMG. |
| `android-release.yml` | Tag `v*` or manual. Uploads the APK. |

Secrets: `ARTIFACT_TOKEN`, `GATEWAY_PUBLIC_URL`, `GATEWAY_TOKEN`, `NODE_ENV`. Release builds write the two `GATEWAY_*` values into `.env.example` on the runner. Do not commit the token.
