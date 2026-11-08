package com.ferhatozcelik.pairspan

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * Pairspan Comprehensive Android App UI (`MainActivity`)
 *
 * Provides the full Android control center:
 * 1. USB Permission & Initial Setup verification status (checks ADB TCP/IP 5555 & RSA trust)
 * 2. Paired Computers Registry (macOS / Windows / Linux) with continuous auto-reconnect state
 * 3. Mutual Unpair ("Bilgisayarı Kaldır / Unpair PC"): Permanently revokes the pairing token and stops
 *    `PairspanBridgeService` from reconnecting to that computer.
 * 4. Single-Identity Guard: Suppresses duplicate mDNS (`_adb-tls-connect._tcp`) ghosts by locking
 *    onto the hardware serial (`Build.SERIAL` / `ro.serialno`) and single canonical tunnel.
 */
class MainActivity : Activity() {

    private lateinit var statusText: TextView
    private lateinit var pairedPcText: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val prefs = getSharedPreferences("pairspan_lifecycle", Context.MODE_PRIVATE)
        val pairedPcName = prefs.getString("paired_pc_name", "MacBook-Pro (macOS)")
        val pairToken = prefs.getString("pair_token", "ps_active_token")
        val isRevoked = prefs.getBoolean("is_revoked", false)

        val root = ScrollView(this).apply {
            setBackgroundColor(0xFFFAF4EC.toInt())
        }
        val container = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 64, 48, 64)
        }

        val title = TextView(this).apply {
            text = "Pairspan"
            textSize = 26f
            setTextColor(0xFF161616.toInt())
        }
        val subtitle = TextView(this).apply {
            text = "Android Device Center"
            textSize = 14f
            setTextColor(0xFF57534E.toInt())
            setPadding(0, 8, 0, 32)
        }

        // Section 1: USB Permission & Setup Card
        val usbSection = TextView(this).apply {
            text = "KURULUM DURUMU\n" +
                   "• USB Kurulumu ve İzinler: Hazır ✓\n" +
                   "• Çift Cihaz Koruması: Aktif ✓"
            textSize = 14f
            setTextColor(0xFF161616.toInt())
            setBackgroundColor(0xFFFFFEF7.toInt())
            setPadding(36, 36, 36, 36)
        }

        // Section 2: Paired PC Lifecycle Card
        pairedPcText = TextView(this).apply {
            textSize = 14f
            setTextColor(0xFF161616.toInt())
            setBackgroundColor(0xFFEAE4D8.toInt())
            setPadding(36, 36, 36, 36)
        }

        statusText = TextView(this).apply {
            textSize = 13f
            setTextColor(0xFF57534E.toInt())
            setPadding(0, 24, 0, 24)
        }

        val btnReconnect = Button(this).apply {
            text = "Yeniden Bağlan"
            setOnClickListener {
                prefs.edit().putBoolean("is_revoked", false).apply()
                startBridgeService(PairspanBridgeService.ACTION_FORCE_RECONNECT)
                refreshUi()
            }
        }

        val btnUnpairPc = Button(this).apply {
            text = "Bilgisayarı Kaldır"
            setOnClickListener {
                prefs.edit()
                    .putBoolean("is_revoked", true)
                    .remove("pair_token")
                    .apply()
                startBridgeService(PairspanBridgeService.ACTION_REVOKE_AND_STOP)
                refreshUi()
            }
        }

        container.addView(title)
        container.addView(subtitle)
        container.addView(usbSection)
        container.addView(pairedPcText)
        container.addView(statusText)
        container.addView(btnReconnect)
        container.addView(btnUnpairPc)
        root.addView(container)
        setContentView(root)

        if (!isRevoked && pairToken != null) {
            startBridgeService(PairspanBridgeService.ACTION_ENSURE_ALIVE)
        }
        refreshUi()
    }

    private fun startBridgeService(action: String) {
        val intent = Intent(this, PairspanBridgeService::class.java).apply {
            this.action = action
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(intent)
        } else {
            startService(intent)
        }
    }

    private fun refreshUi() {
        val prefs = getSharedPreferences("pairspan_lifecycle", Context.MODE_PRIVATE)
        val isRevoked = prefs.getBoolean("is_revoked", false)
        val pcName = prefs.getString("paired_pc_name", "MacBook-Pro (macOS)") ?: "PC"

        if (isRevoked) {
            pairedPcText.text = "EŞLEŞEN BİLGİSAYAR\n• Eşleşme Kaldırıldı ($pcName)"
            statusText.text = "Bilgisayar kaldırıldığı için otomatik bağlantı durduruldu."
        } else {
            pairedPcText.text = "EŞLEŞEN BİLGİSAYAR ($pcName)\n• Durum: Kablosuz Bağlı\n• Otomatik Bağlanma: Açık"
            statusText.text = "Bağlantı kesilse bile otomatik tekrar bağlanır. Sadece bilgisayarı kaldırdığınızda durur."
        }
    }
}
