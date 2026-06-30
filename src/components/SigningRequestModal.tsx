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
import * as Clipboard from 'expo-clipboard';
import { AppModal } from '@/components/ui/AppModal';
import { VelaButton } from '@/components/ui/VelaButton';
import { SlideToConfirmButton } from '@/components/ui/SlideToConfirmButton';
import { useDAppConnection } from '@/models/dapp-connection';
import { useWallet } from '@/models/wallet-state';
import { shortAddr, isAddress, tokenLogoURLsByAddress, type BLEIncomingRequest } from '@/models/types';
import { chainName, nativeSymbol, nativeCoinLogoURL, explorerBaseURL, DEFAULT_NETWORKS } from '@/models/network';
import { openURL } from '@/services/platform';
import {
  resolveTransaction, resolveTypedData,
  type ClearSignResult, type ClearSignField, type SigningRisk,
} from '@/services/clear-signing';
import { color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';
import { BundlerFundingModal } from '@/components/ui/BundlerFundingModal';
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
import { KnownContactBadge } from '@/components/contacts/KnownContactBadge';
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
  onApprove: (opts?: { maxFeePerGas?: bigint; bundlerCostWei?: bigint; paramsOverride?: any[] }) => void;
  onReject: () => void;
  onDismiss: () => void;
  /**
   * Read-only replay: re-render a PAST signature exactly as it was shown, with no
   * approve/reject and no live-only work (gas estimate, simulation, funding). Used
   * by the Connections panel to "look back at what I signed" (and to re-open an
   * in-flight op's status after the sheet was closed). Defaults to false.
   */
  readOnly?: boolean;
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
        simulateAssetChanges(
          activeAccount.address,
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
  }, [incomingRequest, chainId, activeAccount?.address, readOnly]);

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
      simulateAssetChanges(activeAccount.address, simCalls, chainId)
        .then((r) => { if (!cancelled) setSim(r); })
        .catch(() => { if (!cancelled) setSim(null); });
    }
    return () => { cancelled = true; };
  }, [incomingRequest, chainId, activeAccount?.address, readOnly]);

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
      return <ClearSignView cs={clearSign} />;
    }
    // EIP-5792 batch — list each call instead of blind-signing the bundle.
    if (isBatch && batch) {
      return <BatchCallsView items={batch} />;
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
      return <BlindTransactionView tx={params[0]} chainId={chainId} />;
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
      const i = clearSign.intent;
      // Truncate long intents: "Manage operator rights for" → "Confirm"
      if (i.length > 12) return t('componentsUi.signing.confirmLabel');
      return t('componentsUi.signing.confirmIntentLabel', { intent: i.charAt(0).toUpperCase() + i.slice(1) });
    }
    if (isPersonalSign || isTypedData) return t('componentsUi.signing.signLabel');
    if (isBatch) return t('componentsUi.signing.confirmLabel');
    return t('componentsUi.signingApprove.verbApprove');
  };

  const buttonVariant = (): 'accent' | 'secondary' => 'accent';

  // Danger actions must not be one tap from a normal transfer: an unlimited/
  // grant-all approval or an opaque eth_sign requires a deliberate press-and-hold.
  // This covers single requests AND every leg of an EIP-5792 batch / a
  // non-editable unbounded approval (e.g. permit2-batch), which would otherwise
  // slip through the gate even though the submit guard rejects them.
  const isGrantingApproval = (a: DetectedApproval | null | undefined) =>
    !!a?.isUnbounded && !a.isReducing && !a.isBooleanGrant;
  const batchHasDanger =
    isBatch && (batch?.some((it) => isGrantingApproval(it.approval) || it.clearSign?.risk === 'danger') ?? false);
  const requiresHold =
    isEthSign ||
    clearSign?.risk === 'danger' ||
    (!!approval?.editable && approveChoice?.type === 'grant') ||
    isGrantingApproval(approval) ||
    batchHasDanger;

  const confirm = () => {
    // For an edited approval, re-encode to the chosen finite amount BEFORE submit.
    // The independent guard re-checks at the submit chokepoint, so a rewrite
    // failure fails closed (never unbounded).
    let paramsOverride: any[] | undefined;
    if (approval?.editable && approveChoice) {
      try { paramsOverride = rewriteApprovalParams(method, params, approval, approveChoice); }
      catch { paramsOverride = undefined; }
    }
    onApprove({
      maxFeePerGas: feeEstimate?.maxFeePerGas,
      // Raw bundler cost (tier markup removed) drives the funding pre-check.
      bundlerCostWei: feeEstimate ? rawBundlerGasCost(feeEstimate) : undefined,
      paramsOverride,
    });
  };

  const confirmDisabled =
    resolving
    || (isTx && (estimatingGas || gasEstimateFailed))
    || (!!approval?.editable && !approveChoice);

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
              render path shared with Send's confirm step. Live-only (skipped on replay). */}
          {!readOnly && (isTx || isBatch) && <BalanceChangePreview result={sim} chainId={chainId} />}

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
                tone="danger"
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
    incomingRequest, isSigning, signError, pendingOpHash, chainId, dappInfo,
    approveRequest, rejectRequest, dismissRequest,
    fundingNeeded, handleFundingComplete, handleFundingCancel,
  } = useDAppConnection();
  const { activeAccount } = useWallet();

  if (!incomingRequest) return null;

  return (
    <>
      {/* Closing AFTER submit (pendingOpHash set) must not reject — the op is
          already in-flight and will complete + record; only dismiss the sheet. */}
      <AppModal visible={true} onClose={signError || pendingOpHash ? dismissRequest : rejectRequest}>
        <SigningSheet
          request={incomingRequest}
          chainId={chainId}
          account={activeAccount ?? null}
          dappInfo={dappInfo}
          isSigning={isSigning}
          signError={signError}
          pendingOpHash={pendingOpHash}
          onApprove={approveRequest}
          onReject={rejectRequest}
          onDismiss={dismissRequest}
        />
      </AppModal>

      {fundingNeeded && (
        <BundlerFundingModal
          visible={true}
          funding={fundingNeeded}
          onFunded={handleFundingComplete}
          onCancel={handleFundingCancel}
        />
      )}
    </>
  );
}

// ===========================================================================
// dApp Banner
// ===========================================================================

function DAppBanner({ name, domain, icon, chainId, accountName, accountAddress }: {
  name: string;
  domain?: string;
  icon?: string;
  chainId: number;
  accountName?: string;
  accountAddress?: string;
}) {
  const net = DEFAULT_NETWORKS.find(n => n.chainId === chainId);

  return (
    <View style={styles.dappBanner}>
      {/* Row 1: dApp identity ←→ chain */}
      <View style={styles.dappRow1}>
        {icon ? (
          <Image source={{ uri: icon }} style={styles.dappLogo} />
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

function ClearSignView({ cs }: {
  cs: ClearSignResult;
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

      {/* L1: Intent */}
      <IntentHeader intent={cs.intent} color={rc} />

      {/* Best-effort decode (recovered from a 4-byte selector DB, no verified
          descriptor) — show what it does, but be honest it isn't verified. */}
      {cs.bestEffort && (
        <WarningBanner
          severity="caution"
          text={t('componentsUi.signing.bestEffortWarning')}
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

function BlindTransactionView({ tx, chainId }: {
  tx: any;
  chainId: number;
}) {
  const { t } = useTranslation();
  const [showRaw, setShowRaw] = useState(false);
  const sym = nativeSymbol(chainId);
  const value = formatTxValue(tx.value, chainId);
  const hasData = tx.data && tx.data !== '0x';
  const dataSize = hasData ? Math.floor((tx.data.length - 2) / 2) : 0;

  return (
    <View>
      {/* context shown in dApp banner */}
      <IntentHeader
        intent={hasData ? t('componentsUi.signing.intentUnknown') : t('componentsUi.signing.intentSend')}
        color={hasData ? color.error.base : color.fg.base}
      />

      {/* Value card */}
      {value !== `0 ${sym}` && (
        <TokenCard
          field={{ label: t('componentsUi.signing.valueLabel'), value, format: 'amount', role: 'send-amount' }}
          variant={hasData ? 'danger' : 'send'}
        />
      )}

      {hasData && <FlowArrow danger />}

      {/* Contract */}
      <ContractBar
        label={hasData ? t('componentsUi.signing.unverifiedLabel') : t('componentsUi.signing.recipientLabel')}
        address={tx.to}
        verified={false}
        warning={hasData}
      />

      {/* Blind sign warning */}
      {hasData && (
        <>
          <WarningBanner
            severity="danger"
            text={t('componentsUi.signing.blindDecodeWarning', { bytes: dataSize })}
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

function BatchCallsView({ items }: { items: BatchItem[] }) {
  const { t } = useTranslation();
  const anyUnlimited = items.some((it) => it.approval?.isUnbounded && !it.approval.isReducing && !it.approval.isBooleanGrant);

  return (
    <View>
      <IntentHeader intent={t('componentsUi.signing.batchIntent')} color={color.fg.base} />
      <Text style={styles.batchSub}>{t('componentsUi.signing.batchSubtitle', { count: items.length })}</Text>

      {items.map((it, i) => {
        const unlimited = !!it.approval?.isUnbounded && !it.approval.isReducing && !it.approval.isBooleanGrant;
        const title = it.clearSign?.intent
          ?? (it.approval ? t('componentsUi.signingApprove.verbApprove') : t('componentsUi.signing.batchCall'));
        const summary = batchSummary(it);
        return (
          <View key={i} style={[styles.batchRow, unlimited && styles.batchRowDanger]}>
            <View style={styles.batchNum}>
              <Text style={styles.batchNumText}>{i + 1}</Text>
            </View>
            <View style={styles.batchInfo}>
              <Text style={styles.batchTitle} numberOfLines={1}>{title}</Text>
              {!!summary && <Text style={styles.batchDetail} numberOfLines={1}>{summary}</Text>}
              <Text style={styles.batchAddr} numberOfLines={1}>{it.to ? shortAddr(it.to) : '—'}</Text>
            </View>
            {unlimited && <ShieldAlert size={14} color={riskColors().danger} strokeWidth={2} />}
          </View>
        );
      })}

      {anyUnlimited && (
        <WarningBanner severity="danger" text={t('componentsUi.signing.unlimitedWarning')} />
      )}
    </View>
  );
}

// ===========================================================================
// Shared sub-components
// ===========================================================================

function IntentHeader({ intent, color: intentColor }: { intent: string; color: string }) {
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
  // Calm by default: benign amounts sit on a neutral card; color is reserved for
  // caution/danger so a tinted card always means "pay attention".
  const bgMap = {
    send: { backgroundColor: color.bg.sunken },
    receive: { backgroundColor: color.bg.sunken },
    caution: { backgroundColor: color.warning.soft },
    danger: { backgroundColor: color.error.soft },
  };

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

  return (
    <View style={[styles.tokenCard, bgMap[variant]]}>
      <TokenLogo
        symbol={symbol ?? '?'}
        logoUrls={logoUrls}
        size={40}
      />
      <View style={styles.tokenInfo}>
        <Text style={styles.tokenAmount} numberOfLines={1}>{field.value}</Text>
        <View style={styles.tokenSubRow}>
          <Text style={styles.tokenLabel}>{field.label}</Text>
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
        <KnownContactBadge address={address} compact />
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
        <Pressable onPress={() => openURL(explorerUrl)} hitSlop={8} style={styles.copyBtn}>
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
      <Text style={styles.genLabel}>{field.label}</Text>
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
              <Text style={styles.genLabel}>{f.label}</Text>
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
  dappBanner: {
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.xl,
    marginBottom: space['2xl'],
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

  // ===== Token Card =====
  tokenCard: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xl,
    paddingVertical: space['2xl'],
    paddingHorizontal: space['2xl'],
    borderRadius: radius['2xl'],
    marginVertical: space.sm,
  },
  tokenInfo: { flex: 1 },
  tokenAmount: {
    fontSize: text['3xl'],
    ...inter.bold,
    color: color.fg.base,
    letterSpacing: -0.5,
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
  contractBar: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.xl,
    marginVertical: space.md,
  },
  contractBarWarning: {
    borderWidth: 1,
    borderColor: color.error.base,
  },
  contractInfo: { flex: 1, gap: 2 },
  contractLabel: {
    fontSize: 10, ...inter.semibold,
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
    fontSize: 9, ...inter.semibold, color: color.fg.subtle,
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
    gap: space.sm,
    marginVertical: space.md,
  },
  genRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    gap: space.lg,
  },
  genRowWarning: {
    backgroundColor: color.warning.soft,
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
    fontSize: 10, ...inter.semibold,
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
    maxHeight: 80,
    marginBottom: space.lg,
  },
  rawText: {
    fontSize: 9, fontFamily: font.mono, fontWeight: '400' as const,
    color: color.fg.subtle, lineHeight: 14,
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

  // ===== Advanced panel =====
  advancedBody: { gap: space.sm, marginBottom: space.md },
  advancedRaw: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.lg,
    maxHeight: 180,
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
    fontSize: 9, ...inter.semibold, color: color.fg.subtle,
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
    fontSize: 10, ...inter.semibold, color: color.fg.subtle,
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
    fontSize: 11, fontFamily: font.mono, fontWeight: '400' as const,
    color: color.fg.muted, backgroundColor: color.bg.sunken,
    padding: space.md, borderRadius: radius.md,
  },
}));
