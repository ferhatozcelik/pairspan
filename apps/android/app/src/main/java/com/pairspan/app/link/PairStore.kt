package com.pairspan.app.link

import android.content.Context
import android.content.SharedPreferences
import com.pairspan.app.BuildConfig
import com.pairspan.app.core.P256
import com.pairspan.app.core.hex
import com.pairspan.app.core.peerId
import com.pairspan.app.core.unhex
import org.json.JSONArray
import org.json.JSONObject
import java.security.KeyPair

data class PairedPc(val id: String, val pub: String, val name: String, val os: String, val pairedAt: Long)

/** App-private state: identity key, gateway, paired computers. Not backed up (allowBackup=false). */
class PairStore(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("pairspan", Context.MODE_PRIVATE)

    val identity: KeyPair by lazy {
        val scalar = prefs.getString("identity", null)
        if (scalar != null) {
            val priv = P256.privateFromScalar(scalar.unhex())
            KeyPair(P256.decompress(prefs.getString("identity_pub", "")!!.unhex()), priv)
        } else {
            val kp = P256.generate()
            prefs.edit()
                .putString("identity", P256.scalar(kp.private).hex())
                .putString("identity_pub", P256.compress(kp.public).hex())
                .apply()
            kp
        }
    }

    val publicKey: ByteArray get() = P256.compress(identity.public)
    val deviceId: String get() = peerId(publicKey)

    var gatewayUrl: String
        get() = prefs.getString("gateway", null) ?: BuildConfig.GATEWAY_URL
        set(v) = prefs.edit().putString("gateway", v).apply()

    val gatewayToken: String get() = BuildConfig.GATEWAY_TOKEN

    var enabled: Boolean
        get() = prefs.getBoolean("enabled", true)
        set(v) = prefs.edit().putBoolean("enabled", v).apply()

    var startOnBoot: Boolean
        get() = prefs.getBoolean("start_on_boot", true)
        set(v) = prefs.edit().putBoolean("start_on_boot", v).apply()

    fun pcs(): List<PairedPc> {
        val arr = JSONArray(prefs.getString("pcs", "[]"))
        return (0 until arr.length()).map { i ->
            val o = arr.getJSONObject(i)
            PairedPc(o.getString("id"), o.getString("pub"), o.optString("name", "Computer"), o.optString("os"), o.optLong("paired_at"))
        }
    }

    fun pc(id: String): PairedPc? = pcs().firstOrNull { it.id == id }

    private fun savePcs(list: List<PairedPc>) {
        val arr = JSONArray()
        list.forEach {
            arr.put(JSONObject().put("id", it.id).put("pub", it.pub).put("name", it.name).put("os", it.os).put("paired_at", it.pairedAt))
        }
        prefs.edit().putString("pcs", arr.toString()).apply()
    }

    @Synchronized fun upsertPc(pc: PairedPc) = savePcs(pcs().filter { it.id != pc.id } + pc)

    @Synchronized fun removePc(id: String) = savePcs(pcs().filter { it.id != id })

    @Synchronized fun renamePc(id: String, name: String, os: String) {
        pcs().firstOrNull { it.id == id }?.let { upsertPc(it.copy(name = name.take(64), os = os.take(16))) }
    }
}
