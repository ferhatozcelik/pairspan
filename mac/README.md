# Pairspan for macOS

Electron tray app for Pairspan 1.0.0. It pairs with the Android app through the gateway and exposes the ADB of every paired phone on its own local port, starting at `127.0.0.1:44755`.

This is a macOS desktop app. It is not an iOS app.

## Requirements

- Node.js 22 or newer
- Nothing else for the installed app: `adb` is bundled. When running from source, run `node scripts/fetch-mac-adb.mjs` (or `fetch-win-adb.mjs`) once, or have `adb` in the Android SDK or on `PATH`.

## Install

Download [pairspan-mac-1.0.0.dmg](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0). It is a universal build and is not notarized. If macOS blocks the first open, right-click Pairspan and choose Open.

`npm run dist` rebuilds that DMG into `release/`.

`npm run install:debug` quits Pairspan, builds the arm64 app, and replaces `/Applications/Pairspan.app`.

## Windows

Download [pairspan-windows-1.0.0.exe](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0). It is an x64 installer and is not code-signed. If SmartScreen warns, choose More info, then Run anyway. `adb.exe` is inside the app.

`npm run dist:win` rebuilds that installer into `release/`.

## Run from source

```bash
npm install
npm run build
npm start
```

`npm run dev` starts Vite and Electron together for UI work.

## Settings

Open the tray window and set the gateway URL and optional token under **Settings**.

`npm start` and the installed app read `.env.example`:

```text
GATEWAY_PUBLIC_URL=https://pairspan.ferhatozcelik.com
GATEWAY_TOKEN=
```

`GATEWAY_TOKEN` is sent on every `hello`. Leave it empty only when the server does not require one.

## Grant the phone permission

**One-time phone permission** in the window lists phones attached by USB. **Grant permission** runs `adb -s <serial> shell pm grant com.pairspan android.permission.WRITE_SECURE_SETTINGS` with the bundled `adb`.

## What the tray does

1. Connects to `GATEWAY_URL/ws` as role `mac`.
2. Shows the pairing token and QR from the gateway `welcome` message.
3. Lists every connected phone with its device id and this app's client id. Up to 8 phones can be paired.
4. When a phone sends `adb_tunnel_ready`, listens on a free port from `127.0.0.1:44755` upward and relays bytes for that phone.
5. Runs `adb connect` for each port and retries after a drop.

The switch in the header, or **Enabled** in the tray menu, turns the whole app on or off. The choice and the client id are kept in `settings.json` in the app's user data folder.

Check the link:

```bash
adb -s 127.0.0.1:44755 shell id
```
