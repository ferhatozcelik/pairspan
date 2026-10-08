# Architecture

Pairspan 1.0.0 is three processes plus a static site.

```text
Android app                         macOS tray                         your tools
    |                                    |                                  |
    |  WebSocket /ws                     |  WebSocket /ws                   |
    +---------------> Gateway <----------+                                  |
    |                  Node :3000        |                                  |
    |                                    |                                  |
    |  adbd (wireless)                   |  TCP 127.0.0.1:44755             |
    +<----- binary relay frames ---------+<----------- adb connect ---------+
```

| Piece | Responsibility |
| --- | --- |
| Gateway | Pairs one desktop session with one phone. Relays JSON events. Does not speak ADB itself. |
| macOS app | Shows the pairing code and QR, opens the loopback relay, runs `adb connect`. |
| Android app | Joins the gateway, pairs with the phone's own `adbd`, and carries the ADB byte stream. |
| `web/public/` | Static landing page and `/p/` redirect from an HTTPS link to `pairspan://`. The same `pairspan` process serves these pages, `/ws`, and `/health` at `https://pairspan.ferhatozcelik.com/`. |

The desktop is the only ADB client your shell talks to. The phone connects to its own `adbd`. Bytes between them cross the gateway as bounded `adb_tunnel_*` events. See [protocol.md](protocol.md).

## Session rules

- A desktop `hello` with role `mac` creates a session and a single-use pairing token (20 characters, shown as five groups of four).
- A phone can join with that code, resume a known session id, or join directly when exactly one desktop is waiting.
- Direct join is rejected when no desktop is connected, and when more than one desktop is waiting.
- Using a code, or letting it expire, rotates it.

## What 1.0.0 does not include

There is no iOS client and no Play Store or App Store listing. The gateway is the `pairspan` container. Build or sideload the clients.
