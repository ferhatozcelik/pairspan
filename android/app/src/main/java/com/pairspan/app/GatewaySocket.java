package com.pairspan.app;

import android.content.Context;
import org.json.JSONException;
import org.json.JSONObject;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;

final class GatewaySocket {
    interface Listener {
        void onWelcome(String sessionId, String clientId);
        void onEvent(String event, JSONObject payload);
        void onState(String state);
    }

    private static final OkHttpClient CLIENT = new OkHttpClient.Builder()
            .pingInterval(20, TimeUnit.SECONDS).build();
    private final Listener listener;
    private final String pairCode;
    private final String resumeSessionId;
    private final boolean direct;
    private final Context context;
    private volatile WebSocket socket;
    private volatile boolean ready;

    GatewaySocket(Context context, String pairCode, String resumeSessionId, boolean direct, Listener listener) {
        this.context = context.getApplicationContext();
        this.pairCode = pairCode == null ? "" : pairCode;
        this.resumeSessionId = resumeSessionId == null ? "" : resumeSessionId;
        this.direct = direct;
        this.listener = listener;
    }

    /** Stable id of this phone, sent on every connection and shown next to the desktop's client id. */
    static String deviceId(Context context) {
        android.content.SharedPreferences prefs = context.getSharedPreferences("gateway", Context.MODE_PRIVATE);
        String id = prefs.getString("deviceId", "");
        if (!id.matches("[0-9a-f]{32}")) {
            id = java.util.UUID.randomUUID().toString().replace("-", "");
            prefs.edit().putString("deviceId", id).apply();
        }
        return id;
    }

    /** The gateway comes from the pairing QR, otherwise from the built-in default. */
    static String resolveUrl(Context context) {
        android.content.SharedPreferences prefs = context.getSharedPreferences("gateway", Context.MODE_PRIVATE);
        String url = BuildConfig.GATEWAY_URL;
        String saved = prefs.getString("gatewayUrl", "");
        boolean paired = prefs.getString("sessionId", "").matches("[0-9a-f]{48}");
        boolean recentQr = System.currentTimeMillis() - prefs.getLong("gatewayUrlAt", 0) < 10 * 60 * 1000L;
        if (!saved.isEmpty() && (paired || recentQr)) url = saved;
        url = url.trim().split("\\s+")[0];
        if (url.isEmpty()) return "";
        if (url.startsWith("http://")) url = "ws://" + url.substring(7);
        else if (url.startsWith("https://")) url = "wss://" + url.substring(8);
        else if (!url.startsWith("ws://") && !url.startsWith("wss://")) url = "ws://" + url;
        if (url.endsWith("/")) url = url.substring(0, url.length() - 1);
        if (url.endsWith("/ws")) url = url.substring(0, url.length() - 3);
        return url + "/ws";
    }

    private String errorText(JSONObject message) {
        switch (message.optString("code")) {
            case "bad_code": return context.getString(R.string.error_bad_code);
            case "code_expired": return context.getString(R.string.error_code_expired);
            case "rate_limited": return context.getString(R.string.error_rate_limited);
            default: return message.optString("message", context.getString(R.string.error_rejected));
        }
    }

    void connect() {
        String url = resolveUrl(context);
        if (url.isEmpty()) {
            listener.onState("error: " + context.getString(R.string.error_no_url));
            return;
        }

        Request request;
        try { request = new Request.Builder().url(url).build(); }
        catch (IllegalArgumentException e) {
            listener.onState("error: " + context.getString(R.string.error_bad_url));
            return;
        }
        socket = CLIENT.newWebSocket(request, new WebSocketListener() {
            @Override public void onOpen(WebSocket webSocket, Response response) {
                listener.onState("joining");
                try {
                    JSONObject hello = new JSONObject().put("type", "hello").put("role", "phone").put("deviceId", deviceId(context)).put("token", BuildConfig.GATEWAY_TOKEN);
                    if (!resumeSessionId.isEmpty()) hello.put("sessionId", resumeSessionId);
                    if (!pairCode.isEmpty()) hello.put("pairCode", pairCode);
                    webSocket.send(hello.toString());
                } catch (JSONException e) {
                    listener.onState("error: " + e.getMessage());
                }
            }

            @Override public void onMessage(WebSocket webSocket, String text) {
                try {
                    JSONObject message = new JSONObject(text);
                    String type = message.optString("type");
                    if ("welcome".equals(type)) {
                        ready = true;
                        listener.onState("joined");
                        listener.onWelcome(message.optString("sessionId"), message.optString("clientId"));
                    } else if ("error".equals(type)) {
                        String prefix = message.optBoolean("retry") ? "gateway_retry: " : "gateway_error: ";
                        listener.onState(prefix + errorText(message));
                    } else if ("event".equals(type)) {
                        JSONObject payload = message.optJSONObject("payload");
                        listener.onEvent(message.optString("event"), payload == null ? new JSONObject() : payload);
                    }
                } catch (JSONException e) {
                    listener.onState("error: invalid message");
                }
            }

            @Override public void onFailure(WebSocket webSocket, Throwable error, Response response) {
                ready = false;
                listener.onState("error: " + error.getMessage());
            }

            @Override public void onClosed(WebSocket webSocket, int code, String reason) {
                ready = false;
                listener.onState("closed");
            }
        });
    }

    boolean sendEvent(String event, JSONObject payload) {
        if (!ready) return false;
        WebSocket current = socket;
        if (current == null) return false;
        try {
            JSONObject body = payload == null ? new JSONObject() : payload;
            // Gateways that predate device ids pass payloads through untouched, so every event names its phone itself.
            if (!body.has("deviceId")) body.put("deviceId", deviceId(context));
            JSONObject message = new JSONObject()
                    .put("type", "event")
                    .put("event", event)
                    .put("payload", body);
            return current.send(message.toString());
        } catch (JSONException e) {
            return false;
        }
    }

    void close() {
        ready = false;
        WebSocket current = socket;
        socket = null;
        if (current != null) current.close(1000, "done");
    }
}
