package com.pairspan.app.link

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/**
 * The desktop app pairs over the USB cable with
 * `adb shell am broadcast -n com.pairspan/com.pairspan.app.link.PairReceiver --es secret … --es pc …`.
 * The manifest requires the sender to hold DUMP, which `adb shell` has and ordinary apps cannot get.
 */
class PairReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val secret = intent.getStringExtra("secret") ?: return
        val pc = intent.getStringExtra("pc") ?: return
        LinkService.pair(context, secret, pc)
    }
}
