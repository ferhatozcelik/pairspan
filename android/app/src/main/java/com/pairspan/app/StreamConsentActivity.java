package com.pairspan.app;

import android.app.Activity;
import android.content.Intent;
import android.media.projection.MediaProjectionManager;
import android.os.Bundle;

/** Only a user interaction with the app/notification opens the system consent dialog. */
public final class StreamConsentActivity extends Activity {
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        if (!GatewayService.hasStreamRequest()) { finish(); return; }
        if (state == null) {
            MediaProjectionManager manager = getSystemService(MediaProjectionManager.class);
            Intent consent = android.os.Build.VERSION.SDK_INT >= 34
                ? manager.createScreenCaptureIntent(android.media.projection.MediaProjectionConfig.createConfigForDefaultDisplay())
                : manager.createScreenCaptureIntent();
            startActivityForResult(consent, 1);
        }
    }
    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == 1 && result == RESULT_OK && data != null && GatewayService.hasStreamRequest()) {
            Intent service = GatewayService.consumeStreamConsent(this, data);
            if (service != null) startForegroundService(service);
        } else GatewayService.rejectStreamConsent();
        finish();
    }
}
