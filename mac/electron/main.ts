import { app, BrowserWindow, clipboard, ipcMain, powerSaveBlocker, Tray, Menu, nativeImage } from 'electron';
import { join } from 'node:path';
import dotenv from 'dotenv';
import * as gateway from './gateway';
import { t } from './i18n';
import * as store from './store';
import * as adb from './adb';

dotenv.config({ path: join(app.getAppPath(), '.env.example') });
let window: BrowserWindow | null = null;
let tray: Tray | null = null;
let quitting = false;
let powerBlockerId: number | undefined;
let menu: Menu | null = null;
let saved = { enabled: true, clientId: '' };

function setEnabled(enabled: boolean): void {
  saved = { ...saved, enabled };
  store.save(saved);
  gateway.setEnabled(enabled);
  buildMenu();
}

function buildMenu(): void {
  menu = Menu.buildFromTemplate([
    { label: t('open'), click: showWindow },
    { label: t('enabled'), type: 'checkbox', checked: saved.enabled, click: item => setEnabled(item.checked) },
    { type: 'separator' },
    { label: t('quit'), click: () => app.quit() }
  ]);
  if (process.platform !== 'darwin') tray?.setContextMenu(menu);
}

function showWindow(): void {
  if (!window) return;
  if (tray && process.platform === 'darwin') {
    const bounds = tray.getBounds();
    const size = window.getBounds();
    window.setPosition(Math.round(bounds.x + bounds.width / 2 - size.width / 2), Math.round(bounds.y + bounds.height + 6));
  }
  window.show(); window.focus();
  if (process.platform === 'darwin') app.dock?.hide();
}

if (process.platform === 'darwin') app.setActivationPolicy('accessory');

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
app.on('second-instance', () => showWindow());
app.whenReady().then(async () => {
  saved = store.load();
  gateway.setClientId(saved.clientId);
  gateway.setSettings({ url: process.env.GATEWAY_PUBLIC_URL || process.env.PAIRSPAN_GATEWAY_URL || process.env.VITE_GATEWAY_URL || gateway.DEFAULT_GATEWAY_URL, token: process.env.GATEWAY_TOKEN || '' });
  ipcMain.handle('gateway:state', event => {
    if (event.sender !== window?.webContents) throw new Error('Invalid window.');
    return gateway.snapshot();
  });
  ipcMain.handle('gateway:pair:copy', event => {
    if (event.sender !== window?.webContents) throw new Error('Invalid window.');
    const code = gateway.snapshot().pairCode;
    if (code) clipboard.writeText(code);
  });
  ipcMain.handle('gateway:adb:copy', (event, deviceId: unknown) => {
    if (event.sender !== window?.webContents || typeof deviceId !== 'string') throw new Error('Invalid window.');
    const proxy = gateway.adbProxyFor(deviceId);
    if (proxy) clipboard.writeText(proxy);
  });
  ipcMain.handle('gateway:copy', (event, value: unknown) => {
    if (event.sender !== window?.webContents || typeof value !== 'string' || value.length > 128) throw new Error('Invalid window.');
    clipboard.writeText(value);
  });
  ipcMain.handle('adb:devices', event => {
    if (event.sender !== window?.webContents) throw new Error('Invalid window.');
    return adb.listUsbDevices();
  });
  ipcMain.handle('adb:grant', (event, serial: unknown) => {
    if (event.sender !== window?.webContents || typeof serial !== 'string') throw new Error('Invalid window.');
    return adb.grantSecureSettings(serial);
  });
  ipcMain.handle('gateway:enabled', (event, enabled: unknown) => {
    if (event.sender !== window?.webContents || typeof enabled !== 'boolean') throw new Error('Invalid window.');
    setEnabled(enabled);
  });
  ipcMain.handle('gateway:command', async (event, deviceId: unknown, command: unknown) => {
    if (event.sender !== window?.webContents || typeof deviceId !== 'string' || typeof command !== 'string') throw new Error('Invalid command.');
    await gateway.runCommand(deviceId, command);
  });

  window = new BrowserWindow({
    width: 380, height: 520, minWidth: 340, minHeight: 420, show: false,
    resizable: true, skipTaskbar: true, title: 'Pairspan',
    icon: join(app.getAppPath(), 'assets', 'icon.png'),
    webPreferences: { preload: join(__dirname, 'preload.js'), contextIsolation: true, nodeIntegration: false, sandbox: true }
  });
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.on('will-navigate', event => event.preventDefault());
  window.on('blur', () => { if (process.platform === 'darwin') window?.hide(); });
  window.on('close', event => { if (!quitting) { event.preventDefault(); window?.hide(); } });
  gateway.subscribe(state => {
    window?.webContents.send('gateway:changed', state);
    const count = state.phones.length;
    tray?.setToolTip(`Pairspan · ${!state.enabled ? t('disabled') : count > 0 ? t('phones', { count: String(count) }) : state.status === 'online' ? t('code', { code: state.pairCode }) : t('down')}`);
  });
  const icon = nativeImage.createFromPath(join(app.getAppPath(), 'assets', process.platform === 'darwin' ? 'trayTemplate.png' : 'tray.png'));
  if (process.platform === 'darwin') icon.setTemplateImage(true);
  tray = new Tray(icon);
  tray.setToolTip('Pairspan');
  tray.on('click', () => { if (window?.isVisible()) window.hide(); else showWindow(); });
  buildMenu();
  if (process.platform === 'darwin') tray.on('right-click', () => { if (menu) tray?.popUpContextMenu(menu); });
  if (process.platform === 'darwin') app.dock?.hide();
  if (saved.enabled) gateway.start(); else gateway.setEnabled(false);
  powerBlockerId = powerSaveBlocker.start('prevent-app-suspension');
  const devUrl = process.env.PAIRSPAN_DEV_URL;
  if (devUrl) await window.loadURL(devUrl);
  else await window.loadFile(join(app.getAppPath(), 'dist', 'index.html'));
  showWindow();
});
}

app.on('activate', showWindow);
app.on('window-all-closed', () => { /* tray owns app lifetime */ });
app.on('before-quit', () => {
  quitting = true;
  gateway.stop();
  if (powerBlockerId !== undefined) powerSaveBlocker.stop(powerBlockerId);
});
