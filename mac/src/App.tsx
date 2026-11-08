import { useEffect, useState } from 'react';
import { ScreenStream } from './ScreenStream';
import icon from '../assets/icon.png';
import type { GatewayState, UsbDevice } from './types';
import { t, errorText } from './i18n';

const initial: GatewayState = {
  enabled: true, status: 'connecting', error: null, clientId: '', pairCode: '', qrPayload: '', phones: [],
  command: { pending: false, deviceId: '', command: '', output: '', exitCode: null, error: null }
};

export default function App() {
  const [state, setState] = useState<GatewayState>(initial);
  const [streamDevice, setStreamDevice] = useState<string | null>(null);
  const [notice, setNotice] = useState('');

  useEffect(() => {
    void window.pairspan.gatewayState().then(setState);
    return window.pairspan.onGatewayState(setState);
  }, []);

  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(''), 2500);
    return () => clearTimeout(timer);
  }, [notice]);

  const [usb, setUsb] = useState<UsbDevice[] | null>(null);
  const [scanning, setScanning] = useState(false);
  const [grants, setGrants] = useState<Record<string, { busy: boolean; ok?: boolean; message?: string }>>({});

  const scan = () => {
    setScanning(true);
    void window.pairspan.usbDevices().then(setUsb).catch(() => setUsb([])).finally(() => setScanning(false));
  };

  useEffect(() => {
    scan();
    const interval = setInterval(scan, 4000);
    return () => clearInterval(interval);
  }, []);

  const grant = (serial: string) => {
    setGrants(current => ({ ...current, [serial]: { busy: true } }));
    void window.pairspan.grantPermission(serial)
      .then(result => {
        setGrants(current => ({ ...current, [serial]: { busy: false, ok: result.ok, message: result.message } }));
        scan();
      })
      .catch(error => setGrants(current => ({ ...current, [serial]: { busy: false, ok: false, message: String(error) } })));
  };

  const installApk = (serial: string) => {
    setGrants(current => ({ ...current, [serial]: { busy: true } }));
    void window.pairspan.installApk(serial)
      .then(result => {
        setGrants(current => ({ ...current, [serial]: { busy: false, ok: result.ok, message: result.message } }));
        scan();
      })
      .catch(error => setGrants(current => ({ ...current, [serial]: { busy: false, ok: false, message: String(error) } })));
  };

  const copy = (value: string) => void window.pairspan.copyText(value).then(() => setNotice(t('copied')));
  const gateway = state.status === 'online' ? { text: t('gw.ok'), cls: 'online' }
    : state.status === 'connecting' ? { text: t('gw.connecting'), cls: 'connecting' }
    : { text: t('gw.down'), cls: 'offline' };

  return <div className="app">
    <header>
      <img className="logo" src={icon} alt=""/>
      <div className="title"><strong>Pairspan</strong><small>{t('tagline')}</small></div>
      <div className="header-status">
        <span className="dot online" />
        <span>{t('active')}</span>
      </div>
    </header>
    <main>
      {streamDevice && <ScreenStream deviceId={streamDevice} gateway={state} onClose={() => setStreamDevice(null)} />}
      <section className="card status">
        <span className={`dot ${gateway.cls}`}/>
        <div>
          <h2>{gateway.text}</h2>
          <p>{state.status === 'online' ? t('gw.ok.d') : errorText(state.error) || t('gw.retry')}</p>
        </div>
      </section>

      {state.clientId && <section className="card ids">
        <span className="eyebrow">{t('client.id')}</span>
        <button className="id" onClick={() => copy(state.clientId)}>{state.clientId}</button>
      </section>}

      <section className="card devices">
        <span className="eyebrow">{t('devices')} · {state.phones.length}</span>
        {state.phones.length === 0 && <p>{t('phone.wait.d')}</p>}
        {state.phones.map(phone => <div className="device" key={phone.deviceId}>
          <div className="device-head">
            <span className={`dot ${phone.adbReady ? 'online' : 'connecting'}`}/>
            <strong>{phone.name || t('phone.on')}</strong>
            {phone.model && phone.model !== phone.name && <small>{phone.model}</small>}
          </div>
          <div className="kv"><span>{t('device.id')}</span><button className="id" onClick={() => copy(phone.deviceId)}>{phone.deviceId}</button></div>
          <div className="kv"><span>{t('client.id')}</span><button className="id" onClick={() => copy(state.clientId)}>{state.clientId}</button></div>
          <button className="primary small" disabled={!phone.connected} onClick={() => setStreamDevice(phone.deviceId)}>View screen</button>
          <p>{phone.adbReady ? t('adb.ready') : phone.adbDetail || t('adb.waiting')}</p>
          {phone.adbProxy && <button className="text-button" onClick={() => void window.pairspan.copyAdbProxy(phone.deviceId).then(() => setNotice(t('copied')))}>{phone.adbProxy}</button>}
        </div>)}
      </section>

      <section className="card grant">
        <span className="eyebrow">{t('grant.eyebrow')}</span>
        <p>{t('auto.hint')}</p>
        {usb && usb.length === 0 && <p>{t('grant.none')}</p>}
        {usb?.map(device => {
          const result = grants[device.serial];
          return <div className="device" key={device.serial}>
            <div className="device-head">
              <span className={`dot ${device.state === 'device' ? 'online' : 'connecting'}`}/>
              <strong>{device.model || device.serial}</strong>
            </div>
            <div className="kv"><span>{t('grant.serial')}</span><span className="id static">{device.serial}</span></div>
            {device.state === 'device' && <div className="kv">
              <span>APK</span>
              <span className="apk-badge">
                {device.apkInstalled
                  ? (device.hasUpdate ? `v${device.apkVersion} (${t('apk.update')} → v${device.latestVersion})` : `v${device.apkVersion} ✓`)
                  : t('apk.not_installed')}
              </span>
            </div>}

            {result?.busy && <div className="loading-banner">
              <span className="loading-spinner" />
              <span>{t('apk.installing_progress')}</span>
            </div>}

            {device.state === 'device' ? <div className="button-row">
              <button className="primary small" disabled={result?.busy} onClick={() => grant(device.serial)}>
                {result?.busy ? t('grant.working') : t('grant.button')}
              </button>
              {(!device.apkInstalled || device.hasUpdate) && (
                <button className="secondary small" disabled={result?.busy} onClick={() => installApk(device.serial)}>
                  {result?.busy ? t('apk.installing') : (device.hasUpdate ? t('apk.update') : t('apk.install'))}
                </button>
              )}
            </div> : <p>{t('grant.unauthorized')}</p>}
            {result && !result.busy && <p className={result.ok ? 'ok' : 'fail'}>{result.ok ? (result.message || t('grant.done')) : result.message || t('grant.failed')}</p>}
          </div>;
        })}
      </section>
    </main>
    {notice && <div className="notice">{notice}</div>}
  </div>;
}
