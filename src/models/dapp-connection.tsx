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
import { saveTransaction, updateTransaction, loadTransactions } from '@/services/storage';
import { buildSigningRecord } from '@/services/dapp-history';
import { serializeAssetSim, type AssetSimResult } from '@/services/tx-simulation';
import { waitForReceipt } from '@/services/safe-transaction';
import {
  attemptSilentSponsorship,
  fetchBundlerAccountInfo,
  clearBundlerCache,
  checkBundlerFunding,
  parseBundlerUnderfunded,
  recommendedFundingWei,
  underfundedRequiredWei,
  formatWei,
  type FundingNeeded,
} from '@/services/bundler-service';
import { nativeSymbol } from '@/models/network';
import type { BLEIncomingRequest } from '@/models/types';
import { responseTransport, requestChainId as reqChainId, requestDApp } from '@/models/dapp-request-routing';

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'vela.remoteInjectSession';

/**
 * Grace window before an automatic reconnect is surfaced in the UI. A relay blip
 * — the dApp momentarily blurring/reloading, a `channel_not_found` while it
 * re-establishes its own socket — usually self-heals within ~1s, so we keep the
 * connection shown as active for this long and only flip to "Reconnecting…" if it
 * hasn't recovered by then. Manual "Reconnect now" taps bypass this (the user
 * pressed it and wants immediate feedback).
 */
const RECONNECT_GRACE_MS = 4000;

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
  /** True once the passkey/submit phase has STARTED (past the gas pre-check). At
   *  this point the tx is committed — a swipe-dismiss must dismiss (the op proceeds),
   *  never reject, or a "cancelled" tx would still broadcast (BUG-2 submit window). */
  isSubmitting: boolean;
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
  /**
   * Begin a Safari-extension sign: install a one-shot ExtensionBridgeTransport
   * into the transient sign slot (never clobbers a live WalletPair/bridge session)
   * and render the real SigningRequestModal for it. Used only by src/app/sign.tsx.
   */
  beginExtensionSign: (transport: DAppTransport) => void;
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
  isSubmitting: false,
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
  beginExtensionSign: () => {},
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
  // Synchronous mirror for guards inside long-running async recovery (the
  // reactive silent-sponsorship attempt can take ~25s — the request on the
  // sheet may have changed by the time it resolves).
  const incomingRequestRef = useRef<BLEIncomingRequest | null>(null);
  useEffect(() => { incomingRequestRef.current = incomingRequest; }, [incomingRequest]);
  const [isSigning, setIsSigning] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
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

  // Holds the grace-window timer that debounces the "Reconnecting…" indicator, so
  // a brief, self-healing reconnect never flickers the UI off "connected".
  const reconnectGraceTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clearReconnectGrace = useCallback(() => {
    if (reconnectGraceTimer.current) { clearTimeout(reconnectGraceTimer.current); reconnectGraceTimer.current = null; }
  }, []);
  // Don't let a pending grace timer fire setStatus after the provider unmounts.
  useEffect(() => () => clearReconnectGrace(), [clearReconnectGrace]);

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
  /**
   * Transient slot for a Safari-extension sign transport (beginExtensionSign).
   * SEPARATE from transportRef so a live WalletPair/bridge session is NOT clobbered
   * by an extension sign. Responses route per-request (incomingRequest.__transport),
   * never through this ref, so concurrency can't misroute a signature.
   */
  const signTransportRef = useRef<DAppTransport | null>(null);
  const lastApproveOptsRef = useRef<{ maxFeePerGas?: bigint; bundlerCostWei?: bigint; paramsOverride?: any[]; assetSim?: AssetSimResult | null } | undefined>(undefined);
  // The request id the pending funding view belongs to. lastApproveOptsRef is a
  // single shared ref; without pinning the rid, a funding "Continue" could replay
  // the OLD request's (capped) opts under a DIFFERENT request that has since taken
  // the sheet — submitting the wrong params under the wrong id. handleFundingComplete
  // bails if the current request no longer matches.
  const fundingRidRef = useRef<string | null>(null);
  // Fund-safety guards on the single submit path (approveRequest):
  //  - approveInFlightRef: synchronous re-entrancy lock so a double-tap (Approve or
  //    the funding "Continue") can't fire two concurrent approves → two submits
  //    (BUG-3). isSigning is async React state, useless for a same-tick second tap.
  //  - signCancelledRef: set by rejectRequest so a reject/swipe DURING an in-flight
  //    approve (e.g. the ≤15s gas pre-check) aborts before submit — never a "rejected"
  //    tx that still broadcasts + a contradictory success response (BUG-2).
  const approveInFlightRef = useRef(false);
  const signCancelledRef = useRef(false);
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
  const handleIncoming = useCallback((
    id: string,
    method: string,
    params: any[],
    origin: string,
    meta?: { transport?: DAppTransport; chainId?: number; dapp?: DAppInfo },
  ) => {
    const addr = addressRef.current;
    const cid = chainIdRef.current;
    // The transport that OWNS this request. Responses MUST route here, never a
    // shared transportRef — with a concurrent WalletPair session live, using
    // transportRef would deliver an extension signature over the WP socket (F2).
    const owner = meta?.transport ?? transportRef.current;

    if (isSigningMethod(method)) {
      // A fresh signing request supersedes any funding prompt left over from a
      // prior request (e.g. one abandoned mid-pre-check) — the sheet swaps to the
      // funding view on `fundingNeeded`, so a stale value would hijack this sheet.
      setFundingNeeded(null);
      if (meta?.chainId != null) {
        // EXTENSION sign: chain is per-request (F4). Do NOT touch the global
        // chainId — it is shared with any live WalletPair session. Validate, then
        // stamp the owning transport + chain + identity on the request so the
        // sheet, sign, response and history are fully self-contained (F2/F3/F4).
        try {
          assertChainSupported(meta.chainId);
        } catch (err: any) {
          owner?.sendResponse(id, undefined, { code: err.code ?? 4902, message: err.message ?? `Unsupported chain: ${meta.chainId}` });
          return;
        }
        setIncomingRequest({ id, method, params, origin, __transport: meta.transport, __chainId: meta.chainId, __dapp: meta.dapp });
        return;
      }
      // Ordinary bridge/WalletPair request — auto-switch the global chain to match
      // an embedded request chainId (typedData.domain.chainId, tx.chainId, …).
      const requestChainId = extractRequestChainId(method, params);
      if (requestChainId != null && requestChainId !== chainIdRef.current) {
        try {
          assertChainSupported(requestChainId);
        } catch (err: any) {
          owner?.sendResponse(id, undefined, { code: err.code ?? 4902, message: err.message ?? `Unsupported chain: ${requestChainId}` });
          return;
        }
        chainIdRef.current = requestChainId;
        setChainId(requestChainId);
      }
      setIncomingRequest({ id, method, params, origin, __transport: meta?.transport });
      return;
    }

    if (method === 'wallet_switchEthereumChain') {
      const cp = params?.[0] as { chainId?: string } | undefined;
      const nc = cp?.chainId
        ? (cp.chainId.startsWith('0x') ? parseInt(cp.chainId, 16) : parseInt(cp.chainId, 10))
        : NaN;
      if (isNaN(nc)) {
        // Missing/malformed chainId — don't report a phantom success.
        owner?.sendResponse(id, undefined, { code: -32602, message: 'Invalid params: missing chainId' });
        return;
      }
      try {
        assertChainSupported(nc);
      } catch (err: any) {
        owner?.sendResponse(id, undefined, {
          code: err.code ?? 4902,
          message: err.message ?? `Unsupported chain: ${nc}`,
        });
        return;
      }
      chainIdRef.current = nc;
      setChainId(nc);
      owner?.sendResponse(id, null);
      return;
    }

    // Network-bound reads go through the dedupe + concurrency gate so a flood
    // can't starve the signing path; instant local methods bypass it.
    const dispatch = INSTANT_READONLY_METHODS.has(method)
      ? handleReadOnlyRPC(method, params, addr, cid)
      : gateReadOnly(readOnlyKey(cid, addr, method, params), () => handleReadOnlyRPC(method, params, addr, cid));
    dispatch.then(res => {
      if (res.handled) owner?.sendResponse(id, res.result);
      else owner?.sendResponse(id, undefined, { code: -32603, message: `RPC failed: ${method}` });
    }).catch((err: any) => {
      // Gate overflow (too many concurrent reads) — answer with a retryable error.
      owner?.sendResponse(id, undefined, { code: err?.code ?? -32603, message: err?.message ?? `RPC failed: ${method}` });
    });
  }, []);

  // --- Wire transport events (shared by both transport types) ---
  const wireTransport = useCallback((transport: DAppTransport, type: ConnectionType) => {
    transport.on('connected', () => {
      // Recovered (possibly within the grace window) — cancel any pending
      // "Reconnecting…" flip so a self-healing blip never showed at all.
      clearReconnectGrace();
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
      clearReconnectGrace();
      setStatus('disconnected');
      setConnectionType(null);
      // Owner-aware: only clear a request THIS transport owns. Otherwise a terminal
      // WalletPair/bridge drop would tear down a concurrent extension sign's modal
      // (which lives in the same shared incomingRequest but is owned by the ext
      // transport). Mirrors the per-request sendResponse routing. Fund-safe either
      // way (no sendResponse fires here), but this keeps the sign UI on screen.
      setIncomingRequest((prev) => (prev && prev.__transport && prev.__transport !== transport ? prev : null));
      transportRef.current = null;
    });

    transport.on('reconnecting', () => {
      // Transient disconnect — SDK is auto-reconnecting. Keep transport ref and
      // dApp info intact. Don't flip the indicator immediately: hold "connected"
      // for the grace window so a sub-second blip self-heals invisibly, and only
      // surface "Reconnecting…" if it hasn't recovered by then. Already-pending
      // timer is left to run (don't extend the window on repeated blips).
      if (reconnectGraceTimer.current) return;
      reconnectGraceTimer.current = setTimeout(() => {
        reconnectGraceTimer.current = null;
        setStatus('reconnecting');
      }, RECONNECT_GRACE_MS);
    });

    // Stamp the OWNING transport on every inbound request so responses route back
    // to it, not a shared ref (matters once a second transport — the extension
    // sign slot — can be live at the same time; see F2).
    transport.on('request', (id, method, params, origin) => handleIncoming(id, method, params, origin, { transport }));

    transport.on('error', (msg) => {
      setErrorMessage(msg);
    });

    transportRef.current = transport;
  }, [handleIncoming, clearReconnectGrace]);

  // --- Disconnect any active transport ---
  const disconnectCurrent = useCallback(() => {
    clearReconnectGrace();
    pendingWpTransportRef.current = null;
    setPendingFingerprint(null);
    if (transportRef.current) {
      transportRef.current.disconnect();
      transportRef.current = null;
    }
  }, [clearReconnectGrace]);

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
    clearReconnectGrace(); // manual tap → show "Reconnecting…" now, don't wait out the grace
    setStatus('reconnecting');
    setReconnectStuck(false);
    setReconnectNonce((n) => n + 1); // re-arm the stuck timer even if status was already 'reconnecting'
    transport.reconnect?.().catch(() => { /* SDK keeps retrying; UI stays reconnecting */ });
  }, [clearReconnectGrace]);

  // --- Begin an extension sign (Safari extension → App Group) ---
  // Installs `transport` into the TRANSIENT signTransportRef, NOT transportRef, and
  // does NOT call disconnectCurrent — so a live WalletPair/bridge session survives.
  // It wires ONLY 'request' (stamping the owning transport + per-request chain +
  // identity onto incomingRequest for F2/F3/F4) and a scoped, identity-guarded
  // 'disconnected' that clears just its own slot — NEVER incomingRequest, NEVER
  // transportRef. Deliberately not wireTransport(), whose 'disconnected' handler
  // would null transportRef + clear incomingRequest mid-sign.
  const beginExtensionSign = useCallback((transport: DAppTransport) => {
    transport.on('request', (id, method, params, origin) => {
      let host = origin;
      try { host = new URL(origin).host || origin; } catch { /* keep origin */ }
      handleIncoming(id, method, params, origin, {
        transport,
        chainId: (transport as { requestChainId?: number }).requestChainId,
        dapp: { name: host, url: origin },
      });
    });
    transport.on('disconnected', () => {
      if (signTransportRef.current === transport) signTransportRef.current = null;
    });
    transport.on('error', (msg) => setErrorMessage(msg));
    signTransportRef.current = transport;
  }, [handleIncoming]);

  // --- Approve ---
  const approveRequest = useCallback(async (opts?: { maxFeePerGas?: bigint; bundlerCostWei?: bigint; paramsOverride?: any[]; assetSim?: AssetSimResult | null }) => {
    const base = incomingRequest;
    const account = activeAccountRef.current;
    if (!base || !account) return;

    // Re-entrancy lock: a rapid second tap (Approve, or the funding "Continue")
    // must not start a SECOND concurrent approve → two passkey prompts / two
    // submits (BUG-3). isSigning is async React state — it hasn't flipped yet on a
    // same-tick second tap — so guard on a synchronous ref. Released on every exit.
    if (approveInFlightRef.current) return;
    approveInFlightRef.current = true;
    signCancelledRef.current = false; // fresh approve — not (yet) cancelled

    // The modal may hand us rewritten params (e.g. an approval capped to a finite
    // amount). Sign/submit/record THOSE, never the original unbounded request.
    const request = opts?.paramsOverride ? { ...base, params: opts.paramsOverride } : base;

    // The "what moved" preview the sheet just showed — persisted (JSON-safe) on the
    // record so the Connections-panel replay can render it without re-simulating.
    const assetChanges = opts?.assetSim ? serializeAssetSim(opts.assetSim) : undefined;

    // Remember opts so a funding-driven retry resubmits the SAME (capped) request.
    lastApproveOptsRef.current = opts;

    // Per-request chain for an extension sign (F4): sign against the origin's
    // granted chain, NOT the global provider chain (which a concurrent WalletPair
    // session owns). Ordinary requests carry no __chainId → use the global chain.
    const cid = reqChainId(base, chainIdRef.current);

    // Immediate feedback: the gas pre-check below can take up to 15s and the sign
    // is async — flip to the signing state the instant the user taps so Approve is
    // never a silent dead zone (BUG-1 secondary). Cleared again below if we hand
    // off to the funding view.
    setIsSigning(true);
    setSignError(null);
    setPendingOpHash(null);

    // Proactive gas-account pre-check — mirror the Send flow so funding is
    // resolved BEFORE the passkey prompt + submit, not after a failed UserOp.
    // Raced with a timeout so a slow RPC can't hang approval; on timeout/error
    // we fall through to submit and the post-submit catch below is the safety
    // net. Covers wallet_sendCalls too (EIP-5792 batches previously only hit
    // the reactive path after a doomed submit).
    if (request.method === 'eth_sendTransaction' || request.method === 'wallet_sendCalls') {
      try {
        const funding = await Promise.race([
          checkBundlerFunding(cid, account.address, opts?.bundlerCostWei),
          new Promise<FundingNeeded | null>(resolve => setTimeout(() => resolve(null), 15_000)),
        ]);
        if (funding) {
          // Silent sponsorship first: the approve tap IS the commitment moment
          // on the dApp path (the passkey prompt and submit follow immediately,
          // so the grant starts recouping via the settlement split right away).
          // Only a non-funded outcome surfaces any UI.
          const silent = await attemptSilentSponsorship(funding);
          if (signCancelledRef.current) {
            setIsSigning(false);
            approveInFlightRef.current = false;
            return;
          }
          if (silent.outcome !== 'funded') {
            // Hand off to the funding view — rendered IN-SHEET (BUG-1 primary),
            // not stacked over the sheet. Drop the signing spinner; the request
            // stays pending and handleFundingComplete retries it after top-up.
            setIsSigning(false);
            setFundingNeeded(
              silent.outcome === 'confirming'
                ? { ...funding, presentation: 'confirming' }
                : { ...funding, presentation: 'topup', denialReason: silent.denialReason },
            );
            fundingRidRef.current = request.id; // pin funding to THIS request
            approveInFlightRef.current = false; // released — the funding retry re-acquires
            return;
          }
        }
      } catch { /* proceed to submit */ }
    }

    // Abort if the user rejected / swipe-dismissed DURING the (async) gas pre-check:
    // rejectRequest already sent 4001 and cleared the request, so submitting now
    // would broadcast a "rejected" tx and send a contradictory success response for
    // the same id (BUG-2). Checked here — after the only pre-submit await.
    if (signCancelledRef.current) {
      setIsSigning(false);
      approveInFlightRef.current = false;
      return;
    }

    // Entering the passkey/submit phase: the tx is now committed once the user
    // authenticates. From here a swipe-dismiss must DISMISS (the op proceeds + its
    // real result is delivered), never reject — else a "cancelled" tx would still
    // broadcast + send a contradictory success (BUG-2 submit window). onClose reads
    // this to route to dismissRequest. Reset in the finally.
    setIsSubmitting(true);

    // For eth_sendTransaction, the op is recorded 'pending' the moment the bundler
    // accepts it (in onSubmitted) — BEFORE the long on-chain receipt wait — so the
    // Connections panel shows it immediately and closing the sheet (or reloading
    // the page) can't lose its status. We patch it to confirmed/failed below.
    let pendingRecordId: string | null = null;
    let pendingSave: Promise<void> = Promise.resolve();
    // Per-request dApp identity for an extension sign (F3) — the extension origin,
    // never a concurrent WalletPair session's dappInfo.
    const recordOrigin = requestDApp(base, dappInfo)?.name ?? request.origin ?? '';
    try {
      const result = await handleDAppRequest(
        request, account, account.address, cid, opts?.maxFeePerGas,
        // Surface the hash the moment the op is submitted so the modal can show
        // "submitted, waiting for confirmation" — and persist it so the pending
        // state survives the sheet closing.
        (hash) => {
          setPendingOpHash(hash);
          const pending = buildSigningRecord({
            method: request.method, params: request.params, result: '',
            from: account.address, chainId: cid, dappOrigin: recordOrigin,
            nowMs: Date.now(), status: 'pending', userOpHash: hash, assetChanges,
          });
          pendingRecordId = pending.id;
          pendingSave = saveTransaction(pending).catch(e => console.warn('[DAppConnection] Failed to save pending record:', e));
        },
      );
      // §4: the DURABLE, app-owned record must precede the result the extension
      // polls, for EVERY method. eth_sendTransaction already persisted its pending
      // record in onSubmitted (above) before this point; signatures/batches (no
      // onSubmitted) are persisted HERE, before sendResponse, so the extension's
      // result file never lands before Vela Activity has the record.
      if (!pendingRecordId) {
        const record = buildSigningRecord({
          method: request.method,
          params: request.params,
          result,
          from: account.address,
          chainId: cid,
          dappOrigin: recordOrigin,
          nowMs: Date.now(),
          assetChanges,
        });
        await saveTransaction(record).catch(e => console.warn('[DAppConnection] Failed to save record:', e));
      }

      // Route the response to the transport that OWNS the request (per-request,
      // F2) — never a shared transportRef that a concurrent WalletPair session
      // could be sitting on.
      responseTransport(base, transportRef.current)?.sendResponse(request.id, result);

      // For txs, flip the already-persisted pending record to confirmed in place
      // (same id) — never a second record, never a lost pending→confirmed race.
      if (pendingRecordId) {
        await pendingSave;
        await updateTransaction(pendingRecordId, {
          status: 'confirmed',
          txHash: typeof result === 'string' ? result : '',
        }).catch(e => console.warn('[DAppConnection] Failed to confirm record:', e));
      }

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
        console.log('[DAppConnection] Bundler needs funding');
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
            const funding: FundingNeeded = {
              depositAddress,
              safeAddress: addr,
              chainId: cid,
              nativeSym,
              thresholdWei: underfundedRequiredWei(underfunded) ?? thresholdWei,
              recommendedWei,
              currentBalance,
              recommendedFormatted: formatWei(recommendedWei),
              currentFormatted: formatWei(currentBalance),
            };
            // The user may have rejected while the account lookup ran — a
            // treasury grant must not fire for an abandoned request.
            if (signCancelledRef.current) return;
            // Try to heal silently before asking the user for anything (the
            // typical reactive cause is a gas spike past the funded float).
            // Success shows the sheet's confirming state, whose first poll
            // flips to the funded beat and replays this request.
            const silent = await attemptSilentSponsorship(funding, { force: true });
            // The recovery took a while — only surface the funding sheet if
            // the user hasn't rejected meanwhile (4001 already sent) and THIS
            // request still owns the sheet; a late sheet over a NEWER request
            // would cancel the wrong one (BUG-2 family).
            if (signCancelledRef.current) return;
            if (incomingRequestRef.current && incomingRequestRef.current.id !== request.id) {
              // A new request took the slot — fall through to the generic
              // error response for THIS request instead of hijacking the UI.
              throw err;
            }
            setFundingNeeded(
              silent.outcome === 'denied'
                ? { ...funding, presentation: 'topup', denialReason: silent.denialReason }
                : { ...funding, presentation: 'confirming' },
            );
            fundingRidRef.current = request.id; // pin funding to THIS request
            return; // Don't send error to dApp — keep request pending
          }
        } catch { /* fall through to generic error */ }
      }

      console.error('[DAppConnection] Request failed:', msg);
      // A terminal failure after the op was already submitted (e.g. dropped from
      // the mempool / no receipt in time) — flip its persisted record to 'failed'
      // so it doesn't linger as 'pending' forever in the Connections panel.
      if (pendingRecordId) {
        pendingSave
          .then(() => updateTransaction(pendingRecordId!, { status: 'failed' }))
          .catch(() => {});
      }
      setSignError(msg);
      responseTransport(base, transportRef.current)?.sendResponse(request.id, undefined, { code: -32603, message: msg });
      // Keep modal open so user can see the error — they dismiss manually
    } finally {
      approveInFlightRef.current = false; // release the re-entrancy lock on every exit
      setIsSigning(false);
      setIsSubmitting(false);
    }
  }, [incomingRequest]);

  // --- Reject ---
  const rejectRequest = useCallback(() => {
    if (!incomingRequest) return;
    // Signal any approve that's mid-flight (e.g. inside the ≤15s gas pre-check) to
    // abort before it submits — otherwise a swipe/reject would 4001 the dApp while
    // the tx still broadcasts + returns a success for the same id (BUG-2).
    signCancelledRef.current = true;
    responseTransport(incomingRequest, transportRef.current)?.sendResponse(incomingRequest.id, undefined, { code: 4001, message: 'User rejected' });
    setIncomingRequest(null);
    setSignError(null);
    setPendingOpHash(null);
    setFundingNeeded(null);
  }, [incomingRequest]);

  // --- Dismiss (after error, response already sent) ---
  const dismissRequest = useCallback(() => {
    setIncomingRequest(null);
    setSignError(null);
    setPendingOpHash(null);
    setFundingNeeded(null);
  }, []);

  // --- Bundler funding complete → retry the pending request ---
  const handleFundingComplete = useCallback(() => {
    setFundingNeeded(null);
    // Request-bind: only replay if the request that asked for funding is STILL the
    // one on the sheet. If it changed (a new sign took the slot), the pinned opts
    // (lastApproveOptsRef) belong to the old request — replaying them would submit
    // the wrong params under the wrong id. Bail rather than mis-submit.
    const pinnedRid = fundingRidRef.current;
    fundingRidRef.current = null;
    if (pinnedRid && incomingRequest && incomingRequest.id !== pinnedRid) return;
    // Drop the cached (stale, underfunded) balance so the pre-check on retry reads
    // the freshly-funded amount instead of re-prompting. Clear the REQUEST's chain
    // (an extension sign may be on a different chain than the global one — F4);
    // clearing the wrong chain would re-read the stale balance and loop funding.
    const account = activeAccountRef.current;
    const retryChainId = reqChainId(incomingRequest, chainIdRef.current);
    if (account) clearBundlerCache(retryChainId, account.address);
    // Retry approve with the SAME opts (esp. the capped paramsOverride) so funding
    // never resubmits the original (possibly unbounded) request.
    approveRequest(lastApproveOptsRef.current);
  }, [approveRequest, incomingRequest]);

  // --- Bundler funding cancelled → reject the pending request ---
  const handleFundingCancel = useCallback(() => {
    setFundingNeeded(null);
    if (incomingRequest) {
      responseTransport(incomingRequest, transportRef.current)?.sendResponse(incomingRequest.id, undefined, { code: -32603, message: 'Gas account funding cancelled' });
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
          const dropIfDead = () => {
            // A restored session whose channel is gone (the relay answers a join with
            // `terminate: channel_not_found`) can NEVER come back. Left alone, the SDK
            // treats it as the durable session and the snapshot restore-loops on every
            // launch — and a live reconnect attempt to a dead channel collides with a
            // fresh pairing on the relay (BUG-6, and a contributor to BUG-5). So if it
            // isn't live shortly after the reconnect attempt, drop it AND clear the
            // snapshot so the next launch starts clean.
            if (transportRef.current === wpTransport && !wpTransport.connected) {
              wpTransport.disconnect();
              if (transportRef.current === wpTransport) transportRef.current = null;
              clearWalletPairSession();
            }
          };
          try {
            await wpTransport.reconnect();
            setTimeout(dropIfDead, 8000); // real reconnects settle well under this; dead channels 404 fast
          } catch {
            dropIfDead();
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

  // --- Resume in-flight dApp txs left 'pending' (sheet closed / page reloaded
  //     mid-confirmation) so their status still resolves instead of showing as
  //     forever-pending in the Connections panel. ---
  const resumedRef = useRef(false);
  useEffect(() => {
    if (resumedRef.current || !state.hasWallet || state.isLoading) return;
    resumedRef.current = true;
    (async () => {
      const txs = await loadTransactions().catch(() => []);
      const cutoff = Math.floor(Date.now() / 1000) - 24 * 3600; // ignore ancient stuck ops
      const pending = txs.filter(
        (t) => t.status === 'pending' && (t.type ?? '') === 'dapp_tx' && !!t.userOpHash && t.timestamp >= cutoff,
      );
      for (const t of pending) {
        waitForReceipt(t.userOpHash, t.chainId)
          .then((txHash) => updateTransaction(t.id, { status: 'confirmed', txHash }))
          .catch(() => { /* still unconfirmed or dropped — leave for the user to clear */ });
      }
    })();
  }, [state.hasWallet, state.isLoading]);

  const value = React.useMemo(() => ({
    status, errorMessage, session, dappInfo,
    incomingRequest, isSigning, isSubmitting, signError, pendingOpHash, chainId,
    connectionType, pendingFingerprint,
    connectToBridge, connectToWalletPair, confirmFingerprint, cancelFingerprint,
    disconnectBridge, beginExtensionSign, reconnect, reconnectStuck,
    approveRequest, rejectRequest, dismissRequest, switchChain,
    fundingNeeded, handleFundingComplete, handleFundingCancel,
  }), [
    status, errorMessage, session, dappInfo,
    incomingRequest, isSigning, isSubmitting, signError, pendingOpHash, chainId,
    connectionType, pendingFingerprint,
    connectToBridge, connectToWalletPair, confirmFingerprint, cancelFingerprint,
    disconnectBridge, beginExtensionSign, reconnect, reconnectStuck,
    approveRequest, rejectRequest, dismissRequest, switchChain,
    fundingNeeded, handleFundingComplete, handleFundingCancel,
  ]);

  return (
    <DAppConnectionContext.Provider value={value}>
      {children}
    </DAppConnectionContext.Provider>
  );
}
