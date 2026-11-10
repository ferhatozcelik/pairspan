import type { Metadata } from 'next';
import Link from 'next/link';

export const metadata: Metadata = {
  title: 'Pairspan docs — install, pair, CLI, MCP, self-hosting',
  description: 'Install Pairspan, pair an Android phone, use the pairspan CLI and MCP server, and run your own gateway.',
};

const sections = [
  ['install', 'Install'],
  ['pair', 'Pair a phone'],
  ['cli', 'CLI'],
  ['mcp', 'MCP'],
  ['adb', 'adb & Android Studio'],
  ['security', 'Security'],
  ['self-host', 'Self-hosting'],
  ['troubleshooting', 'Troubleshooting'],
] as const;

const commands: [string, string][] = [
  ['pairspan status', 'App, gateway and phone status'],
  ['pairspan devices', 'Paired phones (● online)'],
  ['pairspan pair', 'Show a pairing QR code in the terminal and wait for the phone'],
  ['pairspan unpair <phone>', 'Remove a phone (it is told and the link is cut)'],
  ['pairspan shell <cmd…>', 'Run a shell command on the phone; exits with its status'],
  ['pairspan adb -- <args…>', 'Run any adb command against the phone'],
  ['pairspan tap <x> <y>', 'Tap at coordinates'],
  ['pairspan click <text> [--by text|desc|id]', 'Wait for an element and tap it'],
  ['pairspan swipe <x1> <y1> <x2> <y2> [--ms 300]', 'Swipe'],
  ['pairspan text "<text>"', 'Type into the focused field (ASCII)'],
  ['pairspan key <home|back|enter|…>', 'Press a key (or any KEYCODE_ name)'],
  ['pairspan elements [query]', 'On-screen elements with tap centers'],
  ['pairspan dump', 'Raw UI hierarchy XML'],
  ['pairspan screenshot [file.png]', 'Save a screenshot'],
  ['pairspan launch|stop <package>', 'Start or force-stop an app'],
  ['pairspan install <app.apk>', 'Install an APK'],
  ['pairspan apps · current · logcat', 'Installed apps, foreground activity, recent logs'],
  ['pairspan usb [prepare <serial>]', 'USB phones; enable wireless ADB on one'],
  ['pairspan mcp [--read-only]', 'Run the MCP server on stdio'],
];

export default function Docs() {
  return (
    <>
      <header className="nav">
        <div className="wrap">
          <Link className="brand" href="/">
            <img src="/icon.svg" alt="" width={36} height={36} />
            <span>Pairspan</span>
          </Link>
          <nav className="nav-links">
            <a href="https://github.com/ferhatozcelik/pairspan">GitHub</a>
            <a className="btn sm dark" href="https://github.com/ferhatozcelik/pairspan/releases/latest">Download</a>
          </nav>
        </div>
      </header>
      <div className="wrap docs">
        <nav>
          {sections.map(([id, label]) => (
            <a key={id} href={`#${id}`}>{label}</a>
          ))}
        </nav>
        <article>
          <div className="kicker">Docs · v1.0.0</div>
          <h1>Pairspan</h1>
          <p>
            Pairspan connects Android phones to your computer through an end-to-end encrypted relay, then exposes each
            phone as an adb device, a <code>pairspan</code> command and an MCP server for AI agents.
          </p>

          <h2 id="install">Install</h2>
          <h3>macOS</h3>
          <pre>curl -fsSL https://pairspan.ferhatozcelik.com/install.sh | sh</pre>
          <p>
            This installs <code>Pairspan.app</code> into <code>/Applications</code> and links the bundled{' '}
            <code>pairspan</code> command into <code>/usr/local/bin</code>. You can also download the DMG from{' '}
            <a href="https://github.com/ferhatozcelik/pairspan/releases/latest">GitHub Releases</a> and run{' '}
            <code>install.sh --link-only</code> afterwards. The app bundles <code>adb</code>; if you already have
            Android platform-tools, yours is used.
          </p>
          <h3>Android</h3>
          <p>
            Install <code>pairspan-android-1.0.0.apk</code> from the release page (Android 8 or later), or let the desktop
            app install it over USB from <b>First-time USB setup → Install app</b>.
          </p>

          <h2 id="pair">Pair a phone</h2>
          <ol>
            <li>On the computer, click <b>Pair a phone</b> in the app (or run <code>pairspan pair</code>).</li>
            <li>On the phone, open Pairspan and tap <b>Scan QR code</b> — or <b>Enter code</b> and type it.</li>
            <li>
              Connect the phone with USB once and click <b>Enable wireless ADB</b> (or run{' '}
              <code>pairspan usb prepare &lt;serial&gt;</code>). This runs <code>adb tcpip 5555</code>, which Android
              forgets after a reboot.
            </li>
          </ol>
          <p className="note">
            The phone and the computer do not need to be on the same network. Both connect out to the gateway.
          </p>

          <h2 id="cli">CLI</h2>
          <p>
            <code>pairspan</code> talks to the running app over a local socket that only your user can open. If the app
            is not running, the CLI starts it in the background. Pick a phone with <code>-d &lt;name|id|serial&gt;</code>;
            with one phone online you can leave it out. Add <code>--json</code> for machine-readable output.
          </p>
          <table>
            <thead>
              <tr><th>Command</th><th>What it does</th></tr>
            </thead>
            <tbody>
              {commands.map(([c, d]) => (
                <tr key={c}><td><code>{c}</code></td><td>{d}</td></tr>
              ))}
            </tbody>
          </table>

          <h2 id="mcp">MCP</h2>
          <p>Claude Code:</p>
          <pre>claude mcp add pairspan -- pairspan mcp</pre>
          <p>Claude Desktop, Cursor, VS Code and other clients:</p>
          <pre>{`{
  "mcpServers": {
    "pairspan": { "command": "pairspan", "args": ["mcp"] }
  }
}`}</pre>
          <p>
            Tools: <code>list_devices</code>, <code>screenshot</code>, <code>ui_elements</code>, <code>tap_element</code>,{' '}
            <code>tap</code>, <code>swipe</code>, <code>type_text</code>, <code>press_key</code>, <code>launch_app</code>,{' '}
            <code>stop_app</code>, <code>list_apps</code>, <code>current_app</code>, <code>screen_size</code>,{' '}
            <code>logcat</code>, <code>shell</code>, <code>install_apk</code>. With <code>--read-only</code> only the
            tools that look at the phone are offered. You can switch CLI and MCP access off entirely in the app
            (<b>Allow CLI and MCP</b>).
          </p>

          <h2 id="adb">adb &amp; Android Studio</h2>
          <p>
            Each paired phone gets a local port (from 47100 up). While the phone is online the app runs{' '}
            <code>adb connect 127.0.0.1:&lt;port&gt;</code>, so it appears in <code>adb devices</code>, Android Studio and
            any tool built on adb.
          </p>

          <h2 id="security">Security</h2>
          <ul>
            <li>Every installation has its own P-256 key. The gateway checks a signature before it routes anything.</li>
            <li>
              Pairing uses a 20-character code (~99 bits). The gateway only sees its SHA-256 hash, and the code is mixed
              into the first handshake, so a gateway that swaps keys cannot finish pairing.
            </li>
            <li>
              After a handshake (ephemeral + static ECDH, HKDF-SHA256), traffic is AES-256-GCM with per-direction keys and
              replay protection. The gateway forwards ciphertext.
            </li>
            <li>Removing a device on either side cuts the link at once.</li>
          </ul>
          <p>
            Full specification: <a href="https://github.com/ferhatozcelik/pairspan/blob/main/docs/protocol.md">docs/protocol.md</a>.
          </p>

          <h2 id="self-host">Self-hosting</h2>
          <p>
            The gateway and this website are one 5 MB container. It keeps no pairing state, so restarts are safe.
          </p>
          <pre>{`git clone https://github.com/ferhatozcelik/pairspan && cd pairspan
echo "GATEWAY_TOKEN=$(openssl rand -hex 24)" > temp.env
docker build -t pairspan:latest .
docker compose up -d     # listens on 127.0.0.1:16200 — put TLS in front (Caddy, nginx)`}</pre>
          <p>
            Then set the gateway to <code>wss://your-host/ws</code> in the desktop app (Settings) and the Android app
            (Settings → Gateway). Builds made with <code>PAIRSPAN_GATEWAY_TOKEN</code> / <code>GATEWAY_TOKEN</code> send
            the token automatically.
          </p>
          <table>
            <thead><tr><th>Variable</th><th>Meaning</th></tr></thead>
            <tbody>
              <tr><td><code>GATEWAY_TOKEN</code></td><td>Shared secret clients must present (optional)</td></tr>
              <tr><td><code>TRUST_PROXY=1</code></td><td>Read client IPs from <code>X-Forwarded-For</code></td></tr>
              <tr><td><code>MAX_CONNECTIONS</code></td><td>Default 10000</td></tr>
              <tr><td><code>PORT</code></td><td>Default 3000 in the container</td></tr>
            </tbody>
          </table>
          <p><code>/health</code> returns JSON; <code>/metrics</code> is Prometheus text.</p>

          <h2 id="troubleshooting">Troubleshooting</h2>
          <h3>The phone is online but adb says offline / unauthorized</h3>
          <p>
            Wireless ADB is not enabled (the phone rebooted) or the phone has not trusted this computer yet. Connect USB,
            accept the prompt on the phone and click <b>Enable wireless ADB</b>.
          </p>
          <h3>“Several phones are online”</h3>
          <p>Choose one with <code>-d</code>: a name, the start of its id, or its serial.</p>
          <h3>The connection drops in the background</h3>
          <p>On the phone, allow notifications and unrestricted battery in Pairspan’s Setup list.</p>
        </article>
      </div>
    </>
  );
}
