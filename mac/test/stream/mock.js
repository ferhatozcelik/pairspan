// Synthetic renderer smoke fixture, never included in the application bundle.
const deviceId = 'a'.repeat(32);
const state = { enabled: true, status: 'online', clientId: 'c'.repeat(32), phones: [{ deviceId, connected: true, adbReady: true, name: 'Synthetic 720p source' }], command: {} };
let listener, sender, track, streamId, drawTimer, canvas, stateListener;
const candidates = [];
window.smoke = { commands: [], answers: 0, stopped: false };
function emit(event, payload = {}) { listener?.({ deviceId, event, payload: { ...payload, streamId } }); }
window.pairspan = {
  gatewayState: async () => state, onGatewayState: fn => { stateListener = fn; return () => { stateListener = null; }; }, streamConfig: async () => [],
  onStreamSignal: fn => { listener = fn; return () => { listener = null; }; },
  streamSignal: async (_device, event, p) => {
    streamId = p.streamId;
    if (event === 'stream_request') { emit('stream_ready'); return; }
    if (event === 'stream_stop') { sender?.close(); track?.stop(); clearInterval(drawTimer); window.smoke.stopped = true; return; }
    if (event === 'stream_ice') {
      if (sender?.remoteDescription) await sender.addIceCandidate(p.candidate); else candidates.push(p.candidate);
      return;
    }
    if (event !== 'stream_offer') return;
    if (!sender) {
      sender = new RTCPeerConnection({ iceServers: [] });
      canvas = document.createElement('canvas'); canvas.width = 1280; canvas.height = 720;
      const ctx = canvas.getContext('2d');
      let frame = 0;
      drawTimer = setInterval(() => { ctx.fillStyle = `hsl(${frame++ % 360} 70% 40%)`; ctx.fillRect(0, 0, 1280, 720); ctx.fillStyle = '#fff'; ctx.font = '50px sans-serif'; ctx.fillText(String(frame), 40, 80); }, 1000 / 30);
      track = canvas.captureStream(30).getVideoTracks()[0];
      sender.addTrack(track);
      sender.onicecandidate = e => { if (e.candidate && / udp /i.test(e.candidate.candidate)) emit('stream_ice', { candidate: e.candidate.toJSON() }); };
    }
    await sender.setRemoteDescription({ type: 'offer', sdp: p.sdp });
    for (const c of candidates.splice(0)) await sender.addIceCandidate(c);
    const answer = await sender.createAnswer(); await sender.setLocalDescription(answer);
    emit('stream_answer', { sdp: answer.sdp }); window.smoke.answers++;
    emit('stream_stats', { screenWidth: 1280, screenHeight: 720, captureFps: 30, encodedFps: 30 });
  },
  runCommand: async (id, command) => { window.smoke.commands.push(command); },
  usbDevices: async () => [], copyText: async () => {}, setEnabled: async () => {}
};

window.smoke.reconnect = () => { stateListener?.({ ...state, status: 'offline', phones: [] }); setTimeout(() => stateListener?.(state), 200); };
window.smoke.rotate = () => { canvas.width = 720; canvas.height = 1280; emit('stream_stats', { screenWidth: 720, screenHeight: 1280 }); };
