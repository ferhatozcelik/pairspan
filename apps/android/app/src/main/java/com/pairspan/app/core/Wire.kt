package com.pairspan.app.core

import java.nio.ByteBuffer

/** Framing of docs/protocol.md (channels inside DATA) and the signalling channel names. */
object Wire {
    const val CH_RPC: Byte = 0x00
    const val CH_OPEN: Byte = 0x01
    const val CH_DATA: Byte = 0x02
    const val CH_CLOSE: Byte = 0x03
    /** With framing and tag a frame stays under the 16 KiB every WebRTC stack accepts. */
    const val MAX_CHUNK = 16 * 1024 - 64

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

    /** Channel shared by one computer and this phone; only the two know both keys. */
    fun linkName(pcPub: ByteArray, phonePub: ByteArray): String =
        "l:" + sha256("pairspan/v2/link".toByteArray(), pcPub, phonePub).hex().take(40)

    /** Channel where the phone and computer exchange keys during the USB pairing. */
    fun roomName(room: String): String = "r:" + room.take(40)
}
