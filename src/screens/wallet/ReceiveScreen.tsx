import { ChainLogo } from '@/components/ChainLogo';
import { QRCode } from '@/components/QRCode';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';
import { useSafeRouter } from '@/hooks/use-safe-router';
import type { Network } from '@/models/network';
import { chainName, getAllNetworksSync } from '@/models/network';
import { formatBalance, tokenBalanceDouble, tokenChainId, tokenId, type APIToken } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import { fetchTokens } from '@/services/wallet-api';
import { copyToClipboard, hapticSuccess, hapticLight, isAppActive } from '@/services/platform';
import { ArrowLeft, Check, Copy, Share2 } from 'lucide-react-native';
import QRCodeLib from 'qrcode';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Image, Platform, Pressable, ScrollView, Text, View } from 'react-native';
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withRepeat,
  withSequence,
  withTiming,
} from 'react-native-reanimated';

// ── Share card helpers ──
const LOGO_ASSET = require('@/../assets/images/icon.png');

/** Truncate network name so it fits in a chip */
function truncateName(name: string, maxLen: number): string {
  return name.length > maxLen ? name.slice(0, maxLen - 1).trimEnd() + '…' : name;
}

function resolveAssetUri(asset: any): string {
  if (typeof asset === 'string') return asset;
  if (typeof asset === 'number') {
    // Metro bundled numeric ID — use resolveAssetSource
    const resolved = Image.resolveAssetSource(asset);
    return resolved?.uri ?? '';
  }
  return asset?.uri ?? asset?.default ?? '';
}

async function renderShareCardToCanvas(
  address: string,
  walletName: string,
  networks: Network[],
): Promise<Blob> {
  const W = 750;
  const PAD = 80;
  const contentW = W - PAD * 2;
  const qrSize = 340;
  const qrPad = 36;
  const qrContainerSize = qrSize + qrPad * 2;
  const chipH = 44;
  const chipGap = 12;
  const chipsPerRow = 2;
  const networkRows = Math.ceil(networks.length / chipsPerRow);
  const networksH = networkRows * chipH + (networkRows - 1) * chipGap;
  const logoSize = 48;

  const H = PAD
    + 46 + 12                              // title + gap
    + 28 + 6 + 24 + 40                    // wallet name + gap + address + gap
    + qrContainerSize + 40                 // QR container + gap
    + 1 + 32                               // divider + gap
    + 22 + 16                              // "Works on" label + gap
    + networksH + 48                       // chips + gap
    + logoSize + 24 + 28 + 20             // footer
    + PAD;

  const canvas = document.createElement('canvas');
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext('2d')!;

  // — Background —
  ctx.fillStyle = '#FFFFFF';
  ctx.fillRect(0, 0, W, H);

  // — Preload logo —
  let logoImg: HTMLImageElement | null = null;
  const logoSources = [
    resolveAssetUri(LOGO_ASSET),
    '/assets/assets/images/icon.png',
    '/assets/images/icon.png',
  ].filter(s => s && s !== '[object Object]');
  for (const src of logoSources) {
    try { logoImg = await loadImageRobust(src); break; } catch {}
  }

  let y = PAD;

  // — Title —
  ctx.fillStyle = '#1A1A18';
  ctx.font = 'bold 40px Inter, system-ui, sans-serif';
  ctx.textAlign = 'center';
  ctx.fillText('Scan to Send Me Crypto', W / 2, y + 34);
  y += 46 + 12;

  // — Wallet name —
  ctx.fillStyle = '#1A1A18';
  ctx.font = '600 28px Inter, system-ui, sans-serif';
  ctx.fillText(walletName, W / 2, y + 22);
  y += 28 + 6;

  // — Short address —
  const shortAddr = `${address.slice(0, 6)}···${address.slice(-4)}`;
  ctx.fillStyle = '#9E9B93';
  ctx.font = '400 24px "SF Mono", "Fira Code", monospace';
  ctx.fillText(shortAddr, W / 2, y + 18);
  y += 24 + 40;

  // — QR container with subtle border —
  const qcX = (W - qrContainerSize) / 2;
  ctx.strokeStyle = '#E8E6E1';
  ctx.lineWidth = 1.5;
  roundRect(ctx, qcX, y, qrContainerSize, qrContainerSize, 24);
  ctx.stroke();

  // QR code inside container
  const qrModules = QRCodeLib.create(address, { errorCorrectionLevel: 'M' }).modules;
  const moduleCount = qrModules.size;
  const moduleSize = qrSize / moduleCount;
  const qrX = qcX + qrPad;
  const qrY = y + qrPad;
  ctx.fillStyle = '#1A1A18';
  for (let row = 0; row < moduleCount; row++) {
    for (let col = 0; col < moduleCount; col++) {
      if (qrModules.data[row * moduleCount + col] === 1) {
        ctx.fillRect(qrX + col * moduleSize, qrY + row * moduleSize, moduleSize + 0.5, moduleSize + 0.5);
      }
    }
  }
  y += qrContainerSize + 40;

  // — Divider —
  ctx.fillStyle = '#ECEBE4';
  ctx.fillRect(PAD, y, contentW, 1);
  y += 1 + 32;

  // — "Works on X networks" label —
  ctx.fillStyle = '#9E9B93';
  ctx.font = '500 22px Inter, system-ui, sans-serif';
  ctx.textAlign = 'left';
  ctx.fillText(`Works on ${networks.length} EVM networks`, PAD, y + 18);
  y += 22 + 16;

  // — Network chips (2-column grid, justified) —
  const chipW = (contentW - chipGap) / 2;
  const logoImages = await Promise.all(
    networks.map(n => loadImage(n.logoURL).catch(() => null)),
  );

  for (let i = 0; i < networks.length; i++) {
    const col = i % chipsPerRow;
    const row = Math.floor(i / chipsPerRow);
    const cx = PAD + col * (chipW + chipGap);
    const cy = y + row * (chipH + chipGap);
    const n = networks[i];

    ctx.fillStyle = '#F7F6F3';
    roundRect(ctx, cx, cy, chipW, chipH, chipH / 2);
    ctx.fill();

    const cLogoSize = 24;
    const cLogoX = cx + 14;
    const cLogoY = cy + (chipH - cLogoSize) / 2;
    const img = logoImages[i];
    if (img) {
      ctx.save();
      ctx.beginPath();
      ctx.arc(cLogoX + cLogoSize / 2, cLogoY + cLogoSize / 2, cLogoSize / 2, 0, Math.PI * 2);
      ctx.clip();
      ctx.drawImage(img, cLogoX, cLogoY, cLogoSize, cLogoSize);
      ctx.restore();
    } else {
      ctx.fillStyle = n.iconBg;
      ctx.beginPath();
      ctx.arc(cLogoX + cLogoSize / 2, cLogoY + cLogoSize / 2, cLogoSize / 2, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = n.iconColor;
      ctx.font = `bold ${cLogoSize * 0.4}px Inter, sans-serif`;
      ctx.textAlign = 'center';
      ctx.fillText(n.iconLabel, cLogoX + cLogoSize / 2, cLogoY + cLogoSize / 2 + 4);
    }

    // Name — truncate to fit
    const textX = cLogoX + cLogoSize + 8;
    const maxTextX = cx + chipW - 16;
    ctx.font = '600 20px Inter, system-ui, sans-serif';
    const maxTextW = maxTextX - textX;
    let label = n.displayName;
    while (ctx.measureText(label).width > maxTextW && label.length > 2) {
      label = label.slice(0, -1);
    }
    if (label !== n.displayName) label = label.trimEnd() + '…';
    ctx.fillStyle = '#4A4843';
    ctx.textAlign = 'left';
    ctx.fillText(label, textX, cy + chipH / 2 + 7);
  }
  y += networksH + 48;

  // — Footer —
  if (logoImg) {
    const flx = (W - logoSize) / 2;
    ctx.save();
    ctx.beginPath();
    ctx.arc(flx + logoSize / 2, y + logoSize / 2, logoSize / 2, 0, Math.PI * 2);
    ctx.clip();
    ctx.drawImage(logoImg, flx, y, logoSize, logoSize);
    ctx.restore();
  }
  y += logoSize + 24;
  ctx.fillStyle = '#1A1A18';
  ctx.font = '600 24px Inter, system-ui, sans-serif';
  ctx.textAlign = 'center';
  ctx.fillText('Vela Wallet', W / 2, y);
  y += 28;
  ctx.fillStyle = '#B0ADA5';
  ctx.font = '400 20px Inter, system-ui, sans-serif';
  ctx.fillText('getvela.app', W / 2, y);

  return new Promise((resolve) => canvas.toBlob(resolve as BlobCallback, 'image/png', 1));
}

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + r);
  ctx.lineTo(x + w, y + h - r);
  ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
  ctx.lineTo(x + r, y + h);
  ctx.quadraticCurveTo(x, y + h, x, y + h - r);
  ctx.lineTo(x, y + r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.closePath();
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new window.Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

/** Load image with fetch+blob fallback to avoid CORS/taint issues on web */
async function loadImageRobust(src: string): Promise<HTMLImageElement> {
  // Try direct first
  try { return await loadImage(src); } catch {}
  // Fetch as blob — works for same-origin assets that fail CORS canvas tainting
  const resp = await fetch(src);
  const blob = await resp.blob();
  const blobUrl = URL.createObjectURL(blob);
  return loadImage(blobUrl);
}

// Aggressive polling: 3s for first 1 min, then 60s for next 4 min, then stop
const FAST_INTERVAL_MS = 3_000;
const SLOW_INTERVAL_MS = 60_000;
const FAST_PHASE_MS = 1 * 60_000;
const TOTAL_LISTEN_MS = 5 * 60_000;

function PulsingDot() {
  const opacity = useSharedValue(1);

  useEffect(() => {
    opacity.value = withRepeat(
      withSequence(
        withTiming(0.3, { duration: 800 }),
        withTiming(1, { duration: 800 }),
      ),
      -1,
      false,
    );
  }, [opacity]);

  const animatedStyle = useAnimatedStyle(() => ({
    opacity: opacity.value,
  }));

  return (
    <Animated.View style={[styles.listeningDot, animatedStyle]} />
  );
}

export default function ReceiveScreen() {
  const router = useSafeRouter();
  const { activeAccount, state } = useWallet();
  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';
  const networks = getAllNetworksSync();

  const [isListening, setIsListening] = useState(false);
  const [depositDetected, setDepositDetected] = useState(false);
  interface DepositEntry { time: string; items: { symbol: string; amount: string; network: string; usd: string | null }[] }
  const [deposits, setDeposits] = useState<DepositEntry[]>([]);
  const previousTokens = useRef<APIToken[] | null>(null);
  const [copied, setCopied] = useState(false);
  const [sharing, setSharing] = useState(false);
  const previousBalance = useRef<number | null>(null);
  const shareCardRef = useRef<View>(null);

  // Deposit detection polling — 3s fast, then 60s slow
  useEffect(() => {
    if (!address) return;
    setIsListening(true);
    previousBalance.current = null;
    const startTime = Date.now();
    let timerId: ReturnType<typeof setTimeout>;

    const checkDeposit = async () => {
      if (!isAppActive()) return;
      try {
        const tokens = await fetchTokens(address, { forceRefresh: true });

        if (previousTokens.current !== null) {
          // Diff: find tokens whose balance increased
          const prevMap = new Map(previousTokens.current.map(t => [tokenId(t), tokenBalanceDouble(t)]));
          const changes: DepositEntry['items'] = [];
          for (const t of tokens) {
            const prevBal = prevMap.get(tokenId(t)) ?? 0;
            const curBal = tokenBalanceDouble(t);
            if (curBal > prevBal) {
              const diff = curBal - prevBal;
              changes.push({
                symbol: t.symbol,
                amount: formatBalance(diff),
                network: chainName(tokenChainId(t)),
                usd: t.priceUsd ? `$${(diff * t.priceUsd).toFixed(2)}` : null,
              });
            }
          }

          if (changes.length > 0) {
            const time = new Date().toLocaleTimeString('en-US', { hour12: false });
            setDepositDetected(true);
            setDeposits(prev => [{ time, items: changes }, ...prev]);
            hapticSuccess();
          }
        }
        previousTokens.current = tokens; // always update baseline
      } catch {}

      const elapsed = Date.now() - startTime;
      if (elapsed >= TOTAL_LISTEN_MS) {
        setIsListening(false);
        return;
      }
      const interval = elapsed < FAST_PHASE_MS ? FAST_INTERVAL_MS : SLOW_INTERVAL_MS;
      timerId = setTimeout(checkDeposit, interval);
    };

    checkDeposit();
    return () => { clearTimeout(timerId); setIsListening(false); };
  }, [address]);

  const copyAddress = useCallback(async () => {
    if (!address) return;
    await copyToClipboard(address);
    hapticLight();
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [address]);

  const shareAsImage = useCallback(async () => {
    if (sharing) return;
    setSharing(true);
    try {
      if (Platform.OS === 'web') {
        const blob = await renderShareCardToCanvas(address!, accountName, networks);
        const file = new File([blob], `${accountName}-address.png`, { type: 'image/png' });
        if (navigator.share && navigator.canShare?.({ files: [file] })) {
          await navigator.share({ files: [file], title: `${accountName} Wallet Address` });
        } else {
          const url = URL.createObjectURL(blob);
          const a = document.createElement('a');
          a.href = url;
          a.download = `${accountName}-address.png`;
          a.click();
          URL.revokeObjectURL(url);
        }
      } else {
        if (!shareCardRef.current) return;
        const { captureRef } = await import('react-native-view-shot');
        const Sharing = await import('expo-sharing');
        const uri = await captureRef(shareCardRef, { format: 'png', quality: 1, result: 'tmpfile' });
        await Sharing.shareAsync(uri, { mimeType: 'image/png', dialogTitle: `${accountName} Wallet Address` });
      }
    } catch (e) {
      console.warn('Share failed:', e);
    }
    setSharing(false);
  }, [address, accountName, sharing, networks]);

  const truncatedAddress = address
    ? `${address.slice(0, 8)}...${address.slice(-6)}`
    : '';

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content}>
        {/* Header */}
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.navBtn}>
            <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <Text style={styles.title}>Receive</Text>
          <View style={styles.headerSpacer} />
        </View>

        {/* QR Card */}
        <Animated.View entering={fadeInDown(100, 400)}>
          <VelaCard elevated style={styles.qrCard}>
            {/* Identity */}
            <Text style={styles.walletName}>{accountName}</Text>
            <Pressable onPress={copyAddress} style={styles.addressRow}>
              <Text style={styles.addressText} numberOfLines={1}>{truncatedAddress}</Text>
              {copied ? (
                <Check size={14} color={color.success.base} strokeWidth={2.5} />
              ) : (
                <Copy size={14} color={color.fg.subtle} strokeWidth={1.8} />
              )}
            </Pressable>

            {/* QR */}
            <View style={styles.qrBorder}>
              {address ? (
                <QRCode value={address} size={200} />
              ) : (
                <View style={styles.qrPlaceholder}>
                  <Text style={styles.qrPlaceholderText}>No address</Text>
                </View>
              )}
            </View>

            {/* Share */}
            <Pressable
              onPress={shareAsImage}
              style={styles.shareBtn}
              disabled={sharing}
            >
              <Share2 size={16} color={color.fg.base} strokeWidth={2} />
              <Text style={styles.shareBtnText}>
                {sharing ? 'Generating...' : 'Share'}
              </Text>
            </Pressable>

            {/* Status */}
            {isListening && !depositDetected && (
              <Animated.View style={styles.listeningRow} entering={fadeIn(0, 300)}>
                <PulsingDot />
                <Text style={styles.listeningText}>Listening for deposits</Text>
              </Animated.View>
            )}
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
        </Animated.View>

        {/* Networks */}
        <Animated.View entering={fadeInDown(200, 400)}>
          <Text style={styles.sectionLabel}>{`Works on ${networks.length} EVM networks`}</Text>

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
      </ScrollView>

      {/* Hidden share card for native image capture (web uses Canvas) */}
      {Platform.OS !== 'web' && (
        <View style={styles.shareCardWrapper} pointerEvents="none">
          <View ref={shareCardRef} style={styles.shareCard} collapsable={false}>
            <Text style={styles.shareCardHeadline}>Scan to Send Me Crypto</Text>
            <Text style={styles.shareCardName}>{accountName}</Text>
            <Text style={styles.shareCardAddr}>
              {address ? `${address.slice(0, 6)}···${address.slice(-4)}` : ''}
            </Text>

            <View style={styles.shareCardQRContainer}>
              {address && <QRCode value={address} size={170} />}
            </View>

            <View style={styles.shareCardDivider} />

            <Text style={styles.shareCardNetworksSub}>
              {`Works on ${networks.length} EVM networks`}
            </Text>

            <View style={styles.shareCardNetworkGrid}>
              {networks.map((network) => (
                <View key={network.id} style={styles.shareCardNetworkChip}>
                  <ChainLogo
                    label={network.iconLabel}
                    color={network.iconColor}
                    bgColor={network.iconBg}
                    logoURL={network.logoURL}
                    size={12}
                  />
                  <Text style={styles.shareCardNetworkName} numberOfLines={1}>
                    {truncateName(network.displayName, 10)}
                  </Text>
                </View>
              ))}
            </View>

            <View style={styles.shareCardFooter}>
              <Image source={require('@/../assets/images/icon.png')} style={styles.shareCardFooterLogo} />
              <Text style={styles.shareCardBrand}>Vela Wallet</Text>
              <Text style={styles.shareCardUrl}>getvela.app</Text>
            </View>
          </View>
        </View>
      )}
    </ScreenContainer>
  );
}

const SHARE_CARD_W = 375;

const styles = createStyles(() => ({
  content: {
    paddingBottom: 100,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.lg,
    marginBottom: space.md,
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

  // QR Card
  qrCard: {
    padding: space['3xl'],
    paddingTop: space['4xl'],
    paddingBottom: space['2xl'],
    alignItems: 'center',
    marginBottom: space.xl,
  },
  walletName: {
    fontSize: text['2xl'],
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space.xs,
  },
  addressRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    alignSelf: 'center',
    marginBottom: space['3xl'],
  },
  addressText: {
    fontSize: text.sm,
    ...inter.regular,
    fontFamily: font.mono,
    color: color.fg.subtle,
  },
  qrBorder: {
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.xl,
    padding: space['2xl'],
    marginBottom: space['2xl'],
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

  // Share button — inside the card
  shareBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.md,
    paddingVertical: space.md,
    paddingHorizontal: space['2xl'],
    borderRadius: radius.full,
    backgroundColor: color.bg.sunken,
    alignSelf: 'center',
  },
  shareBtnText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
  },

  // Status
  listeningRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    marginTop: space.lg,
  },
  listeningDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    backgroundColor: color.success.base,
  },
  listeningText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.success.base,
  },
  depositAlert: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.success.soft,
    paddingHorizontal: space.xl,
    paddingVertical: space.lg,
    borderRadius: radius.lg,
    marginTop: space.lg,
    width: '100%',
  },
  depositText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.success.base,
  },
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

  // ── Hidden share card (rendered off-screen for capture) ──
  shareCardWrapper: {
    position: 'absolute',
    left: -9999,
    top: 0,
  },
  shareCard: {
    width: SHARE_CARD_W,
    backgroundColor: '#FFFFFF',
    padding: 40,
    alignItems: 'center',
  },
  shareCardHeadline: {
    fontSize: 20,
    fontWeight: '700',
    color: '#1A1A18',
    marginBottom: 6,
  },
  shareCardName: {
    fontSize: 14,
    fontWeight: '600',
    color: '#1A1A18',
    marginBottom: 3,
  },
  shareCardAddr: {
    fontSize: 12,
    fontFamily: font.mono,
    fontWeight: '400',
    color: '#9E9B93',
    marginBottom: 20,
  },
  shareCardQRContainer: {
    borderWidth: 0.75,
    borderColor: '#E8E6E1',
    borderRadius: 12,
    padding: 18,
    marginBottom: 20,
  },
  shareCardDivider: {
    height: 1,
    backgroundColor: '#ECEBE4',
    width: '100%',
    marginBottom: 14,
  },
  shareCardNetworksSub: {
    fontSize: 11,
    fontWeight: '500',
    color: '#9E9B93',
    marginBottom: 8,
    alignSelf: 'flex-start',
  },
  shareCardNetworkGrid: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    justifyContent: 'space-between',
    rowGap: 6,
    width: '100%',
    marginBottom: 20,
  },
  shareCardNetworkChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
    backgroundColor: '#F7F6F3',
    borderRadius: 11,
    paddingHorizontal: 7,
    width: '48.5%',
    height: 22,
  },
  shareCardNetworkName: {
    fontSize: 10,
    fontWeight: '600',
    color: '#4A4843',
    flexShrink: 1,
  },
  shareCardFooter: {
    alignItems: 'center',
  },
  shareCardFooterLogo: {
    width: 24,
    height: 24,
    borderRadius: 12,
    marginBottom: 5,
  },
  shareCardBrand: {
    fontSize: 12,
    fontWeight: '600',
    color: '#1A1A18',
    marginBottom: 2,
    textAlign: 'center',
  },
  shareCardUrl: {
    fontSize: 10,
    fontWeight: '400',
    color: '#B0ADA5',
    textAlign: 'center',
  },
}));
