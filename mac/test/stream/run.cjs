const { app, BrowserWindow } = require('electron');
const { createServer } = require('node:http');
const { readFileSync, mkdirSync, writeFileSync } = require('node:fs');
const { join, resolve } = require('node:path');
const assert = require('node:assert/strict');
const root = resolve(__dirname, '../..');
const artifacts = join(root, 'node_modules/.cache/stream-smoke');
mkdirSync(artifacts, { recursive: true });
app.setPath('userData', join(artifacts, 'user-data'));
app.setPath('crashDumps', artifacts);
const wait = ms => new Promise(r => setTimeout(r, ms));
let server, win;
app.whenReady().then(async () => {
  try {
    server = createServer((req, res) => {
      const path = req.url === '/' ? 'index.html' : req.url.slice(1);
      if (path.includes('..')) { res.writeHead(404).end(); return; }
      try {
        let data = readFileSync(join(root, 'dist', path));
        if (path === 'index.html') data = Buffer.from(data.toString().replace('<head>', `<head><script>${readFileSync(join(__dirname, 'mock.js'), 'utf8')}</script>`));
        res.setHeader('content-type', path.endsWith('.js') ? 'text/javascript' : path.endsWith('.css') ? 'text/css' : path.endsWith('.png') ? 'image/png' : 'text/html'); res.end(data);
      } catch { res.writeHead(404).end(); }
    });
    await new Promise(r => server.listen(0, '127.0.0.1', r));
    win = new BrowserWindow({ width: 680, height: 900, show: false, webPreferences: { backgroundThrottling: false } });
    win.webContents.on('console-message', (_event, _level, message) => console.log('renderer:', message));
    await win.loadURL(`http://127.0.0.1:${server.address().port}/`);
    const js = code => win.webContents.executeJavaScript(code);
    for (let i = 0; i < 50; i++) { if (await js(`Array.from(document.querySelectorAll('button')).some(b => b.textContent === 'View screen')`)) break; await wait(100); }
    await js(`Array.from(document.querySelectorAll('button')).find(b => b.textContent === 'View screen').click()`);
    await wait(8000);
    const read = () => js(`({ video: { width: document.querySelector('video').videoWidth, height: document.querySelector('video').videoHeight, frames: document.querySelector('video').getVideoPlaybackQuality().totalVideoFrames }, metrics: Object.fromEntries(Array.from(document.querySelectorAll('dl div')).map(d => [d.querySelector('dt').textContent,d.querySelector('dd').textContent])), smoke: { commands: window.smoke.commands, answers: window.smoke.answers, stopped: window.smoke.stopped } })`);
    const first = await read();
    assert.equal(first.video.width, 1280); assert.equal(first.video.height, 720); assert.ok(first.video.frames > 60, 'real frames rendered');
    assert.equal(first.metrics.Transport, 'udp'); assert.equal(first.metrics.Codec, 'video/H264');
    await js(`Array.from(document.querySelectorAll('button')).find(b => b.textContent === 'Retry ICE').click()`);
    await wait(4000);
    const second = await read();
    assert.ok(second.smoke.answers >= 2, 'ICE restart negotiated');
    assert.ok(second.video.frames > first.video.frames + 30, 'rendering continues after restart');
    await js('window.smoke.reconnect()'); await wait(2000);
    assert.ok((await read()).smoke.answers >= 3, 'signaling reconnect renegotiates the same stream');
    await js('window.smoke.rotate()'); await wait(3000);
    const rotated = await read(); assert.equal(rotated.video.width, 720); assert.equal(rotated.video.height, 1280);
    await js(`Array.from(document.querySelectorAll('button')).find(b => b.textContent === 'Home').click()`);
    assert.deepEqual(await js('window.smoke.commands'), ['input keyevent 3']);
    await js(`Array.from(document.querySelectorAll('button')).find(b => b.textContent === 'Stop').click()`);
    assert.equal(await js('window.smoke.stopped'), true);
    const result = { source: 'Synthetic canvas in Electron on the same machine; NOT an Android or internet benchmark', first, afterIceRestart: second, rotated };
    writeFileSync(join(artifacts, 'results.json'), JSON.stringify(result, null, 2));
    console.log(JSON.stringify(result, null, 2));
    console.log('PASS renderer H.264/UDP, actual video rendering, ICE restart, signaling reconnect, rotation, ADB control dispatch, stop cleanup');
    win.destroy(); server.close(); app.exit(0);
  } catch (error) { console.error(error); win?.destroy(); server?.close(); app.exit(1); }
});
