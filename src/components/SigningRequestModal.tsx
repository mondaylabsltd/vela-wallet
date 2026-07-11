/**
 * Global signing request modal — ERC-7730 Clear Signing UI.
 *
 * Renders signing requests with intent-driven, human-readable layouts:
 *   - Clear signed transactions/signatures (descriptor found)
 *   - Plain message signing (personal_sign)
 *   - Blind sign fallback (no descriptor)
 *
 * Design principles:
 *   L1 — Intent: large colored action word (Swap, Send, Approve, Sign)
 *   L2 — Substance: token cards with amounts, recipients, flow arrows
 *   L3 — Context: contract info, chain, details (collapsed)
 */
import React, { useEffect, useState, useCallback, useMemo } from 'react';
import {
  View, Text, ScrollView, Image, Pressable, ActivityIndicator,
} from 'react-native';
import { useTranslation } from 'react-i18next';
import i18n from '@/i18n';
import * as Clipboard from 'expo-clipboard';
import { AppModal } from '@/components/ui/AppModal';
import { VelaButton } from '@/components/ui/VelaButton';
import { SlideToConfirmButton } from '@/components/ui/SlideToConfirmButton';
import { useDAppConnection } from '@/models/dapp-connection';
import { useWallet } from '@/models/wallet-state';
import { shortAddr, isAddress, tokenLogoURLsByAddress, type BLEIncomingRequest } from '@/models/types';
import { chainName, nativeSymbol, nativeCoinLogoURL, explorerBaseURL, DEFAULT_NETWORKS } from '@/models/network';
import { openBrowser } from '@/services/platform';
import {
  resolveTransaction, resolveTypedData,
  type ClearSignResult, type ClearSignField, type SigningRisk,
} from '@/services/clear-signing';
import { scaleFont, color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';
import { BundlerFundingView } from '@/components/ui/BundlerFundingModal';
import { requestChainId as reqChainId, requestDApp } from '@/models/dapp-request-routing';
import { GasFeeCard } from '@/components/ui/GasFeeCard';
import { EditableApproveCard } from '@/components/signing/EditableApproveCard';
import {
  detectApproval, rewriteApprovalParams,
  type DetectedApproval, type ApprovalChoice,
} from '@/services/approval-guard';
import { resolveTokenMetadata } from '@/services/token-metadata';
import { resolveRecipientIdentity, type RecipientIdentity } from '@/services/recipient-identity';
import { resolveRecipientRisk, type RecipientRisk } from '@/services/recipient-risk';
import { parseSiwe, checkSiweDomainBinding, siweHost, type SiweBinding } from '@/services/siwe';
import { readErc20Allowance } from '@/services/token-reads';
import { knownTokenSymbol } from '@/services/tokens';
import { fetchChainlinkPrices, resolveChainlinkPrice } from '@/services/price-service';
import { formatTokenAmount as formatRawTokenAmount } from '@/services/approval-guard';
import { simulateAssetChanges, type AssetSimResult } from '@/services/tx-simulation';
import { BalanceChangePreview } from '@/components/signing/BalanceChangePreview';
import { RecipientTrust } from '@/components/contacts/RecipientTrust';
import { ChainLogo } from '@/components/ChainLogo';
import { TokenLogo } from '@/components/TokenLogo';
import {
  estimateTransactionFee,
  rawBundlerGasCost,
  type GasTier,
  type TransactionFeeEstimate,
} from '@/services/safe-transaction';
import {
  Shield, AlertTriangle, Copy, ChevronDown, Check,
  ArrowDown, ShieldAlert, ShieldCheck, Pen, ExternalLink,
} from 'lucide-react-native';

/** Chain id for the active signing sheet — lets leaf rows build explorer links. */
const SigningChainContext = React.createContext<number>(1);

// ---------------------------------------------------------------------------
// Risk → color mapping
// ---------------------------------------------------------------------------

function riskColors(): Record<SigningRisk, string> {
  return {
    safe: color.success.base,
    normal: color.accent.base,
    caution: color.warning.base,
    danger: color.error.base,
  };
}

// ---------------------------------------------------------------------------
// Descriptor label/intent localization.
//
// ERC-7730 descriptors carry English intents ("Send", "Swap") and field labels
// ("Amount", "To") by spec, so a descriptor-driven screen would render half-
// English inside a localized UI (the "确认Send" / "Amount" problem). Map the
// common canonical values to the user's language; anything unrecognized falls
// through to the raw descriptor string (an honest, if English, label).
// ---------------------------------------------------------------------------
const INTENT_L10N: Record<string, string> = {
  send: 'intentSend', transfer: 'intentSend',
  approve: 'intentApprove', 'set allowance': 'intentApprove', 'increase allowance': 'intentApprove',
  swap: 'intentSwap', exchange: 'intentSwap', trade: 'intentSwap',
  deposit: 'intentDeposit', supply: 'intentDeposit',
  withdraw: 'intentWithdraw', redeem: 'intentWithdraw',
  mint: 'intentMint', burn: 'intentBurn',
  stake: 'intentStake', unstake: 'intentUnstake',
  claim: 'intentClaim', 'claim rewards': 'intentClaim',
  bridge: 'intentBridge', wrap: 'intentWrap', unwrap: 'intentUnwrap',
  borrow: 'intentBorrow', repay: 'intentRepay', revoke: 'intentRevoke',
};
const LABEL_L10N: Record<string, string> = {
  amount: 'labelAmount', value: 'labelAmount', assets: 'labelAmount',
  to: 'labelTo', recipient: 'labelTo', receiver: 'labelTo', beneficiary: 'labelTo', destination: 'labelTo',
  from: 'labelFrom', sender: 'labelFrom', owner: 'labelOwner',
  spender: 'labelSpender', operator: 'labelSpender',
  token: 'labelToken', 'token id': 'labelTokenId', tokenid: 'labelTokenId',
  deadline: 'labelDeadline',
  'min received': 'labelMinReceived', 'minimum received': 'labelMinReceived', 'min amount out': 'labelMinReceived',
  'you receive (min)': 'labelMinReceived', 'you receive (minimum)': 'labelMinReceived',
  'you pay': 'labelPay', pay: 'labelPay',
  'you receive': 'labelReceived', 'amount received': 'labelReceived', shares: 'labelShares',
  chain: 'labelChain', 'chain id': 'labelChain', nonce: 'labelNonce',
};
/** Localize a canonical ERC-7730 English intent; unknown → the raw string. */
function localizeIntent(raw?: string): string {
  if (!raw) return '';
  const suffix = INTENT_L10N[raw.trim().toLowerCase()];
  return suffix ? String(i18n.t(('componentsUi.signing.' + suffix) as any, { defaultValue: raw })) : raw;
}
/** Localize a canonical ERC-7730 English field label; unknown → the raw string. */
function localizeLabel(raw?: string): string {
  if (!raw) return '';
  const suffix = LABEL_L10N[raw.trim().toLowerCase()];
  return suffix ? String(i18n.t(('componentsUi.signing.' + suffix) as any, { defaultValue: raw })) : raw;
}

/**
 * Intent-header color. Restrained on purpose: color = meaning. Benign actions
 * (send, sign, deploy) read in neutral ink; only caution (amber) and danger
 * (red) get a hue, so a colored headline always signals real risk.
 */
function intentColor(risk: SigningRisk): string {
  if (risk === 'danger') return color.error.base;
  if (risk === 'caution') return color.warning.base;
  return color.fg.base;
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

/**
 * SigningSheet — the single presentational signing surface.
 *
 * The ONE rendering path for signing (a security UI must not be duplicated). The
 * production modal and the Clear-Signing test harness both render this with the
 * same data; only the action callbacks + signing-state differ. It owns the
 * read-only data fetching (descriptor resolution, gas estimate, token metadata,
 * approval detection) and all presentation; it never touches the dApp transport.
 */
/** One resolved leg of an EIP-5792 batch (wallet_sendCalls). */
interface BatchItem {
  to: string;
  clearSign: ClearSignResult | null;
  approval: DetectedApproval | null;
}

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
      return <ClearSignView cs={clearSign} simConfident={simConfident} />;
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
      return <BlindTransactionView tx={params[0]} chainId={chainId} simConfident={simConfident} />;
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
    // Catch-all (plain send, blind contract call, eth_sign): a neutral "确认",
    // never "授权" — that verb belongs only to an actual token approval.
    return t('componentsUi.signing.confirmLabel');
  };

  const buttonVariant = (): 'accent' | 'secondary' => 'accent';

  // Danger actions must not be one tap from a normal transfer: an unlimited/
  // grant-all approval or an opaque eth_sign requires a deliberate press-and-hold.
  // This covers single requests AND every leg of an EIP-5792 batch / a
  // non-editable unbounded approval (e.g. permit2-batch), which would otherwise
  // slip through the gate even though the submit guard rejects them.
  const isGrantingApproval = (a: DetectedApproval | null | undefined) =>
    !!a?.isUnbounded && !a.isReducing && !a.isBooleanGrant;
  // An unbounded off-chain permit signature (incl. DAI full-balance / Permit2
  // batch) is signed verbatim, so its only safety gate is a deliberate hold.
  const permitGrantsBroad =
    !!approval && approval.locus.type === 'typed-path' && approval.isUnbounded && !approval.isReducing;
  // A batch leg counts as danger only while it STILL grants broad access — once
  // the user caps/revokes it, the hold (and the unlimited banner) drop away.
  const batchHasDanger =
    isBatch && (batch?.some((it, i) => legGrantsBroad(it.approval, batchChoices[i]) || it.clearSign?.risk === 'danger') ?? false);
  // Lift the SIWE domain-binding check to the sheet level: a sign-in whose message
  // domain ≠ the request origin is the canonical phishing pattern, and it must drive
  // the footer (reject-dominant), not just an in-body banner the user can breeze past.
  const siwePhishing =
    isPersonalSign && !!params?.[0] &&
    (() => {
      const s = parseSiwe(decodePersonalMessage(params[0]));
      return !!s && checkSiweDomainBinding(s.domain, dappInfo?.url ?? incomingRequest.origin) === 'mismatch';
    })();

  const requiresHold =
    isEthSign ||
    clearSign?.risk === 'danger' ||
    (!!approval?.editable && approveChoice?.type === 'grant') ||
    isGrantingApproval(approval) ||
    permitGrantsBroad ||
    batchHasDanger ||
    siwePhishing;

  // When the wallet has itself concluded "this looks like phishing — reject it",
  // the SAFE action must dominate: Reject is a solid primary on top, and signing is
  // demoted behind a deliberate slide. (requiresHold-only danger keeps slide-on-top.)
  const recommendReject = siwePhishing;

  const confirm = () => {
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

          {/* Advanced — full untruncated payload + any detail-only fields, for
              power users who want to verify exactly what's being signed. */}
          <AdvancedPanel method={method} params={params} clearSign={clearSign} />

          {/* Simulation summary — revert pre-check + net balance changes, one
              render path shared with Send's confirm step. Live mode shows the fresh
              sim; a read-only replay shows the one persisted at sign time (state has
              moved on, so it can't be recomputed) — same component either way. */}
          {(isTx || isBatch) && <BalanceChangePreview result={readOnly ? replaySim : sim} chainId={chainId} />}

          {/* Gas fee card — only for eth_sendTransaction, and only live (not replay) */}
          {isTx && activeAccount?.address && !readOnly && (
            <GasFeeCard
              feeEstimate={feeEstimate}
              estimating={estimatingGas}
              nativeSymbol={nativeSymbol(chainId)}
              nativeUsdPrice={nativeUsdPrice}
              safeAddress={activeAccount.address}
              chainId={chainId}
              gasTier={gasTier}
              tx={txForEstimate}
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
          ) : recommendReject ? (
            // Detected phishing: the wallet recommends rejecting, so the SAFE action
            // dominates — a solid Reject on top, signing demoted behind a slide.
            <View style={styles.dangerStack}>
              <VelaButton
                title={t('componentsUi.signing.reject')}
                onPress={onReject}
                variant="primary"
                disabled={isSigning}
              />
              <SlideToConfirmButton
                title={buttonLabel()}
                hint={t('componentsUi.signing.slideToConfirm', { defaultValue: 'Slide to confirm' })}
                onConfirm={confirm}
                loading={isSigning || resolving}
                disabled={confirmDisabled}
              />
            </View>
          ) : requiresHold ? (
            // Danger-level signing: a full-width slide-to-confirm gets its own row
            // (a slide needs the width to be usable), with Reject as a secondary
            // full-width button beneath it.
            <View style={styles.dangerStack}>
              <SlideToConfirmButton
                title={buttonLabel()}
                hint={t('componentsUi.signing.slideToConfirm', { defaultValue: 'Slide to confirm' })}
                onConfirm={confirm}
                loading={isSigning || resolving}
                disabled={confirmDisabled}
              />
              <VelaButton
                title={t('componentsUi.signing.reject')}
                onPress={onReject}
                variant="secondary"
                disabled={isSigning}
              />
            </View>
          ) : (
            <>
              <VelaButton
                title={t('componentsUi.signing.reject')}
                onPress={onReject}
                variant="secondary"
                disabled={isSigning}
                style={styles.buttonFlex}
              />
              <VelaButton
                title={buttonLabel()}
                onPress={confirm}
                variant={buttonVariant()}
                loading={isSigning || resolving}
                disabled={confirmDisabled}
                style={styles.buttonFlex}
              />
            </>
          )}
        </View>
      </View>
      </SigningChainContext.Provider>
  );
}

// ===========================================================================
// Production wrapper — wires the dApp connection context to <SigningSheet>
// ===========================================================================

export function SigningRequestModal() {
  const {
    incomingRequest, isSigning, isSubmitting, signError, pendingOpHash, chainId, dappInfo,
    approveRequest, rejectRequest, dismissRequest,
    fundingNeeded, handleFundingComplete, handleFundingCancel,
  } = useDAppConnection();
  const { activeAccount } = useWallet();

  if (!incomingRequest) return null;

  return (
    // A single native sheet. When the gas account needs funding we SWAP the
    // sheet's content to the funding view instead of stacking a second AppModal
    // over it — iOS won't present a second native modal atop a presented one, so
    // a stacked funding modal was invisible and tapping Approve did nothing
    // (docs/KNOWN-BUGS.md BUG-1). Swipe-to-dismiss over the funding view cancels
    // the pending request (handleFundingCancel), matching the funding "取消".
    //
    // Swipe-dismiss routing: once submitting (isSubmitting) or already submitted
    // (pendingOpHash), the tx is committed → DISMISS (op proceeds, real result
    // delivered), never reject — a "cancelled" tx must not still broadcast + send a
    // contradictory success (BUG-2). Only a pre-submit swipe rejects (4001).
    <AppModal
      visible={true}
      onClose={
        fundingNeeded
          ? handleFundingCancel
          : signError || pendingOpHash || isSubmitting
            ? dismissRequest
            : rejectRequest
      }
    >
      {fundingNeeded ? (
        <BundlerFundingView
          funding={fundingNeeded}
          onFunded={handleFundingComplete}
          onCancel={handleFundingCancel}
          dappVariant
        />
      ) : (
        /* Per-request chain/identity for a Safari-extension sign (F3/F4): sign +
           display against the ORIGIN's granted chain and identity, never a
           concurrent WalletPair session's global chainId/dappInfo. Ordinary
           requests carry no __chainId/__dapp → fall back to the global state. */
        <SigningSheet
          request={incomingRequest}
          chainId={reqChainId(incomingRequest, chainId)}
          account={activeAccount ?? null}
          dappInfo={requestDApp(incomingRequest, dappInfo)}
          isSigning={isSigning}
          signError={signError}
          pendingOpHash={pendingOpHash}
          onApprove={approveRequest}
          onReject={rejectRequest}
          onDismiss={dismissRequest}
        />
      )}
    </AppModal>
  );
}

// ===========================================================================
// dApp Banner
// ===========================================================================

/**
 * The site's OWN favicon, derived from its host — no third-party favicon service,
 * so signing a tx never leaks the dApp you're on to Google/DuckDuckGo/etc. Returns
 * undefined for non-registrable hosts (the test harness `clear-signing-test`,
 * `localhost`, bare IPs) so the banner falls back to a letter monogram.
 */
function faviconForHost(domain?: string): string | undefined {
  if (!domain) return undefined;
  const host = domain.replace(/^[a-z]+:\/\//i, '').split('/')[0].split(':')[0].trim();
  if (!host || !host.includes('.') || /^\d+(\.\d+){3}$/.test(host)) return undefined;
  return `https://${host}/favicon.ico`;
}

function DAppBanner({ name, domain, icon, chainId, accountName, accountAddress }: {
  name: string;
  domain?: string;
  icon?: string;
  chainId: number;
  accountName?: string;
  accountAddress?: string;
}) {
  const net = DEFAULT_NETWORKS.find(n => n.chainId === chainId);

  // Prefer an explicit icon (e.g. the in-app browser's captured favicon); otherwise
  // derive the site's own /favicon.ico. Fall back to a letter monogram if the image
  // fails to load (404 / not an image). Reset the failure flag when the target changes.
  const logoUri = icon ?? faviconForHost(domain);
  const [logoFailed, setLogoFailed] = useState(false);
  useEffect(() => { setLogoFailed(false); }, [logoUri]);
  const showLogo = !!logoUri && !logoFailed;

  return (
    <View style={styles.dappBanner}>
      {/* Row 1: dApp identity ←→ chain */}
      <View style={styles.dappRow1}>
        {showLogo ? (
          <Image
            source={{ uri: logoUri }}
            style={styles.dappLogo}
            onError={() => setLogoFailed(true)}
          />
        ) : (
          <View style={[styles.dappLogo, styles.dappLogoFallback]}>
            <Text style={styles.dappLogoText}>{(name[0] ?? '?').toUpperCase()}</Text>
          </View>
        )}
        <View style={styles.dappInfo}>
          <Text style={styles.dappName} numberOfLines={1}>{name}</Text>
          {domain && <Text style={styles.dappDomain} numberOfLines={1}>{domain}</Text>}
        </View>
        <View style={styles.dappChainRow}>
          {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={16} />}
          <Text style={styles.dappChainName}>{chainName(chainId)}</Text>
        </View>
      </View>

      {/* Row 2: wallet name · address */}
      {accountName && (
        <Text style={styles.dappAccountLine} numberOfLines={1}>
          {accountName}{accountAddress ? `  ·  ${shortAddr(accountAddress)}` : ''}
        </Text>
      )}
    </View>
  );
}

// ===========================================================================
// Clear Sign View (descriptor found)
// ===========================================================================

function ClearSignView({ cs, simConfident }: {
  cs: ClearSignResult;
  /** The tx was simulated and is not expected to revert — a best-effort (4byte)
   *  decode then reads as a calm "here's the gist" note instead of a "carefully
   *  check every detail" nag, because the preview below proves the real effect. */
  simConfident?: boolean;
}) {
  const { t } = useTranslation();
  const rc = intentColor(cs.risk);

  // Separate fields by role
  const sendAmounts = cs.fields.filter(f => f.role === 'send-amount');
  const receiveAmounts = cs.fields.filter(f => f.role === 'receive-amount');
  const recipients = cs.fields.filter(f => f.role === 'recipient');
  const spenders = cs.fields.filter(f => f.role === 'spender');
  // Detail-flagged generics (best-effort raw params) live under the Advanced
  // panel, not the headline body — only body-level generics render here.
  const generic = cs.fields.filter(f => f.role === 'generic' && !f.detail);

  // Determine if this is a swap-like layout (send → receive)
  const isSwapLayout = sendAmounts.length > 0 && receiveAmounts.length > 0;
  const hasRecipient = recipients.length > 0 || spenders.length > 0;

  // Sending a token TO its own contract address burns it irreversibly — a classic
  // costly fat-finger. Flag when a recipient equals the contract being called.
  const sendingToTokenContract =
    !!cs.contractAddress &&
    recipients.some(f => f.address && f.address === cs.contractAddress);

  return (
    <View>
      {/* Context: chain + account */}
      {/* context shown in dApp banner */}

      {/* L1: Intent. A benign, decoded value transfer cedes the headline to the
          asset flow below (verb → eyebrow); anything with elevated risk keeps the
          big hero so the colored headline still shouts. */}
      <IntentHeader
        intent={localizeIntent(cs.intent)}
        color={rc}
        variant={cs.risk === 'normal' && (sendAmounts.length > 0 || receiveAmounts.length > 0) ? 'eyebrow' : 'hero'}
      />

      {/* Best-effort decode (recovered from a 4-byte selector DB, no verified
          descriptor) — show what it does, but be honest it isn't verified. With a
          confident simulation the wording drops the "check every detail" nag (the
          preview below is the real proof); without one it keeps the full caution. */}
      {cs.bestEffort && (
        <WarningBanner
          severity="caution"
          text={simConfident
            ? t('componentsUi.signing.bestEffortSimulated', { defaultValue: 'Decoded from the function signature (not a verified descriptor). The preview below shows the actual effect.' })
            : t('componentsUi.signing.bestEffortWarning')}
        />
      )}

      {/* Incomplete decode — the user must not assume they've seen everything */}
      {cs.partial && (
        <WarningBanner
          severity="caution"
          text={t('componentsUi.signing.partialWarning')}
        />
      )}

      {/* Amount shown with unverified decimals (on-chain lookup failed) */}
      {!cs.partial && cs.fields.some(f => f.unverified) && (
        <WarningBanner
          severity="caution"
          text={t('componentsUi.signing.unverifiedWarning')}
        />
      )}

      {/* L2: Token cards + flow */}
      {isSwapLayout ? (
        <>
          {sendAmounts.map((f, i) => (
            <TokenCard key={`s${i}`} field={f} variant="send" />
          ))}
          <FlowArrow />
          {receiveAmounts.map((f, i) => (
            <TokenCard key={`r${i}`} field={f} variant="receive" />
          ))}
        </>
      ) : sendAmounts.length > 0 ? (
        <>
          {sendAmounts.map((f, i) => (
            <TokenCard key={`s${i}`} field={f} variant={cs.risk === 'caution' ? 'caution' : 'send'} />
          ))}
          {hasRecipient && <FlowArrow />}
        </>
      ) : null}

      {/* Spender / recipient */}
      {spenders.map((f, i) => (
        <ContractBar
          key={`sp${i}`}
          label={t('componentsUi.signing.spenderLabel')}
          name={f.value}
          address={cs.contractAddress}
          verified={cs.verified}
        />
      ))}
      {recipients.map((f, i) => (
        <ContractBar
          key={`re${i}`}
          label={t('componentsUi.signing.recipientLabel')}
          name={f.address ? undefined : f.value}
          address={f.address}
          verified={false}
          riskCheck
        />
      ))}

      {/* Sending a token to its own contract → irreversible loss. */}
      {sendingToTokenContract && (
        <WarningBanner
          severity="danger"
          text={t('componentsUi.signing.tokenToContractWarning')}
        />
      )}

      {/* An already-expired deadline (swap/permit) — the tx will revert. */}
      {cs.fields.some(f => f.expired) && (
        <WarningBanner
          severity="caution"
          text={t('componentsUi.signing.expiredWarning')}
        />
      )}

      {/* Warning for unlimited approvals etc. */}
      {cs.fields.some(f => f.warning) && (
        <WarningBanner
          severity="danger"
          text={t('componentsUi.signing.unlimitedWarning')}
        />
      )}

      {/* Generic fields */}
      {generic.length > 0 && (
        <View style={styles.genericFields}>
          {generic.map((f, i) => (
            <GenericFieldRow key={i} field={f} />
          ))}
        </View>
      )}

      {/* Contract bar (if not already shown via spender/recipient) */}
      {!hasRecipient && cs.contractAddress && (
        <ContractBar
          label={t('componentsUi.signing.interactingLabel')}
          name={cs.contractName ? `${cs.contractName}${cs.owner ? ` · ${cs.owner}` : ''}` : undefined}
          address={cs.contractAddress}
          verified={cs.verified}
        />
      )}
    </View>
  );
}

// ===========================================================================
// Approval View — the editable, never-unlimited spending-cap surface
// ===========================================================================

function ApprovalView({ approval, meta, choice, onChange, chainId, walletAddress, clearSign, requestId }: {
  approval: DetectedApproval;
  meta: { symbol: string; decimals: number; verified: boolean } | null;
  choice: ApprovalChoice | null;
  onChange: (c: ApprovalChoice | null) => void;
  chainId: number;
  walletAddress?: string;
  clearSign: ClearSignResult | null;
  requestId: string;
}) {
  const { t } = useTranslation();
  const isNft = approval.kind === 'setApprovalForAll';

  // increaseAllowance adds to the EXISTING allowance — showing only the increment
  // is dangerously incomplete. Read the current on-chain allowance so we can show
  // the resulting total (current + increment). On a slow/failed read we still warn
  // the increment ADDS to an existing allowance rather than hiding the row.
  const [currentAllowance, setCurrentAllowance] = useState<bigint | null>(null);
  const [allowanceResolved, setAllowanceResolved] = useState(false);
  useEffect(() => {
    setCurrentAllowance(null);
    setAllowanceResolved(false);
    if (approval.kind !== 'increaseAllowance' || !walletAddress || !approval.tokenAddress) return;
    let cancelled = false;
    readErc20Allowance(chainId, approval.tokenAddress, walletAddress, approval.spender)
      .then((a) => { if (!cancelled) { setCurrentAllowance(a); setAllowanceResolved(true); } })
      .catch(() => { if (!cancelled) setAllowanceResolved(true); });
    return () => { cancelled = true; };
  }, [approval.kind, approval.tokenAddress, approval.spender, walletAddress, chainId]);

  const verb = approval.isReducing
    ? t('componentsUi.signingApprove.verbRevoke')
    : isNft && approval.isUnbounded
      ? t('componentsUi.signingApprove.verbApproveAll')
      : t('componentsUi.signingApprove.verbApprove');
  const verbColor = approval.isReducing
    ? color.success.base
    : approval.isUnbounded
      ? color.error.base
      : color.warning.base;

  // Expiry classification (UI-side; the pure resolver injects `now` for tests).
  const deadlineSec = approval.deadline ? Number(approval.deadline) : 0;
  const nowSec = Math.floor(Date.now() / 1000);
  const expired = deadlineSec > 0 && deadlineSec < nowSec;

  const symbol = meta?.symbol ?? '…';
  const decimals = meta?.decimals ?? 18;
  const logoUrls = approval.tokenAddress
    ? tokenLogoURLsByAddress(chainId, approval.tokenAddress)
    : undefined;

  return (
    <View>
      <IntentHeader intent={verb} color={verbColor} />

      <EditableApproveCard
        key={requestId}
        approval={approval}
        symbol={symbol}
        decimals={decimals}
        decimalsVerified={meta?.verified ?? false}
        logoUrls={logoUrls}
        spenderLabel={clearSign?.contractName ?? shortAddr(approval.spender)}
        choice={choice}
        onChange={onChange}
      />

      {/* increaseAllowance: the chosen value is an INCREMENT — surface the
          resulting total so "increase by 100" can't read as "cap at 100". When the
          current allowance couldn't be read, still say the increment ADDS to it. */}
      {approval.kind === 'increaseAllowance' && allowanceResolved && (() => {
        const increment = choice?.type === 'amount' ? choice.amountRaw : (approval.amountRaw ?? 0n);
        const dec = meta?.decimals ?? 18;
        const sym = meta?.symbol ?? '';
        return (
          <View style={styles.allowanceTotalRow}>
            <Text style={styles.allowanceTotalLabel}>{t('componentsUi.signingApprove.resultingTotal')}</Text>
            {currentAllowance !== null ? (
              <Text style={styles.allowanceTotalValue}>
                {`${formatRawTokenAmount(currentAllowance, dec)} + ${formatRawTokenAmount(increment, dec)} = ${formatRawTokenAmount(currentAllowance + increment, dec)} ${sym}`}
              </Text>
            ) : (
              <Text style={styles.allowanceTotalUnknown}>
                {t('componentsUi.signingApprove.resultingTotalUnknown', { amount: `${formatRawTokenAmount(increment, dec)} ${sym}` })}
              </Text>
            )}
          </View>
        );
      })()}

      <ContractBar
        label={isNft ? t('componentsUi.signingApprove.operatorLabel') : t('componentsUi.signingApprove.spenderLabel')}
        address={approval.spender}
        verified={false}
      />

      {approval.tokenAddress && (
        <ContractBar
          label={isNft ? t('componentsUi.signingApprove.collectionLabel') : t('componentsUi.signingApprove.tokenLabel')}
          name={clearSign?.contractName ?? (meta?.verified ? meta.symbol : undefined)}
          address={approval.tokenAddress}
          verified={clearSign?.verified ?? false}
        />
      )}

      {expired && (
        <WarningBanner severity="caution" text={t('componentsUi.signingApprove.expired')} />
      )}
    </View>
  );
}

// ===========================================================================
// Permit Sign View — off-chain spending permit (Permit2 / ERC-2612 / DAI)
// ===========================================================================

/**
 * Off-chain permit signatures are redeemed by the dApp, which submits its OWN
 * permit struct on-chain — so the wallet can't cap the amount (rewriting it only
 * desyncs the signature and reverts the dApp's tx). We therefore show the real
 * risk and sign VERBATIM under a deliberate hold, rather than the cap editor.
 */
function PermitSignView({ approval, meta, clearSign }: {
  approval: DetectedApproval;
  meta: { symbol: string; decimals: number; verified: boolean } | null;
  clearSign: ClearSignResult | null;
}) {
  const { t } = useTranslation();
  const chainId = React.useContext(SigningChainContext);

  const symbol = meta?.symbol ?? '…';
  const decimals = meta?.decimals ?? 18;
  const logoUrls = approval.tokenAddress ? tokenLogoURLsByAddress(chainId, approval.tokenAddress) : undefined;
  const dangerous = approval.isUnbounded && !approval.isReducing;

  const deadlineSec = approval.deadline ? Number(approval.deadline) : 0;
  const expired = deadlineSec > 0 && deadlineSec < Math.floor(Date.now() / 1000);

  const verb = approval.isReducing
    ? t('componentsUi.signingApprove.verbRevoke')
    : t('componentsUi.signingApprove.verbApprove');
  const verbColor = approval.isReducing
    ? color.success.base
    : approval.isUnbounded ? color.error.base : color.warning.base;

  // What the dApp's permit will be authorized to spend.
  const amountText = approval.isBooleanGrant
    ? (approval.isUnbounded
        ? t('componentsUi.signingApprove.fullBalance')
        : t('componentsUi.signingApprove.revokeValue'))
    : approval.kind === 'permit2-batch'
      ? t('componentsUi.signingApprove.multiplePermits', { defaultValue: 'Multiple tokens' })
      : approval.isUnbounded
        ? t('componentsUi.signingApprove.unlimitedValue', { defaultValue: 'Unlimited' })
        : `${formatRawTokenAmount(approval.amountRaw ?? 0n, decimals)} ${symbol}`;

  return (
    <View>
      <IntentHeader intent={verb} color={verbColor} />

      <View style={[styles.tokenCard, dangerous && { backgroundColor: color.error.soft }]}>
        <TokenLogo symbol={approval.tokenAddress ? symbol : '?'} logoUrls={logoUrls} size={40} />
        <View style={styles.tokenInfo}>
          <Text style={styles.tokenAmount} numberOfLines={1}>{amountText}</Text>
          <Text style={styles.tokenLabel}>
            {t('componentsUi.signingApprove.permitTag', { defaultValue: 'Spending permit (signature)' })}
          </Text>
        </View>
        {dangerous && <AlertTriangle size={14} color={riskColors().danger} strokeWidth={2} />}
      </View>

      <ContractBar
        label={t('componentsUi.signingApprove.spenderLabel')}
        name={clearSign?.contractName}
        address={approval.spender}
        verified={false}
      />
      {approval.tokenAddress && (
        <ContractBar
          label={t('componentsUi.signingApprove.tokenLabel')}
          name={clearSign?.contractName ?? (meta?.verified ? meta.symbol : undefined)}
          address={approval.tokenAddress}
          verified={clearSign?.verified ?? false}
        />
      )}

      {expired && <WarningBanner severity="caution" text={t('componentsUi.signingApprove.expired')} />}

      {dangerous ? (
        <>
          <WarningBanner severity="danger" text={t('componentsUi.signing.unlimitedWarning')} />
          <Text style={styles.permitHint}>
            {t('componentsUi.signingApprove.permitCantCap', {
              defaultValue: "A permit is a signature — its amount can't be capped here. To limit spending, use an on-chain Approve instead.",
            })}
          </Text>
        </>
      ) : !approval.isReducing ? (
        <Text style={styles.permitHint}>
          {t('componentsUi.signingApprove.permitNote', {
            defaultValue: "You're signing a spending permit — the dApp can move up to this amount on your behalf.",
          })}
        </Text>
      ) : null}
    </View>
  );
}

// ===========================================================================
// Message Sign View (personal_sign)
// ===========================================================================

function MessageSignView({ hexMsg, requestOrigin }: {
  hexMsg: string;
  requestOrigin?: string;
}) {
  const { t } = useTranslation();
  const decoded = decodePersonalMessage(hexMsg);

  // Sign-In with Ethereum: bind the domain inside the message to the request
  // origin. A mismatch is the canonical phishing pattern.
  const siwe = useMemo(() => parseSiwe(decoded), [decoded]);
  const binding: SiweBinding | null = useMemo(
    () => (siwe ? checkSiweDomainBinding(siwe.domain, requestOrigin) : null),
    [siwe, requestOrigin],
  );

  if (siwe) {
    return (
      <View>
        <IntentHeader
          intent={t('componentsUi.signing.signInIntent')}
          color={binding === 'mismatch' ? color.error.base : color.fg.base}
        />

        <View style={styles.genericFields}>
          <View style={styles.genRow}>
            <Text style={styles.genLabel}>{t('componentsUi.signing.siweDomain')}</Text>
            <Text style={[styles.genValue, binding === 'mismatch' && { color: riskColors().danger }]} numberOfLines={1}>
              {siweHost(siwe.domain) ?? siwe.domain}
            </Text>
          </View>
          {!!siwe.statement && (
            <View style={styles.genRow}>
              <Text style={styles.genLabel}>{t('componentsUi.signing.siweStatement')}</Text>
              <Text style={styles.genValue} numberOfLines={3}>{siwe.statement}</Text>
            </View>
          )}
        </View>

        {binding === 'mismatch' && (
          <WarningBanner
            severity="danger"
            text={t('componentsUi.signing.siweMismatch', { domain: siwe.domain, origin: hostLabel(requestOrigin) })}
          />
        )}
        {binding === 'ok' && (
          <View style={styles.siweOkRow}>
            <ShieldCheck size={13} color={color.success.base} strokeWidth={2} />
            <Text style={styles.siweOkText}>{t('componentsUi.signing.siweOk', { domain: siwe.domain })}</Text>
          </View>
        )}
      </View>
    );
  }

  return (
    <View>
      {/* context shown in dApp banner */}
      <IntentHeader intent={t('componentsUi.signing.signMessage')} color={color.fg.base} />

      <View style={styles.msgBubble}>
        <View style={styles.msgTag}>
          <Pen size={10} color={color.fg.subtle} strokeWidth={2} />
          <Text style={styles.msgTagText}>{t('componentsUi.signing.personalSignTag')}</Text>
        </View>
        <Text style={styles.msgText}>{decoded}</Text>
      </View>
    </View>
  );
}

/** Short host label for messages ("app.uniswap.org"). */
function hostLabel(value: string | undefined): string {
  if (!value) return '—';
  try {
    const withScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(value) ? value : `https://${value}`;
    return new URL(withScheme).host;
  } catch {
    return value;
  }
}

// ===========================================================================
// eth_sign Danger View — opaque-hash blind signing
// ===========================================================================

function EthSignDangerView({ dataHex }: { dataHex: string }) {
  const { t } = useTranslation();
  const hash = typeof dataHex === 'string' ? dataHex : String(dataHex ?? '');
  return (
    <View>
      <IntentHeader intent={t('componentsUi.signing.ethSignIntent')} color={color.error.base} />

      <View style={styles.ethSignCard}>
        <View style={styles.ethSignHeader}>
          <ShieldAlert size={16} color={color.error.base} strokeWidth={2} />
          <Text style={styles.ethSignTitle}>{t('componentsUi.signing.ethSignTitle')}</Text>
        </View>
        <Text style={styles.ethSignBody}>{t('componentsUi.signing.ethSignBody')}</Text>
        <Text style={styles.ethSignHash} numberOfLines={2}>{hash}</Text>
      </View>

      <WarningBanner severity="danger" text={t('componentsUi.signing.ethSignWarning')} />
    </View>
  );
}

// ===========================================================================
// Blind Typed Data View (EIP-712, no descriptor)
// ===========================================================================

function BlindTypedDataView({ params }: {
  params: any[];
}) {
  const { t } = useTranslation();
  const { primaryType, domain, fields } = parseTypedDataForDisplay(params);

  return (
    <View>
      {/* context shown in dApp banner */}
      <IntentHeader intent={t('componentsUi.signing.signTypedData')} color={color.warning.base} />

      {/* Domain info */}
      {domain && (
        <ContractBar
          label={t('componentsUi.signing.signingFor')}
          name={domain.name}
          address={domain.verifyingContract?.toLowerCase()}
          verified={false}
        />
      )}

      {/* Primary type + fields */}
      <View style={styles.genericFields}>
        {primaryType && (
          <View style={styles.genRow}>
            <Text style={styles.genLabel}>{t('componentsUi.signing.typeLabel')}</Text>
            <Text style={styles.genValue}>{primaryType}</Text>
          </View>
        )}
        {fields.map(([k, v], i) => (
          <View key={i} style={styles.genRow}>
            <Text style={styles.genLabel}>{k}</Text>
            <Text style={styles.genValue} numberOfLines={2}>{v}</Text>
          </View>
        ))}
      </View>

      <WarningBanner
        severity="caution"
        text={t('componentsUi.signing.blindTypedWarning')}
      />
    </View>
  );
}

// ===========================================================================
// Blind Transaction View (no descriptor)
// ===========================================================================

function BlindTransactionView({ tx, chainId, simConfident }: {
  tx: any;
  chainId: number;
  /** The tx was simulated and is not expected to revert — the balance-change preview
   *  below shows what actually happens, so the descriptor-absence is a calm note, not
   *  a red alarm. */
  simConfident?: boolean;
}) {
  const { t } = useTranslation();
  const [showRaw, setShowRaw] = useState(false);
  const sym = nativeSymbol(chainId);
  const value = formatTxValue(tx.value, chainId);
  const hasData = tx.data && tx.data !== '0x';
  const dataSize = hasData ? Math.floor((tx.data.length - 2) / 2) : 0;
  // A simulated, non-reverting contract call reads as a neutral "contract
  // interaction", not a red "Unknown" — the preview below carries the real meaning.
  const calm = hasData && !!simConfident;

  return (
    <View>
      {/* context shown in dApp banner */}
      <IntentHeader
        intent={!hasData
          ? t('componentsUi.signing.intentSend')
          : calm
            ? t('componentsUi.signing.intentContractCall', { defaultValue: 'Contract interaction' })
            : t('componentsUi.signing.intentUnknown')}
        color={hasData && !calm ? color.error.base : color.fg.base}
      />

      {/* Value card */}
      {value !== `0 ${sym}` && (
        <TokenCard
          field={{ label: t('componentsUi.signing.valueLabel'), value, format: 'amount', role: 'send-amount' }}
          variant={hasData && !calm ? 'danger' : 'send'}
        />
      )}

      {hasData && <FlowArrow danger={!calm} />}

      {/* Contract */}
      <ContractBar
        label={hasData ? t('componentsUi.signing.unverifiedLabel') : t('componentsUi.signing.recipientLabel')}
        address={tx.to}
        verified={false}
        warning={hasData && !calm}
      />

      {/* Descriptor-absence notice. With a confident simulation it's a calm caption
          that points at the preview below; without one it stays a hard blind-sign
          warning (genuinely opaque — no descriptor AND no simulated outcome). */}
      {hasData && (
        <>
          <WarningBanner
            severity={calm ? 'caution' : 'danger'}
            text={calm
              ? t('componentsUi.signing.blindButSimulated', { defaultValue: "Vela couldn't read this contract's details, but the preview below shows exactly what this transaction does." })
              : t('componentsUi.signing.blindDecodeWarning', { bytes: dataSize })}
          />

          {/* Raw data toggle */}
          <Pressable
            style={styles.detailsToggle}
            onPress={() => setShowRaw(!showRaw)}
          >
            <Text style={styles.detailsToggleText}>
              {t('componentsUi.signing.rawCalldataToggle', { bytes: dataSize })}
            </Text>
            <ChevronDown
              size={12}
              color={color.fg.subtle}
              strokeWidth={2}
              style={showRaw ? { transform: [{ rotate: '180deg' }] } : undefined}
            />
          </Pressable>
          {showRaw && (
            <ScrollView horizontal={false} style={styles.rawBlock}>
              <Text style={styles.rawText}>
                {tx.data.slice(0, 200)}{tx.data.length > 200 ? '...' : ''}
              </Text>
            </ScrollView>
          )}
        </>
      )}

    </View>
  );
}

// ===========================================================================
// Batch View (EIP-5792 wallet_sendCalls) — per-call breakdown
// ===========================================================================

/** First meaningful amount/recipient line for a batch leg. */
function batchSummary(it: BatchItem): string | undefined {
  const f = it.clearSign?.fields.find(
    (x) => x.role === 'send-amount' || x.role === 'receive-amount' || x.format === 'tokenAmount' || x.format === 'amount',
  );
  return f?.value;
}

/**
 * Does this batch approval leg still need a deliberate decision before the bundle
 * can be confirmed? (Unbounded amount not yet capped/revoked, or a grant-all with
 * no choice.) Finite amounts are pre-accepted — editing them is optional.
 */
function legNeedsChoice(ap: DetectedApproval | null, choice: ApprovalChoice | null | undefined): boolean {
  if (!ap || !ap.editable || ap.isReducing) return false;
  if (ap.isBooleanGrant) return !choice;
  if (ap.isUnbounded) return !(choice && (choice.type === 'amount' || choice.type === 'revoke'));
  return false;
}

/** After the user's choice, does this leg still grant broad/unbounded access? */
function legGrantsBroad(ap: DetectedApproval | null, choice: ApprovalChoice | null | undefined): boolean {
  if (!ap || ap.isReducing) return false;
  if (ap.isBooleanGrant) return choice?.type === 'grant' || !choice;
  if (ap.isUnbounded) return !(choice && (choice.type === 'amount' || choice.type === 'revoke'));
  return false;
}

/** Is this leg an editable, amount/grant-bearing approval that gets the inline cap editor? */
function legIsEditableApproval(ap: DetectedApproval | null): boolean {
  return !!ap && ap.editable && !ap.isReducing;
}

function BatchCallsView({ items, choices, onChoiceChange, metaByToken, editable, requestId }: {
  items: BatchItem[];
  choices: Record<number, ApprovalChoice | null>;
  onChoiceChange: (index: number, choice: ApprovalChoice | null) => void;
  metaByToken: Map<string, { symbol: string; decimals: number; verified: boolean }>;
  editable: boolean;
  /** Remounts each leg's editor when the request changes (no stale cap state). */
  requestId: string;
}) {
  const { t } = useTranslation();
  const chainId = React.useContext(SigningChainContext);
  // Banner reflects the EFFECTIVE state: only still-uncapped grants are flagged.
  const anyUncapped = editable
    ? items.some((it, i) => legGrantsBroad(it.approval, choices[i]))
    : items.some((it) => it.approval?.isUnbounded && !it.approval.isReducing && !it.approval.isBooleanGrant);

  return (
    <View>
      <IntentHeader intent={t('componentsUi.signing.batchIntent')} color={color.fg.base} />
      <Text style={styles.batchSub}>{t('componentsUi.signing.batchSubtitle', { count: items.length })}</Text>

      {items.map((it, i) => {
        const ap = it.approval;
        const title = it.clearSign?.intent
          ?? (ap ? t('componentsUi.signingApprove.verbApprove') : t('componentsUi.signing.batchCall'));

        // Editable approval leg → inline spending-cap editor (same control single
        // approvals use), so an unlimited approve can be capped here, not only rejected.
        if (editable && legIsEditableApproval(ap) && ap) {
          const meta = ap.tokenAddress ? metaByToken.get(ap.tokenAddress.toLowerCase()) : undefined;
          const logoUrls = ap.tokenAddress ? tokenLogoURLsByAddress(chainId, ap.tokenAddress) : undefined;
          return (
            <View key={`${requestId}-${i}`} style={styles.batchEditLeg}>
              <View style={styles.batchEditHead}>
                <View style={styles.batchNum}>
                  <Text style={styles.batchNumText}>{i + 1}</Text>
                </View>
                <Text style={styles.batchEditTitle} numberOfLines={1}>{title}</Text>
              </View>
              <EditableApproveCard
                approval={ap}
                symbol={meta?.symbol ?? '…'}
                decimals={meta?.decimals ?? 18}
                decimalsVerified={meta?.verified ?? false}
                logoUrls={logoUrls}
                spenderLabel={it.clearSign?.contractName ?? shortAddr(ap.spender)}
                choice={choices[i] ?? null}
                onChange={(c) => onChoiceChange(i, c)}
              />
            </View>
          );
        }

        // Non-approval / reducing / read-only leg → compact summary row.
        const danger = legGrantsBroad(ap, choices[i]);
        const summary = batchSummary(it);
        return (
          <View key={i} style={[styles.batchRow, danger && styles.batchRowDanger]}>
            <View style={styles.batchNum}>
              <Text style={styles.batchNumText}>{i + 1}</Text>
            </View>
            <View style={styles.batchInfo}>
              <Text style={styles.batchTitle} numberOfLines={1}>{title}</Text>
              {!!summary && <Text style={styles.batchDetail} numberOfLines={1}>{summary}</Text>}
              <Text style={styles.batchAddr} numberOfLines={1}>{it.to ? shortAddr(it.to) : '—'}</Text>
            </View>
            {danger && <ShieldAlert size={14} color={riskColors().danger} strokeWidth={2} />}
          </View>
        );
      })}

      {anyUncapped && (
        <WarningBanner severity="danger" text={t('componentsUi.signing.unlimitedWarning')} />
      )}
    </View>
  );
}

// ===========================================================================
// Shared sub-components
// ===========================================================================

function IntentHeader({ intent, color: intentColor, variant = 'hero' }: {
  intent: string;
  color: string;
  /** 'hero' = the big headline verb (opaque/risky actions own the screen).
   *  'eyebrow' = a small kicker above an asset-flow hero (benign, decoded
   *  value transfers — the money movement is the headline, not the verb). */
  variant?: 'hero' | 'eyebrow';
}) {
  if (variant === 'eyebrow') {
    return (
      <View style={styles.intentEyebrow}>
        <View style={[styles.intentEyebrowDot, { backgroundColor: intentColor }]} />
        <Text style={[styles.intentEyebrowText, { color: intentColor }]}>{intent}</Text>
      </View>
    );
  }
  return (
    <View style={styles.intentHeader}>
      <Text style={[styles.intentText, { color: intentColor }]}>
        {intent}
      </Text>
    </View>
  );
}

function TokenCard({ field, variant }: {
  field: ClearSignField;
  variant: 'send' | 'receive' | 'caution' | 'danger';
}) {
  // Wise-style de-container: benign amounts sit on an OPEN row (no card), letting
  // the number breathe; only caution/danger get a tinted card, so a filled card
  // always means "pay attention".
  const tinted = variant === 'caution' || variant === 'danger';
  const tintBg = variant === 'caution'
    ? { backgroundColor: color.warning.soft }
    : { backgroundColor: color.error.soft };

  const chainId = React.useContext(SigningChainContext);
  // A `amount`-format field with no token address is the chain's native coin
  // (e.g. a plain ETH send) — show the real coin symbol + logo, not a "?".
  const isNative = !field.tokenAddress && field.format === 'amount';
  const symbol = field.tokenAddress
    ? guessTokenSymbol(field.tokenAddress)
    : isNative ? nativeSymbol(chainId) : undefined;
  // Per-chain logo (checksummed + lowercase fallback) — not a mainnet-only guess.
  const logoUrls = field.tokenAddress
    ? tokenLogoURLsByAddress(chainId, field.tokenAddress)
    : isNative ? [nativeCoinLogoURL(chainId)] : undefined;

  // Directional framing (MetaMask/Rainbow "estimated changes" convention, shared
  // with BalanceChangePreview): "+" green for what arrives, "−" neutral ink for
  // what leaves. The signed amount is the hero of a benign transfer.
  const incoming = variant === 'receive';
  const sign = incoming ? '+' : '−';
  const amountTint = incoming ? color.success.base : color.fg.base;

  return (
    <View style={[tinted ? styles.tokenCard : styles.tokenRow, tinted && tintBg]}>
      <TokenLogo
        symbol={symbol ?? '?'}
        logoUrls={logoUrls}
        size={44}
      />
      <View style={styles.tokenInfo}>
        <Text
          style={[styles.tokenAmount, { color: amountTint }]}
          numberOfLines={1}
          adjustsFontSizeToFit
          minimumFontScale={0.6}
        >
          {sign}{field.value}
        </Text>
        <View style={styles.tokenSubRow}>
          <Text style={styles.tokenLabel}>{localizeLabel(field.label)}</Text>
          {!!field.usd && <Text style={styles.tokenUsd}>≈ {field.usd}</Text>}
        </View>
      </View>
      {field.warning && (
        <View style={styles.tokenWarning}>
          <AlertTriangle size={14} color={riskColors().danger} strokeWidth={2} />
        </View>
      )}
    </View>
  );
}

/** Guess token symbol from the shared known-token table, with an address fallback. */
function guessTokenSymbol(addr: string): string {
  return knownTokenSymbol(addr) ?? addr.slice(2, 6).toUpperCase();
}

function FlowArrow({ danger }: { danger?: boolean }) {
  return (
    <View style={styles.flowArrow}>
      <View style={[styles.flowCircle, danger && styles.flowCircleDanger]}>
        <ArrowDown
          size={14}
          color={danger ? riskColors().danger : color.fg.subtle}
          strokeWidth={2.5}
        />
      </View>
    </View>
  );
}

function ContractBar({ label, name, address, verified, warning, riskCheck }: {
  label: string;
  name?: string;
  address?: string;
  verified: boolean;
  warning?: boolean;
  /** Resolve recipient-risk signals (first-interaction + contract/EOA). */
  riskCheck?: boolean;
}) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  // Resolve an on-chain name (ENS / Basename / SPACE ID) when the descriptor
  // didn't supply one — turns a raw hex address into a recognizable identity and
  // helps catch address-poisoning. Cached in the service; descriptor name wins.
  const [ident, setIdent] = useState<RecipientIdentity | null>(null);
  useEffect(() => {
    setIdent(null);
    if (name || !isAddress(address)) return;
    let cancelled = false;
    resolveRecipientIdentity(address).then((r) => { if (!cancelled) setIdent(r); }).catch(() => {});
    return () => { cancelled = true; };
  }, [address, name]);
  const shownName = name ?? ident?.name;

  const chainId = React.useContext(SigningChainContext);

  // Recipient-risk: "first time" (address-poisoning defense) + contract/EOA.
  const [risk, setRisk] = useState<RecipientRisk | null>(null);
  useEffect(() => {
    setRisk(null);
    if (!riskCheck || !isAddress(address)) return;
    let cancelled = false;
    resolveRecipientRisk(chainId, address).then((r) => { if (!cancelled) setRisk(r); }).catch(() => {});
    return () => { cancelled = true; };
  }, [riskCheck, address, chainId]);

  const explorerBase = explorerBaseURL(chainId);
  const isFullAddr = isAddress(address);
  const explorerUrl = explorerBase && isFullAddr ? `${explorerBase}/address/${address}` : undefined;

  const handleCopy = useCallback(async () => {
    if (!address) return;
    await Clipboard.setStringAsync(address);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [address]);

  return (
    <View style={[styles.contractBar, warning && styles.contractBarWarning]}>
      <View style={styles.contractInfo}>
        <Text style={styles.contractLabel}>{label}</Text>
        <View style={styles.contractAddrRow}>
          {/* Verified descriptor name keeps the trust-green; a resolved ENS / raw
              address stays neutral so color always means "verified". */}
          {shownName && (
            <Text style={[styles.contractName, !verified && styles.contractNameNeutral]} numberOfLines={1}>
              {shownName}
            </Text>
          )}
          {!name && ident && <Text style={styles.sourceTag}>{ident.source}</Text>}
          {address && (
            <Text style={styles.contractAddr}>{shortAddr(address)}</Text>
          )}
          {/* First-ever interaction with this address → address-poisoning hint. */}
          {risk?.firstInteraction && (
            <Text style={[styles.riskTag, styles.riskTagWarn]}>{t('componentsUi.signing.firstTimeTag')}</Text>
          )}
          {/* Contract vs wallet — a contract recipient for a plain transfer is
              worth a glance (could be unintended). */}
          {risk?.isContract === true && (
            <Text style={styles.riskTag}>{t('componentsUi.signing.contractTag')}</Text>
          )}
        </View>
        <RecipientTrust address={address} compact />
      </View>
      {address && (
        <Pressable onPress={handleCopy} hitSlop={8} style={[styles.copyBtn, copied && styles.copyBtnDone]}>
          {copied
            ? <Check size={12} color={color.success.base} strokeWidth={2.5} />
            : <Copy size={12} color={color.fg.muted} strokeWidth={2} />
          }
        </Pressable>
      )}
      {/* Jump out to the block explorer to audit the contract / address. */}
      {explorerUrl && (
        <Pressable onPress={() => openBrowser(explorerUrl)} hitSlop={8} style={styles.copyBtn}>
          <ExternalLink size={12} color={color.fg.muted} strokeWidth={2} />
        </Pressable>
      )}
      {verified && (
        <View style={styles.verifiedBadge}>
          <ShieldCheck size={12} color={color.success.base} strokeWidth={2} />
        </View>
      )}
      {warning && (
        <ShieldAlert size={14} color={riskColors().danger} strokeWidth={2} />
      )}
    </View>
  );
}

function WarningBanner({ severity, text: msg }: {
  severity: 'caution' | 'danger';
  text: string;
}) {
  const isDanger = severity === 'danger';
  return (
    <View style={[styles.warnBanner, isDanger ? styles.warnDanger : styles.warnCaution]}>
      <AlertTriangle
        size={14}
        color={isDanger ? riskColors().danger : riskColors().caution}
        strokeWidth={2}
      />
      <Text style={[styles.warnText, { color: isDanger ? riskColors().danger : riskColors().caution }]}>
        {msg}
      </Text>
    </View>
  );
}

function GenericFieldRow({ field }: { field: ClearSignField }) {
  return (
    <View style={[styles.genRow, field.warning && styles.genRowWarning]}>
      <Text style={styles.genLabel}>{localizeLabel(field.label)}</Text>
      <Text
        style={[
          styles.genValue,
          field.warning && { color: riskColors().danger },
          field.expired && { color: riskColors().caution },
        ]}
        numberOfLines={2}
      >
        {field.value}
      </Text>
    </View>
  );
}

// ===========================================================================
// Advanced panel — full untruncated payload + detail-only fields
// ===========================================================================

function AdvancedPanel({ method, params, clearSign }: {
  method: string;
  params: any[];
  clearSign: ClearSignResult | null;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  // The exact bytes/JSON being signed — untruncated, so a power user can verify.
  const raw = useMemo(() => {
    try {
      if (method === 'eth_sendTransaction') {
        const tx = params?.[0] ?? {};
        return [
          tx.to ? `to: ${tx.to}` : null,
          tx.value && tx.value !== '0x0' ? `value: ${tx.value}` : null,
          tx.data && tx.data !== '0x' ? `data: ${tx.data}` : null,
        ].filter(Boolean).join('\n\n');
      }
      if (method.includes('signTypedData')) {
        const rawData = params?.[1] ?? params?.[0];
        const obj = typeof rawData === 'string' ? JSON.parse(rawData) : rawData;
        return JSON.stringify(obj, null, 2);
      }
      if (method === 'personal_sign') return String(params?.[0] ?? '');
      if (method === 'eth_sign') return String((params?.length > 1 ? params[1] : params?.[0]) ?? '');
      return '';
    } catch { return ''; }
  }, [method, params]);

  const detailFields = clearSign?.fields.filter((f) => f.detail) ?? [];
  if (!raw && detailFields.length === 0) return null;

  return (
    <View>
      <Pressable style={styles.detailsToggle} onPress={() => setOpen((o) => !o)}>
        <Text style={styles.detailsToggleText}>{t('componentsUi.signing.advancedToggle')}</Text>
        <ChevronDown
          size={12} color={color.fg.subtle} strokeWidth={2}
          style={open ? { transform: [{ rotate: '180deg' }] } : undefined}
        />
      </Pressable>
      {open && (
        <View style={styles.advancedBody}>
          {detailFields.map((f, i) => (
            <View key={i} style={styles.genRow}>
              <Text style={styles.genLabel}>{localizeLabel(f.label)}</Text>
              <Text style={styles.genValue} numberOfLines={4}>{f.value}</Text>
            </View>
          ))}
          {!!raw && (
            <ScrollView style={styles.advancedRaw} nestedScrollEnabled>
              <Text style={styles.rawText} selectable>{raw}</Text>
            </ScrollView>
          )}
        </View>
      )}
    </View>
  );
}

// ContextStrip merged into DAppBanner

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function decodePersonalMessage(hexMsg: string): string {
  try {
    const clean = hexMsg.startsWith('0x') ? hexMsg.slice(2) : hexMsg;
    const bytes = new Uint8Array(clean.match(/.{1,2}/g)!.map(b => parseInt(b, 16)));
    const decoded = new TextDecoder().decode(bytes);
    if (/^[\x20-\x7E\n\r\t]+$/.test(decoded)) return decoded;
    return `0x${clean.slice(0, 64)}${clean.length > 64 ? '...' : ''}`;
  } catch {
    return hexMsg.slice(0, 66) + (hexMsg.length > 66 ? '...' : '');
  }
}

function formatTxValue(value: string | undefined, cid: number): string {
  const sym = nativeSymbol(cid);
  if (!value || value === '0x0' || value === '0x') return `0 ${sym}`;
  try {
    const clean = value.startsWith('0x') ? value.slice(2) : value;
    const wei = BigInt('0x' + clean);
    const eth = Number(wei) / 1e18;
    if (eth === 0) return `0 ${sym}`;
    if (eth < 0.0001) return `< 0.0001 ${sym}`;
    return eth.toFixed(4).replace(/\.?0+$/, '') + ' ' + sym;
  } catch {
    return value ?? '0';
  }
}

function parseTypedDataForDisplay(params: any[]): {
  primaryType: string | null;
  domain: any;
  fields: [string, string][];
} {
  try {
    const data = typeof params[1] === 'string' ? JSON.parse(params[1]) : (params[1] ?? params[0]);
    const primaryType = data?.primaryType ?? null;
    const domain = data?.domain;
    const msg = data?.message;
    const fields: [string, string][] = msg
      ? Object.entries(msg).slice(0, 5).map(([k, v]) => [k, (
          v && typeof v === 'object' ? JSON.stringify(v) : String(v)
        ).slice(0, 60)])
      : [];
    return { primaryType, domain, fields };
  } catch {
    return { primaryType: null, domain: null, fields: [] };
  }
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = createStyles(() => ({
  container: {
    flex: 1,
    padding: space['3xl'],
  },

  // ===== dApp Banner =====
  // De-containered (Wise): an open "who's asking" header, separated from the
  // action below by a hairline instead of a gray card.
  dappBanner: {
    paddingTop: space.sm,
    paddingBottom: space.xl,
    borderBottomWidth: 1,
    borderBottomColor: color.border.base,
    marginBottom: space.xl,
    gap: space.md,
  },
  dappRow1: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  dappLogo: {
    width: 36, height: 36, borderRadius: 10,
  },
  dappLogoFallback: {
    backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  dappLogoText: {
    fontSize: text.lg, ...inter.bold, color: color.accent.base,
  },
  dappInfo: { flex: 1, gap: 1 },
  dappName: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  dappDomain: {
    fontSize: text.xs, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.fg.muted,
  },
  dappChainRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    marginLeft: 'auto',
  },
  dappChainName: {
    fontSize: text.xs, ...inter.semibold, color: color.fg.base,
  },
  dappAccountLine: {
    fontSize: text.xs, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.fg.muted,
    paddingLeft: space.sm,
  },

  // ===== Intent Header =====
  intentHeader: {
    alignItems: 'center',
    paddingTop: space.lg,
    paddingBottom: space['2xl'],
  },
  intentText: {
    fontSize: text['5xl'],
    ...inter.bold,
    textAlign: 'center',
    letterSpacing: -1,
  },
  // Eyebrow kicker — a small colored verb that cedes the headline to the asset flow.
  intentEyebrow: {
    flexDirection: 'row',
    alignItems: 'center',
    alignSelf: 'center',
    gap: space.sm,
    paddingTop: space.md,
    paddingBottom: space.lg,
  },
  intentEyebrowDot: { width: 7, height: 7, borderRadius: 4 },
  intentEyebrowText: {
    fontSize: text.base,
    ...inter.semibold,
    letterSpacing: 0.2,
  },

  // ===== Token Card =====
  // Open row (Wise de-container) for benign amounts — no card, just the number
  // breathing next to its logo, aligned to the sheet's content edge.
  tokenRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xl,
    paddingVertical: space.lg,
  },
  // Tinted card — caution/danger only, so a filled surface always means "attention".
  tokenCard: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xl,
    paddingVertical: space['2xl'],
    paddingHorizontal: space['2xl'],
    borderRadius: radius['2xl'],
    marginVertical: space.sm,
  },
  tokenInfo: { flex: 1, minWidth: 0 },
  tokenAmount: {
    // The hero of a benign transfer now that the verb is a mere eyebrow.
    // adjustsFontSizeToFit shrinks long amounts so the ticker never clips.
    fontSize: text['3xl'],
    ...inter.bold,
    color: color.fg.base,
    letterSpacing: -0.6,
  },
  tokenLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
    marginTop: space.xs,
  },
  tokenWarning: {
    width: 28, height: 28, borderRadius: 14,
    backgroundColor: color.error.soft,
    alignItems: 'center', justifyContent: 'center',
  },

  // ===== Flow Arrow =====
  flowArrow: {
    alignItems: 'center',
    marginVertical: -space.sm,
    zIndex: 1,
  },
  flowCircle: {
    width: 30, height: 30, borderRadius: 15,
    backgroundColor: color.bg.raised,
    borderWidth: 2, borderColor: color.border.base,
    alignItems: 'center', justifyContent: 'center',
    ...shadow.sm,
  },
  flowCircleDanger: {
    borderColor: color.error.base,
  },

  // ===== Contract Bar =====
  // De-containered (Wise): an open recipient/contract row separated from the
  // asset flow above by a hairline, not a gray card.
  contractBar: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    borderTopWidth: 1,
    borderTopColor: color.border.base,
  },
  // A flagged recipient DOES get a tinted card back — danger should contain itself.
  contractBarWarning: {
    borderTopWidth: 0,
    paddingHorizontal: space.xl,
    backgroundColor: color.error.soft,
    borderWidth: 1,
    borderColor: color.error.base,
    borderRadius: radius.xl,
    marginVertical: space.md,
  },
  contractInfo: { flex: 1, gap: 2 },
  contractLabel: {
    fontSize: scaleFont(10), ...inter.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.3,
  },
  contractAddrRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    flexWrap: 'wrap',
  },
  contractName: {
    fontSize: text.sm, ...inter.semibold, color: color.success.base,
  },
  contractNameNeutral: { color: color.fg.base },
  sourceTag: {
    fontSize: scaleFont(9), ...inter.semibold, color: color.fg.subtle,
    backgroundColor: color.bg.sunken, overflow: 'hidden',
    paddingHorizontal: 5, paddingVertical: 1, borderRadius: radius.sm,
    textTransform: 'uppercase', letterSpacing: 0.3,
  },
  contractAddr: {
    fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.fg.muted,
  },
  copyBtn: {
    width: 28, height: 28, borderRadius: radius.md,
    borderWidth: 1, borderColor: color.border.base,
    backgroundColor: color.bg.raised,
    alignItems: 'center', justifyContent: 'center',
  },
  copyBtnDone: {
    borderColor: color.success.base,
    backgroundColor: color.success.soft,
  },
  verifiedBadge: {
    width: 24, height: 24,
    alignItems: 'center', justifyContent: 'center',
  },

  // ===== Warning Banner =====
  warnBanner: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    borderRadius: radius.xl,
    marginVertical: space.md,
  },
  warnCaution: {
    backgroundColor: color.warning.soft,
    borderWidth: 1, borderColor: color.warning.border,
  },
  warnDanger: {
    backgroundColor: color.error.soft,
    borderWidth: 1, borderColor: color.error.base,
  },
  warnText: {
    fontSize: text.sm, ...inter.semibold, flex: 1, lineHeight: 18,
  },

  // ===== Generic Fields =====
  genericFields: {
    marginVertical: space.md,
  },
  // De-containered (Wise): open rows split by hairlines, not stacked gray cards.
  genRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    paddingVertical: space.md,
    borderTopWidth: 1,
    borderTopColor: color.border.base,
    gap: space.lg,
  },
  genRowWarning: {
    marginHorizontal: -space.xl,
    paddingHorizontal: space.xl,
    borderTopWidth: 0,
    backgroundColor: color.warning.soft,
    borderRadius: radius.lg,
  },
  genLabel: {
    fontSize: text.sm, ...inter.medium, color: color.fg.muted,
    flexShrink: 0,
  },
  genValue: {
    fontSize: text.sm, ...inter.semibold, color: color.fg.base,
    textAlign: 'right', flex: 1,
    fontFamily: font.mono, fontWeight: '500' as const,
  },

  // ===== Message Bubble =====
  msgBubble: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius['2xl'],
    padding: space['2xl'],
    marginVertical: space.md,
  },
  msgTag: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    alignSelf: 'center',
    paddingVertical: space.xs,
    paddingHorizontal: space.lg,
    backgroundColor: color.border.base,
    borderRadius: radius.full,
    marginBottom: space.xl,
  },
  msgTagText: {
    fontSize: scaleFont(10), ...inter.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.3,
  },
  msgText: {
    fontSize: text.base, ...inter.regular,
    color: color.fg.base,
    lineHeight: 22,
    textAlign: 'center',
  },

  // (context strip merged into dApp banner)

  // ===== Details Toggle =====
  detailsToggle: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    paddingVertical: space.md,
  },
  detailsToggleText: {
    fontSize: text.xs, ...inter.semibold, color: color.fg.subtle,
  },

  // ===== Raw Data =====
  rawBlock: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.lg,
    maxHeight: 160,
    marginBottom: space.lg,
  },
  rawText: {
    // Readable calldata: 9px + lowest-contrast ink made the raw viewer illegible.
    // 12px mono on fg.muted stays quiet without forcing a squint.
    fontSize: scaleFont(12), fontFamily: font.mono, fontWeight: '400' as const,
    color: color.fg.muted, lineHeight: 18,
  },

  // ===== Fallback =====
  fallback: {
    alignItems: 'center',
    paddingVertical: space['5xl'],
    gap: space.lg,
  },
  fallbackText: {
    fontSize: text.lg, ...inter.regular, color: color.fg.muted,
  },

  // ===== Error =====
  errorCard: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.error.soft,
    borderRadius: radius.lg,
    marginBottom: space.lg,
  },
  errorText: { fontSize: text.sm, ...inter.regular, color: color.error.base, flex: 1 },

  // ===== Pending (submitted, awaiting receipt) =====
  pendingCard: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.info.soft,
    borderRadius: radius.lg,
    marginBottom: space.lg,
  },
  pendingText: {
    fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.info.base, flex: 1,
  },

  // Read-only replay banner
  historyNote: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingVertical: space.md,
    paddingHorizontal: space.lg,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    marginBottom: space.lg,
  },
  historyNoteText: {
    fontSize: text.sm, ...inter.medium, color: color.fg.muted, flex: 1,
  },

  // ===== Buttons =====
  buttonRow: {
    flexDirection: 'row', gap: space.lg,
    paddingTop: space.xl,
    borderTopWidth: 1,
    borderTopColor: color.border.base,
    marginTop: space.sm,
  },
  buttonFlex: { flex: 1 },
  // Danger signing stacks the full-width slide over a full-width Reject.
  dangerStack: { flex: 1, gap: space.md },

  // ===== Batch (EIP-5792) breakdown =====
  batchSub: {
    fontSize: text.sm, ...inter.regular, color: color.fg.muted,
    textAlign: 'center', marginTop: -space.md, marginBottom: space.lg,
  },
  batchRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.lg, paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken, borderRadius: radius.xl, marginVertical: space.sm,
  },
  batchRowDanger: { borderWidth: 1, borderColor: color.error.base },
  batchNum: {
    width: 24, height: 24, borderRadius: 12, backgroundColor: color.bg.raised,
    alignItems: 'center', justifyContent: 'center',
  },
  batchNumText: { fontSize: text.xs, ...inter.bold, color: color.fg.muted },
  batchInfo: { flex: 1, gap: 1 },
  batchTitle: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  batchDetail: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  batchAddr: { fontSize: text.xs, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.subtle },
  // Off-chain permit risk hint, under the permit card.
  permitHint: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, lineHeight: 18, marginTop: space.xs },
  // Editable approval leg: numbered header above the inline spending-cap editor.
  batchEditLeg: { marginVertical: space.sm },
  batchEditHead: { flexDirection: 'row', alignItems: 'center', gap: space.md, marginBottom: space.xs },
  batchEditTitle: { flex: 1, fontSize: text.base, ...inter.semibold, color: color.fg.base },

  // ===== Advanced panel =====
  advancedBody: { gap: space.sm, marginBottom: space.md },
  advancedRaw: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.lg,
    maxHeight: 260,
  },

  // ===== Token-card USD line =====
  tokenSubRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    marginTop: space.xs, flexWrap: 'wrap',
  },
  tokenUsd: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle },

  // ===== SIWE verified-domain confirmation row =====
  siweOkRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    paddingVertical: space.sm, paddingHorizontal: space.sm, marginBottom: space.xs,
  },
  siweOkText: { fontSize: text.xs, ...inter.medium, color: color.success.base },

  // ===== Recipient-risk tags (first-time / contract) =====
  riskTag: {
    fontSize: scaleFont(9), ...inter.semibold, color: color.fg.subtle,
    backgroundColor: color.bg.raised, overflow: 'hidden',
    paddingHorizontal: 5, paddingVertical: 1, borderRadius: radius.sm,
    textTransform: 'uppercase', letterSpacing: 0.3,
  },
  riskTagWarn: {
    color: color.warning.base, backgroundColor: color.warning.soft,
  },

  // ===== increaseAllowance resulting total =====
  allowanceTotalRow: {
    paddingVertical: space.md, paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken, borderRadius: radius.lg,
    marginVertical: space.sm, gap: 2,
  },
  allowanceTotalLabel: {
    fontSize: scaleFont(10), ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase' as const, letterSpacing: 0.3,
  },
  allowanceTotalValue: {
    fontSize: text.sm, ...inter.semibold, color: color.fg.base, fontFamily: font.mono,
  },
  allowanceTotalUnknown: {
    fontSize: text.sm, ...inter.medium, color: color.warning.base, lineHeight: 18,
  },

  // ===== eth_sign danger surface =====
  ethSignCard: {
    backgroundColor: color.error.soft, borderRadius: radius['2xl'],
    padding: space['2xl'], marginVertical: space.md, gap: space.md,
    borderWidth: 1, borderColor: color.error.base + '40',
  },
  ethSignHeader: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  ethSignTitle: { fontSize: text.base, ...inter.bold, color: color.error.base },
  ethSignBody: { fontSize: text.sm, ...inter.regular, color: color.fg.base, lineHeight: 19 },
  ethSignHash: {
    fontSize: scaleFont(11), fontFamily: font.mono, fontWeight: '400' as const,
    color: color.fg.muted, backgroundColor: color.bg.sunken,
    padding: space.md, borderRadius: radius.md,
  },
}));
