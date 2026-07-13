export const VELA_WEB_CHANNEL = 'vela-web-wallet-v1' as const;
export const VELA_WEB_READY = 'VELA_WEB_READY' as const;
export const VELA_WEB_INIT = 'VELA_WEB_INIT' as const;
export const VELA_WEB_RESPONSE = 'VELA_WEB_RESPONSE' as const;

export interface VelaRpcError {
  code: number;
  message: string;
  data?: unknown;
}

export interface VelaWebRequest {
  id: string;
  method: string;
  params: unknown[];
  chainId: number;
  /** Address cached by the dApp after connect. The wallet verifies it against its grant. */
  address?: string;
}

export interface VelaDAppMetadata {
  name: string;
  url?: string;
  icon?: string;
}

export interface VelaWebReadyMessage {
  channel: typeof VELA_WEB_CHANNEL;
  type: typeof VELA_WEB_READY;
  sessionId: string;
}

export interface VelaWebInitMessage {
  channel: typeof VELA_WEB_CHANNEL;
  type: typeof VELA_WEB_INIT;
  sessionId: string;
  request: VelaWebRequest;
  dapp: VelaDAppMetadata;
}

export interface VelaWebResponseMessage {
  channel: typeof VELA_WEB_CHANNEL;
  type: typeof VELA_WEB_RESPONSE;
  sessionId: string;
  id: string;
  result?: unknown;
  error?: VelaRpcError;
}

export function isVelaWebReady(value: unknown): value is VelaWebReadyMessage {
  const v = value as Partial<VelaWebReadyMessage> | null;
  return !!v && v.channel === VELA_WEB_CHANNEL && v.type === VELA_WEB_READY && typeof v.sessionId === 'string';
}

export function isVelaWebInit(value: unknown): value is VelaWebInitMessage {
  const v = value as Partial<VelaWebInitMessage> | null;
  const r = v?.request as Partial<VelaWebRequest> | undefined;
  if (!v || v.channel !== VELA_WEB_CHANNEL || v.type !== VELA_WEB_INIT ||
      typeof v.sessionId !== 'string' || v.sessionId.length < 1 || v.sessionId.length > 128 ||
      !r || typeof r.id !== 'string' || r.id.length < 1 || r.id.length > 128 ||
      typeof r.method !== 'string' || r.method.length < 1 || r.method.length > 100 ||
      !Array.isArray(r.params) || !Number.isSafeInteger(r.chainId) || Number(r.chainId) <= 0 ||
      !v.dapp || typeof v.dapp.name !== 'string' || v.dapp.name.length > 200 ||
      (r.address != null && (typeof r.address !== 'string' || !/^0x[0-9a-fA-F]{40}$/.test(r.address)))) {
    return false;
  }
  // Refuse cyclic/non-JSON or unreasonably large requests before they reach the
  // signing UI. 512 KiB is ample for typed data and batches but bounds popup RAM.
  try { return JSON.stringify(r.params).length <= 512 * 1024; } catch { return false; }
}

export function isVelaWebResponse(value: unknown): value is VelaWebResponseMessage {
  const v = value as Partial<VelaWebResponseMessage> | null;
  return !!v && v.channel === VELA_WEB_CHANNEL && v.type === VELA_WEB_RESPONSE &&
    typeof v.sessionId === 'string' && typeof v.id === 'string' &&
    (v.error == null || (typeof v.error.code === 'number' && typeof v.error.message === 'string'));
}
