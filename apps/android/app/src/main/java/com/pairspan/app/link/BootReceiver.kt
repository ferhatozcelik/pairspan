package com.pairspan.app.link

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Reconnects to paired computers after a reboot. */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
        val store = PairStore(context)
        if (store.enabled && store.startOnBoot && store.pcs().isNotEmpty()) LinkService.start(context)
    }
}
