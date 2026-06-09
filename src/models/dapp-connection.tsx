/**
 * Global dApp connection context.
 *
 * Lives at the root layout level so the SSE connection persists across screens.
 * Manages:
 *   - Remote-inject bridge connection (SSE + POST)
 *   - Incoming signing requests (surfaced as a global modal)
 *   - Session persistence to AsyncStorage for auto-reconnect
 */
import React, {
  createContext, useContext, useCallback, useEffect, useRef, useState,
  type ReactNode,
} from 'react';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { useWallet } from '@/models/wallet-state';
import {
  RemoteInjectTransport,
  type DAppTransport,
  type DAppInfo,
  type RemoteInjectSession,
} from '@/services/dapp-transport';
import {
  WalletPairTransport,
  loadWalletPairSnapshot,
  clearWalletPairSession,
} from '@/services/walletpair-transport';
import { isSigningMethod, handleDAppRequest, handleReadOnlyRPC } from '@/hooks/use-dapp-signing';
import { PasskeyErrorCode } from '@/modules/passkey';
import { saveTransaction } from '@/services/storage';
import { nativeSymbol } from '@/models/network';
import type { BLEIncomingRequest } from '@/models/types';

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'vela.remoteInjectSession';

export async function saveSession(session: RemoteInjectSession): Promise<void> {
  await AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(session));
}

export async function loadSession(): Promise<RemoteInjectSession | null> {
  try {
    const raw = await AsyncStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export async function clearSession(): Promise<void> {
  await AsyncStorage.removeItem(STORAGE_KEY);
}

// ---------------------------------------------------------------------------
// Context shape
// ---------------------------------------------------------------------------

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'error';
export type ConnectionType = 'remote-inject' | 'walletpair' | null;

interface DAppConnectionContextValue {
  /** Current connection status. */
  status: ConnectionStatus;
  /** Error message (when status === 'error'). */
  errorMessage: string | null;
  /** The current session (if any). */
  session: RemoteInjectSession | null;
  /** DApp metadata (name, url, icon) from the relay session. */
  dappInfo: DAppInfo | null;
  /** Current incoming signing request (shown in global modal). */
  incomingRequest: BLEIncomingRequest | null;
  /** Whether a signing operation is in progress. */
  isSigning: boolean;
  /** Last signing error message. */
  signError: string | null;
  /** Current chain ID for the bridge connection. */
  chainId: number;
  /** Which transport is active. */
  connectionType: ConnectionType;
  /** 4-digit fingerprint pending user verification (WalletPair only). */
  pendingFingerprint: string | null;
  /** Connect to a remote-inject bridge. */
  connectToBridge: (session: RemoteInjectSession) => Promise<void>;
  /** Connect via WalletPair pairing URI. */
  connectToWalletPair: (uri: string) => Promise<void>;
  /** Confirm the WalletPair fingerprint and complete connection. */
  confirmFingerprint: () => Promise<void>;
  /** Cancel a pending WalletPair fingerprint verification. */
  cancelFingerprint: () => void;
  /** Disconnect from the current bridge. */
  disconnectBridge: () => void;
  /** Approve the current incoming request. */
  approveRequest: () => Promise<void>;
  /** Reject the current incoming request. */
  rejectRequest: () => void;
  /** Dismiss the modal after an error (response already sent). */
  dismissRequest: () => void;
  /** Switch chain for the bridge connection. */
  switchChain: (chainId: number) => void;
}

const DAppConnectionContext = createContext<DAppConnectionContextValue>({
  status: 'disconnected',
  errorMessage: null,
  session: null,
  dappInfo: null,
  incomingRequest: null,
  isSigning: false,
  signError: null,
  chainId: 1,
  connectionType: null,
  pendingFingerprint: null,
  connectToBridge: async () => {},
  connectToWalletPair: async () => {},
  confirmFingerprint: async () => {},
  cancelFingerprint: () => {},
  disconnectBridge: () => {},
  approveRequest: async () => {},
  rejectRequest: () => {},
  dismissRequest: () => {},
  switchChain: () => {},
});

export function useDAppConnection() {
  return useContext(DAppConnectionContext);
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

export function DAppConnectionProvider({ children }: { children: ReactNode }) {
  const { state, activeAccount } = useWallet();
  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';

  const [status, setStatus] = useState<ConnectionStatus>('disconnected');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [session, setSession] = useState<RemoteInjectSession | null>(null);
  const [dappInfo, setDappInfo] = useState<DAppInfo | null>(null);
  const [incomingRequest, setIncomingRequest] = useState<BLEIncomingRequest | null>(null);
  const [isSigning, setIsSigning] = useState(false);
  const [signError, setSignError] = useState<string | null>(null);
  const [chainId, setChainId] = useState(1);
  const [connectionType, setConnectionType] = useState<ConnectionType>(null);
  const [pendingFingerprint, setPendingFingerprint] = useState<string | null>(null);

  const transportRef = useRef<DAppTransport | null>(null);
  /** Holds WalletPairTransport during fingerprint verification (before connect). */
  const pendingWpTransportRef = useRef<WalletPairTransport | null>(null);
  const addressRef = useRef(address);
  const chainIdRef = useRef(chainId);
  const accountNameRef = useRef(accountName);
  const accountsRef = useRef(state.accounts);
  const activeAccountRef = useRef(activeAccount);

  useEffect(() => { addressRef.current = address; }, [address]);
  useEffect(() => { chainIdRef.current = chainId; }, [chainId]);
  useEffect(() => { accountNameRef.current = accountName; }, [accountName]);
  useEffect(() => { accountsRef.current = state.accounts; }, [state.accounts]);
  useEffect(() => { activeAccountRef.current = activeAccount; }, [activeAccount]);

  // Push wallet info when account/chain changes while connected
  useEffect(() => {
    if (status === 'connected' && transportRef.current?.connected) {
      transportRef.current.pushWalletInfo({
        address,
        chainId,
        name: accountName,
        accounts: state.accounts.map(a => ({ name: a.name, address: a.address })),
      });
    }
  }, [address, chainId, accountName, status, state.accounts]);

  // --- Handle incoming request ---
  const handleIncoming = useCallback((id: string, method: string, params: any[], origin: string) => {
    const addr = addressRef.current;
    const cid = chainIdRef.current;

    if (isSigningMethod(method)) {
      setIncomingRequest({ id, method, params, origin });
      return;
    }

    if (method === 'wallet_switchEthereumChain') {
      const cp = params?.[0] as { chainId?: string } | undefined;
      if (cp?.chainId) {
        const nc = parseInt(cp.chainId, 16);
        if (!isNaN(nc)) { chainIdRef.current = nc; setChainId(nc); }
      }
      transportRef.current?.sendResponse(id, null);
      return;
    }

    handleReadOnlyRPC(method, params, addr, cid).then(res => {
      if (res.handled) transportRef.current?.sendResponse(id, res.result);
      else transportRef.current?.sendResponse(id, undefined, { code: -32603, message: `RPC failed: ${method}` });
    });
  }, []);

  // --- Wire transport events (shared by both transport types) ---
  const wireTransport = useCallback((transport: DAppTransport, type: ConnectionType) => {
    transport.on('connected', () => {
      setStatus('connected');
      setConnectionType(type);
      transport.pushWalletInfo({
        address: addressRef.current,
        chainId: chainIdRef.current,
        name: accountNameRef.current,
        accounts: accountsRef.current.map(a => ({ name: a.name, address: a.address })),
      });
    });

    transport.on('disconnected', () => {
      setStatus('disconnected');
      setConnectionType(null);
      setIncomingRequest(null);
      transportRef.current = null;
    });

    transport.on('request', handleIncoming);

    transport.on('error', (msg) => {
      setErrorMessage(msg);
    });

    transportRef.current = transport;
  }, [handleIncoming]);

  // --- Disconnect any active transport ---
  const disconnectCurrent = useCallback(() => {
    pendingWpTransportRef.current = null;
    setPendingFingerprint(null);
    if (transportRef.current) {
      transportRef.current.disconnect();
      transportRef.current = null;
    }
  }, []);

  // --- Connect (Remote Inject) ---
  const connectToBridge = useCallback(async (sess: RemoteInjectSession) => {
    disconnectCurrent();

    setStatus('connecting');
    setErrorMessage(null);
    setSession(sess);

    const transport = new RemoteInjectTransport(sess);
    wireTransport(transport, 'remote-inject');

    try {
      await transport.connect();
      const [info] = await Promise.all([
        transport.fetchDAppInfo().catch(() => null),
        saveSession(sess),
      ]);
      setDappInfo(info);
    } catch (err: any) {
      setStatus('error');
      setErrorMessage(err.message ?? 'Connection failed');
      transportRef.current = null;
    }
  }, [wireTransport, disconnectCurrent]);

  // --- Connect (WalletPair) ---
  const connectToWalletPair = useCallback(async (uri: string) => {
    disconnectCurrent();

    setStatus('connecting');
    setErrorMessage(null);
    setSession(null);

    try {
      const { fingerprint, dappInfo: info, transport } = WalletPairTransport.prepare(uri);
      setPendingFingerprint(fingerprint);
      setDappInfo(info);
      pendingWpTransportRef.current = transport;
      // Wait for user to call confirmFingerprint()
    } catch (err: any) {
      setStatus('error');
      setErrorMessage(err.message ?? 'Failed to prepare WalletPair session');
    }
  }, [disconnectCurrent]);

  // --- Confirm fingerprint (WalletPair) ---
  const confirmFingerprint = useCallback(async () => {
    const transport = pendingWpTransportRef.current;
    if (!transport) return;

    setPendingFingerprint(null);
    pendingWpTransportRef.current = null;

    wireTransport(transport, 'walletpair');

    try {
      await transport.connect();

      // If still not connected after confirmJoin resolved, start a timeout.
      // The relay may silently drop the join message (e.g. CF Worker hibernation),
      // leaving both sides stuck in waiting_accept with no transport-level error.
      if (!transport.connected) {
        const timeout = setTimeout(() => {
          if (!transport.connected && transportRef.current === transport) {
            setStatus('error');
            setErrorMessage('Connection timed out. The relay may be unavailable — try scanning again.');
            transport.disconnect();
            transportRef.current = null;
          }
        }, 15_000);

        // Clear timeout if connection succeeds before deadline
        const unsub = transport.on('connected', () => {
          clearTimeout(timeout);
          unsub();
        });
      }
    } catch (err: any) {
      setStatus('error');
      setErrorMessage(err.message ?? 'WalletPair connection failed');
      transportRef.current = null;
    }
  }, [wireTransport]);

  // --- Cancel fingerprint verification ---
  const cancelFingerprint = useCallback(() => {
    pendingWpTransportRef.current = null;
    setPendingFingerprint(null);
    setStatus('disconnected');
    setDappInfo(null);
    setErrorMessage(null);
  }, []);

  // --- Disconnect ---
  const disconnectBridge = useCallback(() => {
    disconnectCurrent();
    setStatus('disconnected');
    setConnectionType(null);
    setSession(null);
    setDappInfo(null);
    setIncomingRequest(null);
    clearSession();
    clearWalletPairSession();
  }, [disconnectCurrent]);

  // --- Approve ---
  const approveRequest = useCallback(async () => {
    const request = incomingRequest;
    const account = activeAccountRef.current;
    if (!request || !account) return;

    setIsSigning(true);
    setSignError(null);
    try {
      const cid = chainIdRef.current;
      const result = await handleDAppRequest(request, account, account.address, cid);
      transportRef.current?.sendResponse(request.id, result);

      // Record all dApp signing operations to local history
      const dappOrigin = dappInfo?.name ?? request.origin ?? '';
      const now = Math.floor(Date.now() / 1000);

      if (request.method === 'eth_sendTransaction' && typeof result === 'string') {
        const tx = request.params?.[0] as Record<string, string> | undefined;
        saveTransaction({
          id: `dapp-${now}-tx`,
          userOpHash: '',
          txHash: result,
          from: account.address,
          to: tx?.to ?? '',
          value: tx?.value ?? '0x0',
          symbol: nativeSymbol(cid),
          decimals: 18,
          chainId: cid,
          timestamp: now,
          status: 'confirmed',
          type: 'dapp_tx',
          dappOrigin,
        }).catch(() => {});
      } else if (request.method === 'personal_sign') {
        saveTransaction({
          id: `dapp-${now}-msg`,
          userOpHash: '',
          txHash: '',
          from: account.address,
          to: '',
          value: '0',
          symbol: '',
          decimals: 0,
          chainId: cid,
          timestamp: now,
          status: 'confirmed',
          type: 'sign_message',
          dappOrigin,
        }).catch(() => {});
      } else if (request.method.includes('signTypedData')) {
        saveTransaction({
          id: `dapp-${now}-typed`,
          userOpHash: '',
          txHash: '',
          from: account.address,
          to: '',
          value: '0',
          symbol: '',
          decimals: 0,
          chainId: cid,
          timestamp: now,
          status: 'confirmed',
          type: 'sign_typed_data',
          dappOrigin,
        }).catch(() => {});
      }

      setIncomingRequest(null);
    } catch (err: any) {
      if (err?.code === PasskeyErrorCode.CANCELLED) {
        // User cancelled passkey prompt — keep modal open, don't send error
        setIsSigning(false);
        return;
      }
      const msg = err.message ?? 'Signing failed';
      console.error('[DAppConnection] Request failed:', msg);
      setSignError(msg);
      transportRef.current?.sendResponse(request.id, undefined, { code: -32603, message: msg });
      // Keep modal open so user can see the error — they dismiss manually
    } finally {
      setIsSigning(false);
    }
  }, [incomingRequest]);

  // --- Reject ---
  const rejectRequest = useCallback(() => {
    if (!incomingRequest) return;
    transportRef.current?.sendResponse(incomingRequest.id, undefined, { code: 4001, message: 'User rejected' });
    setIncomingRequest(null);
    setSignError(null);
  }, [incomingRequest]);

  // --- Dismiss (after error, response already sent) ---
  const dismissRequest = useCallback(() => {
    setIncomingRequest(null);
    setSignError(null);
  }, []);

  // --- Switch chain ---
  const switchChain = useCallback((newChainId: number) => {
    setChainId(newChainId);
    chainIdRef.current = newChainId;
    if (transportRef.current?.connected) {
      // Notify dApp of chain change via wallet info push
      transportRef.current.pushWalletInfo({
        address: addressRef.current,
        chainId: newChainId,
        name: accountNameRef.current,
        accounts: accountsRef.current.map(a => ({ name: a.name, address: a.address })),
      });
    }
  }, []);

  // --- Auto-reconnect on mount ---
  useEffect(() => {
    if (!state.hasWallet || state.isLoading) return;

    // Try Remote Inject first, then WalletPair
    (async () => {
      const sess = await loadSession();
      if (sess) {
        // Try auto-reconnect — on failure, clear stale session silently
        const transport = new RemoteInjectTransport(sess);
        wireTransport(transport, 'remote-inject');
        try {
          await transport.connect();
          setSession(sess);
          const info = await transport.fetchDAppInfo().catch(() => null);
          setDappInfo(info);
          await saveSession(sess);
        } catch {
          // Stale session — clean up silently, don't show error
          transport.disconnect();
          transportRef.current = null;
          await clearSession();
        }
        return;
      }

      // Try restoring a WalletPair session
      try {
        const wpTransport = await WalletPairTransport.restore();
        if (wpTransport) {
          wireTransport(wpTransport, 'walletpair');
          const info = await wpTransport.fetchDAppInfo();
          setDappInfo(info);
          try {
            await wpTransport.reconnect();
          } catch {
            // Reconnect failed — session will retry via SDK backoff
            // or phase listener will set status to disconnected
          }
        }
      } catch {
        // WalletPair restore failed — clean up
        clearWalletPairSession();
      }
    })();

    return () => {
      transportRef.current?.disconnect();
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.hasWallet, state.isLoading]);

  const value = React.useMemo(() => ({
    status, errorMessage, session, dappInfo,
    incomingRequest, isSigning, signError, chainId,
    connectionType, pendingFingerprint,
    connectToBridge, connectToWalletPair, confirmFingerprint, cancelFingerprint,
    disconnectBridge,
    approveRequest, rejectRequest, dismissRequest, switchChain,
  }), [
    status, errorMessage, session, dappInfo,
    incomingRequest, isSigning, signError, chainId,
    connectionType, pendingFingerprint,
    connectToBridge, connectToWalletPair, confirmFingerprint, cancelFingerprint,
    disconnectBridge,
    approveRequest, rejectRequest, dismissRequest, switchChain,
  ]);

  return (
    <DAppConnectionContext.Provider value={value}>
      {children}
    </DAppConnectionContext.Provider>
  );
}
