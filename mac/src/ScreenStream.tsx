import { useEffect, useRef, useState } from 'react';
import type { GatewayState } from './types';

type Metrics = Record<string, number | string | null>;
const value = (n: unknown, unit = '') => typeof n === 'number' && Number.isFinite(n) ? `${n.toFixed(1)}${unit}` : typeof n === 'string' ? n : '—';

/** The renderer owns media, decoding and rendering. IPC carries only SDP/ICE and small diagnostics. */
export function ScreenStream({ deviceId, gateway, onClose }: { deviceId: string; gateway: GatewayState; onClose: () => void }) {
  const video = useRef<HTMLVideoElement>(null);
  const [status, setStatus] = useState('Requesting screen sharing permission…');
  const [metrics, setMetrics] = useState<Metrics>({});
  const [sender, setSender] = useState<Metrics>({});
  const senderMetrics = useRef<Metrics>({});
  const [ice, setIce] = useState('new');
  const [controlError, setControlError] = useState('');
  const controller = useRef<{ request: () => void; restart: () => void } | null>(null);
  const inputBusy = useRef(false);
  const gesture = useRef<{ x: number; y: number; at: number } | null>(null);
  const ready = gateway.status === 'online' && gateway.phones.some(p => p.deviceId === deviceId && p.connected);
  const adbReady = gateway.phones.some(p => p.deviceId === deviceId && p.adbReady);

  useEffect(() => {
    let closed = false, pc: RTCPeerConnection | null = null, pendingIce: RTCIceCandidateInit[] = [];
    let signalChain = Promise.resolve(), queued = 0, statsBusy = false;
    let previous: any, previousRender = 0, previousTime = performance.now();
    let lastFrame = performance.now(), freezeStart: number | null = null, freezes = 0;
    let restartTimer: ReturnType<typeof setTimeout> | undefined, negotiationTimer: ReturnType<typeof setTimeout> | undefined;
    let restarts = 0, reconnectAt = 0, rendered = 0, callbackId = 0;
    let samples: Record<string, unknown>[] = [];
    const captureToDisplay: number[] = [];
    let recoveryMs: number | null = null;
    let lastFreezeMs: number | null = null;
    const streamId = crypto.randomUUID();
    const element = video.current!;
    const signal = (event: string, payload: Record<string, unknown> = {}) => window.pairspan.streamSignal(deviceId, event, { ...payload, streamId });
    const fail = (error: unknown) => { if (!closed) setStatus(String(error)); };
    const scheduleRestart = (delayMs = 2500) => {
      if (closed || restartTimer) return;
      if (rendered > 0) reconnectAt ||= performance.now();
      restartTimer = setTimeout(() => {
        restartTimer = undefined;
        if (closed || !pc || ['connected', 'completed'].includes(pc.iceConnectionState)) return;
        if (restarts >= 3) { fail('Direct UDP connection failed. These networks may require TURN; no video relay is configured.'); return; }
        restarts++; void offer(true).catch(fail);
      }, delayMs * (restarts + 1));
    };
    async function offer(restart = false) {
      if (!pc || closed || pc.signalingState !== 'stable') return;
      pendingIce = [];
      const desc = await pc.createOffer({ iceRestart: restart });
      if (closed) return;
      // Do not advertise TCP candidates, including in a gathered SDP during an ICE restart.
      desc.sdp = desc.sdp?.split('\r\n').filter(line => !line.startsWith('a=candidate:') || / udp /i.test(line)).join('\r\n');
      await pc.setLocalDescription(desc);
      await signal('stream_offer', { sdp: desc.sdp });
      clearTimeout(negotiationTimer);
      negotiationTimer = setTimeout(() => fail('WebRTC answer timed out. Stop and request screen sharing again.'), 15000);
    }
    async function startPeer() {
      if (pc) { if (pc.signalingState === 'stable') await offer(true); return; }
      const iceServers = await window.pairspan.streamConfig();
      if (closed) return;
      pc = new RTCPeerConnection({ iceServers, bundlePolicy: 'max-bundle' });
      const transceiver = pc.addTransceiver('video', { direction: 'recvonly' });
      const codecs = RTCRtpReceiver.getCapabilities('video')?.codecs.filter(c => c.mimeType.toLowerCase() === 'video/h264') || [];
      if (!codecs.length) throw new Error('H.264 decoding is unavailable on this computer');
      transceiver.setCodecPreferences(codecs);
      pc.ontrack = event => {
        if (closed) return;
        lastFrame = performance.now();
        element.srcObject = new MediaStream([event.track]);
        void element.play().catch(fail);
        setStatus('Streaming');
      };
      pc.onicecandidate = event => {
        if (event.candidate && / udp /i.test(event.candidate.candidate)) void signal('stream_ice', { candidate: event.candidate.toJSON() }).catch(fail);
      };
      pc.oniceconnectionstatechange = () => {
        if (!pc || closed) return;
        setIce(pc.iceConnectionState);
        if (['connected', 'completed'].includes(pc.iceConnectionState)) {
          clearTimeout(restartTimer); restartTimer = undefined; restarts = 0;
          if (reconnectAt) { recoveryMs = performance.now() - reconnectAt; setMetrics(m => ({ ...m, recoveryMs })); reconnectAt = 0; }
          setStatus('Streaming');
        } else if (['failed', 'disconnected'].includes(pc.iceConnectionState)) { setStatus('UDP connection interrupted; recovering…'); scheduleRestart(); }
      };
      await offer();
    }
    const unsubscribe = window.pairspan.onStreamSignal(message => {
      if (closed || message.deviceId !== deviceId || message.payload.streamId !== streamId) return;
      if (++queued > 128) { queued--; fail('Too many signaling messages'); pc?.close(); return; }
      signalChain = signalChain.then(async () => {
        if (closed) return;
        const p = message.payload;
        switch (message.event) {
          case 'stream_ready': await startPeer(); break;
          case 'stream_answer':
            if (pc?.signalingState !== 'have-local-offer') return;
            await pc.setRemoteDescription({ type: 'answer', sdp: p.sdp });
            clearTimeout(negotiationTimer);
            for (const c of pendingIce) await pc.addIceCandidate(c);
            pendingIce = [];
            // A restart that fails without a new state transition still gets a bounded retry.
            scheduleRestart(10000);
            break;
          case 'stream_ice':
            if (!/ udp /i.test(p.candidate?.candidate || '')) return;
            if (pc?.remoteDescription && pc.signalingState === 'stable') await pc.addIceCandidate(p.candidate);
            else if (pendingIce.length < 128) pendingIce.push(p.candidate);
            else throw new Error('ICE candidate limit exceeded');
            break;
          case 'stream_stats': senderMetrics.current = p; setSender(p); break;
          case 'stream_status': setStatus(String(p.state)); break;
          case 'stream_stop': pc?.close(); element.srcObject = null; setStatus(typeof p.reason === 'string' ? p.reason : 'Screen sharing stopped on phone'); setIce('closed'); break;
        }
      }).catch(fail).finally(() => { queued--; });
    });
    const request = () => { void window.pairspan.streamConfig().then(iceServers => { if (!closed) return signal('stream_request', { iceServers }); }).catch(fail); };
    controller.current = { request, restart: () => { restarts = 0; reconnectAt = performance.now(); void offer(true).catch(fail); } };
    const onFrame = (_now: number, metadata: VideoFrameCallbackMetadata) => {
      const captureTime = (metadata as VideoFrameCallbackMetadata & { captureTime?: number }).captureTime;
      if (typeof captureTime === 'number' && Number.isFinite(captureTime)) {
        const elapsed = metadata.expectedDisplayTime - captureTime;
        if (elapsed >= 0 && elapsed < 60000) { captureToDisplay.push(elapsed); if (captureToDisplay.length > 1800) captureToDisplay.shift(); }
      }
      rendered++; lastFrame = performance.now();
      if (freezeStart !== null) { lastFreezeMs = lastFrame - freezeStart; setMetrics(m => ({ ...m, lastFreezeMs })); freezeStart = null; }
      if (!closed) callbackId = element.requestVideoFrameCallback(onFrame);
    };
    callbackId = element.requestVideoFrameCallback(onFrame);
    const timer = setInterval(async () => {
      if (!pc || closed || pc.connectionState === 'closed' || statsBusy) return;
      statsBusy = true;
      try {
        const report = await pc.getStats();
        if (closed) return;
        const now = performance.now(), seconds = Math.max(.001, (now - previousTime) / 1000);
        const current: Metrics = { renderedFps: (rendered - previousRender) / seconds, ice: pc.iceConnectionState };
        if (rendered > 0 && element.srcObject && !element.paused && now - lastFrame > 500 && freezeStart === null) { freezeStart = lastFrame; freezes++; }
        current.freezes = freezes; current.recoveryMs = recoveryMs; current.lastFreezeMs = lastFreezeMs;
        const ordered = [...captureToDisplay].sort((a, b) => a - b);
        for (const [label, percentile] of [['p50', .5], ['p95', .95], ['p99', .99]] as const) current[`estimatedVideoLatency_${label}`] = ordered.length ? ordered[Math.ceil(ordered.length * percentile) - 1] : null;
        report.forEach(stat => {
          if (stat.type === 'inbound-rtp' && stat.kind === 'video') {
            current.codec = report.get(stat.codecId)?.mimeType ?? null;
            current.jitterMs = stat.jitter == null ? null : stat.jitter * 1000;
            current.droppedFrames = stat.framesDropped ?? null;
            current.decoder = stat.decoderImplementation ?? null;
            current.powerEfficientDecoder = stat.powerEfficientDecoder == null ? null : String(stat.powerEfficientDecoder);
            if (previous && previous.id === stat.id) {
              const frames = stat.framesDecoded - previous.framesDecoded;
              current.decodedFps = frames / seconds;
              current.bitrate = (stat.bytesReceived - previous.bytesReceived) * 8 / seconds;
              current.decoderMs = frames > 0 ? (stat.totalDecodeTime - previous.totalDecodeTime) * 1000 / frames : null;
              const emitted = stat.jitterBufferEmittedCount - previous.jitterBufferEmittedCount;
              current.jitterBufferMs = emitted > 0 ? (stat.jitterBufferDelay - previous.jitterBufferDelay) * 1000 / emitted : null;
              const lost = Math.max(0, stat.packetsLost - previous.packetsLost), received = stat.packetsReceived - previous.packetsReceived;
              current.packetLoss = lost + received > 0 ? lost * 100 / (lost + received) : null;
            }
            previous = stat;
          }
          if (stat.type === 'transport' && stat.selectedCandidatePairId) {
            const pair = report.get(stat.selectedCandidatePairId);
            current.rttMs = pair?.currentRoundTripTime == null ? null : pair.currentRoundTripTime * 1000;
            const candidate = pair && report.get(pair.localCandidateId);
            current.transport = candidate?.protocol ?? null;
            current.candidateType = candidate?.candidateType ?? null;
            if (candidate?.protocol && candidate.protocol !== 'udp') { pc?.close(); fail('Non-UDP media path rejected'); }
          }
        });
        previousTime = now; previousRender = rendered;
        setMetrics(m => ({ ...m, ...current }));
        samples.push({ at: new Date().toISOString(), ...senderMetrics.current, ...current });
        if (samples.length > 1800) samples.shift();
      } catch (e) { fail(e); } finally { statsBusy = false; }
    }, 1000);
    const exportStats = () => {
      const blob = new Blob([JSON.stringify({ profile: '720p/30', latencyMethod: captureToDisplay.length ? 'Browser captureTime to expectedDisplayTime estimate; depends on sender timestamp and clock estimation' : 'Capture-to-render unavailable', note: 'RTT is not capture-to-render latency. Samples are bounded to the last 30 minutes; latency percentiles cover the last 1800 timestamped frames.', samples }, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob), a = document.createElement('a'); a.href = url; a.download = 'pairspan-stream-stats.json'; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
    };
    element.addEventListener('export-stats', exportStats);
    return () => {
      closed = true; controller.current = null; unsubscribe();
      clearInterval(timer); clearTimeout(restartTimer); clearTimeout(negotiationTimer);
      element.cancelVideoFrameCallback(callbackId); element.removeEventListener('export-stats', exportStats);
      void signal('stream_stop').catch(() => {});
      pc?.close(); element.srcObject = null; pendingIce = []; samples = [];
    };
  }, [deviceId]);

  useEffect(() => { if (ready) controller.current?.request(); else setStatus('Signaling disconnected; waiting to reconnect…'); }, [ready]);
  const command = async (text: string) => {
    if (inputBusy.current || !adbReady) return;
    inputBusy.current = true; setControlError('');
    try { await window.pairspan.runCommand(deviceId, text); } catch (e) { setControlError(String(e)); } finally { inputBusy.current = false; }
  };
  const point = (event: React.PointerEvent<HTMLVideoElement>) => {
    const el = video.current!, rect = el.getBoundingClientRect();
    if (!el.videoWidth || !el.videoHeight || !sender.screenWidth || !sender.screenHeight) return null;
    const scale = Math.min(rect.width / el.videoWidth, rect.height / el.videoHeight);
    const w = el.videoWidth * scale, h = el.videoHeight * scale;
    const x = event.clientX - rect.left - (rect.width - w) / 2, y = event.clientY - rect.top - (rect.height - h) / 2;
    if (x < 0 || y < 0 || x >= w || y >= h) return null;
    return { x: Math.floor(x / w * Number(sender.screenWidth)), y: Math.floor(y / h * Number(sender.screenHeight)), at: performance.now() };
  };
  return <section className="card stream">
    <div className="device-head"><strong>Screen · 720p / 30 FPS</strong><button onClick={onClose}>Stop</button></div>
    <p role="status">{status}</p>
    <video ref={video} autoPlay playsInline muted style={{ width: '100%', maxHeight: '65vh', objectFit: 'contain', background: '#111', touchAction: 'none' }}
      onPointerDown={e => { if (!adbReady) return; gesture.current = point(e); e.currentTarget.setPointerCapture(e.pointerId); }}
      onPointerCancel={() => { gesture.current = null; }}
      onPointerUp={e => { const start = gesture.current, end = point(e); gesture.current = null; if (!start || !end) return;
        if (Math.hypot(end.x - start.x, end.y - start.y) < 12 && end.at - start.at < 400) void command(`input tap ${end.x} ${end.y}`);
        else void command(`input swipe ${start.x} ${start.y} ${end.x} ${end.y} ${Math.round(Math.min(1500, Math.max(100, end.at - start.at)))}`);
      }} />
    <div className="stream-controls">
      <button disabled={!adbReady} onClick={() => void command('input keyevent 4')}>Back</button>
      <button disabled={!adbReady} onClick={() => void command('input keyevent 3')}>Home</button>
      <button disabled={!adbReady} onClick={() => void command('input keyevent 187')}>Apps</button>
      <button disabled={!ready} onClick={() => controller.current?.restart()}>Retry ICE</button>
      <button onClick={() => video.current?.dispatchEvent(new Event('export-stats'))}>Export stats</button>
    </div>
    {controlError && <p role="alert">{controlError}</p>}
    <details><summary>Stream diagnostics · ICE {ice}</summary>
      <dl className="stream-metrics">{Object.entries({
        'Capture FPS': value(sender.captureFps), 'Encoded FPS': value(sender.encodedFps), 'Rendered FPS': value(metrics.renderedFps),
        'Received bitrate': value(typeof metrics.bitrate === 'number' ? metrics.bitrate / 1e6 : null, ' Mbps'),
        'RTT (network)': value(metrics.rttMs, ' ms'), 'Packet loss': value(metrics.packetLoss, '%'), 'Jitter': value(metrics.jitterMs, ' ms'),
        'Jitter buffer': value(metrics.jitterBufferMs, ' ms'), 'Dropped frames (receiver)': value(metrics.droppedFrames),
        'Capture frames skipped': value(sender.captureDrops), 'Encoder time/frame': value(sender.encoderMs, ' ms'),
        'Decoder time/frame': value(metrics.decoderMs, ' ms'),
        'Estimated video latency P50': value(metrics.estimatedVideoLatency_p50, ' ms'),
        'Estimated video latency P95': value(metrics.estimatedVideoLatency_p95, ' ms'),
        'Estimated video latency P99': value(metrics.estimatedVideoLatency_p99, ' ms'),
        'Render gaps >500 ms': value(metrics.freezes), 'Last recovery': value(metrics.recoveryMs, ' ms'),
        'Encoder': value(sender.encoder), 'Decoder': value(metrics.decoder), 'Efficient decoder': value(metrics.powerEfficientDecoder),
        'Codec': value(metrics.codec), 'Transport': value(metrics.transport), 'Candidate': value(metrics.candidateType)
      }).map(([key, val]) => <div key={key}><dt>{key}</dt><dd>{val}</dd></div>)}</dl>
      <p>Render gaps can also occur on a static screen. RTT is not end-to-end video latency. Video latency is estimated only when browser capture timestamps exist; otherwise it is unavailable.</p>
    </details>
  </section>;
}
