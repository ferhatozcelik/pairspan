package com.pairspan.app.core

import java.nio.ByteBuffer

/** Framing of docs/protocol.md: relay frames on the gateway, channels inside DATA. */
object Wire {
    const val FRAME_RELAY: Byte = 0x01
    const val CH_RPC: Byte = 0x00
    const val CH_OPEN: Byte = 0x01
    const val CH_DATA: Byte = 0x02
    const val CH_CLOSE: Byte = 0x03
    const val MAX_CHUNK = 64 * 1024

    fun relay(peer: String, payload: ByteArray): ByteArray =
        byteArrayOf(FRAME_RELAY) + peer.unhex() + payload

    class Relay(val peer: String, val payload: ByteArray)

    fun parseRelay(frame: ByteArray): Relay? {
        if (frame.size < 18 || frame[0] != FRAME_RELAY) return null
        return Relay(frame.copyOfRange(1, 17).hex(), frame.copyOfRange(17, frame.size))
    }

    fun channel(channel: Byte, stream: Int, body: ByteArray): ByteArray =
        ByteBuffer.allocate(5 + body.size).put(channel).putInt(stream).put(body).array()

    class Channel(val channel: Byte, val stream: Int, val body: ByteArray)

    fun parseChannel(plain: ByteArray): Channel? {
        if (plain.size < 5) return null
        val buf = ByteBuffer.wrap(plain)
        val ch = buf.get()
        val sid = buf.getInt()
        return Channel(ch, sid, plain.copyOfRange(5, plain.size))
    }
}
