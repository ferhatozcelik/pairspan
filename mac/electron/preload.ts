import { contextBridge, ipcRenderer } from 'electron';

contextBridge.exposeInMainWorld('pairspan', {
  gatewayState: () => ipcRenderer.invoke('gateway:state'),
  copyPairCode: () => ipcRenderer.invoke('gateway:pair:copy'),
  copyAdbProxy: (deviceId: string) => ipcRenderer.invoke('gateway:adb:copy', deviceId),
  copyText: (value: string) => ipcRenderer.invoke('gateway:copy', value),
  usbDevices: () => ipcRenderer.invoke('adb:devices'),
  grantPermission: (serial: string) => ipcRenderer.invoke('adb:grant', serial),
  setEnabled: (enabled: boolean) => ipcRenderer.invoke('gateway:enabled', enabled),
  runCommand: (deviceId: string, command: string) => ipcRenderer.invoke('gateway:command', deviceId, command),
  onGatewayState: (callback: (value: unknown) => void) => {
    const listener = (_event: Electron.IpcRendererEvent, value: unknown) => callback(value);
    ipcRenderer.on('gateway:changed', listener);
    return () => ipcRenderer.removeListener('gateway:changed', listener);
  }
});
