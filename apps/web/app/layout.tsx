import type { Metadata, Viewport } from 'next';
import { BASE, SITE } from './site';
import './globals.css';

export const metadata: Metadata = {
  metadataBase: new URL(`${SITE}/`),
  title: 'Pairspan — your Android phone, one line away',
  description:
    'Connect an Android phone to your Mac once over USB, then use adb, scripts and AI agents on it from anywhere. Direct, end-to-end encrypted, free and open source.',
  icons: { icon: [{ url: `${BASE}/icon.svg`, type: 'image/svg+xml' }, { url: `${BASE}/icon.png` }], apple: `${BASE}/apple-touch-icon.png` },
  openGraph: {
    type: 'website',
    url: `${SITE}/`,
    title: 'Pairspan — your Android phone, one line away',
    description: 'adb, CLI and MCP for real Android phones. Connect USB once; direct and end-to-end encrypted.',
    images: [{ url: `${BASE}/og-image.png`, width: 1200, height: 630 }],
  },
  twitter: { card: 'summary_large_image' },
};

export const viewport: Viewport = { themeColor: '#faf4ec', width: 'device-width', initialScale: 1 };

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
