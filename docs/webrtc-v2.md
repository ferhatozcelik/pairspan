# Pairspan WebRTC screen streaming

The existing ADB bridge and APK installation path are retained. Video uses a separate
native WebRTC RTP path. No database, Redis, managed realtime service, media backend,
TURN service, or WebSocket video transport was added.

## Use

1. Update the backend, desktop and Android app together. Existing pairings from the
   old protocol need to be paired again because reconnect now requires a per-member
   secret. The desktop `settings.json` and `clientId` are unchanged.
2. Pair as before, then click **View screen** beside the phone on the desktop.
3. Tap the screen-sharing notification on Android (or **Allow requested screen
   sharing** in the app). Accept Android's system screen-sharing prompt. A request
   expires after 60 seconds. Requests cannot launch capture without this consent.
4. Tap/drag the video for ADB tap/swipe; use Back/Home/Apps. Android ADB authorization
   is still required for control. USB and tunneled `adb install` remain available.
5. Stop from either the desktop or Android's foreground notification/system sharing
   control. Capture, peer, track, texture helper, EGL and worker resources are released.

The capture profile fits inside 1280×720, or 720×1280 in portrait, preserving aspect
ratio (an extra-tall phone will have a narrower image). Target FPS is 30. Native
WebRTC starts at 2 Mbps and adapts within a 250 kbps–4 Mbps connection bitrate range.
Hardware H.264 is required on Android; unsupported devices report an error rather
than quietly switching to CPU encoding. Electron uses Chromium's available decoding
path without disabling hardware acceleration; availability must be checked on the
actual Mac. No hardware-decoding guarantee is inferred from codec negotiation.

Capture uses a SurfaceTexture and native WebRTC MediaCodec integration. Capture
frames arriving above the target rate are skipped before entering the video source.
There is no bitmap, base64 video, main-process video, or application frame queue.
The WebRTC encoder, congestion controller and jitter buffer retain their standard
realtime behavior; unsupported OEM codec controls are not forced.

## Signaling and recovery

The existing `/ws` endpoint carries `stream_request`, `stream_ready`, `stream_offer`,
`stream_answer`, `stream_ice`, `stream_stats`, `stream_status`, and `stream_stop`.
All carry a random `streamId`; desktop messages name a single `deviceId`. The
backend validates direction, size, UDP candidates and active session membership.
The renderer owns the RTCPeerConnection and `<video>`; IPC carries only control
messages and small statistics.

Membership secrets rotate on successful reconnect. They expire after ten minutes
without a live heartbeat, and are kept only in the backend Map. Android stores its
resume secret in private SharedPreferences; the desktop keeps its secret in RAM.
Unknown, expired or mismatched membership cannot resume a session. Pair codes still
expire after five minutes and rotate when consumed. A single process serializes
membership changes to prevent concurrent code reuse and stale connection writes.

WebSocket limits: 64 pending messages / 2 MiB per client, 2048 pending messages
across the process, 2 MiB outbound buffered data, 1024 messages / 8 MiB per second,
512 simultaneous sockets. Overflow closes the connection instead of accumulating
messages. ADB commands have a 16-entry Android queue and explicitly reject overflow.
ADB data is never silently dropped within a live tunnel. Existing tunnel reconnection
handles a signaling connection closed by backpressure.

A temporary signaling outage preserves the Android capture for up to 30 seconds.
The renderer retains the selected stream while the gateway reconnects and negotiates
again. ICE disconnect/failure triggers up to three bounded restart attempts. A
capture that has stopped always needs fresh Android consent, including after process
death or screen-sharing revocation. A different stream cannot replace an active
capture without stopping it first.

## Deployment and connectivity

Internet gateway URLs must use HTTPS/WSS; cleartext is accepted only for loopback
and private LAN addresses / `.local` development hosts. Terminate TLS at the existing
reverse proxy; set `REQUIRE_WSS=1`, `TRUST_PROXY=1`, and have that proxy overwrite
`X-Forwarded-Proto` and `X-Forwarded-For`. The supplied Compose backend port is bound
to loopback. Never trust forwarded headers on an internet-exposed backend port.
`GATEWAY_PUBLIC_URL` should be the externally reachable WSS/HTTPS base URL.

`ALLOWED_ORIGINS` is an exact comma-separated origin allowlist. Browser-originated
WebSockets are rejected by default. Native Android and desktop signaling sockets
have no Origin header. Keep `GATEWAY_TOKEN` configured for private deployments as
before; membership authentication applies regardless of that shared gateway token.

Default ICE configuration: `stun:stun.l.google.com:19302`. There is no TURN service.
Networks with restrictive NAT/firewalls may not connect; the UI reports failure.
TCP ICE candidates and non-UDP selected media paths are rejected. There is no
fallback to WebSocket video or ADB screen mirroring.

Future ICE servers can be supplied to the desktop through `WEBRTC_ICE_SERVERS`, a
JSON array of RTCIceServer objects. It is shared with the authorized Android peer
in the stream request. Use UDP TURN URLs if enabling TURN later, and provide only
short-lived TURN credentials. No protocol redesign is needed.

## Diagnostics

The diagnostics panel displays capture, encoded, decoded/rendered frame rates,
received bitrate, RTT, packet loss, jitter, jitter-buffer time, receiver drops,
capture frame skips, encoder/decoder time per frame, ICE state and codec/transport.
Missing platform statistics display `—`, not zero. Encoder/decoder implementation
and power-efficient decoder indication are shown when exposed by the platform.

Estimated capture-to-display P50/P95/P99 is calculated only when the browser supplies
`requestVideoFrameCallback.captureTime`; it depends on remote timestamp and clock
estimation. Otherwise it is unavailable. RTT is never relabeled as video latency.
Latency percentiles cover the latest 1800 timestamped frames. For defensible glass-to-
glass measurements, film a source timer and the receiver together with a high-speed
camera and calculate differences from those samples.

**Export stats** downloads at most 1800 one-second samples (30 minutes), including
Android measurements, receiver measurements and recovery/render-gap values. A
>500 ms render gap is a potential freeze, but a static screen can produce the same
observation. Evaluate it alongside a continuously moving test scene.

## Verification and remaining device matrix

Automated checks:

```sh
npm run build --prefix web
npm test --prefix web
npm run build --prefix mac
npm test --prefix mac
npm run test:stream --prefix mac
sh android/test/tunnel/run.sh
# From android/, with the project's Gradle cache:
GRADLE_USER_HOME="$PWD/.gradle" ./gradlew :app:assembleDebug
```

The Electron test runs the actual built renderer with a synthetic canvas sender. It
checks H.264 over UDP, frames rendered, ICE restart, signaling reconnect, rotation,
ADB command dispatch and teardown. Its mocked capture/encoded FPS are fixture inputs,
not Android measurements. Raw results are in
`mac/node_modules/.cache/stream-smoke/results.json`.

Real-device acceptance remains a separate required test:

| Scenario | Procedure / evidence |
|---|---|
| Same LAN | Moving 720p scene, export diagnostics and measure glass-to-glass latency |
| Separate Wi-Fi | Repeat with distinct external IPs; record selected candidate and failures |
| Mobile ↔ home | Repeat, document carrier NAT and direct UDP success/failure |
| 30-minute sustained | Moving scene; export samples; inspect P50/P95/P99, drops, gaps and memory slope |
| Loss/jitter | Impair the test link, record exact settings and recovery time |
| Rapid changes | Scroll/draw continuously, compare capture/encoded/rendered rates |
| Rotation | Rotate repeatedly, verify aspect ratio and tap/swipe coordinates |
| Lifecycle | Background/foreground, screen lock, revoke consent, process restart |
| Signaling reconnect | Interrupt WSS for <30 s and >30 s; verify recovery versus fresh consent |
| ICE restart | Change network and use Retry ICE; check bounded failure without TCP fallback |
| APK install | Run authorized `adb install -r` while streaming; inspect FPS and command result |
| Resources | Capture CPU, RSS, Android thermal status and battery temperature over 30 minutes |

No Android/internet latency percentile or sustained-performance result is claimed
until these device tests have been performed.

References: [Android MediaProjection lifecycle](https://developer.android.com/media/grow/media-projection),
[WebRTC texture capture](https://webrtc.googlesource.com/src/+/refs/heads/main/sdk/android/api/org/webrtc/ScreenCapturerAndroid.java),
[Android native WebRTC distribution](https://github.com/webrtc-sdk/android).

### Recorded local run (2026-10-15)

Backend: 4 integration tests passed. Desktop ADB: 3 tests passed. Android ADB
tunnel JVM regression passed. Backend/desktop builds and Android `assembleDebug`
passed. The generated debug APK is `android/app/build/outputs/apk/debug/app-debug.apk`.

The synthetic renderer run received H.264/UDP at 1280×720, then 720×1280 after
rotation. Sampled render rates were 29.0, 29.9 and 31.1 FPS; receiver dropped frames
and >500 ms render gaps were zero in that run. Sampled decode times were 1.7, 1.5
and 1.4 ms/frame. ICE restart, simulated signaling reconnect, control dispatch and
cleanup assertions passed. These short local observations are not a sustained
performance or Android encoder result. Capture timestamps were unavailable, so
video-latency percentiles were not measured.

[Recorded raw smoke result](webrtc-v2-smoke-results.json). Real-phone validation
requires Android user consent and has not been performed in this implementation run.
