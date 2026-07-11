/**
 * SigningSheet — the single presentational signing surface (ERC-7730 Clear
 * Signing UI).
 *
 * The ONE rendering path for signing (a security UI must not be duplicated). The
 * production modal and the Clear-Signing test harness both render this with the
 * same data; only the action callbacks + signing-state differ. It owns the
 * read-only data fetching (descriptor resolution, gas estimate, token metadata,
 * approval detection) and all presentation; it never touches the dApp transport.
 */
import React, { useEffect, useState, useMemo } from 'react';
import {
  View, Text, ScrollView, ActivityIndicator,
} from 'react-native';
import { useTranslation } from 'react-i18next';
import { VelaButton } from '@/components/ui/VelaButton';
import { SlideToConfirmButton } from '@/components/ui/SlideToConfirmButton';
import { type BLEIncomingRequest } from '@/models/types';
import { nativeSymbol } from '@/models/network';
import { hapticLight, hapticSuccess, hapticError, hapticWarning } from '@/services/platform';
import {
  resolveTransaction, resolveTypedData,
  type ClearSignResult,
} from '@/services/clear-signing';
import { color } from '@/constants/theme';
import { GasFeeCard } from '@/components/ui/GasFeeCard';
import {
  detectApproval, rewriteApprovalParams,
  type DetectedApproval, type ApprovalChoice,
} from '@/services/approval-guard';
import { resolveTokenMetadata } from '@/services/token-metadata';
import { parseSiwe, checkSiweDomainBinding } from '@/services/siwe';
import { fetchChainlinkPrices, resolveChainlinkPrice } from '@/services/price-service';
import { simulateAssetChanges, type AssetSimResult } from '@/services/tx-simulation';
import { BalanceChangePreview } from './BalanceChangePreview';
import {
  estimateTransactionFee,
  rawBundlerGasCost,
  type GasTier,
  type TransactionFeeEstimate,
} from '@/services/safe-transaction';
import { Shield, AlertTriangle, Pen } from 'lucide-react-native';
import { styles, localizeIntent, SigningChainContext } from './signing-core';
import { DAppBanner } from './DAppBanner';
import { AdvancedPanel } from './AdvancedPanel';
import { WarningBanner } from './WarningBanner';
import { ClearSignView } from './views/ClearSignView';
import { ApprovalView } from './views/ApprovalView';
import { PermitSignView } from './views/PermitSignView';
import { MessageSignView, decodePersonalMessage } from './views/MessageSignView';
import { EthSignDangerView } from './views/EthSignDangerView';
import { BlindTypedDataView } from './views/BlindTypedDataView';
import { BlindTransactionView } from './views/BlindTransactionView';
import { BatchCallsView, type BatchItem, legNeedsChoice } from './views/BatchCallsView';

export interface SigningSheetProps {
  request: BLEIncomingRequest;
  chainId: number;
  account: { address?: string; name?: string } | null;
  dappInfo: { name?: string; url?: string; icon?: string } | null;
  isSigning: boolean;
  signError: string | null;
  pendingOpHash: string | null;
  onApprove: (opts?: { maxFeePerGas?: bigint; bundlerCostWei?: bigint; paramsOverride?: any[]; assetSim?: AssetSimResult | null; intent?: string }) => void;
  onReject: () => void;
  onDismiss: () => void;
  /**
   * Read-only replay: re-render a PAST signature exactly as it was shown, with no
   * approve/reject and no live-only work (gas estimate, simulation, funding). Used
   * by the Connections panel to "look back at what I signed" (and to re-open an
   * in-flight op's status after the sheet was closed). Defaults to false.
   */
  readOnly?: boolean;
  /**
   * Persisted sign-time simulation for a read-only replay — the "what moved"
   * preview captured when the request was approved. Live mode recomputes its own
   * `sim`; replay can't (state has moved on), so the host passes the stored one.
   */
  replaySim?: AssetSimResult | null;
  /**
   * TEST-HARNESS ONLY: simulate the transaction from this address instead of the
   * signer. The clear-signing demo signs with an empty parallel-space passkey, so
   * a real mainnet sim would revert on balance ("expected to fail"); pointing the
   * sim at a funded address lets the benign scenarios preview green as intended.
   * Never set in production — the sim must reflect the real signer's balances.
   */
  simFromOverride?: string;
}

export function SigningSheet({
  request: incomingRequest,
  chainId,
  account: activeAccount,
  dappInfo,
  isSigning,
  signError,
  pendingOpHash,
  onApprove,
  onReject,
  onDismiss,
  readOnly = false,
  replaySim = null,
  simFromOverride,
}: SigningSheetProps) {
  const { t } = useTranslation();

  const [clearSign, setClearSign] = useState<ClearSignResult | null>(null);
  const [resolving, setResolving] = useState(false);

  // Gas estimation state (for eth_sendTransaction only)
  const [gasTier, setGasTier] = useState<GasTier>('standard');
  const [feeEstimate, setFeeEstimate] = useState<TransactionFeeEstimate | null>(null);
  const [estimatingGas, setEstimatingGas] = useState(false);
  // Explicit flag (vs inferring from null feeEstimate) so the confirm guard doesn't
  // flicker in the frame before estimation starts.
  const [gasEstimateFailed, setGasEstimateFailed] = useState(false);

  // Native-token USD price (Chainlink, cached 3min) → fiat line on the gas card.
  const [nativeUsdPrice, setNativeUsdPrice] = useState(0);
  useEffect(() => {
    let cancelled = false;
    fetchChainlinkPrices()
      .then((prices) => { if (!cancelled) setNativeUsdPrice(resolveChainlinkPrice(nativeSymbol(chainId), prices) ?? 0); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [chainId]);

  // --- Editable approval (the never-unlimited mandate) ---
  // Detected straight off the raw request, independent of any descriptor.
  const approval = useMemo<DetectedApproval | null>(
    () => (incomingRequest ? detectApproval(incomingRequest.method, incomingRequest.params) : null),
    [incomingRequest],
  );

  // --- Branded haptic feedback (no-op on web) ---
  // A danger sheet (opaque eth_sign, or an unbounded approval) buzzes on open — a
  // physical "pay attention" that lands before the eye reaches the warning.
  useEffect(() => {
    if (readOnly) return;
    const m = incomingRequest?.method;
    const p = incomingRequest?.params?.[0];
    const siwePhish = m === 'personal_sign' && !!p && (() => {
      const s = parseSiwe(decodePersonalMessage(p));
      return !!s && checkSiweDomainBinding(s.domain, dappInfo?.url ?? incomingRequest?.origin) === 'mismatch';
    })();
    const dangerous = m === 'eth_sign' || (!!approval?.isUnbounded && !approval.isReducing) || siwePhish;
    if (dangerous) hapticWarning();
  }, [incomingRequest, approval, readOnly, dappInfo?.url]);
  // Physical confirmation of the outcome — success buzz when the signature lands,
  // error buzz when it's rejected or fails.
  useEffect(() => { if (pendingOpHash) hapticSuccess(); }, [pendingOpHash]);
  useEffect(() => { if (signError) hapticError(); }, [signError]);

  const [approveChoice, setApproveChoice] = useState<ApprovalChoice | null>(null);
  const [approveTokenMeta, setApproveTokenMeta] = useState<{ symbol: string; decimals: number; verified: boolean } | null>(null);

  // Client-side simulation: revert pre-check + net balance changes (null = unknown / not run).
  const [sim, setSim] = useState<AssetSimResult | null>(null);

  // EIP-5792 batch (wallet_sendCalls): each leg resolved + approval-checked, so the
  // user sees a per-call breakdown instead of blind-signing the whole bundle.
  const [batch, setBatch] = useState<BatchItem[] | null>(null);
  // Per-leg spending-cap choices keyed by leg index — the same never-unlimited
  // editor single approvals use, applied to each approval leg of the batch.
  const [batchChoices, setBatchChoices] = useState<Record<number, ApprovalChoice | null>>({});
  // On-chain symbol/decimals for every token approved across the batch (one
  // Multicall3 read), keyed by lowercased address, so each leg's editor shows
  // real amounts.
  const [batchMeta, setBatchMeta] = useState<Map<string, { symbol: string; decimals: number; verified: boolean }>>(new Map());

  // Resolve the approved token's symbol/decimals (on-chain via Multicall3, cached).
  useEffect(() => {
    setApproveChoice(null);
    const tokenAddr = approval?.tokenAddress;
    if (!tokenAddr) { setApproveTokenMeta(null); return; }
    let cancelled = false;
    const fallback = { symbol: `${tokenAddr.slice(0, 6)}…`, decimals: 18, verified: false };
    resolveTokenMetadata(chainId, [tokenAddr])
      .then((map) => {
        if (cancelled) return;
        const m = map.get(tokenAddr.toLowerCase());
        setApproveTokenMeta(m ? { symbol: m.symbol, decimals: m.decimals, verified: true } : fallback);
      })
      .catch(() => { if (!cancelled) setApproveTokenMeta(fallback); });
    return () => { cancelled = true; };
  }, [approval?.tokenAddress, chainId]);

  // Resolve clear signing + estimate gas when a new request comes in
  useEffect(() => {
    if (!incomingRequest) {
      setClearSign(null);
      setFeeEstimate(null);
      setGasTier('standard');
      setGasEstimateFailed(false);
      setSim(null);
      return;
    }

    const { method, params } = incomingRequest;
    // Guard the async simulation so a slower previous request can't overwrite
    // the current one's state after it's been replaced.
    let cancelled = false;
    setSim(null);

    if (method === 'eth_sendTransaction' && params?.[0]) {
      setResolving(true);
      resolveTransaction(params[0].to, params[0].data, params[0].value, chainId)
        .then(setClearSign)
        .catch(() => setClearSign(null))
        .finally(() => setResolving(false));

      // Estimate gas fee in parallel — against the REAL tx so the displayed fee and
      // the funding pre-check reflect this contract call/deploy, not a dummy transfer.
      // Skipped in read-only replay: a historical signature isn't about to be sent.
      if (activeAccount?.address && !readOnly) {
        setEstimatingGas(true);
        setGasEstimateFailed(false);
        estimateTransactionFee(activeAccount.address, chainId, 'standard', {
          to: params[0].to, value: params[0].value, data: params[0].data,
        })
          .then((f) => { setFeeEstimate(f); setGasEstimateFailed(false); })
          .catch(() => { setFeeEstimate(null); setGasEstimateFailed(true); })
          .finally(() => setEstimatingGas(false));

        // Simulate the inner Safe→target call: revert pre-check + net balance changes.
        // simFromOverride is a test-harness stand-in only; production uses the signer.
        simulateAssetChanges(
          simFromOverride ?? activeAccount.address,
          [{ to: params[0].to, data: params[0].data, value: params[0].value }],
          chainId,
        )
          .then((r) => { if (!cancelled) setSim(r); })
          .catch(() => { if (!cancelled) setSim(null); });
      }
    } else if (method.includes('signTypedData') && params) {
      setResolving(true);
      const typedDataRaw = params[1] ?? params[0];
      try {
        const typedData = typeof typedDataRaw === 'string' ? JSON.parse(typedDataRaw) : typedDataRaw;
        resolveTypedData(typedData, chainId)
          .then(setClearSign)
          .catch(() => setClearSign(null))
          .finally(() => setResolving(false));
      } catch {
        setClearSign(null);
        setResolving(false);
      }
    } else {
      setClearSign(null);
    }
    return () => { cancelled = true; };
  }, [incomingRequest, chainId, activeAccount?.address, readOnly, simFromOverride]);

  // Real tx for accurate gas estimation in the fee card (re-runs on tier change/refresh).
  const txForEstimate = useMemo(() => {
    const p = incomingRequest?.method === 'eth_sendTransaction' ? incomingRequest.params?.[0] : undefined;
    return p ? { to: p.to, value: p.value, data: p.data } : undefined;
  }, [incomingRequest]);

  // Resolve each leg of an EIP-5792 batch (intent + approval flag per call), and
  // simulate the whole bundle for a net balance-change preview.
  useEffect(() => {
    if (incomingRequest?.method !== 'wallet_sendCalls') { setBatch(null); return; }
    const calls = incomingRequest.params?.[0]?.calls;
    if (!Array.isArray(calls) || calls.length === 0) { setBatch(null); return; }
    let cancelled = false;
    setResolving(true);
    setBatchChoices({}); // fresh request → no carried-over caps
    Promise.all(calls.map(async (c: any): Promise<BatchItem> => {
      const cs = await resolveTransaction(c.to, c.data, c.value, chainId).catch(() => null);
      const approval = detectApproval('eth_sendTransaction', [{ to: c.to, data: c.data, value: c.value }]);
      return { to: c.to ?? '', clearSign: cs, approval };
    }))
      .then((items) => { if (!cancelled) setBatch(items); })
      .catch(() => { if (!cancelled) setBatch(null); })
      .finally(() => { if (!cancelled) setResolving(false); });

    // Net balance changes across all legs (executed sequentially, shared state —
    // e.g. approve + swap nets to −USDC / +WETH), plus the revert + underfunded
    // pre-checks. The engine already accepts the full calls array. Skipped in
    // read-only replay (a historical batch isn't being simulated for submission).
    if (activeAccount?.address && !readOnly) {
      const simCalls = calls.map((c: any) => ({ to: c.to, data: c.data, value: c.value }));
      simulateAssetChanges(simFromOverride ?? activeAccount.address, simCalls, chainId)
        .then((r) => { if (!cancelled) setSim(r); })
        .catch(() => { if (!cancelled) setSim(null); });

      // Gas fee for the WHOLE bundle — a batch is an on-chain UserOp; estimate against
      // the same MultiSend of every call that sendBatchCalls submits.
      setEstimatingGas(true);
      setGasEstimateFailed(false);
      estimateTransactionFee(activeAccount.address, chainId, 'standard', undefined, simCalls)
        .then((f) => { if (!cancelled) { setFeeEstimate(f); setGasEstimateFailed(false); } })
        .catch(() => { if (!cancelled) { setFeeEstimate(null); setGasEstimateFailed(true); } })
        .finally(() => { if (!cancelled) setEstimatingGas(false); });
    }
    return () => { cancelled = true; };
  }, [incomingRequest, chainId, activeAccount?.address, readOnly, simFromOverride]);

  // Resolve symbol/decimals for every token approved across the batch's legs, so
  // each leg's spending-cap editor can show and parse real token amounts.
  useEffect(() => {
    if (!batch) { setBatchMeta(new Map()); return; }
    const tokens = Array.from(new Set(
      batch.map((it) => it.approval?.tokenAddress?.toLowerCase()).filter(Boolean) as string[],
    ));
    if (tokens.length === 0) { setBatchMeta(new Map()); return; }
    let cancelled = false;
    resolveTokenMetadata(chainId, tokens)
      .then((map) => {
        if (cancelled) return;
        const out = new Map<string, { symbol: string; decimals: number; verified: boolean }>();
        for (const tk of tokens) {
          const m = map.get(tk);
          out.set(tk, m
            ? { symbol: m.symbol, decimals: m.decimals, verified: true }
            : { symbol: `${tk.slice(0, 6)}…`, decimals: 18, verified: false });
        }
        setBatchMeta(out);
      })
      .catch(() => { if (!cancelled) setBatchMeta(new Map()); });
    return () => { cancelled = true; };
  }, [batch, chainId]);

  if (!incomingRequest) return null;

  const { method, params } = incomingRequest;
  const isPersonalSign = method === 'personal_sign';
  const isEthSign = method === 'eth_sign';
  const isTypedData = method.includes('signTypedData');
  const isTx = method === 'eth_sendTransaction';
  const isBatch = method === 'wallet_sendCalls';

  // Derive display info
  const displayOrigin = dappInfo?.name ?? incomingRequest.origin ?? 'dApp';
  const displayDomain = dappInfo?.url
    ? (() => { try { return new URL(dappInfo.url).host; } catch { return dappInfo.url; } })()
    : undefined;

  const addr = activeAccount?.address;

  // Choose which view to render — wait for descriptor resolution before showing content
  const renderContent = () => {
    // Engine-verified confidence: the tx was simulated and is NOT expected to revert.
    // A sim's SENT side can't be understated (the real token emits its own transfer
    // log), so this is a trustworthy "here's what actually leaves your wallet" signal
    // that stands independent of any ERC-7730 descriptor. When present, the sheet
    // leads with the outcome and calms the descriptor-absence alarms; RECEIVED amounts
    // still read as 'unverified' (spoofable) per the asymmetric model in tx-simulation.
    const activeSim = readOnly ? replaySim : sim;
    const simConfident = !!activeSim && activeSim.ok === true;

    // Off-chain permit signature (Permit2 / ERC-2612 / DAI). The dApp redeems its
    // OWN struct on-chain, so we can't cap it — capping the signed amount only
    // desyncs the signature and reverts the dApp's tx. Surface the real risk and
    // sign verbatim under deliberate consent, never the cap editor.
    if (approval && approval.locus.type === 'typed-path') {
      return <PermitSignView approval={approval} meta={approveTokenMeta} clearSign={clearSign} />;
    }
    // Editable approval takes precedence — detection is instant (no descriptor),
    // and the spending-cap editor is the primary content for these requests.
    if (approval?.editable) {
      return (
        <ApprovalView
          approval={approval}
          meta={approveTokenMeta}
          choice={approveChoice}
          onChange={setApproveChoice}
          chainId={chainId}
          walletAddress={addr}
          clearSign={clearSign}
          requestId={incomingRequest.id}
        />
      );
    }
    // While loading descriptor, show loading state (prevents blind→clear flash)
    if (resolving) {
      return (
        <View style={styles.fallback}>
          <Text style={styles.fallbackText}>{t('componentsUi.signing.loading')}</Text>
        </View>
      );
    }
    if (clearSign) {
      return <ClearSignView cs={clearSign} simConfident={simConfident} walletAddress={addr} />;
    }
    // EIP-5792 batch — list each call, with an editable spending cap on every
    // approval leg (so an unlimited approve can be capped instead of only rejected).
    if (isBatch && batch) {
      return (
        <BatchCallsView
          items={batch}
          choices={batchChoices}
          onChoiceChange={(i, c) => setBatchChoices((prev) => ({ ...prev, [i]: c }))}
          metaByToken={batchMeta}
          editable={!readOnly}
          requestId={incomingRequest.id}
        />
      );
    }
    // eth_sign signs an OPAQUE 32-byte hash — the classic blind-sign trap. It gets
    // its own hard-warning surface, never the calm personal_sign message view.
    if (isEthSign && params) {
      // eth_sign(address, data) → data is params[1]; fall back to params[0] only
      // for a malformed single-param request.
      return <EthSignDangerView dataHex={params.length > 1 ? params[1] : params[0]} />;
    }
    if (isPersonalSign && params?.[0]) {
      return <MessageSignView hexMsg={params[0]} requestOrigin={dappInfo?.url ?? incomingRequest.origin} />;
    }
    if (isTypedData && params) {
      return <BlindTypedDataView params={params} />;
    }
    if (isTx && params?.[0]) {
      return <BlindTransactionView tx={params[0]} chainId={chainId} simConfident={simConfident} nativeUsdPrice={nativeUsdPrice} />;
    }
    return (
      <View style={styles.fallback}>
        <Shield size={28} color={color.fg.muted} strokeWidth={2} />
        <Text style={styles.fallbackText}>{t('componentsUi.signing.signatureRequest')}</Text>
      </View>
    );
  };

  // Button config — keep label short (max ~15 chars)
  const buttonLabel = (): string => {
    if (isSigning) return t('componentsUi.signing.signing');
    if (approval?.editable) {
      return approveChoice?.type === 'revoke'
        ? t('componentsUi.signingApprove.verbRevoke')
        : t('componentsUi.signingApprove.verbApprove');
    }
    if (clearSign) {
      if (clearSign.type === 'signature') return t('componentsUi.signing.signLabel');
      // Localize the descriptor intent so the button reads "确认兑换", never
      // "确认Swap"/"确认Send". Long or unrecognized intents → neutral "确认".
      const li = localizeIntent(clearSign.intent);
      if (!li || li.length > 12) return t('componentsUi.signing.confirmLabel');
      return t('componentsUi.signing.confirmIntentLabel', { intent: li });
    }
    if (isPersonalSign || isTypedData) return t('componentsUi.signing.signLabel');
    if (isBatch) return t('componentsUi.signing.confirmLabel');
    // A plain native send (value, no calldata) reads as "Confirm Send", matching the
    // eyebrow — same as the decoded ERC-20 transfer.
    if (isTx && (!params?.[0]?.data || params[0].data === '0x')) {
      return t('componentsUi.signing.confirmIntentLabel', { intent: localizeIntent('send') });
    }
    // Catch-all (blind contract call, eth_sign): a neutral "确认", never "授权" —
    // that verb belongs only to an actual token approval.
    return t('componentsUi.signing.confirmLabel');
  };

  // The footer is a single uniform slide-to-confirm for EVERY request (the deliberate
  // slide is the friction; closing the sheet rejects) — danger no longer branches the
  // footer. Phishing/eth_sign still buzz a warning haptic on open (see the effect
  // above) and flag in-body via MessageSignView / EthSignDangerView.

  const confirm = () => {
    hapticLight(); // tactile acknowledgement the moment the user commits to signing
    // For an edited approval, re-encode to the chosen finite amount BEFORE submit.
    // The independent guard re-checks at the submit chokepoint, so a rewrite
    // failure fails closed (never unbounded).
    let paramsOverride: any[] | undefined;
    if (approval?.editable && approveChoice) {
      try { paramsOverride = rewriteApprovalParams(method, params, approval, approveChoice); }
      catch { paramsOverride = undefined; }
    } else if (isBatch && batch && Array.isArray(params?.[0]?.calls)) {
      // Re-encode each approval leg the user capped/revoked, rebuilding the calls
      // array. The per-leg submit guard re-checks each call, so an un-rewritten
      // unbounded leg still fails closed.
      const calls = params[0].calls;
      let changed = false;
      const newCalls = calls.map((c: any, i: number) => {
        const ap = batch[i]?.approval;
        const choice = batchChoices[i];
        if (ap?.editable && choice) {
          try {
            const [rw] = rewriteApprovalParams('eth_sendTransaction', [{ to: c.to, data: c.data, value: c.value }], ap, choice);
            changed = true;
            return { ...c, data: rw.data };
          } catch { return c; }
        }
        return c;
      });
      if (changed) paramsOverride = [{ ...params[0], calls: newCalls }, ...params.slice(1)];
    }
    onApprove({
      maxFeePerGas: feeEstimate?.maxFeePerGas,
      // Raw bundler cost (tier markup removed) drives the funding pre-check.
      bundlerCostWei: feeEstimate ? rawBundlerGasCost(feeEstimate) : undefined,
      paramsOverride,
      // The "what moved" preview the user just saw — persisted with the record so
      // the Connections-panel replay can show it without re-simulating stale state.
      assetSim: sim,
      // The resolved clear-signing intent (e.g. "Swap", "Approve") — persisted so
      // the Connections list/detail label the op meaningfully, not "Contract
      // interaction". Undefined for plain signatures / blind txs (label falls back).
      intent: clearSign?.intent,
    });
  };

  const confirmDisabled =
    resolving
    || (isTx && (estimatingGas || gasEstimateFailed))
    || (!!approval?.editable && !approveChoice)
    // Every granting batch leg must be capped/revoked (or its grant deliberately
    // chosen) before the bundle can be confirmed — mirrors the single-tx rule.
    || (isBatch && (batch?.some((it, i) => legNeedsChoice(it.approval, batchChoices[i])) ?? false));

  return (
      <SigningChainContext.Provider value={chainId}>
      <View style={styles.container}>
        <ScrollView showsVerticalScrollIndicator={false}>
          {/* dApp banner — always shown */}
          <DAppBanner
            name={displayOrigin}
            domain={displayDomain}
            icon={dappInfo?.icon}
            chainId={chainId}
            accountName={activeAccount?.name}
            accountAddress={addr}
          />

          {/* Read-only replay banner — "you're looking back at a past signature". */}
          {readOnly && !pendingOpHash && (
            <View style={styles.historyNote}>
              <Pen size={15} color={color.fg.muted} strokeWidth={2} />
              <Text style={styles.historyNoteText}>{t('componentsUi.signing.historicalNote')}</Text>
            </View>
          )}

          {renderContent()}

          {/* Simulation summary — revert pre-check + net balance changes, one
              render path shared with Send's confirm step. Live mode shows the fresh
              sim; a read-only replay shows the one persisted at sign time (state has
              moved on, so it can't be recomputed) — same component either way.
              Kept ABOVE the raw-data escape hatch: the "what actually changes" is the
              outcome that matters (especially when the contract couldn't be decoded),
              not something to bury under an Advanced toggle. */}
          {(isTx || isBatch) && (
            <BalanceChangePreview
              result={readOnly ? replaySim : sim}
              chainId={chainId}
              // The decoded hero's asset flows (token + direction) — a matching sim
              // collapses to a quiet ✓ instead of repeating them. Approvals/permits/
              // batches never corroborate a balance move (an approve cap is decoded as
              // a send-amount, so a malicious approve()-that-also-transfers must never
              // collapse), so they pass [].
              heroFlows={
                (approval || isBatch)
                  ? []
                  : clearSign
                    ? (clearSign.fields
                        .filter(f => f.role === 'send-amount' || f.role === 'receive-amount')
                        .map(f => ({ token: f.tokenAddress?.toLowerCase(), dir: (f.role === 'send-amount' ? 'out' : 'in') as 'out' | 'in' }))
                        ?? [])
                    // Plain native send (value, no calldata, no descriptor): the hero
                    // already shows the −native amount, so the sim's matching outflow
                    // collapses to a quiet ✓ instead of repeating it (F2). Any UNmatched
                    // change still expands the full list (per-token reconciliation).
                    : (isTx && params?.[0]?.value && params[0].value !== '0x0' && (!params[0].data || params[0].data === '0x'))
                      ? [{ token: undefined, dir: 'out' as const }]
                      : []
              }
            />
          )}

          {/* Advanced — full untruncated payload + any detail-only fields, for
              power users who want to verify exactly what's being signed. */}
          <AdvancedPanel method={method} params={params} clearSign={clearSign} />

          {/* Gas fee card — for an on-chain tx OR a batch (both cost gas), live only. */}
          {(isTx || isBatch) && activeAccount?.address && !readOnly && (
            <GasFeeCard
              feeEstimate={feeEstimate}
              estimating={estimatingGas}
              nativeSymbol={nativeSymbol(chainId)}
              nativeUsdPrice={nativeUsdPrice}
              safeAddress={activeAccount.address}
              chainId={chainId}
              gasTier={gasTier}
              tx={isBatch ? undefined : txForEstimate}
              batchCalls={isBatch ? params?.[0]?.calls : undefined}
              onTierChange={setGasTier}
              onFeeUpdate={(f) => { setFeeEstimate(f); setGasEstimateFailed(false); }}
            />
          )}

          {/* Gas estimation failed — block the blind submit that would otherwise
              hang for 2 min on the bundler. Retry lives in the gas card above. */}
          {gasEstimateFailed && !isSigning && !readOnly && (
            <WarningBanner
              severity="caution"
              text={t('componentsUi.signing.gasEstimateFailed')}
            />
          )}

          {/* Submitted — show the hash + "waiting" instead of a silent spinner.
              Also shown on replay of an op still awaiting its on-chain receipt. */}
          {pendingOpHash && (isSigning || readOnly) && (
            <View style={styles.pendingCard}>
              <ActivityIndicator size="small" color={color.info.base} />
              <Text style={styles.pendingText}>
                {t('componentsUi.signing.submitted')} · {pendingOpHash.slice(0, 10)}…{pendingOpHash.slice(-6)}
              </Text>
            </View>
          )}

          {/* Error */}
          {signError && (
            <View style={styles.errorCard}>
              <AlertTriangle size={16} color={color.error.base} strokeWidth={2} />
              <Text style={styles.errorText}>{signError}</Text>
            </View>
          )}
        </ScrollView>

        {/* Buttons */}
        <View style={styles.buttonRow}>
          {readOnly ? (
            <VelaButton
              title={t('componentsUi.signing.close')}
              onPress={onDismiss}
              variant="secondary"
              style={styles.buttonFlex}
            />
          ) : signError ? (
            <VelaButton
              title={t('componentsUi.signing.dismiss')}
              onPress={onDismiss}
              variant="secondary"
              style={styles.buttonFlex}
            />
          ) : (
            // Unified: ONE slide-to-confirm for every request, benign or dangerous.
            // There is no Reject button — dismissing the sheet (swipe down / tap
            // outside) already rejects the request (AppModal onClose → rejectRequest),
            // so a deliberate slide is the only way to APPROVE and closing is the easy,
            // safe default. requiresHold/recommendReject no longer branch the footer;
            // the slide itself is the friction, uniformly.
            <SlideToConfirmButton
              title={buttonLabel()}
              hint={t('componentsUi.signing.slideToConfirm', { defaultValue: 'Slide to confirm' })}
              onConfirm={confirm}
              loading={isSigning || resolving}
              disabled={confirmDisabled}
              style={styles.buttonFlex}
            />
          )}
        </View>
      </View>
      </SigningChainContext.Provider>
  );
}
