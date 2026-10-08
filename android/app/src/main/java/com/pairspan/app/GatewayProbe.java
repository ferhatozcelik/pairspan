package com.pairspan.app;

import android.content.Context;
import java.io.IOException;
import java.util.concurrent.TimeUnit;
import okhttp3.Call;
import okhttp3.Callback;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;

/** Tells whether the gateway really accepts live connections, or is only online over plain HTTP. */
final class GatewayProbe {
    static final int OK = 1;
    static final int UNREACHABLE = 2;
    /** The service answers over HTTP but refuses WebSocket connections. */
    static final int NO_LIVE_CONNECTIONS = 3;

    interface Result { void done(int state); }

    private static final OkHttpClient CLIENT = new OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS).readTimeout(5, TimeUnit.SECONDS).build();

    private GatewayProbe() { }

    static void check(Context context, Result result) {
        String url = GatewaySocket.resolveUrl(context);
        if (url.isEmpty()) { result.done(UNREACHABLE); return; }
        Request request;
        try { request = new Request.Builder().url(url).build(); }
        catch (IllegalArgumentException e) { result.done(UNREACHABLE); return; }
        CLIENT.newWebSocket(request, new WebSocketListener() {
            private boolean reported;
            private synchronized boolean first() { if (reported) return false; reported = true; return true; }
            @Override public void onOpen(WebSocket webSocket, Response response) {
                if (first()) result.done(OK);
                webSocket.close(1000, "probe");
            }
            @Override public void onFailure(WebSocket webSocket, Throwable error, Response response) {
                if (first()) checkHttp(url, result);
            }
        });
    }

    private static void checkHttp(String wsUrl, Result result) {
        String http = wsUrl.replaceFirst("^ws", "http").replaceFirst("/ws$", "/health");
        Request request;
        try { request = new Request.Builder().url(http).build(); }
        catch (IllegalArgumentException e) { result.done(UNREACHABLE); return; }
        CLIENT.newCall(request).enqueue(new Callback() {
            @Override public void onFailure(Call call, IOException e) { result.done(UNREACHABLE); }
            @Override public void onResponse(Call call, Response response) {
                result.done(response.isSuccessful() ? NO_LIVE_CONNECTIONS : UNREACHABLE);
                response.close();
            }
        });
    }
}
