package com.pairspan.app.core

import org.json.JSONObject
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import java.io.File
import java.security.KeyPair

/** Replays docs/test-vectors.json (written by the Rust core tests) byte for byte. */
class CryptoVectorsTest {
    private val v: JSONObject = run {
        var dir: File? = File("").absoluteFile
        while (dir != null && !File(dir, "docs/test-vectors.json").exists()) dir = dir.parentFile
        JSONObject(File(dir!!, "docs/test-vectors.json").readText())
    }

    private fun hex(key: String) = v.getString(key).unhex()

    private fun pair(scalar: String, pub: String) =
        KeyPair(P256.decompress(hex(pub)), P256.privateFromScalar(hex(scalar)))

    @Test fun compressedKeysRoundTrip() {
        for (k in listOf("s_i_pub", "s_r_pub", "e_i_pub", "e_r_pub")) {
            assertArrayEquals(k, hex(k), P256.compress(P256.decompress(hex(k))))
        }
    }

    @Test fun idsRoomAndPsk() {
        assertEquals(v.getString("peer_id_s_i"), peerId(hex("s_i_pub")))
        assertEquals(v.getString("room"), Pairing.room(v.getString("secret")))
        assertArrayEquals(hex("psk"), Pairing.psk(v.getString("secret")))
        assertEquals(true, Pairing.isValid(v.getString("secret")))
    }

    @Test fun responderMatchesRust() {
        val static = pair("s_r", "s_r_pub")
        val ephemeral = pair("e_r", "e_r_pub")
        val (session, resp) = Handshake.respond(static, hex("s_i_pub"), hex("psk"), hex("hs_init"), ephemeral)
        assertArrayEquals(hex("hs_resp"), resp)
        assertArrayEquals("pairspan".toByteArray(), session.opener.open(hex("data_i2r_pairspan")))
        assertArrayEquals(hex("data_r2i_android"), session.sealer.seal("android".toByteArray()))
        // Replays are rejected.
        assertThrows(IllegalArgumentException::class.java) { session.opener.open(hex("data_i2r_pairspan")) }
    }

    @Test fun wrongPairSecretGivesDifferentConfirmation() {
        val static = pair("s_r", "s_r_pub")
        val ephemeral = pair("e_r", "e_r_pub")
        val (_, resp) = Handshake.respond(static, hex("s_i_pub"), Pairing.psk("WRONGSECRET"), hex("hs_init"), ephemeral)
        assert(!resp.contentEquals(hex("hs_resp")))
    }

    @Test fun channelFraming() {
        val f = Wire.channel(Wire.CH_DATA, 7, byteArrayOf(1, 2, 3))
        val c = Wire.parseChannel(f)!!
        assertEquals(Wire.CH_DATA, c.channel)
        assertEquals(7, c.stream)
        assertArrayEquals(byteArrayOf(1, 2, 3), c.body)
        val r = Wire.parseRelay(Wire.relay(v.getString("peer_id_s_i"), byteArrayOf(9)))!!
        assertEquals(v.getString("peer_id_s_i"), r.peer)
    }
}
