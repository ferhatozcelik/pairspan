# Pairspan protocol

Protocol version **2**, used by Pairspan 1.1.0 (desktop app, CLI and Android app). Not compatible with 1.0.0.

There are two layers:

1. **Transport.** The PC and the phone find each other on Supabase Realtime channels (presence + broadcast) and connect directly over a WebRTC data channel. Nothing secret travels over signalling.
2. **End-to-end layer.** Over the data channel the PC and the phone run a handshake and then exchange AES-256-GCM encrypted frames. Only the two endpoints hold the keys.

All integers are big-endian. `b64` means standard base64 with padding. `hex` means lowercase hex.

---

## 1. Identity

Every installation has one long-term **P-256** key pair (Android Keystore on the phone, the OS keychain on the PC).

- `pub` is the 33-byte SEC1 *compressed* public key.
- The **peer id** is the first 16 bytes of `SHA-256(pub)`. It is written as 32 hex characters in JSON and as 16 raw bytes in binary frames.

## 2. Transport

### 2.1 Signalling channels

One Phoenix WebSocket to `wss://<project>.supabase.co/realtime/v1/websocket?apikey=<publishable key>&vsn=1.0.0`, joining public channels (`private: false`) named `realtime:pairspan:<name>`. Every message the apps send is a broadcast with event `sig` and a JSON payload; each side also tracks presence with key `pc` or `phone`. Heartbeat every 25 seconds.

| Channel | `<name>` |
|---|---|
| Pairing room | `r:` + first 40 hex characters of `SHA-256("pairspan/v1/room" ‖ T)` |
| Link | `l:` + first 40 hex characters of `SHA-256("pairspan/v2/link" ‖ pc_pub ‖ phone_pub)` |

`T` is the pair secret and `*_pub` are compressed public keys (§1). Only the two devices know both keys, so only they can name a link channel. `docs/test-vectors.json` pins both names.

### 2.2 Pairing over USB

1. The PC creates `T` (20 characters from `23456789ABCDEFGHJKMNPQRSTUVWXYZ`, ~99 bits, valid for 2 minutes) and joins the room.
2. Over USB it opens the app (`am start`) and sends `adb shell am broadcast -n com.pairspan/com.pairspan.app.link.PairReceiver --es secret T --es pc <PC peer id>`. The receiver requires `android.permission.DUMP`, which `adb shell` holds and other apps cannot get.
3. The phone joins the room and repeats `{ "t": "phone", "pub": "<b64>" }` every 1.5 s.
4. The PC answers `{ "t": "pc", "pub": "<b64>" }`. The phone accepts it only if its peer id equals the id from step 2.
5. Both join their link channel; the WebRTC link (§2.3) comes up and the PC starts the handshake with `psk = SHA-256("pairspan/v1/psk" ‖ T)` (§3.1). The phone keeps the pairing once the PC's first DATA frame decrypts; the PC keeps it when the handshake completes.

Afterwards the PC runs `adb tcpip 5555` over USB so the phone's adbd listens for the tunnel. After a phone reboot, plugging it in again repeats only this step.

### 2.3 Link

The PC is always the offerer.

| Message | From | When |
|---|---|---|
| `{ "t": "want" }` | phone | after joining the link channel, when the PC's presence appears, and when its link drops |
| `{ "t": "offer", "g": <n>, "sdp": { "type": "offer", "sdp": "…" } }` | PC | on `want`, or when the phone's presence appears and no link is up |
| `{ "t": "answer", "g": <n>, "sdp": { "type": "answer", "sdp": "…" } }` | phone | answer to offer `g`; the PC ignores answers to older offers |

ICE is not trickled: each side sends its description once candidate gathering is complete (STUN `stun.l.google.com:19302`, `stun.cloudflare.com:3478`; no TURN). A new offer replaces the previous link. The PC creates one reliable, ordered data channel labelled `pairspan`; every message on it is one binary frame of §3, at most 16 KiB.

The link survives signalling outages. It ends when the data channel closes or the peer connection fails; the phone then sends `want` again.

## 3. End-to-end layer

Each data channel message is one frame. The first byte is the **kind**.

| Kind | Name |
|---|---|
| `0x10` | `HS_INIT` |
| `0x11` | `HS_RESP` |
| `0x20` | `DATA` |
| `0x2F` | `REKEY` (responder asks initiator to start a new `HS_INIT`) |

### 3.1 Session handshake

The **PC is always the initiator**. It sends `HS_INIT` as soon as the data channel opens, and the phone answers.

```
HS_INIT  = 0x10 | e_i (33, compressed ephemeral public key)
HS_RESP  = 0x11 | e_r (33) | confirm (16)
```

Both sides compute, with static keys `s_i`/`s_r` and ephemerals `e_i`/`e_r`:

```
ee = ECDH(e_i, e_r)   es = ECDH(e_i, s_r)   se = ECDH(s_i, e_r)   ss = ECDH(s_i, s_r)
psk = SHA-256("pairspan/v1/psk" || T) during pairing, otherwise 32 zero bytes
th  = SHA-256("pairspan/v1/hs" || s_i_pub || s_r_pub || e_i || e_r)
okm = HKDF-SHA256(salt = th, ikm = ee || es || se || ss || psk, info = "pairspan/v1/keys", L = 96)
k_i2r = okm[0..32]   k_r2i = okm[32..64]   k_confirm = okm[64..96]
confirm = AES-256-GCM(k_confirm, nonce = 12 zero bytes, aad = th, plaintext = empty)   # the 16-byte tag
```

ECDH output is the 32-byte x-coordinate. The initiator verifies `confirm` and drops the session on mismatch.

- `ee` gives forward secrecy.
- `es`/`se` prove possession of each static key, so whoever tampers with signalling cannot finish.
- `psk` binds the first handshake to the pair secret, so someone who swaps keys in the pairing room cannot pass as either side.

After pairing, both sides pin each other's `pub`; later sessions use `psk` = zeros.

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
| `0x00` | Control JSON (stream id 0): `{"m":"<message>","p":{...}}` |
| `0x01` | Stream open, sent by the PC. Body is the service name; the only service is `adb` (the phone connects to its own adbd at `127.0.0.1:5555`). The PC picks the stream id. |
| `0x02` | Stream data (≤ 16 320 bytes per frame) |
| `0x03` | Stream close, either direction. Body may hold a reason (`adb not reachable on port 5555`). |

Control messages:

| `m` | From | `p` |
|---|---|---|
| `hello` | both, right after a handshake | PC: `name`, `os`, `version`. Phone: `name`, `model`, `android`, `app` |
| `unpair` | either | — the receiver deletes the pairing and leaves the link channel |

Rules:

- The phone registers a stream as soon as it receives `0x01` and queues data until adbd answers, because the PC may send the first ADB packet immediately.
- During pairing the phone keeps the new computer only after the computer's first DATA frame decrypts, which proves it derived keys with the pair secret.
- Seal and send happen in one critical section per peer so counters arrive in order.
- Each side applies backpressure instead of buffering without bound: both wait while more than 4 MiB is queued on the data channel.
