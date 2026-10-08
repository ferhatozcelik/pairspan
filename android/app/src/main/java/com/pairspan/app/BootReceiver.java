package com.pairspan.app;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

public final class BootReceiver extends BroadcastReceiver {
    @Override public void onReceive(Context context, Intent intent) {
        if (!Intent.ACTION_BOOT_COMPLETED.equals(intent.getAction())) return;
        android.content.SharedPreferences prefs = context.getSharedPreferences("gateway", Context.MODE_PRIVATE);
        if (!prefs.getBoolean("startOnBoot", true) || !prefs.getBoolean("enabled", true)) return;
        String sessionId = prefs.getString("sessionId", "");
        if (sessionId != null && sessionId.matches("[0-9a-f]{48}")) {
            context.startForegroundService(new Intent(context, GatewayService.class));
        }
    }
}
