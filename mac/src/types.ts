export interface PhoneState {
  deviceId: string;
  adbProxy: string;
  connected: boolean;
  name: string;
  model: string;
  sdk: number | null;
  lastSeenAt: string | null;
  adbReady: boolean;
  adbDetail: string;
}

export interface CommandState {
  pending: boolean;
  deviceId: string;
  command: string;
  output: string;
  exitCode: number | null;
  error: string | null;
}

export interface GatewayState {
  enabled: boolean;
  clientId: string;
  status: 'connecting' | 'online' | 'offline';
  error: string | null;
  pairCode: string;
  qrPayload: string;
  phones: PhoneState[];
  command: CommandState;
}

export interface UsbDevice { serial: string; state: 'device' | 'unauthorized' | 'offline'; model: string }
export interface GrantResult { ok: boolean; message: string }

export interface PairspanApi {
  usbDevices(): Promise<UsbDevice[]>;
  grantPermission(serial: string): Promise<GrantResult>;
  gatewayState(): Promise<GatewayState>;
  copyPairCode(): Promise<void>;
  copyAdbProxy(deviceId: string): Promise<void>;
  copyText(value: string): Promise<void>;
  setEnabled(enabled: boolean): Promise<void>;
  runCommand(deviceId: string, command: string): Promise<void>;
  onGatewayState(callback: (state: GatewayState) => void): () => void;
}

declare global { interface Window { pairspan: PairspanApi } }
