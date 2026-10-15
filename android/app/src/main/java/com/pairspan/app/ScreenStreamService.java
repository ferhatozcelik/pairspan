package com.pairspan.app;

import android.app.*;
import android.content.*;
import android.content.pm.ServiceInfo;
import android.hardware.display.DisplayManager;
import android.media.projection.MediaProjection;
import android.os.*;
import android.graphics.Point;
import android.view.WindowManager;
import org.json.*;
import org.webrtc.*;
import java.util.*;
import java.util.concurrent.atomic.AtomicLong;

/** Surface textures -> native WebRTC -> MediaCodec H.264 -> RTP. No pixel IPC or bitmap queue. */
public final class ScreenStreamService extends Service implements DisplayManager.DisplayListener {
    private static volatile ScreenStreamService active;
    private HandlerThread streamThread;
    private Handler worker;
    @Override public void onCreate() { super.onCreate(); streamThread = new HandlerThread("PairspanWebRTC"); streamThread.start(); worker = new Handler(streamThread.getLooper()); }
    static boolean isRunning() { return active != null; }
    private EglBase egl;
    private PeerConnectionFactory factory;
    private PeerConnection peer;
    private VideoSource source;
    private VideoTrack track;
    private ScreenCapturerAndroid capture;
    private SurfaceTextureHelper textures;
    private RtpSender sender;
    private String streamId = "";
    private String stopReason = "Screen sharing stopped on phone";
    private JSONObject request;
    private boolean remoteSet, negotiating;
    private volatile boolean closing;
    private final List<IceCandidate> pendingIce = new ArrayList<>();
    private final AtomicLong captured = new AtomicLong();
    private long previousCaptured, previousAt;
    private int width, height, screenWidth, screenHeight;
    private long lastAcceptedNs;
    private final AtomicLong captureDrops = new AtomicLong();
    private Map<String, Object> previousOutbound;
    private boolean statsPending;
    private final Runnable disconnectedTimeout = () -> fail("Connection lost; start screen sharing again");

    static boolean handleSignal(String event, JSONObject payload) {
        ScreenStreamService service = active;
        if (service == null || !service.streamId.equals(payload.optString("streamId"))) return false;
        service.worker.post(() -> service.signal(event, payload));
        return true;
    }
    static void signalingLost() { ScreenStreamService s = active; if (s != null) s.worker.post(() -> { s.worker.removeCallbacks(s.disconnectedTimeout); s.worker.postDelayed(s.disconnectedTimeout, 30000); }); }
    static void signalingRestored() { ScreenStreamService s = active; if (s != null) s.worker.post(() -> { s.worker.removeCallbacks(s.disconnectedTimeout); s.send("stream_ready", new JSONObject()); }); }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent == null || "stop".equals(intent.getAction())) { stopSelf(); return START_NOT_STICKY; }
        if (active != null) { stopSelf(); return START_NOT_STICKY; }
        NotificationManager manager = getSystemService(NotificationManager.class);
        manager.createNotificationChannel(new NotificationChannel("pairspan_stream", "Screen sharing", NotificationManager.IMPORTANCE_LOW));
        PendingIntent stop = PendingIntent.getService(this, 8, new Intent(this, ScreenStreamService.class).setAction("stop"), PendingIntent.FLAG_IMMUTABLE);
        Notification notice = new Notification.Builder(this, "pairspan_stream").setSmallIcon(R.drawable.ic_pairspan_notification)
            .setContentTitle("Pairspan is sharing your screen").setContentText("720p · 30 FPS · WebRTC")
            .addAction(android.R.drawable.ic_menu_close_clear_cancel, "Stop sharing", stop).setOngoing(true).build();
        if (Build.VERSION.SDK_INT >= 29) startForeground(2, notice, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION);
        else startForeground(2, notice);
        try {
            request = new JSONObject(intent.getStringExtra("request"));
            streamId = request.getString("streamId");
            if (!streamId.matches("[0-9a-fA-F-]{36}")) throw new IllegalArgumentException("Invalid stream id");
            Intent permission = intent.getParcelableExtra("permission");
            if (permission == null) throw new IllegalArgumentException("Screen permission required");
            active = this;
            worker.post(() -> {
                try {
                    if (closing) return;
                    initialize(permission);
                    send("stream_ready", new JSONObject());
                    previousAt = SystemClock.elapsedRealtime();
                    worker.postDelayed(stats, 1000);
                    worker.postDelayed(disconnectedTimeout, 60000);
                } catch (Exception e) { fail("Screen sharing unavailable: " + e.getMessage()); }
            });
        } catch (Exception e) { fail("Screen sharing unavailable: " + e.getMessage()); }
        return START_NOT_STICKY;
    }

    private void initialize(Intent permission) {
        PeerConnectionFactory.initialize(PeerConnectionFactory.InitializationOptions.builder(this).createInitializationOptions());
        egl = EglBase.create();
        HardwareVideoEncoderFactory hardware = new HardwareVideoEncoderFactory(egl.getEglBaseContext(), true, true);
        VideoEncoderFactory h264 = new VideoEncoderFactory() {
            @Override public VideoEncoder createEncoder(VideoCodecInfo info) { return "H264".equalsIgnoreCase(info.name) ? hardware.createEncoder(info) : null; }
            @Override public VideoCodecInfo[] getSupportedCodecs() { return Arrays.stream(hardware.getSupportedCodecs()).filter(c -> "H264".equalsIgnoreCase(c.name)).toArray(VideoCodecInfo[]::new); }
        };
        if (h264.getSupportedCodecs().length == 0) throw new IllegalStateException("Hardware H.264 encoder not available");
        factory = PeerConnectionFactory.builder().setVideoEncoderFactory(h264)
            .setVideoDecoderFactory(new HardwareVideoDecoderFactory(egl.getEglBaseContext())).createPeerConnectionFactory();
        source = factory.createVideoSource(true);
        textures = SurfaceTextureHelper.create("PairspanCapture", egl.getEglBaseContext());
        capture = new ScreenCapturerAndroid(permission, new MediaProjection.Callback() {
            @Override public void onStop() { worker.post(() -> fail("Screen sharing stopped by Android")); }
            @Override public void onCapturedContentResize(int w, int h) { worker.post(() -> resize(w, h)); }
        });
        CapturerObserver observer = source.getCapturerObserver();
        capture.initialize(textures, this, new CapturerObserver() {
            @Override public void onCapturerStarted(boolean success) { observer.onCapturerStarted(success); }
            @Override public void onCapturerStopped() { observer.onCapturerStopped(); }
            @Override public void onFrameCaptured(VideoFrame frame) {
                captured.incrementAndGet();
                // Surface capture can produce display-rate frames. Discard obsolete samples, never queue them.
                long now = frame.getTimestampNs();
                if (now - lastAcceptedNs < 30_000_000L) { captureDrops.incrementAndGet(); return; }
                lastAcceptedNs = now; observer.onFrameCaptured(frame);
            }
        });
        Point size = new Point(); getSystemService(WindowManager.class).getDefaultDisplay().getRealSize(size);
        resize(size.x, size.y);
        capture.startCapture(width, height, 30);
        track = factory.createVideoTrack("screen", source);
        getSystemService(DisplayManager.class).registerDisplayListener(this, worker);
        createPeer();
    }

    private void resize(int w, int h) {
        if (closing || w <= 0 || h <= 0) return;
        screenWidth = w; screenHeight = h;
        // Fit inside 1280x720 (or portrait 720x1280), preserving the device's aspect ratio.
        double scale = Math.min(1.0, Math.min(1280.0 / Math.max(w, h), 720.0 / Math.min(w, h)));
        int nextW = Math.max(2, ((int) (w * scale) / 2) * 2), nextH = Math.max(2, ((int) (h * scale) / 2) * 2);
        if (width == nextW && height == nextH) return;
        width = nextW; height = nextH;
        source.adaptOutputFormat(width, height, 30);
        capture.changeCaptureFormat(width, height, 30);
    }
    @Override public void onDisplayChanged(int id) { if (Build.VERSION.SDK_INT < 34) { Point p = new Point(); getSystemService(WindowManager.class).getDefaultDisplay().getRealSize(p); resize(p.x, p.y); } }
    @Override public void onDisplayAdded(int id) { }
    @Override public void onDisplayRemoved(int id) { }

    private void createPeer() {
        List<PeerConnection.IceServer> servers = new ArrayList<>();
        JSONArray configs = request.optJSONArray("iceServers");
        if (configs == null) servers.add(PeerConnection.IceServer.builder("stun:stun.l.google.com:19302").createIceServer());
        else for (int i = 0; i < Math.min(configs.length(), 8); i++) {
            JSONObject config = configs.optJSONObject(i); if (config == null) continue;
            Object urls = config.opt("urls");
            List<String> values = new ArrayList<>();
            if (urls instanceof String) values.add((String) urls);
            else if (urls instanceof JSONArray) for (int j = 0; j < Math.min(((JSONArray) urls).length(), 8); j++) values.add(((JSONArray) urls).optString(j));
            if (!values.isEmpty()) servers.add(PeerConnection.IceServer.builder(values).setUsername(config.optString("username")).setPassword(config.optString("credential")).createIceServer());
        }
        PeerConnection.RTCConfiguration config = new PeerConnection.RTCConfiguration(servers);
        config.sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN;
        config.tcpCandidatePolicy = PeerConnection.TcpCandidatePolicy.DISABLED;
        peer = factory.createPeerConnection(config, new PeerConnection.Observer() {
            @Override public void onSignalingChange(PeerConnection.SignalingState s) { }
            @Override public void onIceConnectionChange(PeerConnection.IceConnectionState s) { worker.post(() -> {
                send("stream_status", json("state", "ICE " + s.name().toLowerCase()));
                if (s == PeerConnection.IceConnectionState.CONNECTED || s == PeerConnection.IceConnectionState.COMPLETED) worker.removeCallbacks(disconnectedTimeout);
                if (s == PeerConnection.IceConnectionState.FAILED || s == PeerConnection.IceConnectionState.DISCONNECTED) { worker.removeCallbacks(disconnectedTimeout); worker.postDelayed(disconnectedTimeout, 30000); }
            }); }
            @Override public void onIceConnectionReceivingChange(boolean receiving) { }
            @Override public void onIceGatheringChange(PeerConnection.IceGatheringState s) { }
            @Override public void onIceCandidate(IceCandidate c) { if (c.sdp.toLowerCase().contains(" udp ")) worker.post(() -> {
                JSONObject candidate = json("candidate", c.sdp); put(candidate, "sdpMid", c.sdpMid); put(candidate, "sdpMLineIndex", c.sdpMLineIndex);
                send("stream_ice", json("candidate", candidate));
            }); }
            @Override public void onIceCandidatesRemoved(IceCandidate[] c) { }
            @Override public void onAddStream(MediaStream s) { }
            @Override public void onRemoveStream(MediaStream s) { }
            @Override public void onDataChannel(DataChannel c) { c.close(); }
            @Override public void onRenegotiationNeeded() { }
            @Override public void onAddTrack(RtpReceiver r, MediaStream[] s) { }
        });
        if (peer == null) throw new IllegalStateException("Cannot create WebRTC connection");
        peer.setBitrate(250_000, 2_000_000, 4_000_000);
        sender = peer.addTrack(track, Collections.singletonList("screen"));
        RtpParameters params = sender.getParameters();
        params.degradationPreference = RtpParameters.DegradationPreference.MAINTAIN_FRAMERATE;
        for (RtpParameters.Encoding e : params.encodings) { e.maxBitrateBps = 4_000_000; e.maxFramerate = 30; }
        sender.setParameters(params);
    }

    private void signal(String event, JSONObject p) {
        if (closing || peer == null) return;
        if (event.equals("stream_stop")) { stopSelf(); return; }
        if (event.equals("stream_request")) { send("stream_ready", new JSONObject()); return; }
        if (event.equals("stream_offer")) {
            if (negotiating) { fail("Overlapping WebRTC negotiation"); return; }
            negotiating = true; remoteSet = false;
            peer.setRemoteDescription(new SdpAdapter() {
                @Override public void onSetSuccess() { worker.post(() -> {
                    if (closing) return;
                    remoteSet = true;
                    for (IceCandidate candidate : pendingIce) peer.addIceCandidate(candidate);
                    pendingIce.clear();
                    peer.createAnswer(new SdpAdapter() {
                        @Override public void onCreateSuccess(SessionDescription answer) { worker.post(() -> {
                            if (closing) return;
                            peer.setLocalDescription(new SdpAdapter() {
                                @Override public void onSetSuccess() { worker.post(() -> { negotiating = false; send("stream_answer", json("sdp", answer.description)); }); }
                            }, answer);
                        }); }
                    }, new MediaConstraints());
                }); }
            }, new SessionDescription(SessionDescription.Type.OFFER, p.optString("sdp")));
        } else if (event.equals("stream_ice")) {
            JSONObject c = p.optJSONObject("candidate"); if (c == null || !c.optString("candidate").toLowerCase().contains(" udp ")) return;
            IceCandidate candidate = new IceCandidate(c.optString("sdpMid"), c.optInt("sdpMLineIndex"), c.optString("candidate"));
            if (remoteSet) peer.addIceCandidate(candidate);
            else if (pendingIce.size() < 128) pendingIce.add(candidate);
            else fail("Too many ICE candidates");
        }
    }
    private class SdpAdapter implements SdpObserver {
        @Override public void onCreateSuccess(SessionDescription s) { }
        @Override public void onSetSuccess() { }
        @Override public void onCreateFailure(String error) { worker.post(() -> fail("WebRTC negotiation: " + error)); }
        @Override public void onSetFailure(String error) { worker.post(() -> fail("WebRTC negotiation: " + error)); }
    }
    private static JSONObject json(String key, Object value) { JSONObject p = new JSONObject(); put(p, key, value); return p; }
    private static void put(JSONObject p, String key, Object value) { try { p.put(key, value); } catch (JSONException ignored) { } }
    private void send(String event, JSONObject p) { if (!closing) { put(p, "streamId", streamId); GatewayService.streamSend(event, p); } }
    private void fail(String reason) { if (closing) return; stopReason = reason; send("stream_status", json("state", reason)); stopSelf(); }
    private static double number(Map<String, Object> m, String key) { Object v = m.get(key); return v instanceof Number ? ((Number) v).doubleValue() : 0; }
    private final Runnable stats = new Runnable() {
        @Override public void run() {
            if (closing || peer == null) return;
            if (!statsPending) {
                statsPending = true;
                peer.getStats(report -> worker.post(() -> {
                    statsPending = false; if (closing) return;
                    long now = SystemClock.elapsedRealtime(), count = captured.get();
                    double seconds = Math.max(.001, (now - previousAt) / 1000.0);
                    JSONObject p = new JSONObject();
                    put(p, "captureFps", (count - previousCaptured) / seconds);
                    put(p, "captureDrops", captureDrops.get()); put(p, "screenWidth", screenWidth); put(p, "screenHeight", screenHeight);
                    put(p, "width", width); put(p, "height", height);
                    for (RTCStats stat : report.getStatsMap().values()) {
                        Map<String, Object> m = stat.getMembers();
                        if (stat.getType().equals("outbound-rtp") && "video".equals(m.get("kind"))) {
                            if (previousOutbound != null && m.get("framesEncoded") instanceof Number && previousOutbound.get("framesEncoded") instanceof Number) {
                                double frames = number(m, "framesEncoded") - number(previousOutbound, "framesEncoded");
                                put(p, "encodedFps", frames / seconds);
                                if (m.get("bytesSent") instanceof Number && previousOutbound.get("bytesSent") instanceof Number) put(p, "bitrate", (number(m, "bytesSent") - number(previousOutbound, "bytesSent")) * 8 / seconds);
                                if (frames > 0 && m.get("totalEncodeTime") instanceof Number && previousOutbound.get("totalEncodeTime") instanceof Number) put(p, "encoderMs", (number(m, "totalEncodeTime") - number(previousOutbound, "totalEncodeTime")) * 1000 / frames);
                            }
                            put(p, "encoder", m.get("encoderImplementation")); put(p, "qualityLimitation", m.get("qualityLimitationReason"));
                            previousOutbound = new HashMap<>(m);
                        }
                    }
                    previousCaptured = count; previousAt = now; send("stream_stats", p);
                }));
            }
            worker.postDelayed(this, 1000);
        }
    };
    @Override public void onDestroy() {
        send("stream_stop", json("reason", stopReason)); closing = true;
        if (active == this) active = null;
        worker.removeCallbacksAndMessages(null);
        getSystemService(DisplayManager.class).unregisterDisplayListener(this);
        worker.post(() -> {
        if (capture != null) { try { capture.stopCapture(); } catch (Exception ignored) { } capture.dispose(); }
        if (peer != null) { peer.close(); peer.dispose(); }
        if (track != null) track.dispose();
        if (source != null) source.dispose();
        if (textures != null) textures.dispose();
        if (factory != null) factory.dispose();
        if (egl != null) egl.release();
        streamThread.quitSafely();
        });
        stopForeground(STOP_FOREGROUND_REMOVE);
        super.onDestroy();
    }
    @Override public IBinder onBind(Intent intent) { return null; }
}
