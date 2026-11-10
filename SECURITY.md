# Security

A paired computer can run adb on the phone (the `shell` user). Treat pairings and the gateway token accordingly.

## Reporting a vulnerability

Open a private report with [GitHub Security Advisories](https://github.com/ferhatozcelik/pairspan/security/advisories/new). Don't file a public issue for an unfixed vulnerability, and don't include live tokens, keys or device identifiers.

## Design

- Each installation has its own P-256 key (desktop: `~/Library/Application Support/Pairspan/identity.key`, mode 0600; Android: app-private storage, excluded from backups). The gateway verifies a signature over a fresh challenge before routing.
- Pairing codes are 20 characters (~99 bits) and expire after 5 minutes. The gateway sees only `SHA-256("pairspan/v1/room" || code)`; the code itself is a pre-shared key in the first handshake, so a gateway that substitutes keys cannot complete pairing. Both sides also check that the announced key hashes to the announced id, and the phone keeps a pairing only after the computer's first frame decrypts.
- Sessions use ephemeral + static ECDH, HKDF-SHA256 and AES-256-GCM with per-direction keys and strictly increasing counters (replays are rejected). The gateway only forwards ciphertext.
- Removing a device on either side removes it from the trust list the gateway enforces; routing stops at once.
- The CLI/MCP socket is a Unix socket readable only by the current user. It can be switched off in the app (**Allow CLI and MCP**).
- The phone side only bridges to its own adbd on `127.0.0.1:5555`; it opens no other connections for the computer.

## Running a gateway

Put it behind TLS (`wss://`), set a long random `GATEWAY_TOKEN`, and set `TRUST_PROXY=1` only behind a proxy you control.
