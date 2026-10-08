package com.pairspan.app;

import android.content.Context;
import android.os.Build;
import org.json.JSONObject;
import org.json.JSONException;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicInteger;
import com.pairspan.app.adb.AdbStream;
import com.pairspan.app.adb.android.AdbMdns;

/** Runs an ADB client inside Pairspan, connected to the phone's own adbd. */
final class ShellBridge {
    private static volatile LocalAdbManager manager;
    private static volatile String detail = null;

    private static String s(int id, Object... args) { return context == null ? "" : context.getString(id, args); }
    private static volatile boolean connecting;
    /** Cached so the UI never waits on the ADB library's lock, which is held while a connect waits for the phone's approval prompt. */
    private static volatile boolean connected;
    /** Port of the ADB daemon this bridge is connected to; 5555 means classic ADB that a computer on the same network can reach too. */
    private static volatile int connectedPort;
    private static volatile boolean refreshing;
    private static volatile long lastRefreshMs;
    /** The first connect waits so the app is on screen before the system's "Allow USB debugging?" prompt appears. */
    private static volatile long notBeforeMs;
    private static final long STARTUP_DELAY_MS = 3500;
    private static Context context;
    private static final ExecutorService worker = Executors.newCachedThreadPool();

    static synchronized void init(Context appContext) {
        if (context != null) return;
        context = appContext.getApplicationContext();
        notBeforeMs = System.currentTimeMillis() + STARTUP_DELAY_MS;
        worker.execute(() -> {
            try { manager = new LocalAdbManager(context); }
            catch (Exception error) { detail = s(R.string.adb_identity_fmt, shortError(error)); }
        });
        new android.os.Handler(android.os.Looper.getMainLooper()).postDelayed(ShellBridge::reconnect, STARTUP_DELAY_MS);
    }

    static int port() { return connected ? connectedPort : 0; }

    static boolean ready() {
        LocalAdbManager current = manager;
        if (current == null) return false;
        long now = System.currentTimeMillis();
        if (!refreshing && !connecting && now - lastRefreshMs > 2000) {
            refreshing = true;
            lastRefreshMs = now;
            worker.execute(() -> {
                try { connected = current.isConnected(); } finally { refreshing = false; }
            });
        }
        return connected;
    }
    static String detail() { return ready() ? s(R.string.adb_ready) : (detail == null ? s(R.string.adb_waiting_default) : detail); }

    static void reconnect() {
        reconnect(false);
    }

    static void reconnect(boolean force) {
        LocalAdbManager current = manager;
        if (current == null || connecting) return;
        if (!force && System.currentTimeMillis() < notBeforeMs) return;
        if (!force && ready()) return;
        connecting = true;
        detail = s(R.string.adb_searching);
        worker.execute(() -> {
            try {
                try { current.disconnect(); } catch (Exception ignored) { }
                connected = false;
                if (connectLocal(current)) { connected = true; detail = s(R.string.adb_ready); }
                else detail = s(R.string.adb_not_enabled);
            } catch (Exception error) {
                String message = shortError(error);
                detail = message.contains("CERTIFICATE") || message.contains("Handshake") || message.contains("verify")
                        || error instanceof java.io.IOException
                        ? s(R.string.adb_needs_trust)
                        : s(R.string.adb_waiting_fmt, message);
            } finally { connecting = false; }
        });
    }

    private static int discoverPort(String serviceType, int timeoutSeconds) throws Exception {
        AtomicInteger port = new AtomicInteger(-1);
        CountDownLatch found = new CountDownLatch(1);
        AdbMdns discovery = new AdbMdns(context, serviceType, (host, discoveredPort) -> {
            if (host != null && discoveredPort > 0) { port.set(discoveredPort); found.countDown(); }
        });
        discovery.start();
        try {
            if (!found.await(timeoutSeconds, TimeUnit.SECONDS)) return -1;
        } finally { discovery.stop(); }
        return port.get();
    }

    static int discoverTlsPort() throws Exception {
        return discoverPort(AdbMdns.SERVICE_TYPE_TLS_CONNECT, 8);
    }

    private static boolean connectLocal(LocalAdbManager current) throws Exception {
        Exception lastError = null;
        // Classic ADB on 5555 (prepared once from the desktop app) answers instantly; wireless debugging needs pairing.
        try { if (current.connect("127.0.0.1", 5555)) { connectedPort = 5555; return true; } } catch (Exception classic) { lastError = classic; }
        int port = discoverTlsPort();
        if (port > 0) {
            try { if (current.connect("127.0.0.1", port)) { connectedPort = port; return true; } } catch (Exception tls) { lastError = tls; }
        }
        if (lastError != null) throw lastError;
        return false;
    }

    static String run(String command) {
        String first = runOnce(command);
        try {
            JSONObject parsed = new JSONObject(first);
            if (parsed.optInt("exitCode", -1) >= 0) return first;
            String output = parsed.optString("output");
            if (!output.contains("Stream closed") && !output.contains("kesildi")) return first;
        } catch (JSONException ignored) { return first; }
        reconnectAndWait();
        return runOnce(command);
    }

    private static void reconnectAndWait() {
        reconnect(true);
        for (int i = 0; i < 40; i++) {
            if (ready()) return;
            try { Thread.sleep(250); } catch (InterruptedException ignored) { return; }
        }
    }

    private static String runOnce(String command) {
        if (!ready()) return result(-1, s(R.string.adb_not_ready));
        String marker = "__PAIRSPAN_EXIT_" + UUID.randomUUID().toString().replace("-", "") + "__";
        String shell = command + "; printf '\\n" + marker + "%s\\n' \"$?\"";
        LocalAdbManager current = manager;
        if (current == null) return result(-1, s(R.string.adb_not_ready));
        Future<String> future = worker.submit(() -> {
            try (AdbStream stream = current.openStream("shell:" + shell);
                 InputStream input = stream.openInputStream();
                 ByteArrayOutputStream output = new ByteArrayOutputStream()) {
                byte[] buffer = new byte[4096];
                int length;
                while ((length = input.read(buffer)) != -1) {
                    if (output.size() + length > 262144) throw new IllegalStateException(s(R.string.adb_output_limit));
                    output.write(buffer, 0, length);
                }
                return output.toString(StandardCharsets.UTF_8);
            }
        });
        try {
            String output = future.get(20, TimeUnit.SECONDS);
            int at = output.lastIndexOf(marker);
            if (at < 0) return result(-1, output.isBlank() ? s(R.string.adb_incomplete) : output);
            String code = output.substring(at + marker.length()).trim();
            return result(Integer.parseInt(code), output.substring(0, at).stripTrailing());
        } catch (Exception error) {
            future.cancel(true);
            detail = s(R.string.adb_lost_fmt, shortError(error));
            connected = false;
            try { current.disconnect(); } catch (Exception ignored) { }
            return result(-1, detail);
        }
    }

    private static String result(int exitCode, String output) {
        try { return new JSONObject().put("exitCode", exitCode).put("output", output).toString(); }
        catch (JSONException error) { return "{\"exitCode\":-1,\"output\":\"Could not build response\"}"; }
    }
    private static String shortError(Throwable error) {
        String message = error.getMessage();
        return message == null ? error.getClass().getSimpleName() : message.substring(0, Math.min(100, message.length()));
    }
    interface Callback { void done(String error); }
}
