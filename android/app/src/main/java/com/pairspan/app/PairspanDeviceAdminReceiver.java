package com.pairspan.app;

import android.app.admin.DeviceAdminReceiver;
import android.app.admin.DevicePolicyManager;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;

public final class PairspanDeviceAdminReceiver extends DeviceAdminReceiver {
    public static ComponentName component(Context context) {
        return new ComponentName(context, PairspanDeviceAdminReceiver.class);
    }

    public static boolean isActive(Context context) {
        DevicePolicyManager manager = context.getSystemService(DevicePolicyManager.class);
        return manager != null && manager.isAdminActive(component(context));
    }

    public static void deactivate(Context context) {
        DevicePolicyManager manager = context.getSystemService(DevicePolicyManager.class);
        if (manager != null) manager.removeActiveAdmin(component(context));
    }

    public static Intent activationIntent(Context context) {
        Intent intent = new Intent(DevicePolicyManager.ACTION_ADD_DEVICE_ADMIN);
        intent.putExtra(DevicePolicyManager.EXTRA_DEVICE_ADMIN, component(context));
        intent.putExtra(DevicePolicyManager.EXTRA_ADD_EXPLANATION,
                context.getString(R.string.admin_explanation));
        return intent;
    }

    @Override public void onEnabled(Context context, Intent intent) {
        String sessionId = context.getSharedPreferences("gateway", Context.MODE_PRIVATE).getString("sessionId", "");
        if (sessionId != null && sessionId.matches("[0-9a-f]{48}")) {
            context.startForegroundService(new Intent(context, GatewayService.class));
        }
    }

    @Override public CharSequence onDisableRequested(Context context, Intent intent) {
        return context.getString(R.string.admin_disable_warning);
    }
}
