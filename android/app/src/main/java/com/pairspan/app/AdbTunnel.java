package com.pairspan.app;

import android.util.Base64;
import org.json.JSONObject;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

final class AdbTunnel {
    interface Sender {
        void send(String event, JSONObject payload);
    }

    private final Sender sender;
    private final ExecutorService openWorker = Executors.newSingleThreadExecutor();
    private final ExecutorService writeWorker = Executors.newSingleThreadExecutor();
    private final AtomicBoolean open = new AtomicBoolean(false);
    private volatile Socket socket;
    private volatile String tunnelId = "";

    AdbTunnel(Sender sender) {
        this.sender = sender;
    }

    void open(String id) {
        if (id == null || id.isBlank()) return;
        close();
        tunnelId = id;
        open.set(true);
        openWorker.execute(() -> {
            try {
                // Classic adb over TCP (adb tcpip 5555) works without Wi-Fi; wireless debugging needs mDNS discovery.
                int port = 5555;
                Socket next = new Socket();
                try {
                    next.connect(new InetSocketAddress("127.0.0.1", port), 1500);
                } catch (Exception classic) {
                    try { next.close(); } catch (Exception ignored) { }
                    port = ShellBridge.discoverTlsPort();
                    if (port < 1) {
                        sendClose("ADB port not found");
                        return;
                    }
                    next = new Socket();
                    next.connect(new InetSocketAddress("127.0.0.1", port), 8000);
                }
                next.setTcpNoDelay(true);
                next.setSoTimeout(0);
                if (!open.get() || !id.equals(tunnelId)) {
                    try { next.close(); } catch (Exception ignored) { }
                    return;
                }
                socket = next;
                sender.send("adb_tunnel_ready", new JSONObject().put("id", id).put("port", port));
                InputStream input = next.getInputStream();
                byte[] buffer = new byte[64 * 1024];
                int length;
                while (open.get() && id.equals(tunnelId) && (length = input.read(buffer)) != -1) {
                    String data = Base64.encodeToString(buffer, 0, length, Base64.NO_WRAP);
                    sender.send("adb_tunnel_data", new JSONObject().put("id", id).put("data", data));
                }
            } catch (Exception error) {
                sendClose(error.getMessage() == null ? "tunnel open failed" : error.getMessage());
            } finally {
                closeLocal();
            }
        });
    }

    void write(String id, String base64) {
        if (!open.get() || id == null || !id.equals(tunnelId) || base64 == null || base64.isEmpty()) return;
        writeWorker.execute(() -> {
            try {
                Socket current = socket;
                if (current == null || current.isClosed()) return;
                byte[] bytes = Base64.decode(base64, Base64.DEFAULT);
                OutputStream output = current.getOutputStream();
                output.write(bytes);
                output.flush();
            } catch (Exception error) {
                sendClose(error.getMessage() == null ? "tunnel write failed" : error.getMessage());
                close();
            }
        });
    }

    void close() {
        open.set(false);
        closeLocal();
        tunnelId = "";
    }

    void closeFromRemote(String id) {
        if (id != null && id.equals(tunnelId)) close();
    }

    private void closeLocal() {
        Socket current = socket;
        socket = null;
        if (current != null) {
            try { current.close(); } catch (Exception ignored) { }
        }
    }

    private void sendClose(String reason) {
        String id = tunnelId;
        if (id.isEmpty()) return;
        try {
            sender.send("adb_tunnel_close", new JSONObject().put("id", id).put("reason", reason == null ? "" : reason));
        } catch (Exception ignored) { }
    }
}
