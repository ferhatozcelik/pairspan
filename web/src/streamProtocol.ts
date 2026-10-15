/** Only control-plane data is allowed; video is RTP between peers. */
export function validStreamEvent(event: string, p: Record<string, unknown>, role: 'mac' | 'phone'): boolean {
  if (JSON.stringify(p).length > 100_000 || typeof p.streamId !== 'string' || !/^[0-9a-f-]{36}$/i.test(p.streamId)) return false;
  switch (event) {
    case 'stream_request': return role === 'mac';
    case 'stream_ready': case 'stream_stats': return role === 'phone' && JSON.stringify(p).length < 8192;
    case 'stream_status': return role === 'phone' && typeof p.state === 'string' && p.state.length < 256;
    case 'stream_stop': return true;
    case 'stream_offer': return role === 'mac' && typeof p.sdp === 'string' && p.sdp.length < 90_000;
    case 'stream_answer': return role === 'phone' && typeof p.sdp === 'string' && p.sdp.length < 90_000;
    case 'stream_ice': {
      const c = p.candidate as Record<string, unknown> | undefined;
      return !!c && typeof c.candidate === 'string' && c.candidate.length < 4096
        && /\sudp\s/i.test(c.candidate) && typeof c.sdpMid === 'string' && c.sdpMid.length < 64
        && Number.isInteger(c.sdpMLineIndex) && Number(c.sdpMLineIndex) >= 0;
    }
    default: return false;
  }
}
