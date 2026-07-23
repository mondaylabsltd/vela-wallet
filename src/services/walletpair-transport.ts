/**
 * WalletPair transport adapter.
 *
 * Implements the documented WalletPair wallet session directly and adapts it to
 * DAppTransport so it plugs into DAppConnectionProvider seamlessly.
 *
 * Transport: WebSocket relay (web and mobile). Pairing happens over a relay
 * URL carried in the pairing URI — no Bluetooth.
 */

import { AppState, Platform, type AppStateStatus } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import {
  parsePairingUri,
  type SessionPersistence,
  WalletPairSession,
  type WalletPairPhase,
  type EthereumRequest,
} from './walletpair-protocol';
import type { DAppTransport, DAppTransportEvents, DAppInfo, WalletInfo } from './dapp-transport';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'vela.walletpairSession';

/** Max time to stay in the transient "reconnecting" state before surfacing a
 *  recoverable error. The session is kept (a later retry may still recover / user can
 *  manually reconnect) — we just stop pretending it's about to come back. */
const RECONNECT_MAX_MS = 60_000;

const SUPPORTED_METHODS = new Set([
  'eth_requestAccounts', 'eth_accounts', 'eth_chainId', 'net_version',
  'wallet_switchEthereumChain', 'wallet_addEthereumChain',
  'wallet_getPermissions', 'wallet_requestPermissions',
  'eth_sendTransaction', 'personal_sign',
  'eth_signTypedData', 'eth_signTypedData_v1', 'eth_signTypedData_v3', 'eth_signTypedData_v4',
  'wallet_sendCalls', 'wallet_getCallsStatus', 'wallet_getCapabilities',
  'web3_clientVersion', 'eth_syncing', 'eth_blockNumber', 'eth_call',
  'eth_estimateGas', 'eth_createAccessList', 'eth_gasPrice',
  'eth_maxPriorityFeePerGas', 'eth_feeHistory', 'eth_getBalance',
  'eth_getCode', 'eth_getStorageAt', 'eth_getProof', 'eth_getTransactionCount',
  'eth_getBlockByHash', 'eth_getBlockByNumber', 'eth_getBlockTransactionCountByHash',
  'eth_getBlockTransactionCountByNumber', 'eth_getTransactionByHash',
  'eth_getTransactionByBlockHashAndIndex', 'eth_getTransactionByBlockNumberAndIndex',
  'eth_getTransactionReceipt', 'eth_getLogs',
]);

function caip2ToChainId(caip2: string): number {
  const parsed = Number.parseInt(caip2.slice('eip155:'.length), 10);
  if (!Number.isSafeInteger(parsed) || parsed < 1) throw new TypeError('invalid EIP-155 chain ID');
  return parsed;
}

function declaredChainId(value: unknown): number | null {
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) return value;
  if (typeof value !== 'string') return null;
  if (/^0x[0-9a-fA-F]+$/.test(value)) return Number.parseInt(value, 16);
  if (/^[0-9]+$/.test(value)) return Number.parseInt(value, 10);
  return null;
}

/** The encrypted CAIP-2 suffix is the authoritative chain context. */
function assertRequestChainContext(method: string, params: any[], caip2: string): void {
  const expected = caip2ToChainId(caip2);
  let candidate: unknown;
  if (method === 'eth_sendTransaction' || method === 'wallet_sendCalls') candidate = params[0]?.chainId;
  if (method.includes('signTypedData')) {
    const typed = params[1] ?? params[0];
    try {
      candidate = typeof typed === 'string' ? JSON.parse(typed)?.domain?.chainId : typed?.domain?.chainId;
    } catch {
      // The signing validator will return a method-specific invalid-params error.
      return;
    }
  }
  if (candidate === undefined || candidate === null) return;
  const actual = declaredChainId(candidate);
  if (actual === null || actual !== expected) throw new TypeError('request chain ID does not match its WalletPair chain context');
}


// ---------------------------------------------------------------------------
// AsyncStorage-backed persistence
// ---------------------------------------------------------------------------

function createPersistence(): SessionPersistence {
  return {
    save(snapshot: string) {
      return AsyncStorage.setItem(STORAGE_KEY, snapshot) as unknown as Promise<void>;
    },
    async load() {
      return AsyncStorage.getItem(STORAGE_KEY);
    },
    async clear() {
      await AsyncStorage.removeItem(STORAGE_KEY);
    },
  };
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/** Check if a raw string is a WalletPair pairing URI. */
export function isWalletPairURI(raw: string): boolean {
  return raw.trimStart().startsWith('walletpair:');
}

/** Load a persisted WalletPair session snapshot (for auto-reconnect). */
export async function loadWalletPairSnapshot(): Promise<string | null> {
  return AsyncStorage.getItem(STORAGE_KEY);
}

/** Clear persisted WalletPair session. */
export async function clearWalletPairSession(): Promise<void> {
  await AsyncStorage.removeItem(STORAGE_KEY);
}

// ---------------------------------------------------------------------------
// Prepare result
// ---------------------------------------------------------------------------

export interface WalletPairPrepareResult {
  fingerprint: string;
  dappInfo: DAppInfo;
  transport: WalletPairTransport;
}

// ---------------------------------------------------------------------------
// WalletPairTransport
// ---------------------------------------------------------------------------

export class WalletPairTransport implements DAppTransport {
  readonly name = 'WalletPair';

  private session: WalletPairSession;
  private _connected = false;
  private _dappInfo: DAppInfo;
  private listeners = new Map<string, Set<Function>>();
  private pingTimer: ReturnType<typeof setInterval> | null = null;
  private appStateSub: { remove(): void } | null = null;
  /** Detach fns for the web-only recovery listeners (online / tab-visible). */
  private webRecoverCleanups: (() => void)[] | null = null;
  /** Epoch ms of the last forced recovery — throttles back-to-back triggers. */
  private lastRecoverAt = 0;
  /** Epoch ms when the app last went to background (null while foregrounded). */
  private backgroundedAt: number | null = null;
  /** Bounds the transient "reconnecting" state so it can't spin forever. */
  private reconnectDeadline: ReturnType<typeof setTimeout> | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempt = 0;
  private connectEventSent = false;

  private constructor(session: WalletPairSession, dappInfo: DAppInfo) {
    this.session = session;
    this._dappInfo = dappInfo;
    this.wireSessionEvents();
    // App-foreground recovery must outlive disconnect/reconnect cycles, so it is
    // set up here (for the session's whole lifetime) rather than inside the
    // heartbeat — otherwise it would be torn down the instant we disconnect,
    // exactly when foreground recovery matters most.
    this.setupAppStateRecovery();
    this.setupWebRecovery();
  }

  // -----------------------------------------------------------------------
  // Static factory
  // -----------------------------------------------------------------------

  /**
   * Prepare a WalletPair connection from a pairing URI.
   * Returns the fingerprint for user verification and a transport ready to connect().
   */
  static prepare(uri: string): WalletPairPrepareResult {
    const parsed = parsePairingUri(uri.trim());
    const persistence = createPersistence();
    const session = new WalletPairSession({
      meta: {
        name: 'Vela Wallet',
        url: 'https://getvela.app',
        icon: 'https://getvela.app/icon.png',
      },
      persistence,
    });

    const fingerprint = session.prepareJoin(uri);

    const dappInfo: DAppInfo = {
      name: parsed.name ?? 'Unknown dApp',
      url: parsed.url ?? '',
      icon: parsed.icon,
    };

    const transport = new WalletPairTransport(session, dappInfo);
    return { fingerprint, dappInfo, transport };
  }

  /**
   * Restore a WalletPair session from a persisted snapshot for auto-reconnect.
   * Returns null if no snapshot exists or restoration fails.
   */
  static async restore(): Promise<WalletPairTransport | null> {
    const snapshot = await loadWalletPairSnapshot();
    if (!snapshot) return null;

    let dappInfo: DAppInfo;
    try {
      const data = JSON.parse(snapshot) as { dapp?: DAppInfo };
      if (!data.dapp?.name || !data.dapp.url) throw new TypeError('missing dApp metadata');
      dappInfo = { name: data.dapp.name, url: data.dapp.url, icon: data.dapp.icon };
    } catch {
      await clearWalletPairSession();
      return null;
    }

    const persistence = createPersistence();
    const session = new WalletPairSession({
      meta: {
        name: 'Vela Wallet',
        url: 'https://getvela.app',
        icon: 'https://getvela.app/icon.png',
      },
      persistence,
    });

    const ok = session.restore(snapshot);
    if (!ok) {
      await clearWalletPairSession();
      return null;
    }

    const transport = new WalletPairTransport(session, dappInfo);
    return transport;
  }

  // -----------------------------------------------------------------------
  // DAppTransport interface
  // -----------------------------------------------------------------------

  get connected() { return this._connected; }

  async connect(): Promise<void> {
    console.log('[WalletPair] confirmJoin starting...');
    try {
      await this.session.confirmJoin();
      console.log('[WalletPair] confirmJoin resolved, phase:', this.session.phase);
    } catch (err) {
      console.log('[WalletPair] confirmJoin failed:', err);
      throw err;
    }
  }

  /** Force an immediate reconnect (also used for the manual "Reconnect now"). */
  async reconnect(): Promise<void> {
    if (this.session.phase !== 'disconnected') return;
    this.clearScheduledReconnect();
    this.emit('reconnecting');
    await this.session.reconnect();
  }

  disconnect(): void {
    const wasConnected = this._connected;
    this._connected = false;
    this.stopHeartbeat();
    this.clearReconnectDeadline();
    this.clearScheduledReconnect();
    this.teardownAppStateRecovery();
    this.teardownWebRecovery();
    this.session.destroy();
    if (!wasConnected) this.emit('disconnected');
    this.listeners.clear();
  }

  sendResponse(id: string, result?: any, error?: { code: number; message: string }): void {
    // Guard the channel calls: if the relay/socket is gone, approve/reject can
    // throw — that must not bubble into use-dapp-signing and break the UI. The
    // dApp will time out on its side; we log so the failure is diagnosable.
    try {
      if (error) {
        this.session.reject(id, error.code, error.message);
      } else {
        // The Ethereum protocol resolves the EIP-1193 result itself, not a
        // WalletPair-specific wrapper object.
        this.session.approve(id, result ?? null);
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      console.warn(`[WalletPair] failed to deliver response ${id}:`, message);
      // A bad application result can be converted into a valid ProviderRpcError
      // while the request ID is still outstanding (WalletPairSession validates
      // before deleting it). This is much better than leaving the dApp waiting.
      if (!error) {
        try {
          this.session.reject(id, -32603, 'Wallet could not encode the response');
          return;
        } catch { /* the channel/request is already gone */ }
      }
      this.emit('error', `Unable to return WalletPair response: ${message}`);
    }
  }

  pushWalletInfo(info: WalletInfo): void {
    if (!this._connected) return;
    try {
      const caip2 = `eip155:${info.chainId}`;
      const chainId = '0x' + info.chainId.toString(16);
      if (!this.connectEventSent) {
        this.session.pushEvent('connect', { chainId }, caip2);
        this.connectEventSent = true;
      }
      this.session.pushEvent('accountsChanged', info.accounts.map(a => a.address), caip2);
      this.session.pushEvent('chainChanged', chainId, caip2);
    } catch (e) {
      console.warn('[WalletPair] failed to push wallet info:', e instanceof Error ? e.message : e);
    }
  }

  async fetchDAppInfo(): Promise<DAppInfo | null> {
    return this._dappInfo;
  }

  on<K extends keyof DAppTransportEvents>(event: K, listener: DAppTransportEvents[K]): () => void {
    if (!this.listeners.has(event)) this.listeners.set(event, new Set());
    const set = this.listeners.get(event)!;
    set.add(listener);
    return () => { set.delete(listener); };
  }

  // -----------------------------------------------------------------------
  // Internal
  // -----------------------------------------------------------------------

  private emit(event: string, ...args: any[]) {
    const set = this.listeners.get(event);
    if (set) set.forEach(fn => fn(...args));
  }

  // -----------------------------------------------------------------------
  // Heartbeat — keeps the WebSocket alive through CF/NAT idle timeouts
  // -----------------------------------------------------------------------

  private startHeartbeat() {
    this.stopHeartbeat();
    // Ping every 25s (CF Workers idle timeout = 30s)
    this.pingTimer = setInterval(() => {
      if (this._connected) this.session.ping();
    }, 25_000);
  }

  private stopHeartbeat() {
    if (this.pingTimer) { clearInterval(this.pingTimer); this.pingTimer = null; }
  }

  // -----------------------------------------------------------------------
  // Reconnect deadline — stop the UI spinning "reconnecting" forever
  // -----------------------------------------------------------------------

  private startReconnectDeadline() {
    if (this.reconnectDeadline) return; // already counting down this episode
    this.reconnectDeadline = setTimeout(() => {
      this.reconnectDeadline = null;
      if (this._connected) return; // recovered in the meantime
      // The relay is taking too long (or is down). Keep the session — the protocol may
      // still recover and the user can hit "Reconnect now" — but surface a clear,
      // recoverable error so the UI exits the indefinite "reconnecting" state.
      console.log('[WalletPair] reconnect deadline hit — still not connected');
      this.emit('error', 'Still trying to reconnect to the dApp. Check your connection or reconnect manually.');
    }, RECONNECT_MAX_MS);
  }

  private clearReconnectDeadline() {
    if (this.reconnectDeadline) { clearTimeout(this.reconnectDeadline); this.reconnectDeadline = null; }
  }

  // -----------------------------------------------------------------------
  // App foreground/background recovery (mobile)
  // -----------------------------------------------------------------------
  //
  // When the OS backgrounds the app, React Native suspends JS timers — the
  // heartbeat and our reconnect backoff freeze — and the
  // relay/NAT idle-closes the WebSocket (~30s). On return to foreground the
  // socket is usually dead while `_connected` may still read true (the close
  // event never fired while JS was suspended). So on foreground we force a
  // reconnect rather than trusting the stale flag.

  private setupAppStateRecovery() {
    if (this.appStateSub) return;
    this.appStateSub = AppState.addEventListener('change', (next: AppStateStatus) => {
      if (next === 'active') {
        this.onForeground();
      } else if (next === 'background' || next === 'inactive') {
        if (this.backgroundedAt == null) this.backgroundedAt = Date.now();
      }
    });
  }

  private onForeground() {
    const backgroundedMs = this.backgroundedAt != null ? Date.now() - this.backgroundedAt : 0;
    this.backgroundedAt = null;

    // Nothing to recover before pairing has started ('idle') or once the session
    // is finished ('closed'); the normal connect() flow owns the initial join.
    if (this.session.phase !== 'disconnected' && this.session.phase !== 'connected') return;

    // If we're not connected, or we were backgrounded long enough that the relay
    // has almost certainly idle-closed the socket, force an immediate reconnect
    // (cancels any frozen backoff timer and retries now). For a brief blur, a
    // ping is enough to confirm the socket is still alive.
    const STALE_AFTER_MS = 20_000;
    if (!this._connected || backgroundedMs >= STALE_AFTER_MS) {
      console.log('[WalletPair] foreground: forcing reconnect (bg', backgroundedMs, 'ms)');
      if (this.session.phase === 'disconnected') {
        this.reconnect().catch((e) => console.log('[WalletPair] foreground reconnect failed:', e));
      }
    } else {
      this.session.ping();
    }
  }

  private teardownAppStateRecovery() {
    if (this.appStateSub) { this.appStateSub.remove(); this.appStateSub = null; }
    this.backgroundedAt = null;
  }

  // -----------------------------------------------------------------------
  // Web foreground/network recovery
  // -----------------------------------------------------------------------
  //
  // On mobile web the relay socket dies whenever the tab is backgrounded or the
  // network flaps (Wi-Fi↔5G, VPN reconnect), and a reconnect backoff can
  // stretch out — leaving the panel stuck on "重新连接中…" until the user taps
  // "立即重连". React Native's AppState only models tab visibility, never the
  // `online` event, so we listen for both browser signals directly and force an
  // immediate reconnect the moment the tab returns or the network comes back.

  private setupWebRecovery() {
    if (Platform.OS !== 'web' || typeof window === 'undefined') return;
    if (this.webRecoverCleanups) return;

    const onOnline = () => this.recoverNow('network online');
    const onVisible = () => {
      if (typeof document !== 'undefined' && document.visibilityState === 'visible') {
        this.recoverNow('tab visible');
      }
    };
    window.addEventListener('online', onOnline);
    document.addEventListener('visibilitychange', onVisible);
    this.webRecoverCleanups = [
      () => window.removeEventListener('online', onOnline),
      () => document.removeEventListener('visibilitychange', onVisible),
    ];
  }

  /**
   * Force a reconnect now (tab returned / network restored). Throttled so two
   * signals firing together (e.g. `online` + `visibilitychange`) don't double-
   * reconnect, and a no-op before pairing ('idle') or after teardown ('closed').
   */
  private recoverNow(reason: string) {
    if (this.session.phase !== 'disconnected' && this.session.phase !== 'connected') return;
    const now = Date.now();
    if (now - this.lastRecoverAt < 3000) return;
    this.lastRecoverAt = now;
    if (!this._connected) {
      console.log('[WalletPair] web recovery: forcing reconnect (', reason, ')');
      this.emit('reconnecting');
      this.startReconnectDeadline();
      this.reconnect().catch((e) => console.log('[WalletPair] web recovery reconnect failed:', e));
    } else {
      this.session.ping();
    }
  }

  private teardownWebRecovery() {
    if (this.webRecoverCleanups) { this.webRecoverCleanups.forEach((fn) => fn()); this.webRecoverCleanups = null; }
  }

  // -----------------------------------------------------------------------
  // Reconnect backoff
  // -----------------------------------------------------------------------

  private scheduleReconnect() {
    if (this.reconnectTimer || this.session.phase !== 'disconnected') return;
    const delay = Math.min(1_000 * 2 ** this.reconnectAttempt, 30_000);
    this.reconnectAttempt += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.reconnect().catch(() => this.scheduleReconnect());
    }, delay);
  }

  private clearScheduledReconnect() {
    if (this.reconnectTimer) { clearTimeout(this.reconnectTimer); this.reconnectTimer = null; }
  }

  // -----------------------------------------------------------------------
  // Session event wiring
  // -----------------------------------------------------------------------

  private wireSessionEvents() {
    this.session.on('phase', (phase: WalletPairPhase) => {
      console.log('[WalletPair] phase:', phase);
      if (phase === 'connected') {
        this._connected = true;
        this.reconnectAttempt = 0;
        this.clearScheduledReconnect();
        this.clearReconnectDeadline();
        this.startHeartbeat();
        this.emit('connected', this._dappInfo.name || 'WalletPair');
      } else if (phase === 'closed') {
        const wasConnected = this._connected;
        this._connected = false;
        this.stopHeartbeat();
        this.clearScheduledReconnect();
        this.clearReconnectDeadline();
        this.teardownAppStateRecovery();
        this.teardownWebRecovery();
        if (wasConnected) {
          this.emit('disconnected');
        } else {
          // Closed before ever connecting — channel expired or dApp rejected
          this.emit('error', 'Connection closed by dApp or relay. Try a fresh pairing URI.');
        }
      } else if (phase === 'disconnected') {
        // Transport-level disconnect. Keep the session and retry with a bounded
        // backoff; the encrypted counter state survives the new WebSocket.
        this._connected = false;
        this.connectEventSent = false;
        this.stopHeartbeat();
        this.emit('reconnecting');
        this.startReconnectDeadline();
        this.scheduleReconnect();
        console.log('[WalletPair] transport disconnected; reconnecting...');
      }
    });

    this.session.on('request', (req: EthereumRequest) => {
      console.log('[WalletPair] request:', req.method, req.id);
      try {
        if (!SUPPORTED_METHODS.has(req.method)) {
          console.warn(`[WalletPair] rejecting unsupported method ${req.method}; no signing sheet will open`);
          this.session.reject(req.id, 4200, `Unsupported WalletPair method: ${req.method}`);
          return;
        }
        if (!Array.isArray(req.params)) throw new TypeError('Ethereum RPC params must be an array for this method');
        assertRequestChainContext(req.method, req.params, req.caip2);
        const origin = this._dappInfo.url || this._dappInfo.name || 'walletpair';
        if (req.method === 'eth_sendTransaction' || req.method === 'personal_sign' || req.method.includes('signTypedData') || req.method === 'wallet_sendCalls') {
          console.log('[WalletPair] routing signing request to approval sheet:', req.method, req.id);
        }
        this.emit('request', req.id, req.method, req.params, origin, caip2ToChainId(req.caip2));
      } catch (e) {
        console.warn(`[WalletPair] dropping malformed request ${req.id} (${req.method}):`, e instanceof Error ? e.message : e);
        try { this.session.reject(req.id, -32602, 'Invalid params'); } catch { /* channel gone */ }
      }
    });

    this.session.on('error', (err: Error) => {
      console.log('[WalletPair] error:', err.message);
      this.emit('error', err.message);
    });
  }
}
