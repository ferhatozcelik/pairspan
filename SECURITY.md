# Security

A paired computer can run adb on the phone (the `shell` user). Treat pairings accordingly.

## Reporting a vulnerability

Open a private report with [GitHub Security Advisories](https://github.com/ferhatozcelik/pairspan/security/advisories/new). Don't file a public issue for an unfixed vulnerability, and don't include live keys or device identifiers.

## Design

- Each installation has its own P-256 key (desktop: `~/Library/Application Support/Pairspan/identity.key`, mode 0600; Android: app-private storage, excluded from backups).
- Pairing happens over USB: the computer hands the phone a 20-character secret (~99 bits, valid for 2 minutes) and its id with `adb shell am broadcast`. The receiver requires the `DUMP` permission, which `adb shell` holds and ordinary apps cannot get. The phone only accepts the computer whose key hashes to that id.
- Signalling (Supabase Realtime) sees channel names that are hashes (`SHA-256` of the pair secret, or of both public keys), public keys during pairing, and WebRTC offers/answers. The secret is a pre-shared key in the first handshake, so someone who swaps keys on the signalling channel cannot complete pairing; the phone keeps a pairing only after the computer's first frame decrypts.
- Data goes peer to peer over a WebRTC data channel (DTLS). On top of it, sessions use ephemeral + static ECDH, HKDF-SHA256 and AES-256-GCM with per-direction keys and strictly increasing counters (replays are rejected); a party that tampers with the offer/answer still cannot finish the static-key handshake.
- Removing a device on either side tells the other side and drops the link. The on/off switch on either side leaves signalling and closes every link.
- The CLI/MCP socket is a Unix socket readable only by the current user.
- The phone side only bridges to its own adbd on `127.0.0.1:5555`; it opens no other connections for the computer.
