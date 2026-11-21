package com.pairspan.app.link

import android.content.Context
import org.json.JSONObject
import org.webrtc.DataChannel
import org.webrtc.IceCandidate
import org.webrtc.MediaConstraints
import org.webrtc.MediaStream
import org.webrtc.PeerConnection
import org.webrtc.PeerConnectionFactory
import org.webrtc.RtpReceiver
import org.webrtc.SdpObserver
import org.webrtc.SessionDescription
import java.nio.ByteBuffer

/**
 * The phone's side of one direct link: answers the computer's offer and hands over the data channel
 * the computer opens. ICE is not trickled; the answer goes out once gathering is complete.
 * Callbacks run on WebRTC's threads; the caller hops to its own.
 */
class Rtc(private val listener: Listener) {
    interface Listener {
        fun onAnswer(sdp: JSONObject)
        fun onOpen(channel: DataChannel)
        fun onMessage(bytes: ByteArray)
        fun onClosed()
    }

    companion object {
        @Volatile private var factory: PeerConnectionFactory? = null

        fun factory(ctx: Context): PeerConnectionFactory = factory ?: synchronized(this) {
            factory ?: run {
                PeerConnectionFactory.initialize(PeerConnectionFactory.InitializationOptions.builder(ctx.applicationContext).createInitializationOptions())
                PeerConnectionFactory.builder().createPeerConnectionFactory().also { factory = it }
            }
        }

        private val ICE = listOf(
            PeerConnection.IceServer.builder(listOf("stun:stun.l.google.com:19302", "stun:stun.cloudflare.com:3478")).createIceServer(),
        )
    }

    private var pc: PeerConnection? = null
    @Volatile private var closed = false

    fun answer(ctx: Context, offer: JSONObject) {
        val config = PeerConnection.RTCConfiguration(ICE).apply { sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN }
        val conn = factory(ctx).createPeerConnection(config, observer) ?: return listener.onClosed()
        pc = conn
        val remote = SessionDescription(SessionDescription.Type.OFFER, offer.getString("sdp"))
        conn.setRemoteDescription(object : Sdp() {
            override fun onSetSuccess() = conn.createAnswer(object : Sdp() {
                override fun onCreateSuccess(desc: SessionDescription) = conn.setLocalDescription(Sdp(), desc)
            }, MediaConstraints())
        }, remote)
    }

    fun close() {
        if (closed) return
        closed = true
        pc?.dispose()
        pc = null
    }

    private val observer = object : PeerConnection.Observer {
        override fun onIceGatheringChange(state: PeerConnection.IceGatheringState) {
            if (state != PeerConnection.IceGatheringState.COMPLETE) return
            val local = pc?.localDescription ?: return
            listener.onAnswer(JSONObject().put("type", "answer").put("sdp", local.description))
        }

        override fun onConnectionChange(state: PeerConnection.PeerConnectionState) {
            if (state == PeerConnection.PeerConnectionState.FAILED || state == PeerConnection.PeerConnectionState.CLOSED) {
                if (!closed) listener.onClosed()
            }
        }

        override fun onDataChannel(channel: DataChannel) {
            channel.registerObserver(object : DataChannel.Observer {
                override fun onBufferedAmountChange(previous: Long) {}
                override fun onStateChange() {
                    when (channel.state()) {
                        DataChannel.State.OPEN -> listener.onOpen(channel)
                        DataChannel.State.CLOSED -> if (!closed) listener.onClosed()
                        else -> {}
                    }
                }
                override fun onMessage(buffer: DataChannel.Buffer) {
                    val bytes = ByteArray(buffer.data.remaining())
                    buffer.data.get(bytes)
                    listener.onMessage(bytes)
                }
            })
        }

        override fun onSignalingChange(state: PeerConnection.SignalingState) {}
        override fun onIceConnectionChange(state: PeerConnection.IceConnectionState) {}
        override fun onIceConnectionReceivingChange(receiving: Boolean) {}
        override fun onIceCandidate(candidate: IceCandidate) {}
        override fun onIceCandidatesRemoved(candidates: Array<out IceCandidate>) {}
        override fun onAddStream(stream: MediaStream) {}
        override fun onRemoveStream(stream: MediaStream) {}
        override fun onRenegotiationNeeded() {}
        override fun onAddTrack(receiver: RtpReceiver, streams: Array<out MediaStream>) {}
    }

    private open class Sdp : SdpObserver {
        override fun onCreateSuccess(desc: SessionDescription) {}
        override fun onSetSuccess() {}
        override fun onCreateFailure(error: String) {}
        override fun onSetFailure(error: String) {}
    }
}

/** Sends a binary frame, waiting while more than 4 MiB is queued (backpressure for the ADB tunnel). */
fun DataChannel.sendFrame(frame: ByteArray, alive: () -> Boolean): Boolean {
    while (bufferedAmount() > 4L * 1024 * 1024) {
        if (!alive() || state() != DataChannel.State.OPEN) return false
        Thread.sleep(5)
    }
    return state() == DataChannel.State.OPEN && send(DataChannel.Buffer(ByteBuffer.wrap(frame), true))
}
