import type { Metadata, Viewport } from 'next';
import './globals.css';

const site = 'https://pairspan.ferhatozcelik.com';

export const metadata: Metadata = {
  metadataBase: new URL(site),
  title: 'Pairspan — your Android phone, one line away',
  description:
    'Pair an Android phone with your computer once, then drive it from the terminal, scripts or AI agents over MCP. adb over Wi-Fi or mobile data, end-to-end encrypted. Free and open source.',
  icons: { icon: [{ url: '/icon.svg', type: 'image/svg+xml' }, { url: '/icon.png' }], apple: '/apple-touch-icon.png' },
  openGraph: {
    type: 'website',
    url: site,
    title: 'Pairspan — your Android phone, one line away',
    description: 'adb, CLI and MCP for real Android phones. Pair once with a QR code; end-to-end encrypted.',
    images: [{ url: '/og-image.png', width: 1200, height: 630 }],
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
