/**
 * WalletPair transport adapter.
 *
 * Wraps a WalletSession from walletpair-sdk and implements the DAppTransport
 * interface so it plugs into the existing DAppConnectionProvider seamlessly.
 *
 * Web: WebSocket relay only.
 * Mobile: BLE + WebSocket relay (BLE added in Phase 4).
 */

import { Platform } from 'react-native';
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
import type { DAppTransport, DAppTransportEvents, DAppInfo, WalletInfo } from './dapp-transport';
import { DEFAULT_NETWORKS, getAllNetworksSync } from '@/models/network';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'vela.walletpairSession';

/** Wallet capabilities advertised to dApps. */
function buildCapabilities(): Capabilities {
  const allNetworks = getAllNetworksSync();
  const chains = allNetworks.map(n => evmChainId(n.chainId));

  // Share RPC URLs so the dApp-side can proxy read-only requests locally
  const rpcUrls: Record<string, string> = {};
  for (const n of allNetworks) {
    rpcUrls[evmChainId(n.chainId)] = n.rpcURL;
  }

  return {
    methods: [
      'wallet_getAccounts',
      'wallet_sendTransaction',
      'wallet_signTransaction',
      'wallet_signMessage',
      'wallet_signTypedData',
      'wallet_switchChain',
    ],
    events: ['accountsChanged', 'chainChanged', 'disconnect'],
    chains,
    version: { evm: 1 },
    rpcUrls,
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

  private constructor(session: WalletSession, dappInfo: DAppInfo) {
    this.session = session;
    this._dappInfo = dappInfo;
    this.wireSessionEvents();
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
    let sdkTransport: WPTransport;

    if (relayUrl) {
      sdkTransport = new WebSocketTransport(relayUrl);
    } else {
      // BLE mode — only available on native platforms
      if (Platform.OS === 'web') {
        throw new Error('BLE connections require the mobile app. Use a relay-based QR code on web.');
      }
      // Lazy import to avoid bundling native module on web
      // eslint-disable-next-line @typescript-eslint/no-require-imports
      const { BlePeripheralTransport } = require('./walletpair-ble-transport');
      sdkTransport = new BlePeripheralTransport();
    }

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
      await this.session.confirmJoin();
      console.log('[WalletPair] confirmJoin resolved, phase:', this.session.phase);
    } catch (err) {
      console.log('[WalletPair] confirmJoin failed:', err);
      throw err;
    }
  }

  /** Reconnect a restored session. */
  async reconnect(): Promise<void> {
    await this.session.reconnect();
  }

  disconnect(): void {
    this._connected = false;
    this.session.destroy();
    this.emit('disconnected');
    this.listeners.clear();
  }

  sendResponse(id: string, result?: any, error?: { code: number; message: string }): void {
    if (error) {
      this.session.reject(id, String(error.code), error.message);
    } else {
      // Wrap result in WalletPair EVM sub-protocol format.
      // The dApp's EIP-1193 provider unwraps these via mapResponse().
      this.session.approve(id, this.wrapResult(id, result));
    }
  }

  /** Track which WalletPair method each request came from so we can wrap the response correctly. */
  private requestMethodMap = new Map<string, string>();

  pushWalletInfo(info: WalletInfo): void {
    if (!this._connected) return;
    this.session.pushEvent('accountsChanged', {
      accounts: info.accounts.map(a => ({
        address: a.address,
        chains: DEFAULT_NETWORKS.map(n => evmChainId(n.chainId)),
      })),
    });
    this.session.pushEvent('chainChanged', {
      chain: evmChainId(info.chainId),
    });
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

  private wireSessionEvents() {
    this.session.on('phase', (phase: WalletPhase) => {
      console.log('[WalletPair] phase:', phase);
      if (phase === 'connected') {
        this._connected = true;
        this.emit('connected', this._dappInfo.name || 'WalletPair');
      } else if (phase === 'closed') {
        const wasConnected = this._connected;
        this._connected = false;
        if (wasConnected) {
          this.emit('disconnected');
        } else {
          // Closed before ever connecting — channel expired or dApp rejected
          this.emit('error', 'Connection closed by dApp or relay. Try a fresh pairing URI.');
        }
      } else if (phase === 'disconnected') {
        // Transport-level disconnect — SDK will auto-reconnect
        console.log('[WalletPair] transport disconnected, SDK will retry...');
      }
    });

    this.session.on('request', (req: { id: string; method: string; params: unknown }) => {
      console.log('[WalletPair] request:', req.method, req.id);
      const wpMethod = req.method;
      const mappedMethod = METHOD_MAP[wpMethod] ?? wpMethod;
      const params = walletPairParamsToJsonRpc(mappedMethod, req.params);
      this.requestMethodMap.set(req.id, wpMethod);
      const origin = this._dappInfo.url || this._dappInfo.name || 'walletpair';
      this.emit('request', req.id, mappedMethod, params, origin);
    });

    this.session.on('error', (err: Error) => {
      console.log('[WalletPair] error:', err.message);
      this.emit('error', err.message);
    });
  }
}
