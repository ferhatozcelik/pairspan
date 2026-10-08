# Gateway protocol

The gateway listens on `PORT` (default `3000`).

| Endpoint | Behavior |
| --- | --- |
| `GET /health` | JSON `{ ok, service, sessions, requestId }`. |
| `GET` anything else | `404`. |
| WebSocket `/ws` | JSON text frames. |

Clients send `hello` before any other message. When `GATEWAY_TOKEN` or `GATEWAY_SECRET` is set, `hello.token` (or `hello.auth`) must match. The comparison is length-checked and timing-safe.

## Hello

Desktop:

```json
{ "type": "hello", "role": "mac", "token": "" }
```

The desktop may add a stable `clientId` (32 hex chars) to `hello`. The gateway replies with `welcome`, including `sessionId`, `clientId`, `pairCode`, `qrPayload`, and `expiresAt`.

Several phones can join one desktop session (up to 8). A phone sends a stable `deviceId` (32 hex chars) in `hello`. Events from a phone reach the desktop with that `deviceId` added by the gateway. The desktop addresses one phone by putting `deviceId` next to `event` on its message, and without it the event goes to every phone.

`qrPayload` is `pairspan://pair?code=<20-character token>&g=<encoded base URL>`. The base URL is `GATEWAY_PUBLIC_URL` when set (`https://pairspan.ferhatozcelik.com`), otherwise the scheme and host the socket connected to.

Phone, by code:

```json
{ "type": "hello", "role": "phone", "pairCode": "123456", "token": "" }
```

Phone, direct, only when one desktop session exists:

```json
{ "type": "hello", "role": "phone", "direct": true, "token": "" }
```

Phone, resume:

```json
{ "type": "hello", "role": "phone", "sessionId": "<48 hex chars>", "token": "" }
```

A successful phone join receives `welcome` with `role`, `sessionId`, `clientId` and `deviceId`. The desktop receives `event` / `phone_online`. The used code rotates.

## Events

After `hello`, either side may send:

```json
{ "type": "event", "event": "<name>", "payload": {} }
```

The gateway forwards the event to the other peer. Event names are at most 64 characters.

| Event | Sender | Notes |
| --- | --- | --- |
| `shell_request` | mac | `payload.id` is a UUID, `command` is at most 512 characters, `sentAt` is within 30 seconds. |
| `shell_result` | phone | Forwarded to the desktop. |
| `adb_tunnel_open` | mac | Starts a relay identified by `payload.id`. |
| `adb_tunnel_ready` | phone | Phone accepted the tunnel. |
| `adb_tunnel_data` | either | `payload.data` is at most 200000 characters. |
| `adb_tunnel_close` | either | Closes that tunnel id. |

The desktop may also send `{ "type": "rotate_pair" }` to replace the current code.

## Errors

Failures are `{ "type": "error", "message": "..." }` and often a socket close. Close codes used by the gateway include `4000` replaced, `4001` bad code, `4002` expired, `4003` unauthorized, `4004` desktop offline, `4005` more than one desktop, and `4008` rate limited.

Pairing attempts are counted per remote address and reset on the same five-minute window as the code lifetime. More than 20 attempts in that window are rejected.
