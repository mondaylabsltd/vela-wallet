/**
 * DApp transport adapter interface.
 *
 * Abstracts the communication layer between the wallet and dApps.
 * Implementations: SSE+POST (remote-inject), WebSocket, BLE.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface DAppTransportEvents {
  /** Connection established — ready to receive requests. */
  connected: (peerName: string) => void;
  /** Connection lost or closed permanently. */
  disconnected: () => void;
  /** Transport lost — auto-reconnect in progress. */
  reconnecting: () => void;
  /** Incoming JSON-RPC request from a dApp. */
  request: (id: string, method: string, params: any[], origin: string) => void;
  /** Transport-level error. */
  error: (message: string) => void;
}

export interface DAppTransport {
  /** Human-readable transport name (e.g. "Remote Bridge"). */
  readonly name: string;

  /** Whether the transport is currently connected. */
  readonly connected: boolean;

  /** Connect to the relay / peer. */
  connect(): Promise<void>;

  /** Disconnect and clean up resources. */
  disconnect(): void;

  /** Send a JSON-RPC response back to the dApp. */
  sendResponse(id: string, result?: any, error?: { code: number; message: string }): void;

  /** Push updated wallet info to the dApp side. */
  pushWalletInfo(info: WalletInfo): void;

  /** Fetch DApp metadata from the relay session. */
  fetchDAppInfo(): Promise<DAppInfo | null>;

  /** Register an event listener. Returns an unsubscribe function. */
  on<K extends keyof DAppTransportEvents>(event: K, listener: DAppTransportEvents[K]): () => void;
}

export interface WalletInfo {
  address: string;
  chainId: number;
  name: string;
  accounts: { name: string; address: string }[];
}

// ---------------------------------------------------------------------------
// SSE + POST transport for remote-inject bridge
// ---------------------------------------------------------------------------

export interface RemoteInjectSession {
  serverUrl: string;
  sessionId: string;
  nonce: string;
  secret: string;
}

/** DApp metadata from the relay session. */
export interface DAppInfo {
  name: string;
  url: string;
  icon?: string;
}

export class RemoteInjectTransport implements DAppTransport {
  readonly name = 'Remote Bridge';
  private _connected = false;
  private eventSource: EventSource | null = null;
  private listeners = new Map<string, Set<Function>>();
  private session: RemoteInjectSession;

  constructor(session: RemoteInjectSession) {
    this.session = session;
  }

  get connected() { return this._connected; }

  async connect(): Promise<void> {
    const { serverUrl, sessionId, nonce, secret } = this.session;
    const sseUrl = `${serverUrl}/sse?session=${sessionId}&role=mobile&n=${nonce}&k=${secret}`;

    return new Promise((resolve, reject) => {
      const es = new EventSource(sseUrl);
      this.eventSource = es;
      let resolved = false;

      es.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data);
          // The server sends { type: 'ready' } once the SSE stream is authenticated
          if (msg.type === 'ready' && !resolved) {
            resolved = true;
            this._connected = true;
            this.emit('connected', 'Remote Bridge');
            resolve();
            return;
          }
          this.handleMessage(msg);
        } catch {}
      };

      es.onerror = () => {
        if (!resolved) {
          // Failed to connect — SSE rejected (403, 404, etc.)
          resolved = true;
          es.close();
          this.eventSource = null;
          reject(new Error('Failed to connect to bridge'));
          return;
        }
        // Connection lost after successful connect
        this._connected = false;
        this.eventSource = null;
        this.emit('disconnected');
      };
    });
  }

  disconnect(): void {
    // Send disconnect message before closing
    if (this._connected) {
      this.postMessage({ type: 'disconnect' }).catch(() => {});
    }
    this.eventSource?.close();
    this.eventSource = null;
    this._connected = false;
    this.emit('disconnected');
  }

  sendResponse(id: string, result?: any, error?: { code: number; message: string }): void {
    const msg: any = { type: 'response', id };
    if (error) msg.error = error;
    else msg.result = result ?? null;
    this.postMessage(msg).catch(() => {});
  }

  pushWalletInfo(info: WalletInfo): void {
    this.postMessage({
      type: 'connect',
      address: info.address,
      chainId: info.chainId,
    }).catch(() => {});
  }

  async fetchDAppInfo(): Promise<DAppInfo | null> {
    const { serverUrl, sessionId, nonce, secret } = this.session;
    try {
      const res = await fetch(`${serverUrl}/session/${sessionId}?n=${nonce}&k=${secret}`);
      if (!res.ok) return null;
      const data = await res.json();
      if (data.metadata?.name && data.metadata?.url) {
        return { name: data.metadata.name, url: data.metadata.url, icon: data.metadata.icon };
      }
      return null;
    } catch {
      return null;
    }
  }

  on<K extends keyof DAppTransportEvents>(event: K, listener: DAppTransportEvents[K]): () => void {
    if (!this.listeners.has(event)) this.listeners.set(event, new Set());
    const set = this.listeners.get(event)!;
    set.add(listener);
    return () => { set.delete(listener); };
  }

  // --- Private ---

  private emit(event: string, ...args: any[]) {
    const set = this.listeners.get(event);
    if (set) set.forEach(fn => fn(...args));
  }

  private handleMessage(msg: any) {
    switch (msg.type) {
      case 'ready':
        // Handled in connect() — also fires on SSE reconnect
        if (!this._connected) {
          this._connected = true;
          this.emit('connected', 'Remote Bridge');
        }
        break;
      case 'request':
        if (msg.id && msg.method) {
          this.emit('request', msg.id, msg.method, msg.params ?? [], msg.origin ?? '');
        }
        break;
      case 'disconnect':
        this._connected = false;
        this.eventSource?.close();
        this.eventSource = null;
        this.emit('disconnected');
        break;
      case 'error':
        this.emit('error', msg.message ?? 'Bridge error');
        break;
    }
  }

  private async postMessage(body: any): Promise<void> {
    const { serverUrl, sessionId, nonce, secret } = this.session;
    const url = `${serverUrl}/message?session=${sessionId}&role=mobile&n=${nonce}&k=${secret}`;
    await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
  }
}

// ---------------------------------------------------------------------------
// Parse a remote-inject QR code URL into session credentials
// ---------------------------------------------------------------------------

/**
 * Parse a remote-inject connection URL.
 * Expected formats:
 *   https://server.com/s/{sessionId}?n={nonce}&k={secret}
 *   https://server.com/bridge?session={sessionId}&n={nonce}&k={secret}
 */
export function parseRemoteInjectURL(raw: string): RemoteInjectSession | null {
  try {
    const url = new URL(raw);
    const serverUrl = `${url.protocol}//${url.host}`;
    const nonce = url.searchParams.get('n');
    const secret = url.searchParams.get('k');

    if (!nonce || !secret) return null;

    // Format: /s/{sessionId}?n={nonce}&k={secret}
    const pathMatch = url.pathname.match(/^\/s\/([^/?]+)/);
    if (pathMatch) {
      const sessionId = pathMatch[1];
      if (sessionId) return { serverUrl, sessionId, nonce, secret };
    }

    // Format: /bridge?session={sessionId}&n={nonce}&k={secret}
    const sessionId = url.searchParams.get('session');
    if (sessionId) return { serverUrl, sessionId, nonce, secret };

    return null;
  } catch {
    return null;
  }
}
