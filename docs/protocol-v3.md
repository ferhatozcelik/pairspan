# Pairspan protocol v3

Protocol v3 replaces v2 completely. A v2 client (Pairspan 1.0.x / 1.1.x) cannot talk to a v3 gateway.

There are two layers:

1. **Gateway layer.** The gateway authenticates each client by its public key and routes opaque binary frames between peers that have agreed to talk to each other. It never sees plaintext.
2. **End-to-end layer.** The PC and the phone run a handshake through the gateway and then exchange AES-256-GCM encrypted frames. Only the two endpoints hold the keys.

All integers are big-endian. `b64` means standard base64 with padding. `hex` means lowercase hex.

---

## 1. Identity

Every installation has one long-term **P-256** key pair (Android Keystore on the phone, the OS keychain on the PC).

- `pub` is the 33-byte SEC1 *compressed* public key.
- The **peer id** is the first 16 bytes of `SHA-256(pub)`. It is written as 32 hex characters in JSON and as 16 raw bytes in binary frames.

## 2. Gateway layer

WebSocket endpoint: `GET /ws`. Text frames carry JSON control messages, and binary frames carry relayed data. A text frame can be at most 64 KiB and a binary frame at most 1 MiB.

### 2.1 Authentication

Right after the upgrade, the gateway sends:

```json
{ "t": "challenge", "v": 3, "nonce": "<b64 32 bytes>" }
```

Within 10 seconds the client answers:

```json
{
  "t": "hello",
  "role": "pc" | "phone",
  "pub": "<b64 33-byte compressed key>",
  "sig": "<b64 ECDSA-P256-SHA256 signature, DER or raw r||s>",
  "peers": ["<peer id hex>", ...],
  "token": "<optional gateway token>"
}
```

`sig` signs the bytes `"pairspan/v3/auth" || nonce`. `peers` lists the peer ids this client trusts, up to 64. `token` is required only when the gateway runs with `GATEWAY_TOKEN`.

On success:

```json
{ "t": "welcome", "id": "<own peer id>", "online": ["<peer id>", ...] }
```

`online` holds the trusted peers that are connected and trust this client back. If the same peer id connects again, the older connection is closed with code `4000` (`replaced`).

### 2.2 Links and presence

The gateway routes data from A to B only when B is online and either:

- A lists B in `peers` **and** B lists A (a *mutual link*), or
- A and B met in a pairing room less than 10 minutes ago (a *temporary link*).

When a mutual link comes up or goes down, both sides receive:

```json
{ "t": "presence", "id": "<peer id>", "online": true | false }
```

A client replaces its trust list at any time:

```json
{ "t": "peers", "ids": ["<peer id hex>", ...] }
```

Removing an id cuts the link immediately, and both sides get `presence` with `online: false`. This is how unpairing works: either side removes the other and the gateway stops routing at once. The gateway keeps no pairing state, so a restart loses nothing.

### 2.3 Pairing rooms

Both endpoints know a **pair secret** `T`: 20 characters from `23456789ABCDEFGHJKMNPQRSTUVWXYZ`, about 99 bits. The PC shows it as `XXXX-XXXX-XXXX-XXXX-XXXX` and in a QR code. The gateway never sees `T`. It sees only

```
room = hex(SHA-256("pairspan/v3/room" || T))      # 64 hex chars
```

PC (role `pc`):

```json
{ "t": "room_open", "room": "<64 hex>" }
→ { "t": "room_opened", "room": "<64 hex>", "expires_in": 300 }
{ "t": "room_close", "room": "<64 hex>" }
```

Phone (role `phone`):

```json
{ "t": "room_join", "room": "<64 hex>" }
```

On a match, the room is consumed and both sides receive the other's identity:

```json
{ "t": "room_peer", "room": "<64 hex>", "id": "<peer id>", "pub": "<b64>", "role": "pc" | "phone" }
```

A temporary link now lets the two run the E2E pairing handshake (§3.2). Once it succeeds, each side adds the other to `peers`. A PC can hold up to 4 open rooms. Each IP gets 20 `room_join` attempts per 5 minutes.

QR payload: `pairspan://pair?v=3&g=<url-encoded gateway base URL>&t=<T>&k=<PC peer id>`. With `k`, the phone checks that `room_peer.id` matches before it starts the handshake.

### 2.4 Binary relay frames

```
client → gateway:  0x01 | dest peer id (16) | payload
gateway → client:  0x01 | source peer id (16) | payload
```

The gateway silently drops frames to peers that are offline or not linked, and counts them in `pairspan_frames_dropped_total`. Each connection has a bounded send queue. A sender waits when the receiver is slow, and a receiver that stays stuck for 10 seconds is disconnected (`4008`, `slow consumer`).

### 2.5 Errors and close codes

```json
{ "t": "error", "code": "<code>", "msg": "<text>" }
```

| Close | Meaning |
|---|---|
| 4000 | replaced by a newer connection with the same peer id |
| 4001 | bad or missing hello / signature |
| 4003 | gateway token rejected |
| 4008 | rate limit, slow consumer, or protocol abuse |
| 4013 | gateway full |

The gateway pings every 20 seconds and drops connections that are silent for 60 seconds.

### 2.6 HTTP endpoints

| Path | Body |
|---|---|
| `GET /health` | `{ "ok": true, "service": "pairspan-gateway", "version", "protocol": 3, "connections", "rooms" }` |
| `GET /metrics` | Prometheus text format |
| anything else | static files from `DOCS_ROOT`, if set |

## 3. End-to-end layer

Everything in this section lives inside the `payload` of a relay frame. The first payload byte is the **kind**.

| Kind | Name |
|---|---|
| `0x10` | `HS_INIT` |
| `0x11` | `HS_RESP` |
| `0x20` | `DATA` |
| `0x2F` | `REKEY` (responder asks initiator to start a new `HS_INIT`) |

### 3.1 Session handshake

The **PC is always the initiator**. It sends `HS_INIT` whenever its peer comes online (a `presence` or `welcome` with the phone listed), and the phone answers.

```
HS_INIT  = 0x10 | e_i (33, compressed ephemeral public key)
HS_RESP  = 0x11 | e_r (33) | confirm (16)
```

Both sides compute, with static keys `s_i`/`s_r` and ephemerals `e_i`/`e_r`:

```
ee = ECDH(e_i, e_r)   es = ECDH(e_i, s_r)   se = ECDH(s_i, e_r)   ss = ECDH(s_i, s_r)
psk = SHA-256("pairspan/v3/psk" || T) during pairing, otherwise 32 zero bytes
th  = SHA-256("pairspan/v3/hs" || s_i_pub || s_r_pub || e_i || e_r)
okm = HKDF-SHA256(salt = th, ikm = ee || es || se || ss || psk, info = "pairspan/v3/keys", L = 96)
k_i2r = okm[0..32]   k_r2i = okm[32..64]   k_confirm = okm[64..96]
confirm = AES-256-GCM(k_confirm, nonce = 12 zero bytes, aad = th, plaintext = empty)   # the 16-byte tag
```

ECDH output is the 32-byte x-coordinate. The initiator verifies `confirm` and drops the session on mismatch.

- `ee` gives forward secrecy.
- `es`/`se` prove possession of each static key, so a gateway that substitutes keys cannot finish.
- `psk` binds the first handshake to the pair secret, so a gateway that swaps keys in `room_peer` cannot pass as either side, even for codes typed by hand.

After pairing, both sides pin each other's `pub`. Later sessions use `psk` = zeros and reject a `room_peer` / `presence` peer whose pinned key differs.

### 3.2 Data frames

```
DATA = 0x20 | counter (8) | ciphertext || tag (16)
nonce = 4 zero bytes || counter        aad = 0x20 || counter
```

Each direction has its own key and its own counter, starting at 0. The receiver accepts only counters greater than the last one it accepted, which rules out replays. After 2^32 frames, or on any decryption failure, the initiator runs a new handshake.

### 3.3 Application channels (inside DATA plaintext)

```
plaintext = channel (1) | stream id (4) | body
```

| Channel | Use |
|---|---|
| `0x00` | JSON RPC: `{"id":n,"m":"<method>","p":{...}}` → `{"id":n,"r":...}` or `{"id":n,"e":{"code","msg"}}`; events have no `id` |
| `0x01` | ADB stream open: body = UTF-8 service name (e.g. `shell:`), stream id picked by the opener |
| `0x02` | ADB stream data |
| `0x03` | ADB stream close |
| `0x04` | Bulk transfer (screenshots, APK push) chunks |

Section 3.3 is a draft. The PC/CLI phase will finalize it.
