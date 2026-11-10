package com.pairspan.app.core

import java.math.BigInteger
import java.security.AlgorithmParameters
import java.security.KeyFactory
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.PrivateKey
import java.security.PublicKey
import java.security.SecureRandom
import java.security.Signature
import java.security.interfaces.ECPrivateKey
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.security.spec.ECParameterSpec
import java.security.spec.ECPoint
import java.security.spec.ECPrivateKeySpec
import java.security.spec.ECPublicKeySpec
import javax.crypto.Cipher
import javax.crypto.KeyAgreement
import javax.crypto.Mac
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * End-to-end layer of docs/protocol.md, byte-compatible with crates/core/src/crypto.rs
 * (checked against docs/test-vectors.json in the unit tests).
 */
object P256 {
    val params: ECParameterSpec = AlgorithmParameters.getInstance("EC").run {
        init(ECGenParameterSpec("secp256r1"))
        getParameterSpec(ECParameterSpec::class.java)
    }
    private val p: BigInteger = (params.curve.field as java.security.spec.ECFieldFp).p
    private val b: BigInteger = params.curve.b

    fun generate(): KeyPair = KeyPairGenerator.getInstance("EC").apply {
        initialize(ECGenParameterSpec("secp256r1"), SecureRandom())
    }.generateKeyPair()

    fun compress(key: PublicKey): ByteArray {
        val w = (key as ECPublicKey).w
        val out = ByteArray(33)
        out[0] = if (w.affineY.testBit(0)) 0x03 else 0x02
        unsigned32(w.affineX).copyInto(out, 1)
        return out
    }

    /** Decodes a 33-byte compressed point (y = sqrt(x³ − 3x + b), p ≡ 3 mod 4). */
    fun decompress(bytes: ByteArray): PublicKey {
        require(bytes.size == 33 && (bytes[0].toInt() == 2 || bytes[0].toInt() == 3)) { "not a compressed P-256 key" }
        val x = BigInteger(1, bytes.copyOfRange(1, 33))
        require(x < p) { "x out of range" }
        val rhs = x.pow(3).subtract(x.multiply(BigInteger.valueOf(3))).add(b).mod(p)
        var y = rhs.modPow(p.add(BigInteger.ONE).shiftRight(2), p)
        require(y.modPow(BigInteger.TWO, p) == rhs) { "point is not on the curve" }
        if (y.testBit(0) != (bytes[0].toInt() == 3)) y = p.subtract(y)
        return KeyFactory.getInstance("EC").generatePublic(ECPublicKeySpec(ECPoint(x, y), params))
    }

    fun privateFromScalar(scalar: ByteArray): PrivateKey =
        KeyFactory.getInstance("EC").generatePrivate(ECPrivateKeySpec(BigInteger(1, scalar), params))

    fun scalar(key: PrivateKey): ByteArray = unsigned32((key as ECPrivateKey).s)

    /** Raw x-coordinate of the shared point (32 bytes), same as RustCrypto's `raw_secret_bytes`. */
    fun ecdh(priv: PrivateKey, pub: PublicKey): ByteArray = KeyAgreement.getInstance("ECDH").run {
        init(priv)
        doPhase(pub, true)
        generateSecret()
    }

    fun sign(priv: PrivateKey, message: ByteArray): ByteArray = Signature.getInstance("SHA256withECDSA").run {
        initSign(priv)
        update(message)
        sign() // DER, accepted by the gateway
    }

    private fun unsigned32(v: BigInteger): ByteArray {
        val raw = v.toByteArray()
        return when {
            raw.size == 32 -> raw
            raw.size > 32 -> raw.copyOfRange(raw.size - 32, raw.size)
            else -> ByteArray(32 - raw.size) + raw
        }
    }
}

fun sha256(vararg parts: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").run {
    parts.forEach { update(it) }
    digest()
}

fun ByteArray.hex(): String = joinToString("") { "%02x".format(it) }

fun String.unhex(): ByteArray {
    require(length % 2 == 0) { "odd hex length" }
    return ByteArray(length / 2) { substring(it * 2, it * 2 + 2).toInt(16).toByte() }
}

/** First 16 bytes of SHA-256(compressed public key), as 32 hex chars. */
fun peerId(compressedPub: ByteArray): String = sha256(compressedPub).copyOf(16).hex()

object Pairing {
    fun psk(secret: String): ByteArray = sha256("pairspan/v1/psk".toByteArray(), secret.toByteArray())
    fun room(secret: String): String = sha256("pairspan/v1/room".toByteArray(), secret.toByteArray()).hex()
    private const val ALPHABET = "23456789ABCDEFGHJKMNPQRSTUVWXYZ"
    fun normalize(input: String): String = input.uppercase().filter { it.isLetterOrDigit() }
    fun isValid(secret: String): Boolean = secret.length == 20 && secret.all { it in ALPHABET }
}

const val HS_INIT: Byte = 0x10
const val HS_RESP: Byte = 0x11
const val DATA: Byte = 0x20
const val REKEY: Byte = 0x2F

private fun hmac(key: ByteArray, vararg data: ByteArray): ByteArray = Mac.getInstance("HmacSHA256").run {
    init(SecretKeySpec(key, "HmacSHA256"))
    data.forEach { update(it) }
    doFinal()
}

/** RFC 5869 HKDF-SHA256. */
fun hkdf(salt: ByteArray, ikm: ByteArray, info: ByteArray, length: Int): ByteArray {
    val prk = hmac(salt, ikm)
    val out = java.io.ByteArrayOutputStream()
    var t = ByteArray(0)
    var i = 1
    while (out.size() < length) {
        t = hmac(prk, t, info, byteArrayOf(i.toByte()))
        out.write(t)
        i++
    }
    return out.toByteArray().copyOf(length)
}

private fun gcm(mode: Int, key: ByteArray, nonce: ByteArray, aad: ByteArray, input: ByteArray): ByteArray =
    Cipher.getInstance("AES/GCM/NoPadding").run {
        init(mode, SecretKeySpec(key, "AES"), GCMParameterSpec(128, nonce))
        updateAAD(aad)
        doFinal(input)
    }

private fun nonce(counter: Long): ByteArray = ByteArray(12).also {
    for (i in 0 until 8) it[4 + i] = (counter ushr (56 - 8 * i)).toByte()
}

class Sealer(private val key: ByteArray) {
    private var counter = 0L

    /** `DATA = 0x20 | counter (8) | ciphertext || tag`. Callers serialize seal+send. */
    fun seal(plain: ByteArray): ByteArray {
        check(counter < (1L shl 32)) { "session exhausted" }
        val header = ByteArray(9).also { h ->
            h[0] = DATA
            for (i in 0 until 8) h[1 + i] = (counter ushr (56 - 8 * i)).toByte()
        }
        val ct = gcm(Cipher.ENCRYPT_MODE, key, nonce(counter), header, plain)
        counter++
        return header + ct
    }
}

class Opener(private val key: ByteArray) {
    private var last = -1L

    fun open(frame: ByteArray): ByteArray {
        require(frame.size >= 25 && frame[0] == DATA) { "malformed DATA frame" }
        var counter = 0L
        for (i in 1..8) counter = (counter shl 8) or (frame[i].toLong() and 0xFF)
        require(counter > last) { "replayed or reordered frame" }
        val plain = gcm(Cipher.DECRYPT_MODE, key, nonce(counter), frame.copyOf(9), frame.copyOfRange(9, frame.size))
        last = counter
        return plain
    }
}

class Session(val sealer: Sealer, val opener: Opener)

/** Responder side of the handshake (the phone). Returns the session and the HS_RESP frame. */
object Handshake {
    fun respond(
        static: KeyPair,
        peerPub: ByteArray,
        psk: ByteArray?,
        hsInit: ByteArray,
        ephemeral: KeyPair = P256.generate(),
    ): Pair<Session, ByteArray> {
        require(hsInit.size == 34 && hsInit[0] == HS_INIT) { "malformed HS_INIT" }
        val eIBytes = hsInit.copyOfRange(1, 34)
        val eI = P256.decompress(eIBytes)
        val peer = P256.decompress(peerPub)
        val sPub = P256.compress(static.public)
        val ePub = P256.compress(ephemeral.public)
        val ee = P256.ecdh(ephemeral.private, eI)
        val es = P256.ecdh(static.private, eI)
        val se = P256.ecdh(ephemeral.private, peer)
        val ss = P256.ecdh(static.private, peer)
        val th = sha256("pairspan/v1/hs".toByteArray(), peerPub, sPub, eIBytes, ePub)
        val okm = hkdf(th, ee + es + se + ss + (psk ?: ByteArray(32)), "pairspan/v1/keys".toByteArray(), 96)
        val i2r = okm.copyOfRange(0, 32)
        val r2i = okm.copyOfRange(32, 64)
        val confirm = gcm(Cipher.ENCRYPT_MODE, okm.copyOfRange(64, 96), ByteArray(12), th, ByteArray(0))
        return Session(Sealer(r2i), Opener(i2r)) to (byteArrayOf(HS_RESP) + ePub + confirm)
    }
}
