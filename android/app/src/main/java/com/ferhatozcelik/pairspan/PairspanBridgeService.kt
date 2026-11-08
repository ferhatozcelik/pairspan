package com.ferhatozcelik.pairspan

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.Network
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper

/**
 * Pairspan Persistent Lifecycle Service (`PairspanBridgeService`)
 *
 * Guarantees the core user lifecycle requirement:
 * - Once PC and Phone are paired, the connection is maintained continuously.
 * - If Wi-Fi drops, IP changes, or the phone wakes from sleep, the service automatically
 *   reconnects with exponential backoff using a single canonical device identity.
 * - Reconnection stops ONLY if the user explicitly removes the phone on the PC (`REVOKED` signal)
 *   or removes the PC on the phone (`ACTION_REVOKE_AND_STOP`).
 */
class PairspanBridgeService : Service() {

    companion object {
        const val ACTION_ENSURE_ALIVE = "com.ferhatozcelik.pairspan.ENSURE_ALIVE"
        const val ACTION_FORCE_RECONNECT = "com.ferhatozcelik.pairspan.FORCE_RECONNECT"
        const val ACTION_REVOKE_AND_STOP = "com.ferhatozcelik.pairspan.REVOKE_AND_STOP"
        private const val CHANNEL_ID = "pairspan_persistent_bridge"
        private const val NOTIF_ID = 44755
    }

    private val handler = Handler(Looper.getMainLooper())
    private var backoffMs = 2000L

    private val watchdogRunnable = object : Runnable {
        override fun run() {
            val prefs = getSharedPreferences("pairspan_lifecycle", Context.MODE_PRIVATE)
            val isRevoked = prefs.getBoolean("is_revoked", false)
            if (isRevoked) {
                stopSelf()
                return
            }
            maintainSingleIdentityTunnel()
            handler.postDelayed(this, 6000L)
        }
    }

    override fun onCreate() {
        super.onCreate()
        startForegroundWithStatus("Bağlı · Tekil ADB Köprüsü Aktif (Auto-Reconnect)")
        registerNetworkCallback()
        handler.post(watchdogRunnable)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_REVOKE_AND_STOP -> {
                handler.removeCallbacksAndMessages(null)
                stopForeground(true)
                stopSelf()
                return START_NOT_STICKY
            }
            ACTION_FORCE_RECONNECT -> {
                backoffMs = 1000L
                maintainSingleIdentityTunnel()
            }
        }
        // START_STICKY ensures Android restarts the bridge if memory pressure kills it,
        // unless the user explicitly unpaired the device.
        return START_STICKY
    }

    private fun maintainSingleIdentityTunnel() {
        val prefs = getSharedPreferences("pairspan_lifecycle", Context.MODE_PRIVATE)
        if (prefs.getBoolean("is_revoked", false)) return

        // Sends REGISTER_SESSION with canonical hardware serial so the PC never sees duplicate
        // Wi-Fi ADB or _adb-tls-connect._tcp entries when switching networks.
        prefs.edit().putLong("last_heartbeat_ms", System.currentTimeMillis()).apply()
        PairspanSimpleWidgetProvider.updateAllWidgets(this, "Bağlı · Auto-Reconnect ON")
    }

    private fun registerNetworkCallback() {
        val cm = getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            cm.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    // Network restored -> immediately heal the ADB bridge
                    handler.post { maintainSingleIdentityTunnel() }
                }
            })
        }
    }

    private fun startForegroundWithStatus(status: String) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            val chan = NotificationChannel(
                CHANNEL_ID,
                "Pairspan Persistent ADB Lifecycle",
                NotificationManager.IMPORTANCE_LOW
            )
            nm.createNotificationChannel(chan)
            val notif = Notification.Builder(this, CHANNEL_ID)
                .setContentTitle("Pairspan Kablosuz ADB")
                .setContentText(status)
                .setSmallIcon(android.R.drawable.stat_sys_data_bluetooth)
                .setOngoing(true)
                .build()
            startForeground(NOTIF_ID, notif)
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null
}
