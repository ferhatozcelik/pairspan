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
import org.json.JSONObject
import org.webrtc.DataChannel
import java.io.InputStream
import java.net.InetSocketAddress
import java.net.Socket
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

enum class NetState { OFFLINE, CONNECTING, ONLINE }

sealed interface PairingState {
    data object Idle : PairingState
    data object Waiting : PairingState
    data class Done(val pcName: String) : PairingState
    data class Failed(val reason: String) : PairingState
}

data class PcView(val id: String, val name: String, val os: String, val online: Boolean)

data class LinkState(
    val enabled: Boolean = true,
    val net: NetState = NetState.OFFLINE,
    val pcs: List<PcView> = emptyList(),
    val pairing: PairingState = PairingState.Idle,
    val activeStreams: Int = 0,
    val error: String? = null,
)

/**
 * Keeps the phone's side of the link: a signalling connection (Supabase Realtime), a direct WebRTC
 * link with an encrypted session per paired computer, and ADB streams bridged to this phone's own
 * adbd on 127.0.0.1:5555.
 * All protocol state is touched only on [io]; stream threads only seal and send.
 */
class LinkService : Service() {
    companion object {
        private const val ACTION_START = "start"
        private const val ACTION_PAIR = "pair"
        private const val ACTION_UNPAIR = "unpair"
        private const val ACTION_STOP = "stop"
        private const val CHANNEL = "link"
        private const val NOTIFICATION_ID = 1
        private const val ADB_PORT = 5555

        private val _state = MutableStateFlow(LinkState())
        val state: StateFlow<LinkState> = _state

        fun start(ctx: Context) = send(ctx, Intent(ctx, LinkService::class.java).setAction(ACTION_START))
        fun stop(ctx: Context) = send(ctx, Intent(ctx, LinkService::class.java).setAction(ACTION_STOP))
        fun unpair(ctx: Context, id: String) =
            send(ctx, Intent(ctx, LinkService::class.java).setAction(ACTION_UNPAIR).putExtra("id", id))

        fun pair(ctx: Context, secret: String, pcId: String) = send(
            ctx,
            Intent(ctx, LinkService::class.java).setAction(ACTION_PAIR).putExtra("secret", secret).putExtra("pc", pcId),
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

    private class PendingPair(val secret: String, val room: String, val pcId: String, val deadline: Long) {
        var candidate: ByteArray? = null
    }

    /** [pairingPub] is set until the computer proves the pair secret by sending a frame we can decrypt. */
    private class Link(val pcId: String, val session: Session, val channel: DataChannel, var pairingPub: ByteArray? = null)

    /** Registered as soon as the PC opens it, so bytes that arrive before adbd answers are queued, not lost. */
    private class Stream {
        val outbox = LinkedBlockingQueue<ByteArray>(256)
        @Volatile var socket: Socket? = null
        @Volatile var closed = false
    }

    /** One WebRTC attempt with a computer; [channel] is set once the data channel opens. */
    private class Attempt(val rtc: Rtc) {
        @Volatile var channel: DataChannel? = null
    }

    private lateinit var store: PairStore
    private val io = Executors.newSingleThreadScheduledExecutor { r -> Thread(r, "pairspan-link") }
    private val workers = Executors.newCachedThreadPool()
    private val http = OkHttpClient.Builder()
        .readTimeout(0, TimeUnit.MILLISECONDS)
        .connectTimeout(15, TimeUnit.SECONDS)
        .build()

    private var realtime: Realtime? = null
    private var generation = 0
    private var backoffSeconds = 1L
    private var pending: PendingPair? = null
    private val links = ConcurrentHashMap<String, Link>()
    private val attempts = HashMap<String, Attempt>()
    private val topics = HashMap<String, Pair<String, ByteArray>>() // link channel → (pc id, pc key)
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
                ACTION_PAIR -> beginPairing(intent!!.getStringExtra("secret").orEmpty(), intent.getStringExtra("pc").orEmpty())
                ACTION_UNPAIR -> removePc(intent!!.getStringExtra("id").orEmpty(), notifyPc = true)
                ACTION_STOP -> {
                    shutdown()
                    stopSelf()
                    return@execute
                }
                else -> {}
            }
            if (realtime == null && shouldRun()) connect()
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

    // ------------------------------------------------------------------ signalling

    private fun connect() {
        val gen = ++generation
        _state.update { it.copy(net = NetState.CONNECTING) }
        refreshNotification()
        topics.clear()
        store.pcs().forEach { topics[Wire.linkName(it.pub.unhex(), store.publicKey)] = it.id to it.pub.unhex() }
        val rt = Realtime(http, object : Realtime.Events {
            override fun onOpen() { io.execute { if (gen == generation) onOpen(gen) } }
            override fun onClosed(reason: String) { io.execute { if (gen == generation) onDisconnected(reason) } }
            override fun onJoined(topic: String) { io.execute { if (gen == generation) joined(topic) } }
            override fun onBroadcast(topic: String, payload: JSONObject) { io.execute { if (gen == generation) broadcastReceived(topic, payload) } }
            override fun onPresence(topic: String, key: String, online: Boolean) {
                io.execute { if (gen == generation && key == "pc" && online) wantLink(topic) }
            }
        })
        realtime = rt
        rt.connect()
    }

    private fun onOpen(gen: Int) {
        backoffSeconds = 1
        _state.update { it.copy(net = NetState.ONLINE, error = null) }
        topics.keys.forEach { realtime?.join(it, "phone") }
        pending?.let { realtime?.join(Wire.roomName(it.room), "phone") }
        heartbeat(gen)
        publishPcs()
    }

    private fun heartbeat(gen: Int) {
        io.schedule({ if (gen == generation) { realtime?.heartbeat(); heartbeat(gen) } }, 25, TimeUnit.SECONDS)
    }

    private fun onDisconnected(reason: String) {
        realtime = null
        _state.update { it.copy(net = NetState.OFFLINE, error = reason.take(120)) }
        refreshNotification()
        if (!shouldRun()) return
        // Direct links survive a signalling outage; only reconnect the signalling.
        val gen = ++generation
        io.schedule({ if (gen == generation && realtime == null && shouldRun()) connect() }, backoffSeconds, TimeUnit.SECONDS)
        backoffSeconds = (backoffSeconds * 2).coerceAtMost(30)
    }

    private fun shutdown() {
        generation++
        realtime?.close()
        realtime = null
        dropAllLinks()
        _state.update { it.copy(net = NetState.OFFLINE) }
    }

    private fun joined(topic: String) {
        if (topics.containsKey(topic)) wantLink(topic)
        val p = pending
        if (p != null && topic == Wire.roomName(p.room)) sayHello(p)
    }

    /** Repeats our key in the pairing room until the computer answers with its own. */
    private fun sayHello(p: PendingPair) {
        if (pending !== p || p.candidate != null) return
        realtime?.broadcast(Wire.roomName(p.room), JSONObject().put("t", "phone").put("pub", Base64.encodeToString(store.publicKey, Base64.NO_WRAP)))
        io.schedule({ sayHello(p) }, 1500, TimeUnit.MILLISECONDS)
    }

    /** Asks the computer on [topic] for a fresh offer, unless that link is already up. */
    private fun wantLink(topic: String) {
        val (pcId, _) = topics[topic] ?: return
        if (links.containsKey(pcId)) return
        realtime?.broadcast(topic, JSONObject().put("t", "want"))
    }

    private fun broadcastReceived(topic: String, msg: JSONObject) {
        when (msg.optString("t")) {
            "pc" -> {
                val p = pending ?: return
                if (topic != Wire.roomName(p.room) || p.candidate != null) return
                val pub = try { Base64.decode(msg.getString("pub"), Base64.DEFAULT) } catch (_: Exception) { return }
                // Only the computer that handed us the secret over USB may answer.
                if (com.pairspan.app.core.peerId(pub) != p.pcId) return
                p.candidate = pub
                val link = Wire.linkName(pub, store.publicKey)
                topics[link] = p.pcId to pub
                realtime?.join(link, "phone")
            }
            "offer" -> {
                val (pcId, _) = topics[topic] ?: return
                val sdp = msg.optJSONObject("sdp") ?: return
                val g = msg.opt("g")
                dropLink(pcId)
                lateinit var attempt: Attempt
                attempt = Attempt(Rtc(object : Rtc.Listener {
                    override fun onAnswer(sdp: JSONObject) {
                        io.execute { if (attempts[pcId] === attempt) realtime?.broadcast(topic, JSONObject().put("t", "answer").put("g", g).put("sdp", sdp)) }
                    }
                    override fun onOpen(channel: DataChannel) { io.execute { if (attempts[pcId] === attempt) attempt.channel = channel } }
                    override fun onMessage(bytes: ByteArray) { io.execute { if (attempts[pcId] === attempt) onFrame(pcId, attempt, bytes) } }
                    override fun onClosed() { io.execute { if (attempts[pcId] === attempt) { dropLink(pcId); wantLink(topic) } } }
                }))
                attempts[pcId] = attempt
                attempt.rtc.answer(this, sdp)
            }
        }
    }

    private fun onFrame(pcId: String, attempt: Attempt, payload: ByteArray) {
        if (payload.isEmpty()) return
        when (payload[0]) {
            HS_INIT -> onHandshake(pcId, attempt, payload)
            DATA -> {
                val link = links[pcId] ?: return
                val plain = try {
                    link.session.opener.open(payload)
                } catch (_: Exception) {
                    // Out of sync: ask the computer for a fresh handshake.
                    link.channel.sendFrame(byteArrayOf(REKEY)) { true }
                    return
                }
                link.pairingPub?.let { confirmPairing(link, it) }
                onPlain(link, plain)
            }
        }
    }

    private fun onHandshake(pcId: String, attempt: Attempt, hsInit: ByteArray) {
        val channel = attempt.channel ?: return
        val p = pending
        val candidate = p?.candidate?.takeIf { p.pcId == pcId }
        val paired = store.pc(pcId)
        val (peerPub, psk) = when {
            candidate != null -> candidate to Pairing.psk(p.secret)
            paired != null -> paired.pub.unhex() to null
            else -> return
        }
        val (session, resp) = try {
            Handshake.respond(store.identity, peerPub, psk, hsInit)
        } catch (e: Exception) {
            if (candidate != null) failPairing(e.message ?: "handshake failed")
            return
        }
        links.remove(pcId)
        streams.keys.filter { it.startsWith("$pcId:") }.forEach(::closeStream)
        links[pcId] = Link(pcId, session, channel, pairingPub = if (candidate != null) peerPub else null)
        channel.sendFrame(resp) { true }
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
            if (links[pcId] !== link) return false
            return link.channel.sendFrame(link.session.sealer.seal(plain)) { links[pcId] === link }
        }
    }

    private fun dropLink(pcId: String) {
        links.remove(pcId)
        attempts.remove(pcId)?.rtc?.close()
        streams.keys.filter { it.startsWith("$pcId:") }.forEach(::closeStream)
        publishPcs()
    }

    private fun dropAllLinks() {
        (links.keys + attempts.keys).toSet().forEach(::dropLink)
        publishPcs()
    }

    // ------------------------------------------------------------------ pairing

    /** The computer handed us a pair secret and its id over USB (see PairReceiver). */
    private fun beginPairing(secret: String, pcId: String) {
        if (!Pairing.isValid(secret) || !pcId.matches(Regex("[0-9a-f]{32}"))) return
        store.enabled = true
        pending?.let { realtime?.leave(Wire.roomName(it.room)) }
        val p = PendingPair(secret, Pairing.room(secret), pcId, System.currentTimeMillis() + 90_000)
        pending = p
        _state.update { it.copy(pairing = PairingState.Waiting, enabled = true) }
        if (_state.value.net == NetState.ONLINE) realtime?.join(Wire.roomName(p.room), "phone")
        io.schedule({ if (pending === p) failPairing(getString(R.string.err_pair_timeout)) }, 91, TimeUnit.SECONDS)
    }

    /** The computer's first frame decrypted, so it holds the pair secret: keep the pairing. */
    private fun confirmPairing(link: Link, pub: ByteArray) {
        link.pairingPub = null
        store.upsertPc(PairedPc(link.pcId, pub.hex(), getString(R.string.default_pc_name), "", System.currentTimeMillis()))
        pending?.let { realtime?.leave(Wire.roomName(it.room)) }
        pending = null
        _state.update { it.copy(pairing = PairingState.Done(getString(R.string.default_pc_name))) }
        publishPcs()
    }

    private fun failPairing(reason: String) {
        val p = pending ?: return
        pending = null
        realtime?.leave(Wire.roomName(p.room))
        topics.entries.removeAll { it.value.first == p.pcId && store.pc(p.pcId) == null }
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
        topics.entries.filter { it.value.first == id }.forEach { realtime?.leave(it.key); topics.remove(it.key) }
        publishPcs()
        if (!shouldRun()) {
            shutdown()
            stopSelf()
        }
    }

    // ------------------------------------------------------------------ UI state

    private fun publishPcs() {
        val list = store.pcs().map { PcView(it.id, it.name, it.os, links.containsKey(it.id) && links[it.id]?.pairingPub == null) }
        _state.update { it.copy(pcs = list, enabled = store.enabled) }
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
            s.net == NetState.ONLINE -> getString(R.string.notif_waiting)
            s.net == NetState.CONNECTING -> getString(R.string.notif_connecting)
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
