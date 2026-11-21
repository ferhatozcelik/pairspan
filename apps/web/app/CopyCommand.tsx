'use client';

import { useState } from 'react';

export default function CopyCommand({ command, label, done }: { command: string; label: string; done: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    await navigator.clipboard.writeText(command);
    setCopied(true);
    setTimeout(() => setCopied(false), 1600);
  };
  return (
    <div className="term install">
      <div className="dots"><b /><b /><b /></div>
      <div className="cmd">
        <code><span className="p">$</span> {command}</code>
        <button type="button" className="btn sm" onClick={copy} aria-live="polite">{copied ? done : label}</button>
      </div>
    </div>
  );
}
