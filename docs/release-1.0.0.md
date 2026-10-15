Pairspan 1.0.0 — WebRTC screen streaming update

- Android screen sharing with explicit system consent, hardware H.264 encoding, 720p / 30 FPS target and 2 Mbps initial bitrate.
- Direct WebRTC video in the desktop renderer, rotation, ADB input controls and stream diagnostics.
- Authenticated session reconnect, bounded signaling queues and ICE recovery. Existing ADB/APK workflows and in-memory sessions retained.
- Android versionCode: 6. Update the backend and both clients, then pair devices again.

Validation: backend and desktop builds, Android APK build, WebSocket/ADB regression tests, and a synthetic Electron H.264/UDP rendering test passed. Actual Android/internet latency, sustained 30-minute stability and thermal behavior have not yet been validated. No TURN is configured; restrictive NAT/firewalls can prevent direct streaming. macOS builds use the existing ad-hoc signing distribution model.
