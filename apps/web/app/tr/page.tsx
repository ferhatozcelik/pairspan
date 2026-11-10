import type { Metadata } from 'next';
import Landing from '../Landing';

export const metadata: Metadata = {
  title: 'Pairspan — Android telefonun, bir satır uzağında',
  description:
    'Android telefonunu bilgisayarınla bir kez eşleştir, sonra terminalden, scriptlerden ya da MCP ile yapay zekâ ajanlarından yönet. Wi-Fi veya mobil veri üzerinden adb, uçtan uca şifreli.',
};

export default function Home() {
  return <Landing lang="tr" />;
}
