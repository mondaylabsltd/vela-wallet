import {
  VELA_WEB_CHANNEL,
  VELA_WEB_INIT,
  isVelaWebReady,
  isVelaWebResponse,
  type VelaDAppMetadata,
  type VelaRpcError,
  type VelaWebInitMessage,
} from './protocol';

export interface RequestArguments {
  method: string;
  params?: readonly unknown[] | object;
}

export interface VelaProvider {
  request(args: RequestArguments): Promise<unknown>;
  on(event: string, listener: (...args: any[]) => void): VelaProvider;
  removeListener(event: string, listener: (...args: any[]) => void): VelaProvider;
  disconnect(): void;
  readonly isVela: true;
}

export interface VelaWalletSDKConfig {
  appName: string;
  appLogoUrl?: string;
  appUrl?: string;
  /** Defaults to the production Vela Web wallet request page. */
  walletUrl?: string;
  /** Popup response timeout. Defaults to five minutes. */
  timeoutMs?: number;
  /** Initial chain for a new connection. Defaults to Ethereum mainnet. */
  chainId?: number;
}

const STORAGE_KEY = 'vela.web-wallet.session.v1';

interface CachedSession {
  address: string;
  chainId: number;
}

function randomId(): string {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  const bytes = new Uint8Array(16);
  globalThis.crypto?.getRandomValues?.(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('') || `${Date.now()}-${Math.random()}`;
}

function rpcError(error: VelaRpcError): Error & VelaRpcError {
  return Object.assign(new Error(error.message), error);
}

function normalizeParams(params: RequestArguments['params']): unknown[] {
  if (params == null) return [];
  return Array.isArray(params) ? [...params] : [params];
}

function parseChainId(value: unknown): number | null {
  if (typeof value === 'number' && Number.isSafeInteger(value) && value > 0) return value;
  if (typeof value !== 'string') return null;
  const parsed = value.startsWith('0x') ? Number.parseInt(value.slice(2), 16) : Number.parseInt(value, 10);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null;
}

class Provider implements VelaProvider {
  readonly isVela = true as const;
  private readonly walletUrl: URL;
  private readonly timeoutMs: number;
  private readonly dapp: VelaDAppMetadata;
  private readonly listeners = new Map<string, Set<(...args: any[]) => void>>();
  private session: CachedSession | null;

  constructor(config: VelaWalletSDKConfig) {
    if (!config.appName?.trim()) throw new Error('Vela SDK requires appName');
    this.walletUrl = new URL(config.walletUrl ?? 'https://wallet.getvela.app/web-request');
    if (this.walletUrl.protocol !== 'https:' && this.walletUrl.hostname !== 'localhost') {
      throw new Error('Vela walletUrl must use HTTPS');
    }
    this.timeoutMs = config.timeoutMs ?? 5 * 60_000;
    this.dapp = {
      name: config.appName.trim(),
      url: config.appUrl ?? (typeof location !== 'undefined' ? location.origin : undefined),
      icon: config.appLogoUrl,
    };
    this.session = this.loadSession() ?? { address: '', chainId: config.chainId ?? 1 };
  }

  async request(args: RequestArguments): Promise<unknown> {
    if (!args || typeof args.method !== 'string' || !args.method) {
      throw rpcError({ code: -32602, message: 'Invalid request arguments' });
    }
    const method = args.method;
    const params = normalizeParams(args.params);

    if (method === 'eth_accounts') return this.session?.address ? [this.session.address] : [];
    if (method === 'eth_chainId') return `0x${(this.session?.chainId ?? 1).toString(16)}`;
    if (method === 'net_version') return String(this.session?.chainId ?? 1);
    if (method === 'wallet_getPermissions') {
      return this.session?.address ? [{ parentCapability: 'eth_accounts' }] : [];
    }

    const requestedChain = method === 'wallet_switchEthereumChain'
      ? parseChainId((params[0] as { chainId?: unknown } | undefined)?.chainId)
      : null;
    const result = await this.requestViaPopup(method, params, requestedChain ?? this.session?.chainId ?? 1);

    if (method === 'eth_requestAccounts' && Array.isArray(result) && typeof result[0] === 'string') {
      this.setSession({ address: result[0], chainId: this.session?.chainId ?? 1 });
      this.emit('accountsChanged', [result[0]]);
    } else if (method === 'wallet_switchEthereumChain' && requestedChain) {
      this.setSession({ address: this.session?.address ?? '', chainId: requestedChain });
      this.emit('chainChanged', `0x${requestedChain.toString(16)}`);
    }
    return result;
  }

  on(event: string, listener: (...args: any[]) => void): VelaProvider {
    if (!this.listeners.has(event)) this.listeners.set(event, new Set());
    this.listeners.get(event)!.add(listener);
    return this;
  }

  removeListener(event: string, listener: (...args: any[]) => void): VelaProvider {
    this.listeners.get(event)?.delete(listener);
    return this;
  }

  disconnect(): void {
    this.session = { address: '', chainId: this.session?.chainId ?? 1 };
    try { localStorage.removeItem(STORAGE_KEY); } catch { /* storage unavailable */ }
    this.emit('accountsChanged', []);
    this.emit('disconnect', { code: 4900, message: 'Disconnected' });
  }

  private requestViaPopup(method: string, params: unknown[], chainId: number): Promise<unknown> {
    if (typeof window === 'undefined') {
      return Promise.reject(rpcError({ code: 4200, message: 'Vela Web Wallet requires a browser' }));
    }
    const sessionId = randomId();
    const requestId = randomId();
    const url = new URL(this.walletUrl);
    url.searchParams.set('session', sessionId);
    const popup = window.open(url.toString(), `vela-${sessionId}`, 'popup,width=420,height=720,resizable=yes,scrollbars=yes');
    if (!popup) return Promise.reject(rpcError({ code: 4200, message: 'Vela popup was blocked' }));

    return new Promise((resolve, reject) => {
      let settled = false;
      let port: MessagePort | null = null;
      const finish = (fn: () => void) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        clearInterval(closedPoll);
        window.removeEventListener('message', onWindowMessage);
        port?.close();
        try { if (!popup.closed) popup.close(); } catch { /* cross-window close failed */ }
        fn();
      };
      const timer = window.setTimeout(() => finish(() => reject(rpcError({ code: 4900, message: 'Vela request timed out' }))), this.timeoutMs);
      const closedPoll = window.setInterval(() => {
        if (popup.closed) finish(() => reject(rpcError({ code: 4001, message: 'User closed the Vela popup' })));
      }, 400);

      const onWindowMessage = (event: MessageEvent) => {
        if (event.source !== popup || event.origin !== url.origin || !isVelaWebReady(event.data) || event.data.sessionId !== sessionId) return;
        const channel = new MessageChannel();
        port = channel.port1;
        port.onmessage = (portEvent) => {
          const message = portEvent.data;
          if (!isVelaWebResponse(message) || message.sessionId !== sessionId || message.id !== requestId) return;
          if (message.error) finish(() => reject(rpcError(message.error!)));
          else finish(() => resolve(message.result));
        };
        port.start();
        const init: VelaWebInitMessage = {
          channel: VELA_WEB_CHANNEL,
          type: VELA_WEB_INIT,
          sessionId,
          request: { id: requestId, method, params, chainId, address: this.session?.address || undefined },
          dapp: this.dapp,
        };
        popup.postMessage(init, url.origin, [channel.port2]);
      };
      window.addEventListener('message', onWindowMessage);
    });
  }

  private emit(event: string, ...args: any[]): void {
    this.listeners.get(event)?.forEach((listener) => {
      try { listener(...args); } catch { /* a dApp listener cannot break the provider */ }
    });
  }

  private setSession(session: CachedSession): void {
    this.session = session;
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(session)); } catch { /* storage unavailable */ }
  }

  private loadSession(): CachedSession | null {
    try {
      const value = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as Partial<CachedSession> | null;
      if (value && typeof value.address === 'string' && Number.isSafeInteger(value.chainId) && Number(value.chainId) > 0) {
        return { address: value.address, chainId: Number(value.chainId) };
      }
    } catch { /* missing/malformed storage */ }
    return null;
  }
}

export function createVelaWalletSDK(config: VelaWalletSDKConfig): { getProvider(): VelaProvider } {
  const provider = new Provider(config);
  return { getProvider: () => provider };
}

export * from './protocol';
