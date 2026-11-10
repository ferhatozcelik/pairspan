# Contributing

Thanks for looking at Pairspan. Issues and pull requests are welcome.

## What lives where

| Path | Role |
| --- | --- |
| `crates/gateway` | Rust relay server (also serves the website) |
| `crates/core` | Rust PC agent shared by the desktop app and tests |
| `crates/cli` | `pairspan` CLI and MCP server |
| `apps/desktop` | Tauri desktop app |
| `apps/android` | Android app (Kotlin, Compose) |
| `apps/web` | Next.js website and docs |
| `docs/` | Protocol, design rules, shared test vectors |

The wire protocol is specified in [docs/protocol.md](docs/protocol.md). A change to it must update the Rust core, the Android app and the spec together; `crates/core` writes `docs/test-vectors.json` and the Android unit tests replay it.

## Checks before a pull request

```bash
cargo test -p pairspan-gateway -p pairspan-core -p pairspan-cli
(cd apps/android && ./gradlew :app:testDebugUnitTest :app:assembleDebug)
npm install && npm run build
```

UI changes follow [docs/design.md](docs/design.md). Update [CHANGELOG.md](CHANGELOG.md) for user-visible changes.

## Secrets

Never commit keystores, gateway tokens, ADB keys or device identifiers. Android release signing reads `apps/android/.env.example`, which is git-ignored; copy it from `.env.template`. If a secret lands in a commit, rotate it. See [SECURITY.md](SECURITY.md).

## Conduct

This project uses the [Contributor Covenant](CODE_OF_CONDUCT.md).
