/**
 * Gas-account funding sheet — the FALLBACK surface, shown only when silent
 * sponsorship (attemptSilentSponsorship) could not make the gas account usable.
 * Most users never see it: eligible wallets are sponsored invisibly inside the
 * Send/approve flow.
 *
 * Modes:
 *   topup      — a payment request, not an "account activation" ceremony:
 *                fiat-anchored amount, tap-to-copy address, open-in-wallet
 *                deep link (EIP-681), collapsed QR, exchange network hint.
 *                Carries one honest reason line when sponsorship was denied,
 *                with automatic + manual free-top-up retries where retryable.
 *   confirming — funds are on their way (sponsorship granted, or its outcome
 *                unknown after a timeout); polls until the balance reflects
 *                it. NEVER rendered as an error — a successful sponsorship
 *                must not look like "unavailable" next to a payment QR.
 *   funded     — success beat, then auto-advance via onFunded (Send lands on
 *                the confirm step; dApp replays the pinned request — both
 *                still require the user's explicit confirm/passkey, so the
 *                auto-advance never skips consent).
 *
 * Rendered two ways (unchanged from the old modal):
 *  - standalone as <BundlerFundingModal> (its own AppModal) — the Send screen;
 *  - as an in-sheet content swap inside SigningRequestModal — iOS will NOT
 *    present a second native modal over an already-presented one
 *    (docs/KNOWN-BUGS.md BUG-1), so the sheet swaps content instead.
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { hapticSuccess, hapticLight, openURL } from '@/services/platform';
import { useCopyFeedback } from '@/hooks/use-copy-feedback';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { Check, ChevronDown, ChevronUp, Copy, Fuel, RefreshCw } from 'lucide-react-native';

import { AppModal } from './AppModal';
import { ChainLogo } from '@/components/ChainLogo';
import { formatWeiToEth as formatWei } from '@/services/format-eth';
import { chainName, getAllNetworksSync } from '@/models/network';
import { QRCode } from '@/components/QRCode';
import { VelaCard } from './VelaCard';
import { VelaButton } from './VelaButton';
import { buildEIP681 } from '@/services/eip681';
import { fetchChainlinkPrices, resolveChainlinkPrice } from '@/services/price-service';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';
import {
  fetchBundlerAccountInfo,
  clearBundlerCache,
  attemptSilentSponsorship,
  recommendedFundingWei,
  type FundingNeeded,
} from '@/services/bundler-service';
import { fundingShouldForce } from '@/services/dev/fault-injection';

interface Props {
  visible: boolean;
  funding: FundingNeeded;
  onFunded: () => void;
  onCancel: () => void;
}

interface ViewProps {
  funding: FundingNeeded;
  onFunded: () => void;
  onCancel: () => void;
  /** dApp surface: cancelling rejects the pending request back to the dApp —
   *  the cancel affordance must say so ("Cancel this transaction", not "Not now"). */
  dappVariant?: boolean;
}

type Mode = 'topup' | 'confirming' | 'funded';

/** Poll cadence: tight while funds are known to be in flight, relaxed while
 *  waiting on the user's own deposit. */
const POLL_CONFIRMING_MS = 5_000;
const POLL_TOPUP_MS = 10_000;
/** Give an in-flight sponsorship this long to reflect in the balance before
 *  degrading to the top-up layout (with the honest pending-unknown line). */
const CONFIRMING_MAX_MS = 45_000;
/** Auto-retry schedule for transient denials (index still syncing a brand-new
 *  wallet, service hiccup). Two shots: at 30s and 90s after entering top-up. */
const AUTO_RETRY_DELAYS_MS = [30_000, 60_000];
/** Success beat before auto-advancing — long enough to register, short enough
 *  not to feel like a stop. */
const FUNDED_ADVANCE_MS = 1_200;

/** Transient denials that a retry can genuinely heal (auto + manual). */
const AUTO_RETRYABLE = new Set([
  'no_passkey_registered', // async P-256 index hasn't synced this new wallet yet
  'service_unavailable',
  'passkey_index_unavailable',
  'network_error',
]);
/** Denials where a manual retry makes sense but auto-retry would be noise. */
const MANUAL_RETRYABLE = new Set(['transfer_failed', 'treasury_depleted', 'budget_exhausted']);

function denialKey(reason?: string): string {
  if (!reason) return 'componentsUi.funding.denialDefault';
  if (reason === 'no_passkey_registered') return 'componentsUi.funding.denialNotRecognized';
  if (reason === 'nonce_exceeded') return 'componentsUi.funding.denialNonceExceeded';
  if (reason === 'treasury_depleted' || reason === 'budget_exhausted') return 'componentsUi.funding.denialTreasuryDepleted';
  if (reason === 'wallet_balance_too_low') return 'componentsUi.funding.denialBalanceTooLow';
  if (reason === 'rate_limited') return 'componentsUi.funding.denialRateLimited';
  if (reason === 'pending_unknown') return 'componentsUi.funding.denialPendingUnknown';
  if (reason === 'service_unavailable' || reason === 'passkey_index_unavailable') return 'componentsUi.funding.denialServiceUnavailable';
  if (reason === 'network_error') return 'componentsUi.funding.denialNetworkError';
  if (reason.startsWith('transfer_failed')) return 'componentsUi.funding.denialTransferFailed';
  return 'componentsUi.funding.denialDefault';
}

export function BundlerFundingView({ funding, onFunded, onCancel, dappVariant }: ViewProps) {
  const { t } = useTranslation();
  const dc = useDisplayCurrency();

  const [mode, setMode] = useState<Mode>(funding.presentation === 'confirming' ? 'confirming' : 'topup');
  const [denialReason, setDenialReason] = useState<string | undefined>(funding.denialReason);
  const [retrying, setRetrying] = useState(false);
  const [checking, setChecking] = useState(false);
  const [showQr, setShowQr] = useState(false);
  const [showDetails, setShowDetails] = useState(false);
  const [nativeUsd, setNativeUsd] = useState<number | null>(null);

  const { copied, copy } = useCopyFeedback(2000);
  const confirmingSince = useRef(Date.now());
  const advancedRef = useRef(false);
  const autoRetryCount = useRef(0);
  const mountedRef = useRef(true);
  useEffect(() => () => { mountedRef.current = false; }, []);

  const requiredWei = funding.thresholdWei;
  const isNativeAsset = funding.nativeSym !== 'pathUSD';

  // Fiat anchor for the amount hero. Tempo's pathUSD is already USD.
  useEffect(() => {
    if (!isNativeAsset) { setNativeUsd(1); return; }
    let cancelled = false;
    fetchChainlinkPrices()
      .then(prices => {
        if (cancelled) return;
        setNativeUsd(resolveChainlinkPrice(funding.nativeSym, prices));
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [funding.nativeSym, isNativeAsset]);

  const advance = useCallback(() => {
    if (advancedRef.current) return;
    advancedRef.current = true;
    onFunded();
  }, [onFunded]);

  // Live balance — keeps the "amount to add" honest after a partial grant or
  // a mid-sheet deposit instead of overstating the ask from sheet-open state.
  const [liveBalance, setLiveBalance] = useState(funding.currentBalance);

  const checkBalance = useCallback(async (manual = false) => {
    if (manual) setChecking(true);
    clearBundlerCache(funding.chainId, funding.safeAddress);
    try {
      const info = await fetchBundlerAccountInfo(funding.chainId, funding.safeAddress);
      if (!mountedRef.current) return;
      if (info) setLiveBalance(info.spendableBalance);
      // vela.forceFunding() exists precisely to exercise THIS sheet — the real
      // balance is fine, so without the guard it would self-dismiss in ~1.5s.
      if (info && info.spendableBalance >= requiredWei && !fundingShouldForce(funding.chainId)) {
        setMode('funded');
        hapticSuccess();
      }
    } catch { /* ignore — next poll retries */ }
    if (mountedRef.current) setChecking(false);
  }, [funding.chainId, funding.safeAddress, requiredWei]);

  // Poll from the moment the sheet opens — deposits are detected without any
  // tap. Tight cadence while sponsorship money is in flight.
  useEffect(() => {
    if (mode === 'funded') return;
    checkBalance();
    const interval = setInterval(checkBalance, mode === 'confirming' ? POLL_CONFIRMING_MS : POLL_TOPUP_MS);
    return () => clearInterval(interval);
  }, [mode, checkBalance]);

  // A sponsorship that never reflects must degrade honestly, not hang forever.
  useEffect(() => {
    if (mode !== 'confirming') return;
    confirmingSince.current = Date.now();
    const timer = setTimeout(() => {
      if (!mountedRef.current) return;
      setDenialReason('pending_unknown');
      setMode('topup');
    }, CONFIRMING_MAX_MS);
    return () => clearTimeout(timer);
  }, [mode]);

  // Success beat → auto-advance. Both destinations still require the user's
  // explicit action (Send: confirm step; dApp: passkey prompt), so this never
  // skips consent.
  useEffect(() => {
    if (mode !== 'funded') return;
    const timer = setTimeout(advance, FUNDED_ADVANCE_MS);
    return () => clearTimeout(timer);
  }, [mode, advance]);

  const retryFree = useCallback(async (auto = false) => {
    if (retrying || mode !== 'topup') return;
    setRetrying(true);
    try {
      const result = await attemptSilentSponsorship(funding, { force: true });
      if (!mountedRef.current) return;
      if (result.outcome === 'funded') {
        setMode('funded');
        hapticSuccess();
      } else if (result.outcome === 'confirming') {
        setMode('confirming');
      } else if (!auto) {
        // Manual retry: show the fresh reason. Auto retry keeps the original
        // line — flickering between identical messages reads as a glitch.
        setDenialReason(result.denialReason);
      }
    } catch { /* leave the sheet as-is */ }
    if (mountedRef.current) setRetrying(false);
  }, [retrying, mode, funding]);

  // Transient denials heal themselves (e.g. the P-256 index finishing its sync
  // of a brand-new wallet) — retry quietly so the user who just waits gets the
  // free path with zero taps.
  useEffect(() => {
    if (mode !== 'topup' || !denialReason || !AUTO_RETRYABLE.has(denialReason)) return;
    if (autoRetryCount.current >= AUTO_RETRY_DELAYS_MS.length) return;
    const delay = AUTO_RETRY_DELAYS_MS[autoRetryCount.current];
    const timer = setTimeout(() => {
      autoRetryCount.current += 1;
      retryFree(true);
    }, delay);
    return () => clearTimeout(timer);
  }, [mode, denialReason, retryFree]);

  const copyAddress = () => {
    hapticLight();
    copy(funding.depositAddress);
  };

  const net = getAllNetworksSync().find(n => n.chainId === funding.chainId);

  // Practical top-up amount. On cheap chains the raw deficit can be dust
  // (< 0.000001) — useless to display and annoying to send, so floor at
  // 0.0001 (hundreds of transactions on those chains).
  const MIN_DISPLAY_WEI = 100_000_000_000_000n; // 0.0001
  const rawAmount = funding.thresholdWei > liveBalance
    ? recommendedFundingWei(funding.thresholdWei, liveBalance)
    : funding.recommendedWei;
  const displayWei = rawAmount < MIN_DISPLAY_WEI ? MIN_DISPLAY_WEI : rawAmount;
  const amountText = formatWei(displayWei);
  const amountUsd = nativeUsd ? (Number(displayWei) / 1e18) * nativeUsd : 0;

  const eip681 = isNativeAsset
    ? buildEIP681({ recipient: funding.depositAddress, chainId: funding.chainId, amount: amountText })
    : null;

  const openInWallet = () => {
    if (!eip681) return;
    hapticLight();
    // Best effort — if no wallet handles ethereum: URIs, fall back to putting
    // the address on the clipboard so the tap still leaves something useful.
    Promise.resolve(openURL(eip681)).catch(() => copy(funding.depositAddress));
  };

  const retryable = !!denialReason && (AUTO_RETRYABLE.has(denialReason) || MANUAL_RETRYABLE.has(denialReason));

  const header = (
    <View style={styles.header}>
      <View style={styles.iconWrap}>
        <Fuel size={22} color={color.accent.base} strokeWidth={2} />
      </View>
      <Text style={styles.title}>{t('componentsUi.funding.title')}</Text>
      <View style={styles.networkChip}>
        {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={16} />}
        <Text style={styles.networkLabel}>{chainName(funding.chainId)}</Text>
      </View>
    </View>
  );

  if (mode === 'funded') {
    return (
      <View style={styles.container}>
        {header}
        <View style={styles.centerBlock}>
          <View style={styles.successCircle}>
            <Check size={28} color={color.success.base} strokeWidth={2.5} />
          </View>
          <Text style={styles.fundedTitle}>{t('componentsUi.funding.fundedTitle')}</Text>
        </View>
        <VelaButton
          title={t('componentsUi.funding.continue')}
          variant="accent"
          onPress={advance}
        />
      </View>
    );
  }

  if (mode === 'confirming') {
    return (
      <View style={styles.container}>
        {header}
        <View style={styles.centerBlock}>
          <ActivityIndicator size="large" color={color.accent.base} />
          <Text style={styles.confirmingText}>{t('componentsUi.funding.statusConfirming')}</Text>
          <Text style={styles.autoCheckNote}>{t('componentsUi.funding.autoCheckNote')}</Text>
        </View>
        <Pressable style={styles.cancelBtn} onPress={onCancel}>
          <Text style={styles.cancelText}>
            {t(dappVariant ? 'componentsUi.funding.cancelDapp' : 'componentsUi.funding.cancel')}
          </Text>
        </Pressable>
      </View>
    );
  }

  // --- topup ---
  return (
    <View style={styles.container}>
      {header}

      {/* One honest sentence: why the free path didn't apply (or the lead-in
          when the sheet was entered directly). Informational tone — amber is
          reserved for true failures, not eligibility. */}
      <Text style={styles.statusLine}>
        {denialReason
          ? t(denialKey(denialReason), { symbol: funding.nativeSym, defaultValue: t('componentsUi.funding.denialDefault') })
          : t('componentsUi.funding.lead', { symbol: funding.nativeSym })}
      </Text>

      {retryable && (
        <Pressable style={styles.retryRow} onPress={() => retryFree(false)} disabled={retrying}>
          {retrying
            ? <ActivityIndicator size="small" color={color.accent.base} />
            : <RefreshCw size={13} color={color.accent.base} strokeWidth={2} />}
          <Text style={styles.retryText}>
            {t(retrying ? 'componentsUi.funding.retrying' : 'componentsUi.funding.retryFree')}
          </Text>
        </Pressable>
      )}

      {/* Amount — fiat-anchored (the number people can actually judge), token
          amount subordinated beneath it. */}
      <VelaCard style={styles.amountCard}>
        <Text style={styles.amountLabel}>{t('componentsUi.funding.amountLabel')}</Text>
        {amountUsd > 0.001 ? (
          <>
            <Text style={styles.amountFiat}>≈ {dc.fmt(amountUsd)}</Text>
            <Text style={styles.amountToken}>{amountText} {funding.nativeSym}</Text>
          </>
        ) : (
          <Text style={styles.amountFiat}>{amountText} {funding.nativeSym}</Text>
        )}
      </VelaCard>

      {/* Address — tap-to-copy is the primary affordance on the device the
          user is actually holding (they can't scan their own screen). */}
      <Pressable style={styles.addressCard} onPress={copyAddress}>
        <View style={styles.addressRow}>
          <Text style={styles.addressLabel}>{t('componentsUi.funding.addressLabel')}</Text>
          {copied ? (
            <Check size={14} color={color.accent.base} strokeWidth={3} />
          ) : (
            <Copy size={14} color={color.fg.subtle} strokeWidth={2} />
          )}
        </View>
        <Text style={styles.addressText} selectable>
          {funding.depositAddress}
        </Text>
      </Pressable>

      <Text style={styles.networkHint}>
        {t('componentsUi.funding.networkHint', { network: chainName(funding.chainId) })}
      </Text>

      {eip681 && (
        <VelaButton
          title={t('componentsUi.funding.openInWallet')}
          variant="secondary"
          onPress={openInWallet}
          style={styles.walletBtn}
        />
      )}

      {/* QR stays available for the genuine second-device case, collapsed so
          the on-device flow isn't led by an affordance it can't use. */}
      <Pressable style={styles.toggleRow} onPress={() => { hapticLight(); setShowQr(v => !v); }}>
        <Text style={styles.toggleText}>{t(showQr ? 'componentsUi.funding.hideQr' : 'componentsUi.funding.showQr')}</Text>
        {showQr ? <ChevronUp size={14} color={color.fg.subtle} /> : <ChevronDown size={14} color={color.fg.subtle} />}
      </Pressable>
      {showQr && (
        <View style={styles.qrBlock}>
          <View style={styles.qrWrap}>
            <QRCode value={eip681 ?? funding.depositAddress} size={132} />
          </View>
          {eip681 && <Text style={styles.qrHint}>{t('componentsUi.funding.qrHint')}</Text>}
        </View>
      )}

      {/* Deposits are auto-detected; the manual check is quiet reassurance. */}
      <View style={styles.autoCheckRow}>
        <Text style={styles.autoCheckNote}>{t('componentsUi.funding.autoCheckNote')}</Text>
        <Pressable onPress={() => checkBalance(true)} disabled={checking} hitSlop={8}>
          <Text style={styles.checkNowText}>
            {t(checking ? 'componentsUi.funding.checking' : 'componentsUi.funding.checkNow')}
          </Text>
        </Pressable>
      </View>

      <Pressable style={styles.toggleRow} onPress={() => setShowDetails(v => !v)}>
        <Text style={styles.toggleText}>{t('componentsUi.funding.detailsTitle')}</Text>
        {showDetails ? <ChevronUp size={14} color={color.fg.subtle} /> : <ChevronDown size={14} color={color.fg.subtle} />}
      </Pressable>
      {showDetails && (
        <Text style={styles.detailsBody}>{t('componentsUi.funding.detailsBody')}</Text>
      )}

      <Pressable style={styles.cancelBtn} onPress={onCancel}>
        <Text style={styles.cancelText}>
          {t(dappVariant ? 'componentsUi.funding.cancelDapp' : 'componentsUi.funding.cancel')}
        </Text>
      </Pressable>
    </View>
  );
}

/**
 * Standalone funding modal — its own native <AppModal>. Used by the Send
 * screen, where nothing else is presented so a native pageSheet is fine. The
 * signing sheet renders <BundlerFundingView> directly instead (BUG-1).
 */
export function BundlerFundingModal({ visible, funding, onFunded, onCancel }: Props) {
  return (
    <AppModal visible={visible} onClose={onCancel}>
      <BundlerFundingView funding={funding} onFunded={onFunded} onCancel={onCancel} />
    </AppModal>
  );
}

const styles = createStyles(() => ({
  container: {
    flex: 1,
    backgroundColor: color.bg.base,
    paddingHorizontal: space['2xl'],
    paddingTop: space.xl,
    paddingBottom: space.lg,
  },

  // Header
  header: {
    alignItems: 'center',
    marginBottom: space.lg,
  },
  iconWrap: {
    width: 44,
    height: 44,
    borderRadius: 22,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.sm,
  },
  title: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space.xs,
  },
  networkChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
    paddingHorizontal: space.md,
    paddingVertical: 3,
  },
  networkLabel: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.base,
  },

  // Status / lead line
  statusLine: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 20,
    marginBottom: space.md,
    paddingHorizontal: space.sm,
  },
  retryRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingHorizontal: space.sm,
    marginBottom: space.md,
  },
  retryText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.accent.base,
  },

  // Amount hero
  amountCard: {
    padding: space.lg,
    marginBottom: space.md,
    alignItems: 'center',
  },
  amountLabel: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    marginBottom: space.xs,
  },
  amountFiat: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    fontFamily: font.mono,
  },
  amountToken: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
    fontFamily: font.mono,
    marginTop: 2,
  },

  // Address
  addressCard: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.lg,
    marginBottom: space.sm,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  addressRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: space.xs,
  },
  addressLabel: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
  addressText: {
    fontSize: text.xs,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
    lineHeight: 18,
  },
  networkHint: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    paddingHorizontal: space.sm,
    marginBottom: space.md,
  },
  walletBtn: {
    marginBottom: space.sm,
  },

  // Disclosure rows (QR / details)
  toggleRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.xs,
    paddingVertical: space.md,
  },
  toggleText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
  },
  qrBlock: {
    alignItems: 'center',
    marginBottom: space.sm,
  },
  qrWrap: {
    padding: space.md,
    backgroundColor: '#FFFFFF',
    borderRadius: radius.xl,
    ...shadow.sm,
  },
  qrHint: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    marginTop: space.sm,
  },
  detailsBody: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 20,
    paddingHorizontal: space.sm,
    marginBottom: space.sm,
  },

  // Auto-check
  autoCheckRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.md,
    marginBottom: space.xs,
  },
  autoCheckNote: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
  },
  checkNowText: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.accent.base,
  },

  // Confirming / funded center blocks
  centerBlock: {
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space['3xl'],
  },
  confirmingText: {
    fontSize: text.base,
    ...inter.medium,
    color: color.fg.base,
    textAlign: 'center',
    paddingHorizontal: space.xl,
    lineHeight: 22,
  },
  successCircle: {
    width: 56,
    height: 56,
    borderRadius: 28,
    backgroundColor: color.success.soft,
    alignItems: 'center',
    justifyContent: 'center',
  },
  fundedTitle: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.base,
  },

  cancelBtn: {
    alignItems: 'center',
    paddingVertical: space.md,
  },
  cancelText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
  },
}));
