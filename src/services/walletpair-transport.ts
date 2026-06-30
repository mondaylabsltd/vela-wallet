/**
 * WalletPair transport adapter.
 *
 * Wraps a WalletSession from walletpair-sdk and implements the DAppTransport
 * interface so it plugs into the existing DAppConnectionProvider seamlessly.
 *
 * Transport: WebSocket relay (web and mobile). Pairing happens over a relay
 * URL carried in the pairing URI — no Bluetooth.
 */

import { AppState, type AppStateStatus } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import {
  WalletSession,
  WebSocketTransport,
  evmChainId,
  parsePairingUri,
  type Capabilities,
  type SessionPersistence,
  type Transport as WPTransport,
  type WalletPhase,
} from 'walletpair-sdk';
import * as WalletPairSDK from 'walletpair-sdk';
import type { DAppTransport, DAppTransportEvents, DAppInfo, WalletInfo } from './dapp-transport';
import { DEFAULT_NETWORKS, getAllNetworksSync } from '@/models/network';
import { SAFE_PROXY_RUNTIME_CODE } from './safe-address';

// Pipe the SDK's developer-only disconnect diagnostics (close code, relay
// terminate reason, phase, willReconnect) to the dev console so disconnect
// causes are queryable. NOT shown to end users (Metro/Flipper/Xcode logs only).
// Forward-compatible: feature-detected, so it's a no-op until walletpair-sdk is
// bumped to a version that exposes setDisconnectLogSink, then auto-activates.
(WalletPairSDK as { setDisconnectLogSink?: (fn: (e: unknown) => void) => void }).setDisconnectLogSink?.(
  (entry) => console.log('[WalletPair][disconnect]', entry),
);

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'vela.walletpairSession';

/**
 * SDK-operation deadlines (not fetch timeouts — those live in net.ts). The relay
 * can silently drop a join or stay unreachable, so we bound those states rather
 * than letting the connect promise hang or the UI spin "reconnecting" forever.
 */
const CONFIRM_JOIN_TIMEOUT_MS = 30_000;
/** Max time to stay in the transient "reconnecting" state before surfacing a
 *  recoverable error. The session is kept (SDK may still recover / user can
 *  manually reconnect) — we just stop pretending it's about to come back. */
const RECONNECT_MAX_MS = 60_000;

/**
 * Read-only JSON-RPC methods the wallet will forward to its own RPC when a dApp
 * routes them over the channel (spec §9.6 Tier 2). They are declared explicitly
 * in capabilities.methods so the session-layer allowlist (protocol §7.1) admits
 * them — explicit negotiation rather than implicit pass-through.
 */
const READ_ONLY_RPC_METHODS = [
  'eth_call', 'eth_estimateGas', 'eth_getBalance', 'eth_getCode',
  'eth_getStorageAt', 'eth_getTransactionCount', 'eth_getTransactionByHash',
  'eth_getTransactionReceipt', 'eth_getLogs', 'eth_blockNumber',
  'eth_getBlockByNumber', 'eth_getBlockByHash', 'eth_feeHistory',
  'eth_gasPrice', 'eth_maxPriorityFeePerGas', 'eth_newFilter',
  'eth_newBlockFilter', 'eth_getFilterChanges', 'eth_uninstallFilter',
  'eth_sendRawTransaction', 'eth_syncing',
];

/**
 * Race a promise against a deadline. On timeout, rejects with `message` — used to
 * bound SDK operations (confirmJoin) that would otherwise hang on a silent relay.
 * The underlying SDK operation is not cancelled (no AbortSignal in the SDK API);
 * we just stop awaiting it so the UI isn't frozen.
 */
function withTimeout<T>(p: Promise<T>, ms: number, message: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(message)), ms);
    p.then(
      (v) => { clearTimeout(timer); resolve(v); },
      (e) => { clearTimeout(timer); reject(e); },
    );
  });
}

/** Wallet capabilities advertised to dApps. */
function buildCapabilities(): Capabilities {
  const allNetworks = getAllNetworksSync();
  const chains = allNetworks.map(n => evmChainId(n.chainId));

  // Share RPC URLs so the dApp-side can proxy read-only requests locally
  const rpcUrls: Record<string, string> = {};
  for (const n of allNetworks) {
    rpcUrls[evmChainId(n.chainId)] = n.rpcURL;
  }

  // Declare EIP-5792 capabilities per chain (smart contract wallet)
  const walletCapabilities: Record<string, Record<string, unknown>> = {};
  for (const n of allNetworks) {
    const hexChainId = '0x' + n.chainId.toString(16);
    walletCapabilities[hexChainId] = {
      atomic: { status: 'supported' },
    };
  }

  return {
    methods: [
      'wallet_getAccounts',
      'wallet_sendTransaction',
      'wallet_signTransaction',
      'wallet_signMessage',
      'wallet_signTypedData',
      'wallet_switchChain',
      'wallet_sendCalls',
      'wallet_getCallsStatus',
      ...READ_ONLY_RPC_METHODS,
    ],
    events: ['accountsChanged', 'chainChanged', 'disconnect'],
    chains,
    version: { evm: 1 },
    rpcUrls,
    walletCapabilities,
    // Safe v1.4.1 SafeProxy *runtime* bytecode — what eth_getCode returns once
    // deployed, and identical for every proxy from this factory (the singleton
    // lives in storage slot 0, not in the code). Sent so the extension/SDK can
    // answer eth_getCode with non-empty code for a counterfactual account, so
    // dApps detect a smart contract wallet (EIP-1271) instead of an EOA.
    contractBytecode: SAFE_PROXY_RUNTIME_CODE,
  };
}

/** Map WalletPair method names → Ethereum JSON-RPC names used by use-dapp-signing. */
const METHOD_MAP: Record<string, string> = {
  wallet_signMessage: 'personal_sign',
  wallet_signTypedData: 'eth_signTypedData_v4',
  wallet_sendTransaction: 'eth_sendTransaction',
  wallet_signTransaction: 'eth_sendTransaction',
  wallet_getAccounts: 'eth_requestAccounts',
  wallet_switchChain: 'wallet_switchEthereumChain',
  wallet_sendCalls: 'wallet_sendCalls',
};

/**
 * Convert WalletPair request params (single object) back to Ethereum JSON-RPC
 * array format that use-dapp-signing.ts expects.
 *
 * WalletPair EVM sub-protocol sends:
 *   wallet_signMessage    → { message: "text", address: "0x..." }
 *   wallet_signTypedData  → { address: "0x...", typedData: {...} }
 *   wallet_sendTransaction → { address: "0x...", tx: { to, value, data, ... } }
 *   wallet_switchChain    → { chain: "eip155:137" }
 *   wallet_getAccounts    → {} or undefined
 *
 * Ethereum JSON-RPC expects:
 *   personal_sign          → [hexMessage, address]
 *   eth_signTypedData_v4   → [address, typedDataJSON]
 *   eth_sendTransaction    → [{ to, value, data, ... }]
 *   wallet_switchEthereumChain → [{ chainId: "0x89" }]
 *   eth_requestAccounts    → []
 */
function walletPairParamsToJsonRpc(mappedMethod: string, params: unknown): any[] {
  if (params == null) return [];
  if (Array.isArray(params)) return params;
  const p = params as Record<string, any>;

  switch (mappedMethod) {
    case 'personal_sign': {
      // WalletPair sends UTF-8 text; personal_sign expects hex-encoded bytes
      let hexMsg = p.message ?? '';
      if (typeof hexMsg === 'string' && !hexMsg.startsWith('0x')) {
        const bytes = new TextEncoder().encode(hexMsg);
        hexMsg = '0x' + Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
      }
      return [hexMsg, p.address ?? ''];
    }
    case 'eth_signTypedData_v4': {
      // use-dapp-signing reads params[1] ?? params[0]
      const typedData = typeof p.typedData === 'string' ? p.typedData : JSON.stringify(p.typedData);
      return [p.address ?? '', typedData];
    }
    case 'eth_sendTransaction': {
      // use-dapp-signing reads params[0] as { to, value, data }
      return [p.tx ?? p];
    }
    case 'wallet_switchEthereumChain': {
      // WalletPair sends { chain: "eip155:137" }, JSON-RPC expects [{ chainId: "0x89" }]
      const caip2 = p.chain as string | undefined;
      if (caip2?.startsWith('eip155:')) {
        const num = parseInt(caip2.split(':')[1], 10);
        return [{ chainId: '0x' + num.toString(16) }];
      }
      return [p];
    }
    case 'eth_requestAccounts':
      return [];
    case 'wallet_sendCalls':
      // Pass through as-is — the params object is the EIP-5792 sendCalls payload
      return [p];
    default:
      return [p];
  }
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

  private session: WalletSession;
  private _connected = false;
  private _dappInfo: DAppInfo;
  private listeners = new Map<string, Set<Function>>();
  private pingTimer: ReturnType<typeof setInterval> | null = null;
  private appStateSub: { remove(): void } | null = null;
  /** Epoch ms when the app last went to background (null while foregrounded). */
  private backgroundedAt: number | null = null;
  /** Bounds the transient "reconnecting" state so it can't spin forever. */
  private reconnectDeadline: ReturnType<typeof setTimeout> | null = null;

  private constructor(session: WalletSession, dappInfo: DAppInfo) {
    this.session = session;
    this._dappInfo = dappInfo;
    this.wireSessionEvents();
    // App-foreground recovery must outlive disconnect/reconnect cycles, so it is
    // set up here (for the session's whole lifetime) rather than inside the
    // heartbeat — otherwise it would be torn down the instant we disconnect,
    // exactly when foreground recovery matters most.
    this.setupAppStateRecovery();
  }

  // -----------------------------------------------------------------------
  // Static factory
  // -----------------------------------------------------------------------

  /**
   * Prepare a WalletPair connection from a pairing URI.
   * Returns the fingerprint for user verification and a transport ready to connect().
   */
  static prepare(uri: string): WalletPairPrepareResult {
    const parsed = parsePairingUri(uri);

    const relayUrl = parsed.relay;
    if (!relayUrl) {
      throw new Error('This pairing code has no relay URL. Vela Connect pairs over a relay — please scan a current Vela Connect QR code.');
    }
    const sdkTransport: WPTransport = new WebSocketTransport(relayUrl);

    const persistence = createPersistence();
    const session = new WalletSession({
      transport: sdkTransport,
      capabilities: buildCapabilities(),
      meta: {
        name: 'Vela Wallet',
        description: 'Smart wallet powered by passkeys',
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

    // Parse snapshot to extract relay URL and dApp name for display
    let relayUrl = '';
    let dappName = 'dApp';
    try {
      // Snapshot may be HMAC-signed: <64hex>.<json>
      const json = snapshot.length > 65 && snapshot[64] === '.'
        ? snapshot.slice(65)
        : snapshot;
      const data = JSON.parse(json);
      relayUrl = data.relayUrl ?? '';
      dappName = data.dappName ?? 'dApp';
    } catch {
      await clearWalletPairSession();
      return null;
    }

    if (!relayUrl) {
      await clearWalletPairSession();
      return null;
    }

    const wsTransport = new WebSocketTransport(relayUrl);
    const persistence = createPersistence();
    const session = new WalletSession({
      transport: wsTransport,
      capabilities: buildCapabilities(),
      meta: {
        name: 'Vela Wallet',
        description: 'Smart wallet powered by passkeys',
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

    const dappInfo: DAppInfo = { name: dappName, url: '' };
    const transport = new WalletPairTransport(session, dappInfo);
    // Session is already in 'connected' phase after restore — set flag
    transport._connected = true;
    return transport;
  }

  // -----------------------------------------------------------------------
  // DAppTransport interface
  // -----------------------------------------------------------------------

  get connected() { return this._connected; }

  async connect(): Promise<void> {
    console.log('[WalletPair] confirmJoin starting...');
    try {
      // Bound confirmJoin: if the relay silently drops the join (CF hibernation,
      // dropped packet) the SDK promise can hang forever, freezing the connect UI.
      await withTimeout(
        this.session.confirmJoin(),
        CONFIRM_JOIN_TIMEOUT_MS,
        'WalletPair connection timed out — check the dApp and try a fresh pairing QR.',
      );
      console.log('[WalletPair] confirmJoin resolved, phase:', this.session.phase);
    } catch (err) {
      console.log('[WalletPair] confirmJoin failed:', err);
      throw err;
    }
  }

  /** Force an immediate reconnect (also used for the manual "Reconnect now"). */
  async reconnect(): Promise<void> {
    if (this.session.phase === 'idle' || this.session.phase === 'closed') return;
    this.emit('reconnecting');
    await this.session.reconnect();
  }

  disconnect(): void {
    this._connected = false;
    this.stopHeartbeat();
    this.clearReconnectDeadline();
    this.teardownAppStateRecovery();
    this.session.destroy();
    this.emit('disconnected');
    this.listeners.clear();
  }

  sendResponse(id: string, result?: any, error?: { code: number; message: string }): void {
    // Guard the channel calls: if the relay/socket is gone, approve/reject can
    // throw — that must not bubble into use-dapp-signing and break the UI. The
    // dApp will time out on its side; we log so the failure is diagnosable.
    try {
      if (error) {
        // Drop the tracked method on the reject path too — wrapResult (which clears
        // it on success) is never reached for errors, so otherwise the entry leaks.
        this.requestMethodMap.delete(id);
        this.session.reject(id, String(error.code), error.message);
      } else {
        // Wrap result in WalletPair EVM sub-protocol format.
        // The dApp's EIP-1193 provider unwraps these via mapResponse().
        this.session.approve(id, this.wrapResult(id, result));
      }
    } catch (e) {
      this.requestMethodMap.delete(id);
      console.warn(`[WalletPair] failed to deliver response ${id}:`, e instanceof Error ? e.message : e);
    }
  }

  /** Track which WalletPair method each request came from so we can wrap the response correctly. */
  private requestMethodMap = new Map<string, string>();

  pushWalletInfo(info: WalletInfo): void {
    if (!this._connected) return;
    try {
      this.session.pushEvent('accountsChanged', {
        accounts: info.accounts.map(a => ({
          address: a.address,
          chains: DEFAULT_NETWORKS.map(n => evmChainId(n.chainId)),
        })),
      });
      this.session.pushEvent('chainChanged', {
        chain: evmChainId(info.chainId),
      });
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

  /**
   * Wrap Ethereum-format result into WalletPair EVM sub-protocol format.
   * The dApp's EIP-1193 provider (mapResponse) unwraps these.
   */
  private wrapResult(requestId: string, result: any): any {
    const wpMethod = this.requestMethodMap.get(requestId);
    this.requestMethodMap.delete(requestId);

    switch (wpMethod) {
      case 'wallet_signMessage':
      case 'wallet_signTypedData':
        // dApp expects { signature: "0x..." }
        return { signature: result };
      case 'wallet_sendTransaction':
        // dApp expects { txHash: "0x..." }
        return { txHash: result };
      case 'wallet_sendCalls':
        // dApp expects { id: "0x..." } — the bundle ID (tx hash)
        return { id: result };
      case 'wallet_getAccounts':
        // dApp expects { accounts: [{ address, chains }] }
        if (Array.isArray(result)) {
          return { accounts: result.map((a: string) => ({ address: a, chains: buildCapabilities().chains })) };
        }
        return result;
      default:
        return result;
    }
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
      // The relay is taking too long (or is down). Keep the session — the SDK may
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
  // heartbeat stops and the SDK's reconnect-backoff timers freeze — and the
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
    if (this.session.phase === 'idle' || this.session.phase === 'closed') return;

    // If we're not connected, or we were backgrounded long enough that the relay
    // has almost certainly idle-closed the socket, force an immediate reconnect
    // (cancels any frozen backoff timer and retries now). For a brief blur, a
    // ping is enough to confirm the socket is still alive.
    const STALE_AFTER_MS = 20_000;
    if (!this._connected || backgroundedMs >= STALE_AFTER_MS) {
      console.log('[WalletPair] foreground: forcing reconnect (bg', backgroundedMs, 'ms)');
      this.session.reconnect().catch((e) => console.log('[WalletPair] foreground reconnect failed:', e));
    } else {
      this.session.ping();
    }
  }

  private teardownAppStateRecovery() {
    if (this.appStateSub) { this.appStateSub.remove(); this.appStateSub = null; }
    this.backgroundedAt = null;
  }

  // -----------------------------------------------------------------------
  // Session event wiring
  // -----------------------------------------------------------------------

  private wireSessionEvents() {
    this.session.on('phase', (phase: WalletPhase) => {
      console.log('[WalletPair] phase:', phase);
      if (phase === 'connected') {
        this._connected = true;
        this.clearReconnectDeadline();
        this.startHeartbeat();
        this.emit('connected', this._dappInfo.name || 'WalletPair');
      } else if (phase === 'closed') {
        const wasConnected = this._connected;
        this._connected = false;
        this.stopHeartbeat();
        this.clearReconnectDeadline();
        this.teardownAppStateRecovery();
        if (wasConnected) {
          this.emit('disconnected');
        } else {
          // Closed before ever connecting — channel expired or dApp rejected
          this.emit('error', 'Connection closed by dApp or relay. Try a fresh pairing URI.');
        }
      } else if (phase === 'disconnected') {
        // Transport-level disconnect — SDK will auto-reconnect with backoff.
        // Don't mark as disconnected yet; emit 'reconnecting' so the UI can
        // show a subtle indicator instead of resetting to fully disconnected.
        this._connected = false;
        this.stopHeartbeat();
        this.emit('reconnecting');
        this.startReconnectDeadline();
        console.log('[WalletPair] transport disconnected, SDK will retry...');
      }
    });

    this.session.on('request', (req: { id: string; method: string; params: unknown }) => {
      console.log('[WalletPair] request:', req.method, req.id);
      // Isolate bad data: a malformed params payload must reject THIS request, not
      // throw out of the SDK callback and tear down the whole channel.
      try {
        const wpMethod = req.method;
        const mappedMethod = METHOD_MAP[wpMethod] ?? wpMethod;
        const params = walletPairParamsToJsonRpc(mappedMethod, req.params);
        this.requestMethodMap.set(req.id, wpMethod);
        const origin = this._dappInfo.url || this._dappInfo.name || 'walletpair';
        this.emit('request', req.id, mappedMethod, params, origin);
      } catch (e) {
        console.warn(`[WalletPair] dropping malformed request ${req.id} (${req.method}):`, e instanceof Error ? e.message : e);
        try { this.session.reject(req.id, '-32602', 'Invalid params'); } catch { /* channel gone */ }
      }
    });

    this.session.on('error', (err: Error) => {
      console.log('[WalletPair] error:', err.message);
      this.emit('error', err.message);
    });
  }
}
