#!/bin/sh
# Isolated JVM regression check; Android APIs are replaced by test-only stubs.
set -eu
cd "$(dirname "$0")/../.."
mkdir -p build/tunnel-check/classes
javac -d build/tunnel-check/classes \
  test/tunnel/src/android/util/Base64.java \
  test/tunnel/src/org/json/JSONObject.java \
  test/tunnel/src/com/pairspan/app/ShellBridge.java \
  test/tunnel/src/com/pairspan/app/TunnelCheck.java \
  app/src/main/java/com/pairspan/app/AdbTunnel.java
java -cp build/tunnel-check/classes com.pairspan.app.TunnelCheck
