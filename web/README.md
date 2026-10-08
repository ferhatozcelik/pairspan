# Pairspan web

Node.js site and WebSocket server for Pairspan 1.0.0. It pairs one desktop session with one phone and relays JSON events. It does not run ADB. Public base URL: `https://pairspan.ferhatozcelik.com/`.

## Run

```bash
npm install
npm start
```

Development reload: `npm run dev`. Typecheck: `npm run typecheck`. Tests: `npm test`.

Local listen address: `0.0.0.0:3000`. The same process serves the landing page, `/p/`, WebSocket `/ws`, and `GET /health`. On the VPS it is published as the single `pairspan` container at `127.0.0.1:16200`.

## Environment

The process reads the environment. The committed names are in `.env.example`:

```text
GATEWAY_PUBLIC_URL=https://pairspan.ferhatozcelik.com
GATEWAY_TOKEN=
```

`GATEWAY_TOKEN` is required on every client `hello` when it is set. `GATEWAY_SECRET` is still read when `GATEWAY_TOKEN` is unset. Leave the token empty only on a trusted network. `web-deploy` writes `temp.env` from `NODE_ENV`, `GATEWAY_PUBLIC_URL`, and `GATEWAY_TOKEN`, then deploys with `ARTIFACT_TOKEN`.

Message shapes are documented in [public/protocol.md](public/protocol.md).

## Tests

`npm test` starts a temporary gateway and covers token rejection, pairing, QR payload and ADB relay. The test token is a fixture inside the test file, not a deployment secret.

## Logs

The server writes one JSON object per line to stdout (`session_opened`, `phone_joined`, `adb_tunnel_open`, `client_left`, `rate_limited`, …) and a `stats` line every five minutes. Set `LOG_LEVEL` to `debug`, `info` (default), `warn` or `error`. Pairing codes and tokens are never logged. `/health` also returns the running counters.
