'use client';

import { useEffect, useState } from 'react';

/** https://…/p/?g=…&t=…&k=… opens the Android app with the same pairing data (for links shared as text). */
export default function OpenPairing() {
  const [href, setHref] = useState('');
  useEffect(() => {
    const q = new URLSearchParams(window.location.search);
    const t = (q.get('t') || '').toUpperCase().replace(/[^A-Z0-9]/g, '');
    if (!/^[2-9A-HJKMNP-Z]{20}$/.test(t)) return;
    const params = new URLSearchParams({ t });
    const g = q.get('g');
    const k = q.get('k');
    if (g && /^wss?:\/\//.test(g)) params.set('g', g);
    if (k && /^[0-9a-f]{32}$/.test(k)) params.set('k', k);
    const link = `pairspan://pair?${params}`;
    setHref(link);
    window.location.href = link;
  }, []);
  return (
    <main className="wrap" style={{ padding: '80px 24px', maxWidth: 560 }}>
      <h1 style={{ fontSize: '2.6rem' }}>Open Pairspan</h1>
      {href ? (
        <p>
          Opening the Android app… <a className="btn" href={href}>Open Pairspan</a>
        </p>
      ) : (
        <p className="sub">This pairing link is incomplete. Show a new code in the Pairspan app on your computer.</p>
      )}
    </main>
  );
}
