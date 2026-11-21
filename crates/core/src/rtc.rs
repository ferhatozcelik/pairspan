//! One WebRTC peer connection per phone, with a single reliable data channel that carries the
//! end-to-end encrypted frames of docs/protocol.md. The PC always makes the offer.
//! ICE is not trickled: each side sends its description once gathering is complete.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use webrtc::data_channel::DataChannel;
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder, RTCIceGatheringState,
    RTCIceServer, RTCPeerConnectionState, RTCSessionDescription,
};

/// Bytes queued per channel before `send` waits; also the backpressure for the ADB tunnel.
const SEND_BUFFER: usize = 4 * 1024 * 1024;

struct Handler {
    gathered: Arc<Notify>,
    dead: Arc<Notify>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for Handler {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            self.gathered.notify_one();
        }
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        if matches!(state, RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed | RTCPeerConnectionState::Disconnected) {
            self.dead.notify_one();
        }
    }
}

pub struct Offer {
    pub pc: Arc<dyn PeerConnection>,
    pub dc: Arc<dyn DataChannel>,
    /// Fires when the connection fails or closes.
    pub dead: Arc<Notify>,
    /// The complete offer (all candidates included), as JSON for the signalling channel.
    pub sdp: Value,
}

pub async fn offer() -> Result<Offer> {
    let gathered = Arc::new(Notify::new());
    let dead = Arc::new(Notify::new());
    let pc = PeerConnectionBuilder::new()
        .with_configuration(
            RTCConfigurationBuilder::new()
                .with_ice_servers(vec![RTCIceServer {
                    urls: vec!["stun:stun.l.google.com:19302".to_owned(), "stun:stun.cloudflare.com:3478".to_owned()],
                    ..Default::default()
                }])
                .build(),
        )
        .with_handler(Arc::new(Handler { gathered: gathered.clone(), dead: dead.clone() }))
        .with_udp_addrs(vec!["0.0.0.0:0"])
        .with_data_channel_send_buffer_limit(SEND_BUFFER)
        .build()
        .await
        .map_err(|e| anyhow!("webrtc: {e}"))?;
    let pc: Arc<dyn PeerConnection> = Arc::new(pc);
    let dc = pc.create_data_channel("pairspan", None).await.map_err(|e| anyhow!("data channel: {e}"))?;
    let offer = pc.create_offer(None).await.map_err(|e| anyhow!("offer: {e}"))?;
    pc.set_local_description(offer).await.map_err(|e| anyhow!("offer: {e}"))?;
    // Gathering usually takes well under a second; after the timeout we send what we have.
    let _ = tokio::time::timeout(Duration::from_secs(4), gathered.notified()).await;
    let local = pc.local_description().await.context("no local description")?;
    Ok(Offer { pc, dc, dead, sdp: serde_json::to_value(local)? })
}

pub async fn accept_answer(pc: &Arc<dyn PeerConnection>, sdp: &Value) -> Result<()> {
    let answer: RTCSessionDescription = serde_json::from_value(sdp.clone()).context("bad answer")?;
    pc.set_remote_description(answer).await.map_err(|e| anyhow!("answer: {e}"))
}
