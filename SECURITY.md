# Security

Pairspan connects a desktop you control to a phone you control. Treat the gateway token, pairing QR, and ADB session as access to that phone's `shell` user.

## Reporting a vulnerability

Open a private report with [GitHub Security Advisories](https://github.com/ferhatozcelik/pairspan/security/advisories/new). Do not file a public issue for an unfixed vulnerability, and do not include live tokens, ADB keys, or device identifiers.

## What you should run

- Put a gateway that faces the internet behind TLS (`wss://`) and set a long random `GATEWAY_TOKEN`.
- Scan pairing QR codes only from a desktop you trust. The QR carries the gateway URL.
- The desktop ADB relay binds to `127.0.0.1:44755` only.
- Pairing tokens are 20 characters, single use, and expire after about five minutes. Failed attempts are rate limited.
- `shell_request` messages are limited to 512 characters, require a request id, and must fall inside a 30 second timestamp window.
- On a normal Android `user` build, ADB runs as the `shell` user (UID 2000). Device admin does not grant root.

## Android permissions

The manifest declares network, foreground-service, notification, camera, boot, and wireless-debugging related permissions so pairing and the relay can run. It also declares privileged permissions such as `WRITE_SECURE_SETTINGS`, `READ_LOGS`, and `DUMP`. A normal sideload does not grant those privileged permissions.

`PairspanAccessibilityService` is registered and currently ignores accessibility events. `PairspanDeviceAdminReceiver` is registered with an empty policy list. Neither one raises ADB to root. Review `android/app/src/main/AndroidManifest.xml` before you install a build.

The Android UI does not set `FLAG_SECURE`. Screenshots and ADB screen mirroring are allowed, so do not leave the gateway token on screen while sharing the display.

## Desktop token storage

The macOS app stores the gateway token with Electron safe storage when the OS provides it. If safe storage is unavailable, the token is not written to disk and must be entered again on the next launch.

## Public site

The `pairspan` container serves the landing page, the `/p/` pairing redirect, and the WebSocket gateway. The gateway relays pairing messages. It does not terminate ADB. The desktop relay stays on `127.0.0.1:44755`.
