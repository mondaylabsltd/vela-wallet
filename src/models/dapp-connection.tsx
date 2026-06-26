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
  clearWalletPairSession,
} from '@/services/walletpair-transport';
import { isSigningMethod, handleDAppRequest, handleReadOnlyRPC, extractRequestChainId, assertChainSupported, INSTANT_READONLY_METHODS } from '@/hooks/use-dapp-signing';
import { gateReadOnly, readOnlyKey } from '@/services/readonly-rpc-gate';
import { PasskeyErrorCode } from '@/modules/passkey';
import { saveTransaction } from '@/services/storage';
import { buildSigningRecord } from '@/services/dapp-history';
import {
  fetchBundlerAccountInfo,
  clearBundlerCache,
  checkBundlerFunding,
  parseBundlerUnderfunded,
  recommendedFundingWei,
  formatWei,
  type FundingNeeded,
} from '@/services/bundler-service';
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

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'reconnecting' | 'error';
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
  /** UserOp hash once a tx is submitted, while awaiting the on-chain receipt. */
  pendingOpHash: string | null;
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
  /** Force an immediate reconnect of the active session ("Reconnect now"). */
  reconnect: () => void;
  /** True once an auto-reconnect has dragged on long enough to prompt the user. */
  reconnectStuck: boolean;
  /**
   * Approve the current incoming request. For transactions the modal passes the
   * selected tier's maxFeePerGas plus the raw bundler gas cost (for the funding
   * pre-check) and, for edited approvals, the rewritten (capped) params.
   */
  approveRequest: (opts?: { maxFeePerGas?: bigint; bundlerCostWei?: bigint; paramsOverride?: any[] }) => Promise<void>;
  /** Reject the current incoming request. */
  rejectRequest: () => void;
  /** Dismiss the modal after an error (response already sent). */
  dismissRequest: () => void;
  /** Switch chain for the bridge connection. */
  switchChain: (chainId: number) => void;
  /** Bundler funding needed (gas account underfunded during dApp tx). */
  fundingNeeded: FundingNeeded | null;
  /** Called when user has funded the gas account. Retries the pending request. */
  handleFundingComplete: () => void;
  /** Called when user cancels funding. Rejects the pending request. */
  handleFundingCancel: () => void;
}

const DAppConnectionContext = createContext<DAppConnectionContextValue>({
  status: 'disconnected',
  errorMessage: null,
  session: null,
  dappInfo: null,
  incomingRequest: null,
  isSigning: false,
  signError: null,
  pendingOpHash: null,
  chainId: 1,
  connectionType: null,
  pendingFingerprint: null,
  connectToBridge: async () => {},
  connectToWalletPair: async () => {},
  confirmFingerprint: async () => {},
  cancelFingerprint: () => {},
  disconnectBridge: () => {},
  reconnect: () => {},
  reconnectStuck: false,
  approveRequest: async () => {},
  rejectRequest: () => {},
  dismissRequest: () => {},
  switchChain: () => {},
  fundingNeeded: null,
  handleFundingComplete: () => {},
  handleFundingCancel: () => {},
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
  const [pendingOpHash, setPendingOpHash] = useState<string | null>(null);
  const [chainId, setChainId] = useState(1);
  const [connectionType, setConnectionType] = useState<ConnectionType>(null);
  const [pendingFingerprint, setPendingFingerprint] = useState<string | null>(null);
  const [fundingNeeded, setFundingNeeded] = useState<FundingNeeded | null>(null);
  const [reconnectStuck, setReconnectStuck] = useState(false);
  // Bumped on each manual "Reconnect now" so the stuck timer re-arms even though
  // `status` stays 'reconnecting' (a same-value setState wouldn't re-run the effect).
  const [reconnectNonce, setReconnectNonce] = useState(0);

  // If an auto-reconnect drags on (relay down / session expired), surface a
  // manual-recovery prompt instead of spinning "Reconnecting…" forever.
  useEffect(() => {
    if (status !== 'reconnecting') { setReconnectStuck(false); return; }
    setReconnectStuck(false);
    const timer = setTimeout(() => setReconnectStuck(true), 45_000);
    return () => clearTimeout(timer);
  }, [status, reconnectNonce]);

  const transportRef = useRef<DAppTransport | null>(null);
  /** Holds WalletPairTransport during fingerprint verification (before connect). */
  const pendingWpTransportRef = useRef<WalletPairTransport | null>(null);
  const lastApproveOptsRef = useRef<{ maxFeePerGas?: bigint; bundlerCostWei?: bigint; paramsOverride?: any[] } | undefined>(undefined);
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
      // Extract chain ID embedded in the request (e.g. typedData.domain.chainId, tx.chainId)
      const requestChainId = extractRequestChainId(method, params);
      if (requestChainId != null && requestChainId !== chainIdRef.current) {
        try {
          assertChainSupported(requestChainId);
        } catch (err: any) {
          // Wallet doesn't support this chain — reject immediately
          transportRef.current?.sendResponse(id, undefined, {
            code: err.code ?? 4902,
            message: err.message ?? `Unsupported chain: ${requestChainId}`,
          });
          return;
        }
        // Auto-switch wallet chain to match request
        chainIdRef.current = requestChainId;
        setChainId(requestChainId);
      }
      setIncomingRequest({ id, method, params, origin });
      return;
    }

    if (method === 'wallet_switchEthereumChain') {
      const cp = params?.[0] as { chainId?: string } | undefined;
      const nc = cp?.chainId
        ? (cp.chainId.startsWith('0x') ? parseInt(cp.chainId, 16) : parseInt(cp.chainId, 10))
        : NaN;
      if (isNaN(nc)) {
        // Missing/malformed chainId — don't report a phantom success.
        transportRef.current?.sendResponse(id, undefined, { code: -32602, message: 'Invalid params: missing chainId' });
        return;
      }
      try {
        assertChainSupported(nc);
      } catch (err: any) {
        transportRef.current?.sendResponse(id, undefined, {
          code: err.code ?? 4902,
          message: err.message ?? `Unsupported chain: ${nc}`,
        });
        return;
      }
      chainIdRef.current = nc;
      setChainId(nc);
      transportRef.current?.sendResponse(id, null);
      return;
    }

    // Network-bound reads go through the dedupe + concurrency gate so a flood
    // can't starve the signing path; instant local methods bypass it.
    const dispatch = INSTANT_READONLY_METHODS.has(method)
      ? handleReadOnlyRPC(method, params, addr, cid)
      : gateReadOnly(readOnlyKey(cid, addr, method, params), () => handleReadOnlyRPC(method, params, addr, cid));
    dispatch.then(res => {
      if (res.handled) transportRef.current?.sendResponse(id, res.result);
      else transportRef.current?.sendResponse(id, undefined, { code: -32603, message: `RPC failed: ${method}` });
    }).catch((err: any) => {
      // Gate overflow (too many concurrent reads) — answer with a retryable error.
      transportRef.current?.sendResponse(id, undefined, { code: err?.code ?? -32603, message: err?.message ?? `RPC failed: ${method}` });
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

    transport.on('reconnecting', () => {
      // Transient disconnect — SDK is auto-reconnecting.
      // Keep transport ref and dApp info intact; just update the status indicator.
      setStatus('reconnecting');
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
        }, 120_000);

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

  // --- Manual reconnect ("Reconnect now") ---
  const reconnect = useCallback(() => {
    const transport = transportRef.current;
    if (!transport) return;
    setStatus('reconnecting');
    setReconnectStuck(false);
    setReconnectNonce((n) => n + 1); // re-arm the stuck timer even if status was already 'reconnecting'
    transport.reconnect?.().catch(() => { /* SDK keeps retrying; UI stays reconnecting */ });
  }, []);

  // --- Approve ---
  const approveRequest = useCallback(async (opts?: { maxFeePerGas?: bigint; bundlerCostWei?: bigint; paramsOverride?: any[] }) => {
    const base = incomingRequest;
    const account = activeAccountRef.current;
    if (!base || !account) return;

    // The modal may hand us rewritten params (e.g. an approval capped to a finite
    // amount). Sign/submit/record THOSE, never the original unbounded request.
    const request = opts?.paramsOverride ? { ...base, params: opts.paramsOverride } : base;

    // Remember opts so a funding-driven retry resubmits the SAME (capped) request.
    lastApproveOptsRef.current = opts;

    const cid = chainIdRef.current;

    // Proactive gas-account pre-check — mirror the Send flow so the top-up modal
    // appears BEFORE the passkey prompt + submit, not after a failed UserOp. Raced
    // with a timeout so a slow RPC can't hang approval; on timeout/error we fall
    // through to submit and the post-submit catch below is the safety net.
    if (request.method === 'eth_sendTransaction') {
      try {
        const funding = await Promise.race([
          checkBundlerFunding(cid, account.address, opts?.bundlerCostWei),
          new Promise<FundingNeeded | null>(resolve => setTimeout(() => resolve(null), 15_000)),
        ]);
        if (funding) {
          setFundingNeeded(funding); // keep request pending; retried after funding
          return;
        }
      } catch { /* proceed to submit */ }
    }

    setIsSigning(true);
    setSignError(null);
    setPendingOpHash(null);
    try {
      const result = await handleDAppRequest(
        request, account, account.address, cid, opts?.maxFeePerGas,
        // Surface the hash the moment the op is submitted so the modal can show
        // "submitted, waiting for confirmation" instead of a silent spinner.
        (hash) => setPendingOpHash(hash),
      );
      transportRef.current?.sendResponse(request.id, result);

      // Record EVERY approved dApp operation to local history (see dapp-history)
      // so the Connections panel shows it and its detail. Awaited before clearing
      // the request so the panel's refresh reads up-to-date storage.
      const record = buildSigningRecord({
        method: request.method,
        params: request.params,
        result,
        from: account.address,
        chainId: cid,
        dappOrigin: dappInfo?.name ?? request.origin ?? '',
        nowMs: Date.now(),
      });
      await saveTransaction(record).catch(e => console.warn('[DAppConnection] Failed to save record:', e));

      setIncomingRequest(null);
      setPendingOpHash(null);
    } catch (err: any) {
      if (err?.code === PasskeyErrorCode.CANCELLED) {
        // User cancelled passkey prompt — keep modal open, don't send error
        setIsSigning(false);
        return;
      }

      const msg = err.message ?? 'Signing failed';

      // Gas account underfunded — open the funding modal instead of dumping the
      // raw "Deposit to: 0x… required: …" error at the user (mirrors the Send
      // flow's top-up UX). Detection is wording-tolerant; the deposit address and
      // amounts are read from the bundler's message so we can still show the modal
      // even if the follow-up account lookup fails.
      const underfunded = parseBundlerUnderfunded(msg);
      if (underfunded) {
        console.log('[DAppConnection] Bundler needs funding, showing modal');
        setIsSigning(false);
        try {
          const addr = account.address;
          clearBundlerCache(cid, addr);
          const info = await fetchBundlerAccountInfo(cid, addr);
          // Prefer live account info; fall back to the values parsed from the error.
          const depositAddress = info?.depositAddress || underfunded.depositAddress;
          if (depositAddress) {
            const currentBalance = info?.spendableBalance ?? underfunded.spendableWei ?? 0n;
            const thresholdWei = underfunded.requiredWei ?? currentBalance + 100_000_000_000_000n;
            const recommendedWei = recommendedFundingWei(thresholdWei, currentBalance);
            const nativeSym = info?.nativeSym ?? (underfunded.asset === 'pathUSD' ? 'pathUSD' : nativeSymbol(cid));
            setFundingNeeded({
              reason: 'deposit_needed',
              sponsorshipAvailable: true,
              depositAddress,
              safeAddress: addr,
              chainId: cid,
              nativeSym,
              thresholdWei,
              recommendedWei,
              currentBalance,
              recommendedFormatted: formatWei(recommendedWei),
              currentFormatted: formatWei(currentBalance),
            });
            return; // Don't send error to dApp — keep request pending
          }
        } catch { /* fall through to generic error */ }
      }

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
    setPendingOpHash(null);
  }, [incomingRequest]);

  // --- Dismiss (after error, response already sent) ---
  const dismissRequest = useCallback(() => {
    setIncomingRequest(null);
    setSignError(null);
    setPendingOpHash(null);
  }, []);

  // --- Bundler funding complete → retry the pending request ---
  const handleFundingComplete = useCallback(() => {
    setFundingNeeded(null);
    // Drop the cached (stale, underfunded) balance so the pre-check on retry reads
    // the freshly-funded amount instead of re-prompting.
    const account = activeAccountRef.current;
    if (account) clearBundlerCache(chainIdRef.current, account.address);
    // Retry approve with the SAME opts (esp. the capped paramsOverride) so funding
    // never resubmits the original (possibly unbounded) request.
    approveRequest(lastApproveOptsRef.current);
  }, [approveRequest]);

  // --- Bundler funding cancelled → reject the pending request ---
  const handleFundingCancel = useCallback(() => {
    setFundingNeeded(null);
    if (incomingRequest) {
      transportRef.current?.sendResponse(incomingRequest.id, undefined, { code: -32603, message: 'Gas account funding cancelled' });
      setIncomingRequest(null);
    }
  }, [incomingRequest]);

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
    incomingRequest, isSigning, signError, pendingOpHash, chainId,
    connectionType, pendingFingerprint,
    connectToBridge, connectToWalletPair, confirmFingerprint, cancelFingerprint,
    disconnectBridge, reconnect, reconnectStuck,
    approveRequest, rejectRequest, dismissRequest, switchChain,
    fundingNeeded, handleFundingComplete, handleFundingCancel,
  }), [
    status, errorMessage, session, dappInfo,
    incomingRequest, isSigning, signError, pendingOpHash, chainId,
    connectionType, pendingFingerprint,
    connectToBridge, connectToWalletPair, confirmFingerprint, cancelFingerprint,
    disconnectBridge, reconnect, reconnectStuck,
    approveRequest, rejectRequest, dismissRequest, switchChain,
    fundingNeeded, handleFundingComplete, handleFundingCancel,
  ]);

  return (
    <DAppConnectionContext.Provider value={value}>
      {children}
    </DAppConnectionContext.Provider>
  );
}
