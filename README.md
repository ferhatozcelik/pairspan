<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">
    <img src="web/assets/pairspan-icon.png" alt="Pairspan logo" width="100">
  </a>
</p>

<h1 align="center">Pairspan</h1>

<p align="center">Connect your Android phone to macOS or Windows over wireless ADB.</p>

<p align="center"><strong>Pair once. Connect wirelessly. Deploy effortlessly.</strong></p>

<p align="center">
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0"><img src="https://img.shields.io/badge/release-v1.0.0-39795c" alt="Release v1.0.0"></a>
  <img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Android-39795c" alt="Platforms: macOS, Windows and Android">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-39795c" alt="MIT license"></a>
</p>

<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">Website</a> ·
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0">Download</a> ·
  <a href="#setup-guide">Setup guide</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="README.tr.md">Türkçe</a> ·
  <a href="README.de.md">Deutsch</a> ·
  <a href="README.es.md">Español</a> ·
  <a href="README.fr.md">Français</a> ·
  <a href="README.pt-BR.md">Português (Brasil)</a> ·
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="README.ja.md">日本語</a>
</p>

<p align="center">
  <strong>Support the project</strong><br>
  <a href="https://buymeacoffee.com/ferhatozcelik">Buy Me a Coffee</a> ·
  <a href="https://thanks.dev/donations">thanks.dev</a>
</p>

<br>

<table align="center">
  <tr>
    <th align="center">Android · Connected</th>
    <th align="center">Android · Pairing</th>
    <th align="center">macOS · QR code &amp; token</th>
  </tr>
  <tr>
    <td align="center" valign="top"><img src="docs/screenshots/android-connected.png" alt="Pairspan Android app connected to a computer" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/android-pairing.png" alt="Pairspan Android app with pairing token entry and QR code scanning" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/macos-pairing.png" alt="Pairspan macOS app displaying a pairing token and QR code" width="300"></td>
  </tr>
</table>

## Download

[Release 1.0.0](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0): macOS, Windows, Android.

## Folders

| Piece | Folder |
| --- | --- |
| Android | [`android/`](android/) |
| macOS and Windows | [`mac/`](mac/) |
| Web | [`web/`](web/) |

```bash
cd web && npm install && npm start
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

## Setup guide

Pairspan has two halves: the desktop app (macOS or Windows) and the Android app. Both talk to the gateway at `https://pairspan.ferhatozcelik.com`, so the phone and the computer do not need to be on the same network. One desktop can host several phones at once.

### 1. Install the desktop app

**macOS**

1. Download `pairspan-mac-1.0.0.dmg` from the [release page](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0) and drag Pairspan into Applications.
2. The build is not notarized. If macOS blocks the first launch, right-click Pairspan and choose **Open**.
3. Pairspan lives in the menu bar. Click its icon to see the pairing token and QR code.
4. `adb` is bundled, nothing else to install.

**Windows**

1. Download `pairspan-windows-1.0.0.exe` from the same release page and run it.
2. The installer is not code-signed. If SmartScreen warns, choose **More info**, then **Run anyway**.
3. Pairspan lives in the system tray. Click its icon to see the pairing token and QR code.
4. `adb.exe` is bundled, nothing else to install.

Use the switch in the window header (or **Enabled** in the tray menu) to turn Pairspan on or off. While it is off, no phone can connect.

### 2. Install the Android app

1. Download the APK from the release page and install it (allow installs from your browser or file manager when asked). Android 8.0 or newer is required, and Android 11 or newer for wireless debugging.
2. Open Pairspan and allow notifications and the camera when asked. The camera is only used to scan the QR code.
3. On the phone, open **Settings → About phone**, tap **Build number** seven times, then open **Developer options** and turn on **USB debugging**. Turn on **Wireless debugging** too if your phone has it.

### 3. Grant system access (once)

Pairspan can switch on its own accessibility and notification services and wireless ADB without sending you through Settings. Android only allows that after `WRITE_SECURE_SETTINGS` is granted from a computer. The desktop app does it for you:

1. Connect the phone to the computer with a USB cable, with USB debugging on, and Pairspan installed on the phone.
2. In the desktop window, find **One-time phone permission** and click **Find USB phones**.
3. If the phone shows an "Allow USB debugging?" prompt, accept it and scan again.
4. Click **Grant permission** under your phone. The app runs this for you, using its bundled `adb`, and then switches the phone's ADB to port 5555 so Pairspan can reach it without a cable:

```bash
adb shell pm grant com.pairspan android.permission.WRITE_SECURE_SETTINGS
adb tcpip 5555
```
5. On the phone, tap **Allow** on the "Allow USB debugging?" prompt and tick **Always allow**. You do not have to type any command.

Notes:

- The permission lasts until Pairspan is uninstalled. The port 5555 step lasts until the phone restarts; click **Grant permission** again after a restart.
- If it fails with a security exception, enable **USB debugging (Security settings)** in Developer options (some Xiaomi/Redmi/POCO phones) and try again.
- You can also run the command yourself with any `adb`. The Android app shows it under **Getting ready → System access** with a **Copy command** button.
- For phones without Wireless debugging, run `adb tcpip 5555` once over USB. Pairspan will use that port.

### 4. Pair

1. Make sure the desktop app is enabled and shows **Connected to the Pairspan service**.
2. In the Android app tap **Scan QR code** and scan the code from the desktop, or type the pairing token.
3. The phone appears under **Connected devices** on the desktop with its **Device ID** and the desktop's **Client ID**. The same two ids are in the Android app under **Settings**.
4. To add another phone, repeat this pairing step with the new code (a used code is replaced right away).

### 5. Self-host

The desktop app and the Android app never open ADB to each other. Both connect to the gateway in [`web/`](web/). The public site is `https://pairspan.ferhatozcelik.com`. To run your own, start that process and build both apps with the same URL and token.

### Requirements

- Node.js 22, or Docker
- A hostname with TLS if the gateway is reachable from the internet. Clients use `https://` and the WebSocket `wss://…/ws`
- One `GATEWAY_TOKEN` shared by the server, the desktop app, and the Android app

Sessions stay in memory. Restarting the process drops them. The gateway does not run `adb`. The desktop app does, on `127.0.0.1` only.

### Environment

`web/.env.example`, `mac/.env.example`, and `android/.env.example` use the same two lines. Set the public URL to the address phones and computers can open. Leave the token empty in git.

```text
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=
```

`GATEWAY_PUBLIC_URL` is written into pairing QR codes. `GATEWAY_TOKEN` is sent on every `hello`. If the server token is empty, anyone who can open `/ws` can join. Set a long random token before the process is reachable from the internet.

```bash
openssl rand -hex 32
```

Do not commit the token. Put it in the environment of the process you run, or in `web/temp.env` for Docker. `temp.env` is not part of the repo.

### Run with Node

```bash
cd web
npm install
export GATEWAY_PUBLIC_URL=https://pairspan.example.com
export GATEWAY_TOKEN='paste-the-token'
npm start
```

The process listens on `0.0.0.0:3000`. It serves the landing page, `/p/`, WebSocket `/ws`, and `GET /health`.

```bash
curl -fsS http://127.0.0.1:3000/health
```

A healthy process returns `{"ok":true,"service":"pairspan",...}`.

### Run with Docker

From `web/`:

```bash
docker build -t pairspan:latest .
```

Write `web/temp.env`:

```text
NODE_ENV=production
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=paste-the-token
```

[`web/docker-compose.yml`](web/docker-compose.yml) reads that file and publishes `127.0.0.1:16200` to container port `3000`.

```bash
docker compose up -d
curl -fsS http://127.0.0.1:16200/health
```

Put a TLS reverse proxy in front of `127.0.0.1:16200`. Forward WebSocket upgrades on `/ws`. Check the public URL the same way:

```bash
curl -fsS https://pairspan.example.com/health
```

### Point the apps at your gateway

Write the same URL and token into `mac/.env.example` and `android/.env.example`, then rebuild. The desktop app reads `.env.example` at launch. The Android app copies both values into the APK at compile time.

```bash
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

A client built with a different token is rejected. The desktop window should say **Connected to the Pairspan service** before you scan the QR code. Then follow [Pair](#4-pair) above.

## 6. Use the relay

Each phone gets its own local port on the computer, starting at `127.0.0.1:44755` and going up. The desktop window lists the `adb connect` target for every phone.

```bash
adb devices
adb -s 127.0.0.1:44755 shell id
```

If `adb devices` shows a phone as `authorizing`, unlock it and accept the "Allow USB debugging?" prompt, then tick **Always allow**.

To disconnect a phone, tap **Remove connection** in the Android app. To disconnect everything, turn the desktop switch off.

## License

[MIT](LICENSE)

## Security and responsible use

Use Pairspan only with devices you own or have explicit permission to access, for legitimate development, debugging and device management. Do not use ADB for unauthorized access, covert monitoring, data theft, malware or bypassing security controls.

In the USB setup described above, the device owner must enable USB debugging, unlock the phone and approve Android’s debugging authorization prompt. Only approve computers you trust. Pairing or automatic reconnection must respect this authorization and must not override revoked permissions. See [Android’s ADB documentation](https://developer.android.com/tools/adb).

Keep pairing tokens and ADB keys private. To end access, remove the Pairspan connection and revoke USB debugging authorizations in Android’s Developer options; disable debugging when no longer needed. Follow Android’s security and permission model and the applicable [Google Play Device and Network Abuse policy](https://support.google.com/googleplay/android-developer/answer/16559646). USB authorization alone does not establish Google Play policy compliance.
