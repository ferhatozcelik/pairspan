package com.pairspan.app.link

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.provider.Settings
import android.util.Base64
import com.pairspan.app.BuildConfig
import com.pairspan.app.R
import com.pairspan.app.core.DATA
import com.pairspan.app.core.HS_INIT
import com.pairspan.app.core.Handshake
import com.pairspan.app.core.Pairing
import com.pairspan.app.core.REKEY
import com.pairspan.app.core.Session
import com.pairspan.app.core.Wire
import com.pairspan.app.core.hex
import com.pairspan.app.core.unhex
import com.pairspan.app.ui.MainActivity
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString
import org.json.JSONArray
import org.json.JSONObject
import java.io.InputStream
import java.net.InetSocketAddress
import java.net.Socket
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

enum class GatewayState { OFFLINE, CONNECTING, ONLINE }

sealed interface PairingState {
    data object Idle : PairingState
    data object Waiting : PairingState
    data class Done(val pcName: String) : PairingState
    data class Failed(val reason: String) : PairingState
}

data class PcView(val id: String, val name: String, val os: String, val online: Boolean)

data class LinkState(
    val gateway: GatewayState = GatewayState.OFFLINE,
    val pcs: List<PcView> = emptyList(),
    val pairing: PairingState = PairingState.Idle,
    val activeStreams: Int = 0,
    val error: String? = null,
)

/**
 * Keeps the phone's side of the link: one gateway socket, an encrypted session per paired
 * computer, and ADB streams bridged to this phone's own adbd on 127.0.0.1:5555.
 * All protocol state is touched only on [io]; stream threads only seal and send.
 */
class LinkService : Service() {
    companion object {
        private const val ACTION_START = "start"
        private const val ACTION_PAIR = "pair"
        private const val ACTION_UNPAIR = "unpair"
        private const val ACTION_RECONNECT = "reconnect"
        private const val ACTION_STOP = "stop"
        private const val CHANNEL = "link"
        private const val NOTIFICATION_ID = 1
        private const val ADB_PORT = 5555
        private const val SEND_QUEUE_LIMIT = 4L * 1024 * 1024

        private val _state = MutableStateFlow(LinkState())
        val state: StateFlow<LinkState> = _state

        fun start(ctx: Context) = send(ctx, Intent(ctx, LinkService::class.java).setAction(ACTION_START))
        fun reconnect(ctx: Context) = send(ctx, Intent(ctx, LinkService::class.java).setAction(ACTION_RECONNECT))
        fun stop(ctx: Context) = send(ctx, Intent(ctx, LinkService::class.java).setAction(ACTION_STOP))
        fun unpair(ctx: Context, id: String) =
            send(ctx, Intent(ctx, LinkService::class.java).setAction(ACTION_UNPAIR).putExtra("id", id))

        /** [gateway] and [pcId] come from the QR code; both are null for a typed code. */
        fun pair(ctx: Context, secret: String, gateway: String?, pcId: String?) = send(
            ctx,
            Intent(ctx, LinkService::class.java).setAction(ACTION_PAIR)
                .putExtra("secret", secret).putExtra("gateway", gateway).putExtra("pc", pcId),
        )

        fun clearPairing() = _state.update { it.copy(pairing = PairingState.Idle) }

        private fun send(ctx: Context, intent: Intent) {
            ctx.startForegroundService(intent)
        }

        /** True when classic ADB over TCP answers on this phone (enabled once over USB). */
        fun adbReachable(): Boolean = try {
            Socket().use { it.connect(InetSocketAddress("127.0.0.1", ADB_PORT), 400); true }
        } catch (_: Exception) {
            false
        }
    }

    private class PendingPair(val secret: String, val room: String, val pcId: String?, val deadline: Long) {
        var candidate: Pair<String, ByteArray>? = null
    }

    /** [pairingPub] is set until the computer proves the pair secret by sending a frame we can decrypt. */
    private class Link(val pcId: String, val session: Session, var pairingPub: ByteArray? = null)

    /** Registered as soon as the PC opens it, so bytes that arrive before adbd answers are queued, not lost. */
    private class Stream {
        val outbox = LinkedBlockingQueue<ByteArray>(256)
        @Volatile var socket: Socket? = null
        @Volatile var closed = false
    }

    private lateinit var store: PairStore
    private val io = Executors.newSingleThreadScheduledExecutor { r -> Thread(r, "pairspan-link") }
    private val workers = Executors.newCachedThreadPool()
    private val http = OkHttpClient.Builder()
        .pingInterval(20, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.MILLISECONDS)
        .connectTimeout(15, TimeUnit.SECONDS)
        .build()

    private var socket: WebSocket? = null
    private var generation = 0
    private var backoffSeconds = 1L
    private var pending: PendingPair? = null
    private val links = ConcurrentHashMap<String, Link>()
    private val online = mutableSetOf<String>()
    private val streams = ConcurrentHashMap<String, Stream>() // "<pc>:<sid>"

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        store = PairStore(this)
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, getString(R.string.notif_channel), NotificationManager.IMPORTANCE_LOW),
        )
        startInForeground(getString(R.string.notif_connecting))
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startInForeground(notificationText())
        val action = intent?.action ?: ACTION_START
        io.execute {
            when (action) {
                ACTION_PAIR -> beginPairing(
                    intent!!.getStringExtra("secret").orEmpty(),
                    intent.getStringExtra("gateway"),
                    intent.getStringExtra("pc"),
                )
                ACTION_UNPAIR -> removePc(intent!!.getStringExtra("id").orEmpty(), notifyPc = true)
                ACTION_RECONNECT -> reconnectNow()
                ACTION_STOP -> {
                    shutdown()
                    stopSelf()
                    return@execute
                }
                else -> {}
            }
            if (socket == null && shouldRun()) connect()
            if (!shouldRun()) {
                shutdown()
                stopSelf()
            }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        io.execute { shutdown() }
        io.shutdown()
        super.onDestroy()
    }

    private fun shouldRun() = store.enabled && (store.pcs().isNotEmpty() || pending != null)

    // ------------------------------------------------------------------ gateway

    private fun connect() {
        val gen = ++generation
        _state.update { it.copy(gateway = GatewayState.CONNECTING) }
        refreshNotification()
        val request = try {
            Request.Builder().url(store.gatewayUrl).build()
        } catch (e: IllegalArgumentException) {
            _state.update { it.copy(gateway = GatewayState.OFFLINE, error = getString(R.string.err_gateway_url)) }
            return
        }
        socket = http.newWebSocket(request, object : WebSocketListener() {
            override fun onMessage(webSocket: WebSocket, text: String) = io.execute { if (gen == generation) onText(webSocket, text) }
            override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                val frame = bytes.toByteArray()
                io.execute { if (gen == generation) onFrame(frame) }
            }
            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) = io.execute { if (gen == generation) onDisconnected(reason) }
            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) =
                io.execute { if (gen == generation) onDisconnected(t.message ?: t.javaClass.simpleName) }
        })
    }

    private fun reconnectNow() {
        socket?.cancel()
        socket = null
        generation++
        dropAllLinks()
        backoffSeconds = 1
    }

    private fun onDisconnected(reason: String) {
        socket = null
        dropAllLinks()
        _state.update { it.copy(gateway = GatewayState.OFFLINE, error = reason.take(120)) }
        refreshNotification()
        if (!shouldRun()) return
        val gen = generation
        io.schedule({ if (gen == generation && socket == null && shouldRun()) connect() }, backoffSeconds, TimeUnit.SECONDS)
        backoffSeconds = (backoffSeconds * 2).coerceAtMost(30)
    }

    private fun shutdown() {
        generation++
        socket?.close(1000, "bye")
        socket = null
        dropAllLinks()
        _state.update { it.copy(gateway = GatewayState.OFFLINE) }
    }

    private fun sendJson(obj: JSONObject) {
        socket?.send(obj.toString())
    }

    private fun onText(ws: WebSocket, text: String) {
        val msg = try { JSONObject(text) } catch (_: Exception) { return }
        when (msg.optString("t")) {
            "challenge" -> {
                val nonce = Base64.decode(msg.getString("nonce"), Base64.DEFAULT)
                val sig = com.pairspan.app.core.P256.sign(store.identity.private, "pairspan/v1/auth".toByteArray() + nonce)
                val hello = JSONObject()
                    .put("t", "hello").put("role", "phone")
                    .put("pub", Base64.encodeToString(store.publicKey, Base64.NO_WRAP))
                    .put("sig", Base64.encodeToString(sig, Base64.NO_WRAP))
                    .put("peers", JSONArray(store.pcs().map { it.id }))
                if (store.gatewayToken.isNotEmpty()) hello.put("token", store.gatewayToken)
                ws.send(hello.toString())
            }
            "welcome" -> {
                backoffSeconds = 1
                online.clear()
                val ids = msg.optJSONArray("online") ?: JSONArray()
                for (i in 0 until ids.length()) online.add(ids.getString(i))
                _state.update { it.copy(gateway = GatewayState.ONLINE, error = null) }
                pending?.let { sendJson(JSONObject().put("t", "room_join").put("room", it.room)) }
                publishPcs()
            }
            "presence" -> {
                val id = msg.getString("id")
                if (msg.getBoolean("online")) online.add(id) else {
                    online.remove(id)
                    dropLink(id)
                }
                publishPcs()
            }
            "room_peer" -> {
                val p = pending ?: return
                if (msg.optString("room") != p.room || msg.optString("role") != "pc") return
                val id = msg.getString("id")
                val pub = Base64.decode(msg.getString("pub"), Base64.DEFAULT)
                // The id must be the hash of the key, and match the QR code when there is one.
                if (com.pairspan.app.core.peerId(pub) != id || (p.pcId != null && p.pcId != id)) {
                    failPairing(getString(R.string.err_qr_mismatch))
                    return
                }
                p.candidate = id to pub
            }
            "error" -> {
                val code = msg.optString("code")
                if (pending != null && (code == "room_not_found" || code == "rate_limited")) {
                    failPairing(if (code == "rate_limited") getString(R.string.err_rate_limited) else getString(R.string.err_code_expired))
                } else {
                    _state.update { it.copy(error = msg.optString("msg")) }
                }
            }
        }
    }

    private fun onFrame(frame: ByteArray) {
        val relay = Wire.parseRelay(frame) ?: return
        val payload = relay.payload
        when (payload[0]) {
            HS_INIT -> onHandshake(relay.peer, payload)
            DATA -> {
                val link = links[relay.peer] ?: return
                val plain = try {
                    link.session.opener.open(payload)
                } catch (_: Exception) {
                    // Out of sync: ask the computer for a fresh handshake.
                    socket?.send(Wire.relay(relay.peer, byteArrayOf(REKEY)).toByteString())
                    return
                }
                link.pairingPub?.let { confirmPairing(link, it) }
                onPlain(link, plain)
            }
        }
    }

    private fun onHandshake(pcId: String, hsInit: ByteArray) {
        val p = pending
        val candidate = p?.candidate?.takeIf { it.first == pcId }
        val paired = store.pc(pcId)
        val (peerPub, psk) = when {
            candidate != null -> candidate.second to Pairing.psk(p.secret)
            paired != null -> paired.pub.unhex() to null
            else -> return
        }
        val (session, resp) = try {
            Handshake.respond(store.identity, peerPub, psk, hsInit)
        } catch (e: Exception) {
            if (candidate != null) failPairing(e.message ?: "handshake failed")
            return
        }
        dropLink(pcId)
        links[pcId] = Link(pcId, session, pairingPub = if (candidate != null) peerPub else null)
        online.add(pcId)
        socket?.send(Wire.relay(pcId, resp).toByteString())
        val hello = JSONObject().put("m", "hello").put(
            "p",
            JSONObject().put("name", deviceName()).put("model", "${Build.MANUFACTURER} ${Build.MODEL}".trim())
                .put("android", Build.VERSION.RELEASE).put("app", BuildConfig.VERSION_NAME),
        )
        sealSend(pcId, Wire.channel(Wire.CH_RPC, 0, hello.toString().toByteArray()))
        publishPcs()
    }

    private fun onPlain(link: Link, plain: ByteArray) {
        val c = Wire.parseChannel(plain) ?: return
        val key = "${link.pcId}:${c.stream}"
        when (c.channel) {
            Wire.CH_RPC -> {
                val msg = try { JSONObject(String(c.body)) } catch (_: Exception) { return }
                when (msg.optString("m")) {
                    "hello" -> {
                        val p = msg.optJSONObject("p") ?: return
                        store.renamePc(link.pcId, p.optString("name", "Computer"), p.optString("os"))
                        val st = _state.value.pairing
                        if (st is PairingState.Done) _state.update { it.copy(pairing = PairingState.Done(p.optString("name", st.pcName))) }
                        publishPcs()
                    }
                    "unpair" -> removePc(link.pcId, notifyPc = false)
                }
            }
            Wire.CH_OPEN -> {
                if (String(c.body) != "adb") {
                    sealSend(link.pcId, Wire.channel(Wire.CH_CLOSE, c.stream, "unsupported".toByteArray()))
                    return
                }
                val stream = Stream()
                streams[key] = stream
                publishStreams()
                workers.execute { openAdbStream(link.pcId, c.stream, stream) }
            }
            Wire.CH_DATA -> streams[key]?.let { if (!it.outbox.offer(c.body, 10, TimeUnit.SECONDS)) closeStream(key) }
            Wire.CH_CLOSE -> closeStream(key)
        }
    }

    // ------------------------------------------------------------------ ADB bridge

    private fun openAdbStream(pcId: String, sid: Int, stream: Stream) {
        val key = "$pcId:$sid"
        val sock = Socket()
        try {
            sock.tcpNoDelay = true
            sock.connect(InetSocketAddress("127.0.0.1", ADB_PORT), 3000)
        } catch (e: Exception) {
            runCatching { sock.close() }
            sealSend(pcId, Wire.channel(Wire.CH_CLOSE, sid, "adb not reachable on port 5555".toByteArray()))
            closeStream(key)
            return
        }
        stream.socket = sock
        if (stream.closed) {
            runCatching { sock.close() }
            return
        }
        workers.execute {
            try {
                val out = sock.getOutputStream()
                while (!stream.closed) {
                    val chunk = stream.outbox.poll(1, TimeUnit.SECONDS) ?: continue
                    out.write(chunk)
                    out.flush()
                }
            } catch (_: Exception) {
            } finally {
                closeStream(key)
            }
        }
        try {
            val input: InputStream = sock.getInputStream()
            val buf = ByteArray(Wire.MAX_CHUNK)
            while (!stream.closed) {
                val n = input.read(buf)
                if (n < 0) break
                if (!sealSend(pcId, Wire.channel(Wire.CH_DATA, sid, buf.copyOf(n)))) break
            }
        } catch (_: Exception) {
        }
        if (streams.containsKey(key)) {
            sealSend(pcId, Wire.channel(Wire.CH_CLOSE, sid, ByteArray(0)))
            closeStream(key)
        }
    }

    private fun closeStream(key: String) {
        val s = streams.remove(key) ?: return
        s.closed = true
        runCatching { s.socket?.close() }
        publishStreams()
    }

    /** Seals and sends in one critical section so frame counters reach the PC in order. */
    private fun sealSend(pcId: String, plain: ByteArray): Boolean {
        val link = links[pcId] ?: return false
        synchronized(link) {
            val ws = socket ?: return false
            while (ws.queueSize() > SEND_QUEUE_LIMIT) {
                if (links[pcId] !== link) return false
                Thread.sleep(5)
            }
            if (links[pcId] !== link) return false
            return ws.send(Wire.relay(pcId, link.session.sealer.seal(plain)).toByteString())
        }
    }

    private fun dropLink(pcId: String) {
        links.remove(pcId)
        streams.keys.filter { it.startsWith("$pcId:") }.forEach(::closeStream)
    }

    private fun dropAllLinks() {
        links.keys.toList().forEach(::dropLink)
        online.clear()
        publishPcs()
    }

    // ------------------------------------------------------------------ pairing

    private fun beginPairing(secret: String, gateway: String?, pcId: String?) {
        val normalized = Pairing.normalize(secret)
        if (!Pairing.isValid(normalized)) {
            _state.update { it.copy(pairing = PairingState.Failed(getString(R.string.err_code_invalid))) }
            return
        }
        if (!gateway.isNullOrBlank() && gateway != store.gatewayUrl) {
            store.gatewayUrl = gateway
            reconnectNow()
        }
        pending = PendingPair(normalized, Pairing.room(normalized), pcId, System.currentTimeMillis() + 90_000)
        _state.update { it.copy(pairing = PairingState.Waiting) }
        if (_state.value.gateway == GatewayState.ONLINE) sendJson(JSONObject().put("t", "room_join").put("room", pending!!.room))
        io.schedule({
            val p = pending
            if (p != null && System.currentTimeMillis() >= p.deadline) failPairing(getString(R.string.err_pair_timeout))
        }, 91, TimeUnit.SECONDS)
    }

    /** The computer's first frame decrypted, so it holds the pair secret: keep the pairing. */
    private fun confirmPairing(link: Link, pub: ByteArray) {
        link.pairingPub = null
        store.upsertPc(PairedPc(link.pcId, pub.hex(), getString(R.string.default_pc_name), "", System.currentTimeMillis()))
        pending = null
        sendJson(JSONObject().put("t", "peers").put("ids", JSONArray(store.pcs().map { it.id })))
        _state.update { it.copy(pairing = PairingState.Done(getString(R.string.default_pc_name))) }
        publishPcs()
    }

    private fun failPairing(reason: String) {
        pending = null
        _state.update { it.copy(pairing = PairingState.Failed(reason)) }
        if (!shouldRun()) {
            shutdown()
            stopSelf()
        }
    }

    private fun removePc(id: String, notifyPc: Boolean) {
        if (notifyPc && links.containsKey(id)) {
            sealSend(id, Wire.channel(Wire.CH_RPC, 0, """{"m":"unpair"}""".toByteArray()))
        }
        store.removePc(id)
        dropLink(id)
        online.remove(id)
        sendJson(JSONObject().put("t", "peers").put("ids", JSONArray(store.pcs().map { it.id })))
        publishPcs()
        if (!shouldRun()) {
            shutdown()
            stopSelf()
        }
    }

    // ------------------------------------------------------------------ UI state

    private fun publishPcs() {
        val list = store.pcs().map { PcView(it.id, it.name, it.os, links.containsKey(it.id) && online.contains(it.id)) }
        _state.update { it.copy(pcs = list) }
        refreshNotification()
    }

    private fun publishStreams() = _state.update { it.copy(activeStreams = streams.size) }

    private fun deviceName(): String =
        Settings.Global.getString(contentResolver, Settings.Global.DEVICE_NAME)?.takeIf { it.isNotBlank() } ?: Build.MODEL

    private fun notificationText(): String {
        val s = _state.value
        val connected = s.pcs.filter { it.online }
        return when {
            s.pairing == PairingState.Waiting -> getString(R.string.notif_pairing)
            connected.isNotEmpty() -> getString(R.string.notif_connected, connected.joinToString { it.name })
            s.gateway == GatewayState.ONLINE -> getString(R.string.notif_waiting)
            s.gateway == GatewayState.CONNECTING -> getString(R.string.notif_connecting)
            else -> getString(R.string.notif_offline)
        }
    }

    private fun refreshNotification() {
        getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, buildNotification(notificationText()))
    }

    private fun buildNotification(text: String): Notification {
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_pairspan_notification)
            .setColor(getColor(R.color.ember))
            .setContentTitle(getString(R.string.app_name))
            .setContentText(text)
            .setContentIntent(open)
            .setOngoing(true)
            .build()
    }

    private fun startInForeground(text: String) {
        val n = buildNotification(text)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIFICATION_ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        } else {
            startForeground(NOTIFICATION_ID, n)
        }
    }
}
