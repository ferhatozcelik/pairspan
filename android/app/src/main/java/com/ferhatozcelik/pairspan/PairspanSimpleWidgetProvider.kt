package com.ferhatozcelik.pairspan

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews

/**
 * Pairspan Ultra-Simple Home Screen Widget (`PairspanSimpleWidgetProvider`)
 *
 * Complements the comprehensive `MainActivity` App UI with a minimal, distraction-free
 * home screen widget showing:
 * - Current paired PC name & status (Connected / Reconnecting / Unpaired)
 * - 1-tap Reconnect or Open Full App
 */
class PairspanSimpleWidgetProvider : AppWidgetProvider() {

    override fun onUpdate(
        context: Context,
        appWidgetManager: AppWidgetManager,
        appWidgetIds: IntArray
    ) {
        val prefs = context.getSharedPreferences("pairspan_lifecycle", Context.MODE_PRIVATE)
        val isRevoked = prefs.getBoolean("is_revoked", false)
        val stateLabel = if (isRevoked) "Eşleşme Kaldırıldı" else "Bağlı · Auto-Reconnect ON"
        updateAllWidgets(context, stateLabel)
    }

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        if (intent.action == "com.ferhatozcelik.pairspan.ACTION_WIDGET_RECONNECT") {
            val svc = Intent(context, PairspanBridgeService::class.java).apply {
                action = PairspanBridgeService.ACTION_FORCE_RECONNECT
            }
            context.startService(svc)
        }
    }

    companion object {
        fun updateAllWidgets(context: Context, statusText: String) {
            val mgr = AppWidgetManager.getInstance(context)
            val ids = mgr.getAppWidgetIds(ComponentName(context, PairspanSimpleWidgetProvider::class.java))
            if (ids.isEmpty()) return

            context.getSharedPreferences("pairspan_lifecycle", Context.MODE_PRIVATE)
                .edit()
                .putString("widget_last_status", statusText)
                .apply()

            val reconnectIntent = Intent(context, PairspanSimpleWidgetProvider::class.java).apply {
                action = "com.ferhatozcelik.pairspan.ACTION_WIDGET_RECONNECT"
            }
            val pendingReconnect = PendingIntent.getBroadcast(
                context,
                0,
                reconnectIntent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )

            val openAppIntent = Intent(context, MainActivity::class.java)
            val pendingOpenApp = PendingIntent.getActivity(
                context,
                1,
                openAppIntent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )

            for (id in ids) {
                val views = RemoteViews(context.packageName, R.layout.widget_pairspan_simple)
                views.setTextViewText(R.id.widget_status, statusText)
                views.setOnClickPendingIntent(R.id.widget_btn_action, pendingReconnect)
                views.setOnClickPendingIntent(R.id.widget_root, pendingOpenApp)
                mgr.updateAppWidget(id, views)
            }
        }
    }
}
