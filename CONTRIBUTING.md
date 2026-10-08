# Contributing

Thanks for looking at Pairspan. Issues and pull requests are welcome on the public repository.

## What lives where

| Path | Role |
| --- | --- |
| `android/` | Android app. Gradle, Kotlin, Java. |
| `mac/` | Electron tray app. React UI, Electron main process. |
| `web/` | Site, WebSocket server, landing page, scripts, and icons. |

Keep a change inside the piece it belongs to. Do not add a new shared package unless two of these pieces already need the same code.

## Local setup

Use Node.js 22 or newer for `web/` and `mac/`. Use JDK 17 for Android. Android wireless pairing is exercised on Android 11 or newer.

```bash
cd web && npm install && npm test
cd mac && npm install && npm run build
cd android && ./gradlew :app:assembleDebug
```

Settings live in each package's `.env.example`. Do not add another env file.

## Pull requests

- Describe the behavior you changed and how you checked it.
- Keep the diff limited to that behavior.
- Update [CHANGELOG.md](CHANGELOG.md) when the change is user-visible.
- Update the matching doc in `web/public/`, `android/README.md`, `mac/README.md`, or `web/README.md` when setup or protocol behavior changes.
- Do not add generated `build/`, `dist/`, `node_modules/`, or IDE project files.

## Secrets

Never commit keystores, gateway tokens, ADB keys, or device identifiers. The only env file is `.env.example`. If a secret lands in a commit, rotate it and open a GitHub security advisory. See [SECURITY.md](SECURITY.md).

## Conduct

This project uses the [Contributor Covenant](CODE_OF_CONDUCT.md).
