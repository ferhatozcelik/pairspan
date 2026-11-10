import Link from 'next/link';

type Lang = 'en' | 'tr';

const GITHUB = 'https://github.com/ferhatozcelik/pairspan';
const RELEASES = `${GITHUB}/releases/latest`;
const INSTALL = 'curl -fsSL https://pairspan.ferhatozcelik.com/install.sh | sh';

const copy = {
  en: {
    nav: { how: 'How it works', agents: 'CLI & MCP', security: 'Security', docs: 'Docs' },
    sticker: 'Free · Open source · MIT',
    h1a: 'Your Android phone,',
    h1b: 'one line',
    h1c: 'away.',
    lead: 'Pair once with a QR code. Then drive the phone from your terminal, scripts or AI agents — over Wi-Fi or mobile data, end-to-end encrypted.',
    download: 'Download for macOS',
    navDownload: 'Download',
    github: 'View on GitHub',
    orInstall: 'or install from the terminal:',
    strip: ['adb over any network', 'end-to-end encrypted', 'pairspan CLI', 'MCP for AI agents', 'macOS · Linux · Android', 'self-host in one container'],
    howKicker: 'How it works',
    howTitle: 'Three steps. Then forget the cable.',
    steps: [
      ['01', 'Install Pairspan', 'The Mac app lives in the menu bar and brings the pairspan command with it. Install the Android app from the same release.'],
      ['02', 'Pair with a QR code', 'Click Pair a phone, scan the code with the phone. Connect USB once so wireless ADB is ready.'],
      ['03', 'Use it from anywhere', 'adb, Android Studio, scripts and AI agents reach the phone through an encrypted tunnel — same Wi-Fi or not.'],
    ],
    agentsKicker: 'CLI & MCP',
    agentsTitle: 'Made for scripts and AI agents.',
    agentsSub: 'The pairspan command and its MCP server talk only to the app on your Mac, so keys and pairings never leave it.',
    cliTitle: 'pairspan CLI',
    cliBody: 'Tap, type, swipe, read the screen, install apps — or pass anything straight to adb.',
    mcpTitle: 'MCP for Claude, Cursor and VS Code',
    mcpBody: 'Agents see the UI as a list of elements and act on it. Add --read-only to only let them look.',
    adbTitle: 'Plain adb still works',
    adbBody: 'Every paired phone shows up as an adb device, so Android Studio and your existing tools just see it.',
    secKicker: 'Security',
    secTitle: 'The gateway only ever sees noise.',
    sec: [
      ['End-to-end encrypted', 'P-256 key agreement and AES-256-GCM between your Mac and your phone. The relay forwards ciphertext.'],
      ['Keys stay on your devices', 'Each device has its own key. The pairing code never reaches the server; it only sees a hash.'],
      ['Unpair means unpaired', 'Remove a device on either side and the link is cut instantly — no lingering sessions.'],
      ['Run your own gateway', 'One 5 MB container. Point the apps at it and nothing else is involved.'],
    ],
    faqTitle: 'Questions',
    faq: [
      ['Do I need the phone and computer on the same Wi-Fi?', 'No. Both connect out to the gateway, so it works across networks and over mobile data.'],
      ['Why connect USB once?', 'Android only lets apps reach adb after it has been enabled on port 5555 from a computer. The desktop app does it in one click. After a phone reboot, do it again.'],
      ['Does it cost anything?', 'No. Pairspan is MIT-licensed. The public gateway is free; you can also host your own.'],
      ['Which platforms?', 'macOS today, Linux CLI and app builds, and Android 8 or later on the phone.'],
    ],
    footer: 'Made by Ferhat Özçelik',
  },
  tr: {
    nav: { how: 'Nasıl çalışır', agents: 'CLI & MCP', security: 'Güvenlik', docs: 'Dokümanlar' },
    sticker: 'Ücretsiz · Açık kaynak · MIT',
    h1a: 'Android telefonun,',
    h1b: 'bir satır',
    h1c: 'uzağında.',
    lead: 'Bir kez QR kodla eşleştir. Sonra telefonu terminalden, scriptlerden ya da yapay zekâ ajanlarından yönet — Wi-Fi veya mobil veri üzerinden, uçtan uca şifreli.',
    download: 'macOS için indir',
    navDownload: 'İndir',
    github: "GitHub'da gör",
    orInstall: 'ya da terminalden kur:',
    strip: ['her ağda adb', 'uçtan uca şifreli', 'pairspan CLI', 'yapay zekâ ajanları için MCP', 'macOS · Linux · Android', 'tek container ile kendi sunucun'],
    howKicker: 'Nasıl çalışır',
    howTitle: 'Üç adım. Sonra kabloyu unut.',
    steps: [
      ['01', "Pairspan'i kur", "Mac uygulaması menü çubuğunda çalışır ve pairspan komutunu da getirir. Android uygulamasını aynı sürümden kur."],
      ['02', 'QR kodla eşleştir', "Pair a phone'a tıkla, kodu telefonla tara. Kablosuz ADB için telefonu bir kez USB ile bağla."],
      ['03', 'Her yerden kullan', "adb, Android Studio, scriptler ve yapay zekâ ajanları telefona şifreli tünel üzerinden ulaşır — aynı Wi-Fi'da olsun ya da olmasın."],
    ],
    agentsKicker: 'CLI & MCP',
    agentsTitle: 'Scriptler ve yapay zekâ ajanları için.',
    agentsSub: "pairspan komutu ve MCP sunucusu yalnızca Mac'teki uygulamayla konuşur; anahtarlar ve eşleştirmeler oradan hiç çıkmaz.",
    cliTitle: 'pairspan CLI',
    cliBody: "Dokun, yaz, kaydır, ekranı oku, uygulama kur — ya da her şeyi doğrudan adb'ye ilet.",
    mcpTitle: 'Claude, Cursor ve VS Code için MCP',
    mcpBody: 'Ajanlar arayüzü bir öğe listesi olarak görür ve onun üzerinden işlem yapar. Yalnızca bakmalarına izin vermek için --read-only ekle.',
    adbTitle: 'Düz adb de çalışır',
    adbBody: "Eşleşen her telefon bir adb cihazı olarak görünür; Android Studio ve mevcut araçların onu doğrudan görür.",
    secKicker: 'Güvenlik',
    secTitle: 'Gateway yalnızca anlamsız veri görür.',
    sec: [
      ['Uçtan uca şifreli', "Mac ile telefon arasında P-256 anahtar anlaşması ve AES-256-GCM. Relay yalnızca şifreli veriyi iletir."],
      ['Anahtarlar cihazlarında kalır', 'Her cihazın kendi anahtarı var. Eşleştirme kodu sunucuya hiç gitmez; sunucu yalnızca hash görür.'],
      ['Kaldırınca gerçekten kopar', 'İki taraftan birinde cihazı kaldır, bağlantı anında kesilir.'],
      ['Kendi gateway’ini çalıştır', "Tek bir 5 MB'lık container. Uygulamaları ona yönlendir, başka hiçbir şey araya girmez."],
    ],
    faqTitle: 'Sorular',
    faq: [
      ["Telefon ve bilgisayar aynı Wi-Fi'da mı olmalı?", 'Hayır. İkisi de gateway’e bağlanır; farklı ağlarda ve mobil veride çalışır.'],
      ['Neden bir kez USB?', "Android, adb'nin 5555 portunda bir bilgisayardan açılmasını şart koşar. Masaüstü uygulaması bunu tek tıkla yapar. Telefon yeniden başlayınca tekrarla."],
      ['Ücretli mi?', 'Hayır. Pairspan MIT lisanslıdır. Genel gateway ücretsiz; istersen kendin de barındırabilirsin.'],
      ['Hangi platformlar?', "Bugün macOS, Linux için CLI ve uygulama, telefonda Android 8 ve üstü."],
    ],
    footer: 'Ferhat Özçelik tarafından',
  },
} as const;

export default function Landing({ lang }: { lang: Lang }) {
  const t = copy[lang];
  const base = lang === 'tr' ? '/tr/' : '/';
  return (
    <>
      <header className="nav">
        <div className="wrap">
          <Link className="brand" href={base}>
            <img src="/icon.svg" alt="" width={36} height={36} />
            <span>Pairspan</span>
          </Link>
          <nav className="nav-links">
            <a className="hide-sm" href="#how">{t.nav.how}</a>
            <a className="hide-sm" href="#agents">{t.nav.agents}</a>
            <a className="hide-sm" href="#security">{t.nav.security}</a>
            <Link href="/docs/">{t.nav.docs}</Link>
            <span className="lang">
              <a href="/" className={lang === 'en' ? 'on' : ''}>EN</a>
              <a href="/tr/" className={lang === 'tr' ? 'on' : ''}>TR</a>
            </span>
            <a className="btn sm dark hide-sm" href={RELEASES}>{t.navDownload}</a>
          </nav>
        </div>
      </header>

      <main>
        <div className="wrap hero">
          <div>
            <span className="sticker"><i />{t.sticker}</span>
            <h1>
              {t.h1a} <em>{t.h1b}</em> {t.h1c}
            </h1>
            <p className="lead">{t.lead}</p>
            <div className="cta">
              <a className="btn" href={RELEASES}>{t.download}</a>
              <a className="btn ghost" href={GITHUB}>{t.github}</a>
            </div>
            <p className="install-line">{t.orInstall} <code>{INSTALL}</code></p>
          </div>
          <div className="term" aria-label="Terminal example">
            <div className="dots"><b /><b /><b /></div>
            <pre>
              <span className="p">$</span> pairspan devices{'\n'}
              <span className="ok">●</span> Pixel 8      2f91c0aa  127.0.0.1:47100{'\n'}
              <span className="p">$</span> pairspan launch com.android.settings{'\n'}
              <span className="p">$</span> pairspan click &quot;Network &amp; internet&quot;{'\n'}
              <span className="c">Tapped &apos;Network &amp; internet&apos; at 540,812</span>{'\n'}
              <span className="p">$</span> adb -s 127.0.0.1:47100 logcat -d | tail -1{'\n'}
              <span className="c">I ActivityManager: Displayed …/.SubSettings</span>{'\n'}
              <span className="p">$</span> claude mcp add pairspan -- pairspan mcp{'\n'}
              <span className="ok">✓ Added MCP server pairspan</span>
            </pre>
          </div>
        </div>

        <div className="strip" aria-hidden="true">
          <div className="track">
            {[...t.strip, ...t.strip].map((s, i) => (
              <span key={i}><i />{s}</span>
            ))}
          </div>
        </div>

        <section id="how" className="wrap">
          <div className="kicker">{t.howKicker}</div>
          <h2>{t.howTitle}</h2>
          <div className="bento">
            {t.steps.map(([n, title, body], i) => (
              <div key={n} className={`card${i === 1 ? ' dark' : ''}`}>
                <span className="pill">{n}</span>
                <h3>{title}</h3>
                <p>{body}</p>
                {i === 0 && <div className="code">{INSTALL}</div>}
              </div>
            ))}
          </div>
        </section>

        <section id="agents" className="wrap">
          <div className="kicker">{t.agentsKicker}</div>
          <h2>{t.agentsTitle}</h2>
          <p className="sub">{t.agentsSub}</p>
          <div className="bento">
            <div className="card dark wide">
              <span className="pill">CLI</span>
              <h3>{t.cliTitle}</h3>
              <p>{t.cliBody}</p>
              <div className="code">{`pairspan elements "Log in"
pairspan click "Log in"
pairspan text "hello@example.com"
pairspan screenshot shot.png
pairspan adb -- install -r app-debug.apk`}</div>
            </div>
            <div className="card ember">
              <span className="pill">MCP</span>
              <h3>{t.mcpTitle}</h3>
              <p>{t.mcpBody}</p>
              <div className="code">claude mcp add pairspan -- pairspan mcp</div>
            </div>
            <div className="card wide">
              <span className="pill">adb</span>
              <h3>{t.adbTitle}</h3>
              <p>{t.adbBody}</p>
              <div className="code">{`$ adb devices
127.0.0.1:47100   device`}</div>
            </div>
            <div className="card">
              <span className="pill">E2E</span>
              <h3>P-256 · AES-256-GCM</h3>
              <p>{t.sec[0][1]}</p>
            </div>
          </div>
        </section>

        <section id="security" className="wrap">
          <div className="kicker">{t.secKicker}</div>
          <h2>{t.secTitle}</h2>
          <div className="bento">
            {t.sec.map(([title, body], i) => (
              <div key={title} className={`card${i === 0 ? ' dark wide' : i === 3 ? ' wide' : ''}`}>
                <h3>{title}</h3>
                <p>{body}</p>
                {i === 3 && <div className="code">docker compose up -d</div>}
              </div>
            ))}
          </div>
        </section>

        <section className="wrap">
          <h2>{t.faqTitle}</h2>
          {t.faq.map(([q, a]) => (
            <details key={q} className="faq">
              <summary>{q}</summary>
              <p>{a}</p>
            </details>
          ))}
        </section>
      </main>

      <footer>
        <div className="wrap">
          <span>© 2026 Pairspan · MIT · {t.footer}</span>
          <a href={GITHUB}>GitHub</a>
          <Link href="/docs/">Docs</Link>
        </div>
      </footer>
    </>
  );
}
