Run `sh android/test/tunnel/run.sh` from the repository root with JDK 17 or newer.

This compiles the production `AdbTunnel` against minimal test-only Android/JSON stubs. It checks that retired readers, stale remote closes, and queued writes cannot close a replacement tunnel, and that EOF cleanup reports the correct tunnel exactly once. It does not exercise Android services, discovery, or real-device networking.
