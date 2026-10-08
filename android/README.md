# Pairspan for Android

Android client for Pairspan 1.0.0. Package `com.pairspan`, `minSdk` 26, `targetSdk` 35. Wireless ADB pairing needs Android 11 or newer.

There is no iOS counterpart in this repository.

## Build

JDK 17 and the Android SDK are required. `local.properties` is created by Android Studio and is not committed.

```bash
./gradlew :app:assembleDebug
```

Output: `app/build/outputs/apk/debug/app-debug.apk`.

Version 1.0.0 is `versionName` `1.0.0`, `versionCode` 5.

## Configure

The build reads `.env.example`:

```text
GATEWAY_PUBLIC_URL=https://pairspan.ferhatozcelik.com
GATEWAY_TOKEN=
KEYSTORE_FILE=pairspan.keystore
KEYSTORE_PASSWORD=
KEY_ALIAS=pairspan
KEY_PASSWORD=
```

`GATEWAY_TOKEN` is sent on every `hello`. Leave it empty only when the server does not require one. Put `pairspan.keystore` next to this file for release signing. Do not commit the keystore or its passwords. CI uses `KEYSTORE_BASE64`, `KEYSTORE_PASSWORD`, `KEY_ALIAS`, and `KEY_PASSWORD`.

## First connection

1. Start the [web process](../web/README.md) and the [desktop app](../mac/README.md).
2. Install the APK and open Pairspan.
3. If one desktop is waiting, the phone joins without a code. Otherwise scan the desktop QR or enter the pairing token.
4. On first wireless ADB trust, pair the phone in Android **Wireless debugging**. Pairspan then connects to the phone's own `adbd`. It does not ask for Android's pairing code.

When the phone reports ADB ready, the Mac runs `adb connect 127.0.0.1:44755`.

## Permissions

The app asks for the permissions it needs to stay connected: network, notifications, a connected-device foreground service, camera for the QR scanner, boot completion to resume a saved session, and wireless debugging. Device admin is listed in the app and does not grant ADB or root. See [SECURITY.md](../SECURITY.md).
