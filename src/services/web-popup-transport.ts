import type { DAppInfo, DAppTransport, DAppTransportEvents, WalletInfo } from './dapp-transport';
import {
  VELA_WEB_CHANNEL,
  VELA_WEB_RESPONSE,
  type VelaWebRequest,
  type VelaWebResponseMessage,
} from '../../packages/vela-sdk/src/protocol';

export interface WebPopupPeer {
  sessionId: string;
  origin: string;
  dapp: DAppInfo;
  request: VelaWebRequest;
  port: MessagePort;
}

/** One HTTPS popup request. The MessagePort is capability-bound to the opener that
 * completed the origin-checked handshake; responses never use a wildcard target. */
export class WebPopupTransport implements DAppTransport {
  readonly name = 'Vela Web';
  private _connected = false;
  private _settled = false;
  private listeners = new Map<string, Set<Function>>();

  constructor(private readonly peer: WebPopupPeer) {}

  get connected(): boolean { return this._connected; }
  get requestChainId(): number { return this.peer.request.chainId; }
  get requestAddress(): string | undefined { return this.peer.request.address; }
  get requestOrigin(): string { return this.peer.origin; }

  async connect(): Promise<void> {
    if (this._connected || this._settled) return;
    this._connected = true;
    this.emit('connected', this.peer.dapp.name);
    this.emit('request', this.peer.request.id, this.peer.request.method, this.peer.request.params as any[], this.peer.origin);
  }

  disconnect(): void {
    if (this._settled) return;
    this.sendResponse(this.peer.request.id, undefined, { code: 4001, message: 'User rejected the request' });
  }

  sendResponse(id: string, result?: any, error?: { code: number; message: string }): void {
    if (this._settled) return;
    this._settled = true;
    const message: VelaWebResponseMessage = {
      channel: VELA_WEB_CHANNEL,
      type: VELA_WEB_RESPONSE,
      sessionId: this.peer.sessionId,
      id,
      ...(error ? { error } : { result: result ?? null }),
    };
    try { this.peer.port.postMessage(message); } finally {
      this.peer.port.close();
      this._connected = false;
      this.emit('disconnected');
    }
  }

  pushWalletInfo(_info: WalletInfo): void { /* one-shot channel */ }
  async fetchDAppInfo(): Promise<DAppInfo> { return this.peer.dapp; }

  on<K extends keyof DAppTransportEvents>(event: K, listener: DAppTransportEvents[K]): () => void {
    if (!this.listeners.has(event)) this.listeners.set(event, new Set());
    this.listeners.get(event)!.add(listener);
    return () => this.listeners.get(event)?.delete(listener);
  }

  private emit(event: string, ...args: any[]): void {
    this.listeners.get(event)?.forEach((listener) => listener(...args));
  }
}

export function isAllowedWebDAppOrigin(origin: string): boolean {
  try {
    const url = new URL(origin);
    return url.protocol === 'https:' ||
      (url.protocol === 'http:' && (url.hostname === 'localhost' || url.hostname === '127.0.0.1' || url.hostname === '[::1]'));
  } catch {
    return false;
  }
}
