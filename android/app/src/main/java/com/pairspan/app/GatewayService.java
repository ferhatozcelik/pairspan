package com.pairspan.app;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.PowerManager;
import org.json.JSONException;
import org.json.JSONObject;
import java.util.HashSet;
import java.util.Locale;
import java.util.Set;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public final class GatewayService extends Service {
    private static final String CHANNEL_ID = "pairspan_gateway";
    private static final int NOTIFICATION_ID = 1;
    private static final String PREFS = "gateway";
    static volatile String status = "";
    static volatile String pairError = "";
    static volatile long lastMacAt = 0;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private volatile GatewaySocket socket;
    private PowerManager.WakeLock wakeLock;
    private volatile boolean running;
    private volatile int generation;
    private String activeSessionId = "";
    private String pendingPairCode = "";
    private int retrySeconds = 2;
    private final ExecutorService commandWorker = Executors.newSingleThreadExecutor();
    private final Set<String> handledRequests = new HashSet<>();
    private AdbTunnel adbTunnel;

    @Override public void onCreate() {
        super.onCreate();
        NotificationManager manager = getSystemService(NotificationManager.class);
        manager.createNotificationChannel(new NotificationChannel(CHANNEL_ID, getString(R.string.notif_channel), NotificationManager.IMPORTANCE_LOW));
        ShellBridge.init(this);
        adbTunnel = new AdbTunnel((event, payload) -> {
            GatewaySocket current = socket;
            if (current != null) current.sendEvent(event, payload);
        });
    }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && "com.pairspan.app.STOP".equals(intent.getAction())) {
            getSharedPreferences(PREFS, MODE_PRIVATE).edit().remove("sessionId").apply();
            stopSelf();
            return START_NOT_STICKY;
        }
        SharedPreferences prefs = getSharedPreferences(PREFS, MODE_PRIVATE);
        prefs.edit().remove("token").apply();
        String sessionId = prefs.getString("sessionId", "");
        String pairCode = intent != null ? intent.getStringExtra("pairCode") : null;
        if (pairCode != null) {
            pairCode = pairCode.toUpperCase().replaceAll("[^A-Z0-9]", "");
            if (!pairCode.matches("[2-9A-HJKMNP-Z]{20}")) pairCode = "";
            else pairError = "";
        } else {
            pairCode = "";
        }
        if ((sessionId == null || !sessionId.matches("[0-9a-f]{48}")) && pairCode.isEmpty()) {
            stopSelf();
            return START_NOT_STICKY;
        }
        if (sessionId == null) sessionId = "";

        Notification notification = notification(getString(R.string.notif_connecting));
        if (Build.VERSION.SDK_INT >= 29) startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE);
        else startForeground(NOTIFICATION_ID, notification);

        boolean sessionChanged = !sessionId.equals(activeSessionId) || !pairCode.equals(pendingPairCode)
                || (intent != null && intent.getBooleanExtra("reconnect", false));
        if (running && sessionChanged) {
            generation++;
            if (socket != null) socket.close();
            socket = null;
            activeSessionId = sessionId;
            pendingPairCode = pairCode;
            connect(sessionId, pairCode);
        } else if (!running) {
            running = true;
            activeSessionId = sessionId;
            pendingPairCode = pairCode;
            PowerManager manager = getSystemService(PowerManager.class);
            wakeLock = manager.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "Pairspan:Gateway");
            wakeLock.acquire();
            connect(sessionId, pairCode);
            handler.post(heartbeat);
        }
        return START_STICKY;
    }

    private Notification notification(String text) {
        Intent open = new Intent(this, MainActivity.class);
        PendingIntent pending = PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Intent stop = new Intent(this, GatewayService.class).setAction("com.pairspan.app.STOP");
        PendingIntent stopPending = PendingIntent.getService(this, 1, stop, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return new Notification.Builder(this, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_pairspan_notification)
                .setContentTitle("Pairspan")
                .setContentText(text)
                .setSubText(getString(ShellBridge.ready() ? R.string.notif_adb_ready : R.string.notif_adb_waiting))
                .addAction(android.R.drawable.ic_menu_close_clear_cancel, getString(R.string.notif_disconnect), stopPending)
                .setContentIntent(pending)
                .setOngoing(true)
                .build();
    }

    private void updateStatus(String value) {
        status = value;
        getSystemService(NotificationManager.class).notify(NOTIFICATION_ID, notification(value));
    }

    private void connect(String sessionId, String pairCode) {
        if (!running) return;
        final int currentGeneration = ++generation;
        updateStatus(getString(R.string.status_connecting));
        GatewaySocket next = new GatewaySocket(this, pairCode, sessionId, false, new GatewaySocket.Listener() {
            @Override public void onWelcome(String newSessionId, String clientId) {
                if (currentGeneration != generation) return;
                retrySeconds = 2;
                pendingPairCode = "";
                if (newSessionId != null && newSessionId.matches("[0-9a-f]{48}")) {
                    activeSessionId = newSessionId;
                    getSharedPreferences(PREFS, MODE_PRIVATE).edit().putString("sessionId", newSessionId).apply();
                }
                if (clientId != null && clientId.matches("[0-9a-f]{32}")) getSharedPreferences(PREFS, MODE_PRIVATE).edit().putString("clientId", clientId).apply();
                updateStatus(getString(R.string.status_waiting));
                sendPhone("phone_hello");
            }

            @Override public void onEvent(String event, JSONObject payload) {
                if (currentGeneration != generation) return;
                if ("mac_online".equals(event) || "mac_ack".equals(event)) {
                    String macClientId = payload.optString("clientId");
                    if (macClientId.matches("[0-9a-f]{32}")) getSharedPreferences(PREFS, MODE_PRIVATE).edit().putString("clientId", macClientId).apply();
                    lastMacAt = System.currentTimeMillis();
                    updateStatus(getString(R.string.status_connected));
                } else if ("mac_offline".equals(event)) {
                    lastMacAt = 0;
                    updateStatus(getString(R.string.status_waiting));
                } else if ("shell_request".equals(event)) {
                    handleShellRequest(payload);
                } else if ("adb_tunnel_open".equals(event)) {
                    if (adbTunnel != null) adbTunnel.open(payload.optString("id"));
                } else if ("adb_tunnel_data".equals(event)) {
                    if (adbTunnel != null) adbTunnel.write(payload.optString("id"), payload.optString("data"));
                } else if ("adb_tunnel_close".equals(event)) {
                    if (adbTunnel != null) adbTunnel.closeFromRemote(payload.optString("id"));
                }
            }

            @Override public void onState(String state) {
                if (currentGeneration != generation) return;
                if (state.startsWith("gateway_retry:")) {
                    updateStatus(state.substring("gateway_retry:".length()).trim());
                    return;
                }
                if (state.startsWith("gateway_error:")) {
                    pairError = state.substring("gateway_error:".length()).trim();
                    if (pairError.isEmpty()) pairError = getString(R.string.error_rejected);
                    getSharedPreferences(PREFS, MODE_PRIVATE).edit().remove("sessionId").apply();
                    updateStatus(pairError);
                    stopSelf();
                    return;
                }
                if (state.startsWith("error:") || "closed".equals(state)) {
                    updateStatus(getString(R.string.status_retrying));
                    scheduleReconnect();
                }
            }
        });
        socket = next;
        next.connect();
    }

    private void sendPhone(String event) {
        try {
            JSONObject payload = new JSONObject()
                    .put("deviceId", GatewaySocket.deviceId(this))
                    .put("name", Build.MANUFACTURER + " " + Build.MODEL)
                    .put("model", Build.MODEL)
                    .put("sdk", Build.VERSION.SDK_INT)
                    .put("adbReady", ShellBridge.ready())
                    .put("adbPort", ShellBridge.port())
                    .put("ip", localIpv4())
                    .put("adbDetail", ShellBridge.detail())
                    .put("deviceAdmin", PairspanDeviceAdminReceiver.isActive(this));
            if (socket != null) socket.sendEvent(event, payload);
        } catch (JSONException ignored) { }
    }

    /** The phone's Wi-Fi IPv4 address, so a computer on the same network can skip the relay. */
    private static String localIpv4() {
        try {
            for (java.net.NetworkInterface face : java.util.Collections.list(java.net.NetworkInterface.getNetworkInterfaces())) {
                if (!face.isUp() || face.isLoopback() || face.isVirtual() || !face.getName().startsWith("wlan")) continue;
                for (java.net.InetAddress address : java.util.Collections.list(face.getInetAddresses())) {
                    if (address instanceof java.net.Inet4Address) return address.getHostAddress();
                }
            }
        } catch (Exception ignored) { }
        return "";
    }

    private void handleShellRequest(JSONObject payload) {
        String id = payload.optString("id");
        String command = payload.optString("command");
        long sentAt = payload.optLong("sentAt");
        if (!id.matches("[0-9a-f-]{36}") || command.isBlank() || command.length() > 512
                || Math.abs(System.currentTimeMillis() - sentAt) > 30000) return;
        synchronized (handledRequests) {
            if (handledRequests.size() >= 128) handledRequests.clear();
            if (!handledRequests.add(id)) return;
        }
        commandWorker.execute(() -> {
            try {
                JSONObject result = new JSONObject(ShellBridge.run(command))
                        .put("id", id).put("command", command);
                GatewaySocket current = socket;
                if (current != null) current.sendEvent("shell_result", result);
            } catch (JSONException ignored) { }
        });
    }

    private final Runnable heartbeat = new Runnable() {
        @Override public void run() {
            if (!running) return;
            if (!ShellBridge.ready()) ShellBridge.reconnect();
            if (socket != null) sendPhone("phone_heartbeat");
            if (lastMacAt > 0 && System.currentTimeMillis() - lastMacAt > 30000) updateStatus(getString(R.string.status_waiting));
            handler.postDelayed(this, 10000);
        }
    };

    private void scheduleReconnect() {
        if (!running) return;
        generation++;
        lastMacAt = 0;
        GatewaySocket old = socket;
        socket = null;
        if (old != null) old.close();
        int delay = retrySeconds;
        retrySeconds = Math.min(30, retrySeconds * 2);
        String sessionId = getSharedPreferences(PREFS, MODE_PRIVATE).getString("sessionId", "");
        String pair = pendingPairCode;
        handler.postDelayed(() -> {
            if (running && socket == null) connect(sessionId == null ? "" : sessionId, pair);
        }, delay * 1000L);
    }

    @Override public void onDestroy() {
        running = false;
        generation++;
        handler.removeCallbacksAndMessages(null);
        if (adbTunnel != null) adbTunnel.close();
        if (socket != null) { sendPhone("phone_bye"); socket.close(); socket = null; }
        if (wakeLock != null && wakeLock.isHeld()) wakeLock.release();
        commandWorker.shutdownNow();
        status = "";
        super.onDestroy();
    }

    @Override public IBinder onBind(Intent intent) { return null; }
}
