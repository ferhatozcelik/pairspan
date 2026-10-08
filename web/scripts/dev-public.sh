#!/usr/bin/env bash
# Runs the gateway and the desktop app on this Mac and exposes the gateway over a temporary public
# tunnel (Cloudflare quick tunnel, or localhost.run if cloudflared is missing), so a phone can pair over mobile data without being on the same Wi-Fi.
# The tunnel only forwards the gateway; a session still needs the single-use pairing token.
# Stop everything with: web/scripts/dev-public.sh stop
set -euo pipefail
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
WEB="$(cd "$(dirname "$0")/.." && pwd)"
LOG="${TMPDIR:-/tmp}/pairspan-dev"
mkdir -p "$LOG"

stop() {
  pkill -f "ssh .*localhost.run" 2>/dev/null || true
  pkill -f "cloudflared tunnel --url" 2>/dev/null || true
  pkill -f "$REPO/mac/node_modules" 2>/dev/null || true
  pkill -f "tsx src/server.ts" 2>/dev/null || true
}

if [ "${1:-}" = "stop" ]; then stop; echo "stopped"; exit 0; fi
stop; sleep 1

(cd "$WEB" && nohup npm start >"$LOG/gateway.log" 2>&1 &)
for _ in $(seq 1 20); do curl -fs localhost:3000/health >/dev/null 2>&1 && break; sleep 1; done

if command -v cloudflared >/dev/null 2>&1; then
  nohup cloudflared tunnel --no-autoupdate --url http://localhost:3000 >"$LOG/tunnel.log" 2>&1 &
  PATTERN='https://[a-z0-9-]+\.trycloudflare\.com'
else
  nohup ssh -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 -R 80:localhost:3000 nokey@localhost.run >"$LOG/tunnel.log" 2>&1 &
  PATTERN='https://[a-z0-9]+\.lhr\.life'
fi
URL=""
for _ in $(seq 1 40); do
  URL="$(grep -oE "$PATTERN" "$LOG/tunnel.log" | grep -v '^https://api\.' | head -1 || true)"
  [ -n "$URL" ] && break; sleep 1
done
[ -n "$URL" ] || { echo "Could not open the tunnel. See $LOG/tunnel.log" >&2; exit 1; }

# A fresh tunnel hostname can take a few seconds to resolve; wait so the desktop app does not start too early.
for _ in $(seq 1 30); do curl -fs --max-time 4 "$URL/health" >/dev/null 2>&1 && break; sleep 1; done

(cd "$REPO/mac" && PAIRSPAN_GATEWAY_URL="$URL" nohup npm start >"$LOG/mac.log" 2>&1 &)
echo "Public gateway: $URL"
echo "Logs: $LOG   (QR payload: grep PAIRSPAN_QR_PAYLOAD $LOG/mac.log | tail -1)"
