import { ChainLogo } from '@/components/ChainLogo';
import { QRCode } from '@/components/QRCode';
import { ReceiveRequestControls } from '@/components/ReceiveRequestControls';
import { ReceiveShareCard, type ShareCardModel } from '@/components/ReceiveShareCard';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { SectionLabel } from '@/components/ui/SectionLabel';
import { SegmentedToggle } from '@/components/ui/SegmentedToggle';
import { VelaButton } from '@/components/ui/VelaButton';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, leading, radius, space, text } from '@/constants/theme';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { chainName, getAllNetworksSync } from '@/models/network';
import { formatBalance, tokenBalanceDouble, tokenChainId, tokenId, type APIToken } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import { hapticLight, hapticSuccess, isAppActive, showAlert } from '@/services/platform';
import { useCopyFeedback } from '@/hooks/use-copy-feedback';
import { composeShareBlob, saveReceiveCard } from '@/services/share-card';
import { fetchTokens } from '@/services/wallet-api';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { ArrowLeft, Check, Copy, ImageDown, ShieldAlert } from 'lucide-react-native';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Platform, Pressable, ScrollView, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

// Aggressive polling: 3s for first 1 min, then 60s for next 4 min, then stop
const FAST_INTERVAL_MS = 3_000;
const SLOW_INTERVAL_MS = 60_000;
const FAST_PHASE_MS = 1 * 60_000;
const TOTAL_LISTEN_MS = 5 * 60_000;

// QR quiet zone must stay literal white in BOTH color schemes — scanners need
// the contrast, and every bg.* token darkens in dark mode.
const QR_QUIET_ZONE = '#FFFFFF';
// Deposit "landed" dot; item rows indent past it to align under the time label.
const DEPOSIT_DOT_SIZE = 6;
// The warning gate shows once per account, then decays to a one-line reminder.
const warnedStorageKey = (address: string) => `vela.receiveWarned.${address}`;

export default function ReceiveScreen() {
  const { t } = useTranslation();
  const router = useSafeRouter();
  const { activeAccount, state } = useWallet();
  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';
  const networks = getAllNetworksSync();

  const [depositDetected, setDepositDetected] = useState(false);
  interface DepositEntry { time: string; items: { symbol: string; amount: string; network: string; usd: string | null }[] }
  const [deposits, setDeposits] = useState<DepositEntry[]>([]);
  const previousTokens = useRef<APIToken[] | null>(null);
  const { copied, copy } = useCopyFeedback(2000);

  // Per-account acknowledge flag: null = loading (keep the QR covered so first
  // visits never flash it), false = show the gate, true = one-line reminder.
  const [warned, setWarned] = useState<boolean | null>(null);
  useEffect(() => {
    if (!address) return;
    let cancelled = false;
    setWarned(null);
    AsyncStorage.getItem(warnedStorageKey(address))
      .then((v) => { if (!cancelled) setWarned(v === '1'); })
      .catch(() => { if (!cancelled) setWarned(false); });
    return () => { cancelled = true; };
  }, [address]);
  const acknowledgeWarning = useCallback(() => {
    setWarned(true);
    if (address) AsyncStorage.setItem(warnedStorageKey(address), '1').catch(() => {});
  }, [address]);

  // Entrances play once (design language rule 10) — never replay on tab switch.
  const hasEntered = useRef(false);
  useEffect(() => { hasEntered.current = true; }, []);

  // Address vs EIP-681 payment-request mode.
  const [mode, setMode] = useState<'address' | 'request'>('address');
  const [request, setRequest] = useState<{ qrValue: string; summary: string; payLink: string }>({ qrValue: '', summary: '', payLink: '' });
  const [savingImage, setSavingImage] = useState(false);
  const cardRef = useRef<View>(null);
  // Web only: the most recent pre-rendered share image, so Save can hand it to
  // the OS share sheet synchronously (iOS drops the tap gesture after an await).
  const shareBlobRef = useRef<{ model: ShareCardModel; blob: Blob } | null>(null);

  const isRequest = mode === 'request';
  const qrValue = isRequest ? (request.qrValue || address) : address;

  // Deposit detection polling — quietly watches for incoming transfers while
  // this screen is open and surfaces them as they land (no persistent status).
  useEffect(() => {
    if (!address) return;
    previousTokens.current = null;
    const startTime = Date.now();
    let timerId: ReturnType<typeof setTimeout>;

    const checkDeposit = async () => {
      if (!isAppActive()) return;
      try {
        const tokens = await fetchTokens(address, { forceRefresh: true });

        if (previousTokens.current !== null) {
          // Guard: a smaller token set than baseline means a chain likely
          // failed — skip comparison to avoid false positives.
          if (tokens.length < previousTokens.current.length) {
            scheduleNext();
            return;
          }

          // Diff: find tokens whose balance increased vs baseline.
          const prevMap = new Map(previousTokens.current.map(tk => [tokenId(tk), tokenBalanceDouble(tk)]));
          const changes: DepositEntry['items'] = [];
          for (const tk of tokens) {
            const prevBal = prevMap.get(tokenId(tk)) ?? 0;
            const curBal = tokenBalanceDouble(tk);
            if (curBal > prevBal) {
              const diff = curBal - prevBal;
              changes.push({
                symbol: tk.symbol,
                amount: formatBalance(diff),
                network: chainName(tokenChainId(tk)),
                usd: tk.priceUsd ? `$${(diff * tk.priceUsd).toFixed(2)}` : null,
              });
            }
          }

          if (changes.length > 0) {
            const time = new Date().toLocaleTimeString('en-US', { hour12: false });
            setDepositDetected(true);
            setDeposits(prev => [{ time, items: changes }, ...prev]);
            hapticSuccess();
            previousTokens.current = tokens;
          }
        } else {
          // First fetch — record initial baseline.
          previousTokens.current = tokens;
        }
      } catch {}

      scheduleNext();
    };

    const scheduleNext = () => {
      const elapsed = Date.now() - startTime;
      if (elapsed >= TOTAL_LISTEN_MS) return;
      const interval = elapsed < FAST_PHASE_MS ? FAST_INTERVAL_MS : SLOW_INTERVAL_MS;
      timerId = setTimeout(checkDeposit, interval);
    };

    checkDeposit();
    return () => { clearTimeout(timerId); };
  }, [address]);

  const copyValue = useCallback(() => {
    // In request mode we copy the public payment LINK (a web page that bridges
    // to the Vela web wallet / other wallets), not the raw ethereum: URI.
    const value = isRequest ? request.payLink : address;
    if (!value) return;
    hapticLight();
    copy(value);
  }, [isRequest, request.payLink, address, copy]);

  const truncatedAddress = address
    ? `${address.slice(0, 8)}...${address.slice(-6)}`
    : '';

  // The branded card model for the current mode — drives both the off-screen
  // native capture target and the web canvas.
  const shareModel = useMemo<ShareCardModel>(() => (
    isRequest
      ? {
          variant: 'request',
          name: accountName,
          qrValue: request.qrValue || address || '',
          address: address || '',
          summary: request.summary,
        }
      : {
          variant: 'address',
          name: accountName,
          qrValue: address || '',
          address: address || '',
          networks: networks.map((n) => ({ label: n.iconLabel, name: n.displayName, color: n.iconColor, bg: n.iconBg, logoURL: n.logoURL })),
        }
  ), [isRequest, accountName, request.qrValue, request.summary, address, networks]);

  const shareFileName = `vela-${isRequest ? 'request' : 'address'}-${(address || '').slice(0, 10)}`;

  // Pre-render the share image (web) shortly after the card settles, so Save can
  // pass it to the OS share sheet without an await stealing the tap gesture.
  useEffect(() => {
    if (Platform.OS !== 'web') return;
    let cancelled = false;
    const id = setTimeout(() => {
      composeShareBlob(shareModel)
        .then((blob) => { if (!cancelled) shareBlobRef.current = { model: shareModel, blob }; })
        .catch(() => {});
    }, 400);
    return () => { cancelled = true; clearTimeout(id); };
  }, [shareModel]);

  const onSaveImage = useCallback(async () => {
    setSavingImage(true);
    try {
      const precomposed = shareBlobRef.current?.model === shareModel ? shareBlobRef.current.blob : undefined;
      const result = await saveReceiveCard(cardRef, shareModel, shareFileName, precomposed);
      if (result === 'saved') showAlert(t('receive.request.savedTitle'), t('receive.request.savedBody'));
      else if (result === 'downloaded') showAlert(t('receive.request.savedTitle'), t('receive.request.downloadedBody'));
      else if (result === 'denied') showAlert(t('receive.request.permTitle'), t('receive.request.permBody'));
      // 'shared' → the OS share sheet was the feedback; no alert needed.
    } catch {
      showAlert(t('common.error'), t('receive.request.shareError'));
    } finally {
      setSavingImage(false);
    }
  }, [shareModel, shareFileName, t]);

  // Save-image button — sits right under the copy button inside the QR card,
  // so it serves both Address and Request modes. Secondary action: muted icon +
  // ink label (accent is reserved for the copy action on each tab).
  const saveDisabled = savingImage || warned !== true;
  const saveButton = (
    <Pressable
      style={[styles.saveBtn, savingImage && styles.saveBtnBusy]}
      onPress={onSaveImage}
      disabled={saveDisabled}
      hitSlop={8}
      accessibilityRole="button"
      accessibilityLabel={t('receive.request.saveImage')}
      accessibilityState={{ disabled: saveDisabled, busy: savingImage }}
    >
      <ImageDown size={17} color={color.fg.muted} strokeWidth={2.2} />
      <Text style={styles.saveBtnText}>
        {t(savingImage ? 'receive.shareGenerating' : 'receive.request.saveImage')}
      </Text>
    </Pressable>
  );

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content}>
        {/* Header */}
        <View style={styles.header}>
          <Pressable
            onPress={() => router.back()}
            hitSlop={8}
            style={styles.navBtn}
            accessibilityRole="button"
            accessibilityLabel={t('receive.a11yBack')}
          >
            <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <Text style={styles.title}>{t('receive.title')}</Text>
          <View style={styles.headerSpacer} />
        </View>

        {/* Mode toggle: plain address vs EIP-681 payment request */}
        <View style={styles.segRow}>
          <SegmentedToggle<'address' | 'request'>
            options={[
              { key: 'address', label: t('receive.modeAddress') },
              { key: 'request', label: t('receive.modeRequest') },
            ]}
            value={mode}
            onChange={setMode}
          />
        </View>

        {/* QR — open on the page, no card */}
        <Animated.View entering={hasEntered.current ? undefined : fadeInDown(100, 400)}>
          <View style={styles.qrCardWrap}>
            <View style={styles.qrCard}>
              {/* QR — keeps its own white quiet-zone frame (required to scan) */}
              <View style={styles.qrBorder}>
                {qrValue ? (
                  <QRCode value={qrValue} size={200} />
                ) : (
                  <View style={styles.qrPlaceholder}>
                    <Text style={styles.qrPlaceholderText}>{t('receive.noAddress')}</Text>
                  </View>
                )}
              </View>

              {/* Acknowledged accounts get a one-line reminder instead of the gate */}
              {warned === true && (
                <Text style={styles.warningReminder}>{t('receive.warningReminder')}</Text>
              )}

              {/* Identity — below the QR; request mode also shows the receiving
                  address so the requester can self-check it */}
              <View style={styles.identity}>
                <Text style={styles.walletName}>{accountName}</Text>
                {isRequest && !!truncatedAddress && (
                  <Text style={styles.addressCaption} numberOfLines={1}>{truncatedAddress}</Text>
                )}
              </View>

              {/* Big, easy-to-tap copy button — THE accent action of each tab
                  (address tab: copy address; request tab: copy payment link) */}
              <Pressable
                onPress={warned === true ? copyValue : undefined}
                style={styles.copyBtn}
                accessibilityRole="button"
                accessibilityLabel={isRequest ? t('receive.copyRequestLink') : t('receive.a11yCopyAddress')}
              >
                <Text style={[styles.copyAddr, isRequest && styles.copyAddrRequest, copied && styles.copyAddrCopied]} numberOfLines={1}>
                  {copied ? t('receive.copied') : (isRequest ? t('receive.copyRequestLink') : truncatedAddress)}
                </Text>
                {copied ? (
                  <Check size={18} color={color.success.base} strokeWidth={2.5} />
                ) : (
                  <Copy size={18} color={color.accent.base} strokeWidth={2} />
                )}
              </Pressable>

              {/* Save the QR card as an image (both modes) */}
              {saveButton}

              {/* Deposit detected — surfaced as it lands */}
              {depositDetected && deposits.length > 0 && (
                <Animated.View style={styles.depositBox} entering={fadeIn(0, 300)}>
                  {deposits.map((entry, i) => (
                    <View key={i} style={[styles.depositEntry, i > 0 && styles.depositEntryBorder]}>
                      <View style={styles.depositHeader}>
                        <View style={styles.depositDot} />
                        <Text style={styles.depositTime}>{entry.time}</Text>
                      </View>
                      {entry.items.map((item, j) => (
                        <View key={j} style={styles.depositRow}>
                          <Text style={styles.depositAmount}>+{item.amount} {item.symbol}</Text>
                          <Text style={styles.depositMeta}>{item.network}{item.usd ? `  ${item.usd}` : ''}</Text>
                        </View>
                      ))}
                    </View>
                  ))}
                </Animated.View>
              )}
            </View>

            {/* Warning GATE — covers the QR while the flag loads (null) and until
                first acknowledgement (false); content only once we know it's new */}
            {warned !== true && (
              <View style={styles.warningOverlay}>
                {warned === false && (
                  <View style={styles.warningContent}>
                    <View style={styles.warningIconWrap}>
                      <ShieldAlert size={28} color={color.warning.base} strokeWidth={2} />
                    </View>
                    <Text style={styles.warningTitle}>{t('receive.warningTitle')}</Text>
                    <Text style={styles.warningText}>
                      {t('receive.warningBody')}
                    </Text>
                    <Text style={styles.warningReassure}>
                      {t('receive.warningCounterfactual')}
                    </Text>
                    <VelaButton
                      title={t('receive.warningConfirm')}
                      onPress={acknowledgeWarning}
                      style={styles.warningBtn}
                    />
                  </View>
                )}
              </View>
            )}
          </View>
        </Animated.View>

        {/* Lower half — ONE persistent Animated.View so switching tabs swaps the
            content without remounting (no entrance replay, no jolt) */}
        <Animated.View entering={hasEntered.current ? undefined : fadeInDown(200, 400)}>
          {isRequest ? (
            /* Request builder */
            address ? <ReceiveRequestControls recipient={address} onChange={setRequest} /> : null
          ) : (
            /* Supported networks — a wrapped logo strip, names live in the a11y label */
            <>
              <SectionLabel>{t('receive.networksLabel')}</SectionLabel>
              <View
                style={styles.networkStrip}
                accessible
                accessibilityLabel={networks.map((n) => n.displayName).join(', ')}
              >
                {networks.map((network) => (
                  <ChainLogo
                    key={network.id}
                    label={network.iconLabel}
                    color={network.iconColor}
                    bgColor={network.iconBg}
                    logoURL={network.logoURL}
                    size={22}
                  />
                ))}
              </View>
              <Text style={styles.networksLine}>
                {t('receive.networksLine', { count: networks.length })}
              </Text>
            </>
          )}
        </Animated.View>
      </ScrollView>

      {/* Off-screen branded card — the capture target for native share/save. */}
      {Platform.OS !== 'web' && (
        <View ref={cardRef} collapsable={false} style={styles.offscreen} pointerEvents="none">
          <ReceiveShareCard model={shareModel} />
        </View>
      )}
    </ScreenContainer>
  );
}

const styles = createStyles(() => ({
  content: {
    paddingBottom: 100,
  },
  // Rendered off-screen purely so react-native-view-shot can capture it.
  offscreen: {
    position: 'absolute',
    left: -10000,
    top: 0,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.lg,
    marginBottom: space.md,
  },

  // Mode toggle
  segRow: {
    flexDirection: 'row',
    marginBottom: space.xl,
  },

  // Request tab: the copy row reads as a text button (UI face, accent ink) —
  // inter.semibold replaces the mono face copyAddr sets.
  copyAddrRequest: {
    textAlign: 'center',
    ...inter.semibold,
    color: color.accent.base,
  },
  // Save-image button — secondary action: muted icon, ink label, no accent.
  saveBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    alignSelf: 'center',
    marginTop: space.xs,
    paddingVertical: space.md,
    paddingHorizontal: space.md,
    minHeight: 44,
  },
  saveBtnBusy: {
    opacity: 0.4,
  },
  saveBtnText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
  },
  navBtn: {
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
  },
  title: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  // Mirrors navBtn's width so the title sits optically centered.
  headerSpacer: { width: 40 },

  // QR Card wrapper (for overlay positioning)
  qrCardWrap: {
    position: 'relative',
    marginBottom: space.xl,
  },

  // Warning GATE — a deliberate acknowledge-before-receive surface. Kept as a
  // covering overlay (must obscure the QR until confirmed) but lightened: it
  // sits on the page color, compact, no heavy card fill.
  warningOverlay: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: color.bg.base,
    zIndex: 10,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: space.xl,
  },
  warningContent: {
    alignItems: 'center',
    gap: space.md,
  },
  warningIconWrap: {
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.xs,
  },
  warningTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    textAlign: 'center',
  },
  warningText: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: text.base * leading.relaxed,
  },
  // Positive counterfactual reassurance — reads as trust, not caution, so it's
  // tinted success rather than the muted caution copy above it.
  warningReassure: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.success.base,
    textAlign: 'center',
    lineHeight: text.sm * leading.relaxed,
    marginTop: space.sm,
  },
  warningBtn: {
    alignSelf: 'stretch',
    marginTop: space.md,
  },
  // Post-acknowledgement reminder — replaces the gate on later visits.
  warningReminder: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    marginBottom: space.lg,
  },

  // QR — open on the page (no card), content simply centered
  qrCard: {
    alignItems: 'center',
    paddingTop: space.md,
  },
  qrBorder: {
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.xl,
    padding: space['2xl'],
    marginBottom: space.xl,
    backgroundColor: QR_QUIET_ZONE,
  },
  qrPlaceholder: {
    width: 200,
    height: 200,
    borderRadius: radius.xl,
    backgroundColor: color.bg.sunken,
    alignItems: 'center',
    justifyContent: 'center',
  },
  qrPlaceholderText: {
    fontSize: text.base,
    color: color.fg.subtle,
  },
  identity: {
    alignItems: 'center',
    gap: space.xs,
    marginBottom: space.lg,
  },
  walletName: {
    fontSize: text['2xl'],
    ...inter.bold,
    color: color.fg.base,
  },
  // Request mode: the receiving address, so the requester can self-check it.
  addressCaption: {
    fontSize: text.sm,
    ...inter.regular,
    fontFamily: font.mono,
    color: color.fg.muted,
  },

  // Copy address — plain de-boxed row (no fill/border), still a large tap
  // target; stretched so the row width never jumps as the label changes.
  copyBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.md,
    alignSelf: 'stretch',
    paddingVertical: space.lg,
    paddingHorizontal: space.md,
    minHeight: 44,
  },
  copyAddr: {
    flexShrink: 1,
    fontSize: text.base,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
  },
  copyAddrCopied: {
    color: color.success.base,
  },

  // Deposit detection — an open, de-boxed "just landed" section (no filled card):
  // a hairline separates it from the actions above, success-tinted text carries
  // the positive state.
  depositBox: {
    marginTop: space.lg,
    paddingTop: space.lg,
    width: '100%',
    borderTopWidth: 1,
    borderTopColor: color.border.base,
  },
  depositEntry: {
    paddingVertical: space.md,
  },
  depositEntryBorder: {
    borderTopWidth: 1,
    borderTopColor: color.border.base,
  },
  depositHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginBottom: space.sm,
  },
  depositDot: {
    width: DEPOSIT_DOT_SIZE,
    height: DEPOSIT_DOT_SIZE,
    borderRadius: DEPOSIT_DOT_SIZE / 2,
    backgroundColor: color.success.base,
  },
  // Success ink stays on the dot + amount only; time/meta are plain muted text.
  depositTime: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.muted,
  },
  depositRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'baseline',
    // Inset past the dot so amounts align under the time label.
    paddingLeft: DEPOSIT_DOT_SIZE + space.sm,
    marginTop: space.xs,
  },
  depositAmount: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.success.base,
  },
  depositMeta: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
  },

  // Networks — a single wrapped strip of chain logos (no pill boxes)
  networkStrip: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: space.md,
    marginBottom: space.lg,
  },
  networksLine: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    marginBottom: space['4xl'],
  },
}));
