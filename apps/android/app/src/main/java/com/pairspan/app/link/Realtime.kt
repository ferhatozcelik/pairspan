package com.pairspan.app.link

import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONObject
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

/**
 * Supabase Realtime (Phoenix channels over one WebSocket), the same client as crates/core/src/signal.rs:
 * presence tells both ends of a link the other is around, broadcast carries pairing hellos and
 * WebRTC offers/answers. Nothing secret travels here; data goes peer to peer.
 * Callbacks run on OkHttp's thread; the caller hops to its own.
 */
class Realtime(private val http: OkHttpClient, private val events: Events) {
    interface Events {
        fun onOpen()
        fun onClosed(reason: String)
        fun onJoined(topic: String)
        fun onBroadcast(topic: String, payload: JSONObject)
        fun onPresence(topic: String, key: String, online: Boolean)
    }

    companion object {
        /** The public Pairspan project; a publishable key only reaches public Realtime channels. */
        private const val URL = "wss://jcqkznspriizdmsbjvvj.supabase.co/realtime/v1/websocket" +
            "?apikey=sb_publishable_WFPzgHprFKVKb7CKApBnWQ_fnp4gVcm&vsn=1.0.0"
        private const val PREFIX = "realtime:pairspan:"
    }

    private var ws: WebSocket? = null
    private val ref = AtomicLong(1)
    private val joins = ConcurrentHashMap<String, String>() // join ref → topic

    fun connect() {
        ws = http.newWebSocket(Request.Builder().url(URL).build(), object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) = events.onOpen()
            override fun onMessage(webSocket: WebSocket, text: String) = dispatch(text)
            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) = events.onClosed(reason)
            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) =
                events.onClosed(t.message ?: t.javaClass.simpleName)
        })
    }

    fun close() {
        ws?.close(1000, "bye")
        ws = null
        joins.clear()
    }

    fun heartbeat() = push("phoenix", "heartbeat", JSONObject())

    fun join(name: String, key: String) {
        val config = JSONObject().put(
            "config",
            JSONObject()
                .put("broadcast", JSONObject().put("self", false))
                .put("presence", JSONObject().put("key", key))
                .put("private", false),
        )
        joins[push(PREFIX + name, "phx_join", config)] = name
    }

    fun leave(name: String) = push(PREFIX + name, "phx_leave", JSONObject())

    fun broadcast(name: String, payload: JSONObject) =
        push(PREFIX + name, "broadcast", JSONObject().put("type", "broadcast").put("event", "sig").put("payload", payload))

    private fun push(topic: String, event: String, payload: JSONObject): String {
        val r = ref.getAndIncrement().toString()
        ws?.send(JSONObject().put("topic", topic).put("event", event).put("payload", payload).put("ref", r).toString())
        return r
    }

    private fun dispatch(text: String) {
        val m = try { JSONObject(text) } catch (_: Exception) { return }
        val topic = m.optString("topic").removePrefix(PREFIX).takeIf { it != m.optString("topic") } ?: return
        val payload = m.optJSONObject("payload") ?: return
        when (m.optString("event")) {
            "phx_reply" -> {
                val name = joins.remove(m.optString("ref")) ?: return
                if (payload.optString("status") != "ok") return
                push(PREFIX + name, "presence", JSONObject().put("type", "presence").put("event", "track").put("payload", JSONObject()))
                events.onJoined(name)
            }
            "broadcast" -> payload.optJSONObject("payload")?.let { events.onBroadcast(topic, it) }
            "presence_state" -> payload.keys().forEach { events.onPresence(topic, it, true) }
            "presence_diff" -> {
                payload.optJSONObject("joins")?.keys()?.forEach { events.onPresence(topic, it, true) }
                payload.optJSONObject("leaves")?.keys()?.forEach { events.onPresence(topic, it, false) }
            }
        }
    }
}
