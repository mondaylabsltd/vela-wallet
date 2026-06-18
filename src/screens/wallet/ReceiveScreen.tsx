import { ChainLogo } from '@/components/ChainLogo';
import { QRCode } from '@/components/QRCode';
import { ReceiveRequestControls } from '@/components/ReceiveRequestControls';
import { ReceiveShareCard, type ShareCardModel } from '@/components/ReceiveShareCard';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { chainName, getAllNetworksSync } from '@/models/network';
import { formatBalance, tokenBalanceDouble, tokenChainId, tokenId, type APIToken } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import { copyToClipboard, hapticLight, hapticSuccess, isAppActive, showAlert } from '@/services/platform';
import { composeShareBlob, saveReceiveCard } from '@/services/share-card';
import { fetchTokens } from '@/services/wallet-api';
import { ArrowLeft, Check, Copy, Download, ShieldAlert } from 'lucide-react-native';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Platform, Pressable, ScrollView, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

// Aggressive polling: 3s for first 1 min, then 60s for next 4 min, then stop
const FAST_INTERVAL_MS = 3_000;
const SLOW_INTERVAL_MS = 60_000;
const FAST_PHASE_MS = 1 * 60_000;
const TOTAL_LISTEN_MS = 5 * 60_000;

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
  const [copied, setCopied] = useState(false);
  const [warningDismissed, setWarningDismissed] = useState(false);

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

  const copyValue = useCallback(async () => {
    // In request mode we copy the public payment LINK (a web page that bridges
    // to the Vela web wallet / other wallets), not the raw ethereum: URI.
    const value = isRequest ? request.payLink : address;
    if (!value) return;
    await copyToClipboard(value);
    hapticLight();
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [isRequest, request.payLink, address]);

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
  // so it serves both Address and Request modes.
  const saveButton = (
    <Pressable style={styles.saveBtn} onPress={onSaveImage} disabled={savingImage || !warningDismissed}>
      <Download size={17} color={color.accent.base} strokeWidth={2.2} />
      <Text style={styles.saveBtnText}>{t('receive.request.saveImage')}</Text>
    </Pressable>
  );

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content}>
        {/* Header */}
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.navBtn}>
            <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <Text style={styles.title}>{t('receive.title')}</Text>
          <View style={styles.headerSpacer} />
        </View>

        {/* Mode toggle: plain address vs EIP-681 payment request */}
        <View style={styles.segWrap}>
          {(['address', 'request'] as const).map((m) => (
            <Pressable
              key={m}
              style={[styles.segBtn, mode === m && styles.segBtnActive]}
              onPress={() => { hapticLight(); setMode(m); }}
            >
              <Text style={[styles.segText, mode === m && styles.segTextActive]}>
                {t(m === 'address' ? 'receive.modeAddress' : 'receive.modeRequest')}
              </Text>
            </Pressable>
          ))}
        </View>

        {/* QR Card */}
        <Animated.View entering={fadeInDown(100, 400)}>
          <View style={styles.qrCardWrap}>
            <VelaCard elevated style={styles.qrCard}>
              {/* QR */}
              <View style={styles.qrBorder}>
                {qrValue ? (
                  <QRCode value={qrValue} size={200} />
                ) : (
                  <View style={styles.qrPlaceholder}>
                    <Text style={styles.qrPlaceholderText}>{t('receive.noAddress')}</Text>
                  </View>
                )}
              </View>

              {/* Identity — below the QR */}
              <Text style={styles.walletName}>{accountName}</Text>

              {/* In request mode, show the human-readable summary above the copy button */}
              {isRequest && !!request.summary && (
                <Text style={styles.requestSummary} numberOfLines={2}>{request.summary}</Text>
              )}

              {/* Big, easy-to-tap copy button — copies the address or the EIP-681 URI */}
              <Pressable
                onPress={warningDismissed ? copyValue : undefined}
                style={[styles.copyBtn, copied && styles.copyBtnCopied]}
              >
                <Text style={[styles.copyAddr, isRequest && styles.copyAddrRequest, copied && styles.copyAddrCopied]} numberOfLines={1}>
                  {copied ? t('receive.copied') : (isRequest ? t('receive.copyRequestLink') : truncatedAddress)}
                </Text>
                {copied ? (
                  <Check size={18} color={color.success.base} strokeWidth={2.5} />
                ) : (
                  <Copy size={18} color={color.fg.muted} strokeWidth={2} />
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
            </VelaCard>

            {/* Warning overlay */}
            {!warningDismissed && (
              <View style={styles.warningOverlay}>
                <View style={styles.warningContent}>
                  <View style={styles.warningIconWrap}>
                    <ShieldAlert size={28} color={color.accent.base} strokeWidth={2} />
                  </View>
                  <Text style={styles.warningTitle}>{t('receive.warningTitle')}</Text>
                  <Text style={styles.warningText}>
                    {t('receive.warningBody')}
                  </Text>
                  <Pressable style={styles.warningBtn} onPress={() => setWarningDismissed(true)}>
                    <Text style={styles.warningBtnText}>{t('receive.warningConfirm')}</Text>
                  </Pressable>
                </View>
              </View>
            )}
          </View>
        </Animated.View>

        {isRequest ? (
          /* Request builder */
          <Animated.View entering={fadeInDown(200, 400)}>
            {address ? <ReceiveRequestControls recipient={address} onChange={setRequest} /> : null}
          </Animated.View>
        ) : (
          /* Supported networks */
          <Animated.View entering={fadeInDown(200, 400)}>
            <Text style={styles.sectionLabel}>{t('receive.networksLabel', { count: networks.length })}</Text>

            <View style={styles.networkGrid}>
              {networks.map((network) => (
                <View key={network.id} style={styles.networkChip}>
                  <ChainLogo
                    label={network.iconLabel}
                    color={network.iconColor}
                    bgColor={network.iconBg}
                    logoURL={network.logoURL}
                    size={22}
                  />
                  <Text style={styles.networkChipName} numberOfLines={1}>{network.displayName}</Text>
                </View>
              ))}
            </View>
          </Animated.View>
        )}
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
  segWrap: {
    flexDirection: 'row',
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
    padding: 4,
    marginBottom: space.xl,
  },
  segBtn: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    paddingVertical: space.md,
    borderRadius: radius.full,
  },
  segBtnActive: {
    backgroundColor: color.bg.raised,
    ...shadow.sm,
  },
  segText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.muted,
  },
  segTextActive: {
    color: color.fg.base,
  },

  // Request summary + share/save
  requestSummary: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
    textAlign: 'center',
    marginBottom: space.md,
  },
  copyAddrRequest: {
    fontFamily: undefined,
    textAlign: 'center',
  },
  // Save-image button — a friendly accent pill right under the copy button.
  saveBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    alignSelf: 'stretch',
    marginTop: space.md,
    paddingVertical: space.md,
    borderRadius: radius.lg,
    backgroundColor: color.accent.soft,
  },
  saveBtnText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.accent.base,
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
  headerSpacer: { minWidth: 50 },

  // QR Card wrapper (for overlay positioning)
  qrCardWrap: {
    position: 'relative',
    marginBottom: space.xl,
  },

  // Warning overlay
  warningOverlay: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: color.bg.raised,
    borderRadius: radius['2xl'],
    zIndex: 10,
    alignItems: 'center',
    justifyContent: 'center',
    padding: space['3xl'],
  },
  warningContent: {
    alignItems: 'center',
    gap: space.lg,
  },
  warningIconWrap: {
    width: 56,
    height: 56,
    borderRadius: 28,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.sm,
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
    lineHeight: 22,
  },
  warningBtn: {
    backgroundColor: color.accent.base,
    borderRadius: radius.lg,
    paddingVertical: space.lg,
    paddingHorizontal: space['4xl'],
    marginTop: space.lg,
    alignSelf: 'stretch',
    alignItems: 'center',
  },
  warningBtnText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.inverse,
  },

  // QR Card
  qrCard: {
    padding: space['3xl'],
    paddingTop: space['3xl'],
    paddingBottom: space['2xl'],
    alignItems: 'center',
  },
  qrBorder: {
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.xl,
    padding: space['2xl'],
    marginBottom: space.xl,
    backgroundColor: "#FFFFFF"
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
  walletName: {
    fontSize: text['2xl'],
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space.lg,
  },

  // Copy button — full-width, large tap target
  copyBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: space.md,
    alignSelf: 'stretch',
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    borderRadius: radius.lg,
    backgroundColor: color.bg.sunken,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  copyBtnCopied: {
    backgroundColor: color.success.soft,
    borderColor: color.success.base,
  },
  copyAddr: {
    flex: 1,
    fontSize: text.base,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
  },
  copyAddrCopied: {
    color: color.success.base,
  },

  // Deposit detection
  depositBox: {
    backgroundColor: color.success.soft,
    borderRadius: radius.lg,
    marginTop: space.lg,
    width: '100%',
    overflow: 'hidden',
  },
  depositEntry: {
    paddingHorizontal: space.xl,
    paddingVertical: space.lg,
  },
  depositEntryBorder: {
    borderTopWidth: 1,
    borderTopColor: color.success.base + '20',
  },
  depositHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginBottom: space.sm,
  },
  depositDot: {
    width: 6,
    height: 6,
    borderRadius: 3,
    backgroundColor: color.success.base,
  },
  depositTime: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.success.base,
    opacity: 0.7,
  },
  depositRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'baseline',
    paddingLeft: 14,
    marginTop: 2,
  },
  depositAmount: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.success.base,
  },
  depositMeta: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.success.base,
    opacity: 0.7,
  },

  // Networks — compact chip grid
  sectionLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
    marginBottom: space.lg,
  },
  networkGrid: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    justifyContent: 'space-between',
    rowGap: space.md,
    marginBottom: space['4xl'],
  },
  networkChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
    paddingHorizontal: space.lg,
    paddingVertical: space.md,
    width: '48.5%',
  },
  networkChipName: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    flexShrink: 1,
  },
}));
