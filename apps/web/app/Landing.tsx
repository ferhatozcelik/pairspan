import CopyCommand from './CopyCommand';
import { BASE, GITHUB, INSTALL, RELEASES } from './site';

type Lang = 'en' | 'tr';

const copy = {
  en: {
    nav: { how: 'How it works', agents: 'CLI & MCP', security: 'Security' },
    sticker: 'Free · Open source · MIT',
    h1a: 'Your Android phone,',
    h1b: 'one line',
    h1c: 'away.',
    lead: 'Plug the phone in once. Then drive it from your terminal, scripts or AI agents — over Wi-Fi or mobile data, end-to-end encrypted.',
    download: 'Download for macOS',
    android: 'Android app (APK)',
    navDownload: 'Download',
    installLabel: 'Install on your Mac',
    copy: 'Copy',
    copied: 'Copied',
    strip: ['adb over any network', 'end-to-end encrypted', 'pairspan CLI', 'MCP for AI agents', 'macOS · Android', 'no account, no server to run'],
    howKicker: 'How it works',
    howTitle: 'Three steps. Then forget the cable.',
    steps: [
      ['01', 'Install Pairspan', 'Paste the command above in Terminal. The Mac app lives in the menu bar and brings the pairspan command with it.'],
      ['02', 'Plug the phone in once', 'Connect it over USB. Pairspan installs the Android app, pairs both sides and turns on wireless ADB — no codes to scan.'],
      ['03', 'Use it from anywhere', 'Unplug. adb, Android Studio, scripts and AI agents reach the phone directly and encrypted — same Wi-Fi or not.'],
    ],
    agentsKicker: 'CLI & MCP',
    agentsTitle: 'Made for scripts and AI agents.',
    agents: [
      ['CLI', 'pairspan CLI', 'Tap, type, swipe, read the screen, install apps — or pass anything straight to adb.'],
      ['MCP', 'MCP for Claude, Cursor and VS Code', 'Agents see the UI as a list of elements and act on it, or only look with read-only mode.'],
      ['adb', 'Plain adb still works', 'Every paired phone shows up as an adb device, so Android Studio and your tools just see it.'],
    ],
    secKicker: 'Security',
    secTitle: 'Nobody in between can read it.',
    sec: [
      ['Direct and end-to-end encrypted', 'Phone and Mac talk peer to peer. P-256 key agreement and AES-256-GCM on top, so only the two devices hold the keys.'],
      ['Keys stay on your devices', 'Pairing happens over the USB cable. Nothing to scan, nothing to type, no account.'],
      ['One switch', 'Turn Pairspan off on either side and the link is down until you turn it back on.'],
    ],
    faqTitle: 'Questions',
    faq: [
      ['Do I need the phone and computer on the same Wi-Fi?', 'No. The devices find each other through a lightweight signalling service and then connect directly, across networks and over mobile data. A few very strict networks block direct connections.'],
      ['Why connect USB once?', 'Android only lets apps reach adb after it has been enabled from a computer, and the cable is also how the two devices pair safely. After a phone reboot, plug in again.'],
      ['Does it cost anything?', 'No. Pairspan is MIT-licensed and free. Everything — code, docs and releases — is on GitHub.'],
      ['Which platforms?', 'macOS on the computer, Android 8 or later on the phone.'],
    ],
    footer: 'Made by Ferhat Özçelik',
  },
  tr: {
    nav: { how: 'Nasıl çalışır', agents: 'CLI & MCP', security: 'Güvenlik' },
    sticker: 'Ücretsiz · Açık kaynak · MIT',
    h1a: 'Android telefonun,',
    h1b: 'bir satır',
    h1c: 'uzağında.',
    lead: 'Telefonu bir kez USB ile tak. Sonra onu terminalden, scriptlerden ya da yapay zekâ ajanlarından yönet — Wi-Fi veya mobil veri üzerinden, uçtan uca şifreli.',
    download: 'macOS için indir',
    android: 'Android uygulaması (APK)',
    navDownload: 'İndir',
    installLabel: "Mac'ine kur",
    copy: 'Kopyala',
    copied: 'Kopyalandı',
    strip: ['her ağda adb', 'uçtan uca şifreli', 'pairspan CLI', 'yapay zekâ ajanları için MCP', 'macOS · Android', 'hesap yok, sunucu yok'],
    howKicker: 'Nasıl çalışır',
    howTitle: 'Üç adım. Sonra kabloyu unut.',
    steps: [
      ['01', "Pairspan'i kur", "Yukarıdaki komutu Terminal'e yapıştır. Mac uygulaması menü çubuğunda çalışır ve pairspan komutunu da getirir."],
      ['02', 'Telefonu bir kez tak', 'USB ile bağla. Pairspan Android uygulamasını kurar, iki tarafı eşleştirir ve kablosuz ADB’yi açar — taranacak kod yok.'],
      ['03', 'Her yerden kullan', "Kabloyu çıkar. adb, Android Studio, scriptler ve yapay zekâ ajanları telefona doğrudan ve şifreli ulaşır — aynı Wi-Fi'da olsun ya da olmasın."],
    ],
    agentsKicker: 'CLI & MCP',
    agentsTitle: 'Scriptler ve yapay zekâ ajanları için.',
    agents: [
      ['CLI', 'pairspan CLI', "Dokun, yaz, kaydır, ekranı oku, uygulama kur — ya da her şeyi doğrudan adb'ye ilet."],
      ['MCP', 'Claude, Cursor ve VS Code için MCP', 'Ajanlar arayüzü bir öğe listesi olarak görür ve onun üzerinden işlem yapar; istersen yalnızca bakarlar.'],
      ['adb', 'Düz adb de çalışır', 'Eşleşen her telefon bir adb cihazı olarak görünür; Android Studio ve araçların onu doğrudan görür.'],
    ],
    secKicker: 'Güvenlik',
    secTitle: 'Aradaki kimse okuyamaz.',
    sec: [
      ['Doğrudan ve uçtan uca şifreli', 'Telefon ile Mac doğrudan konuşur. Üstünde P-256 anahtar anlaşması ve AES-256-GCM; anahtarlar yalnızca iki cihazda.'],
      ['Anahtarlar cihazlarında kalır', 'Eşleştirme USB kablosu üzerinden olur. Taranacak, yazılacak bir şey yok; hesap yok.'],
      ['Tek anahtar', 'Pairspan’i iki taraftan birinde kapat, sen tekrar açana kadar bağlantı kapalı kalır.'],
    ],
    faqTitle: 'Sorular',
    faq: [
      ["Telefon ve bilgisayar aynı Wi-Fi'da mı olmalı?", 'Hayır. Cihazlar hafif bir sinyal servisi üzerinden birbirini bulur, sonra doğrudan bağlanır; farklı ağlarda ve mobil veride çalışır. Çok kısıtlı bazı ağlar doğrudan bağlantıyı engelleyebilir.'],
      ['Neden bir kez USB?', "Android, adb'nin bir bilgisayardan açılmasını şart koşar; kablo aynı zamanda iki cihazın güvenle eşleşmesini sağlar. Telefon yeniden başlayınca tekrar tak."],
      ['Ücretli mi?', "Hayır. Pairspan MIT lisanslı ve ücretsiz. Kod, dokümanlar ve sürümler GitHub'da."],
      ['Hangi platformlar?', 'Bilgisayarda macOS, telefonda Android 8 ve üstü.'],
    ],
    footer: 'Ferhat Özçelik tarafından',
  },
} as const;

export default function Landing({ lang }: { lang: Lang }) {
  const t = copy[lang];
  return (
    <>
      <header className="nav">
        <div className="wrap">
          <a className="brand" href={lang === 'tr' ? `${BASE}/tr/` : `${BASE}/`}>
            <img src={`${BASE}/icon.svg`} alt="" width={36} height={36} />
            <span>Pairspan</span>
          </a>
          <nav className="nav-links">
            <a className="hide-sm" href="#how">{t.nav.how}</a>
            <a className="hide-sm" href="#agents">{t.nav.agents}</a>
            <a className="hide-sm" href="#security">{t.nav.security}</a>
            <a href={GITHUB}>GitHub</a>
            <span className="lang">
              <a href={`${BASE}/`} className={lang === 'en' ? 'on' : ''}>EN</a>
              <a href={`${BASE}/tr/`} className={lang === 'tr' ? 'on' : ''}>TR</a>
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
              <a className="btn ghost" href={RELEASES}>{t.android}</a>
            </div>
          </div>
          <div>
            <div className="kicker">{t.installLabel}</div>
            <CopyCommand command={INSTALL} label={t.copy} done={t.copied} />
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
              </div>
            ))}
          </div>
        </section>

        <section id="agents" className="wrap">
          <div className="kicker">{t.agentsKicker}</div>
          <h2>{t.agentsTitle}</h2>
          <div className="bento">
            {t.agents.map(([pill, title, body], i) => (
              <div key={pill} className={`card${i === 0 ? ' dark' : i === 1 ? ' ember' : ''}`}>
                <span className="pill">{pill}</span>
                <h3>{title}</h3>
                <p>{body}</p>
              </div>
            ))}
          </div>
        </section>

        <section id="security" className="wrap">
          <div className="kicker">{t.secKicker}</div>
          <h2>{t.secTitle}</h2>
          <div className="bento">
            {t.sec.map(([title, body], i) => (
              <div key={title} className={`card${i === 0 ? ' dark' : ''}`}>
                <h3>{title}</h3>
                <p>{body}</p>
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
          <a href={RELEASES}>Releases</a>
        </div>
      </footer>
    </>
  );
}
