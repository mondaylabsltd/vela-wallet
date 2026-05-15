/**
 * Transaction receipt — bank-receipt style full-screen view.
 * Web: Canvas-rendered share image (high quality, like ReceiveScreen).
 * Native: react-native-view-shot screenshot.
 */

import { ChainLogo } from '@/components/ChainLogo';
import { QRCode } from '@/components/QRCode';
import { TokenLogo } from '@/components/TokenLogo';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';
import { chainName, getAllNetworksSync } from '@/models/network';
import { formatBalance, shortAddr } from '@/models/types';
import { copyToClipboard, hapticSuccess, openBrowser, showAlert } from '@/services/platform';
import type { RecipientIdentity } from '@/services/recipient-identity';
import { ExternalLink, Share2 } from 'lucide-react-native';
import QRCodeLib from 'qrcode';
import React, { useRef } from 'react';
import { Image, Platform, Pressable, ScrollView, Text, View } from 'react-native';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface Props {
  from: string;
  fromName?: string;
  to: string;
  toName?: string | null;
  amount: string;
  symbol: string;
  chainId: number;
  txHash: string;
  logoUrls: string[];
  usdValue?: number;
  timestamp: Date;
  recipientIdentity?: RecipientIdentity | null;
  onDone: () => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTime(d: Date): string {
  return d.toLocaleString('en-US', {
    year: 'numeric', month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit',
  });
}

function formatUsd(value: number): string {
  return '$' + value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

// ---------------------------------------------------------------------------
// Canvas share image (web) — pixel-perfect, no html2canvas
// ---------------------------------------------------------------------------

const LOGO_ASSET = require('@/../assets/images/icon.png');

function resolveAssetUri(asset: any): string {
  if (typeof asset === 'string') return asset;
  if (typeof asset === 'number') {
    const resolved = Image.resolveAssetSource(asset);
    return resolved?.uri ?? '';
  }
  return asset?.uri ?? asset?.default ?? '';
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

async function loadImageRobust(src: string): Promise<HTMLImageElement> {
  try { return await loadImage(src); } catch {}
  const resp = await fetch(src);
  const blob = await resp.blob();
  const blobUrl = URL.createObjectURL(blob);
  try { return await loadImage(blobUrl); } finally { URL.revokeObjectURL(blobUrl); }
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

async function renderReceiptToCanvas(props: Props): Promise<Blob> {
  const { from, fromName, to, amount, symbol, chainId, txHash, logoUrls, usdValue, timestamp, recipientIdentity } = props;
  const chain = chainName(chainId);
  const net = getAllNetworksSync().find(n => n.chainId === chainId);
  const explorerUrl = `${net?.explorerURL ?? 'https://etherscan.io'}/tx/${txHash}`;
  const displayToName = recipientIdentity?.name ?? props.toName;

  // Phone-like proportions: 9:16 aspect, similar to in-app card
  const SCALE = 2; // @2x for retina
  const W = 390 * SCALE;
  const OUTER_PAD = 20 * SCALE;
  const CARD_PAD = 24 * SCALE;
  const CARD_W = W - OUTER_PAD * 2;
  const CARD_R = 20 * SCALE;
  const qrSize = 100 * SCALE;
  const tokenLogoSize = 44 * SCALE;
  const appLogoSize = 32 * SCALE;

  // Pre-compute height
  const hasUsd = usdValue != null && usdValue > 0;
  const nameRows = [fromName, displayToName].filter(Boolean).length;
  const detailRowH = 40 * SCALE;
  const nameRowH = 52 * SCALE;
  const cardH =
    CARD_PAD                                  // top padding
    + 24 * SCALE + 16 * SCALE                // header + gap
    + 1 + 20 * SCALE                         // divider
    + tokenLogoSize + 10 * SCALE             // token logo
    + 36 * SCALE                              // amount text
    + (hasUsd ? 24 * SCALE : 0)              // usd
    + 16 * SCALE                              // gap
    + 1 + 16 * SCALE                         // divider
    + nameRows * nameRowH + (5 - nameRows) * detailRowH // detail rows
    + 20 * SCALE + 1 + 20 * SCALE            // qr divider
    + qrSize + 10 * SCALE + 16 * SCALE       // qr + hint
    + 24 * SCALE                              // gap before footer
    + appLogoSize + 8 * SCALE + 18 * SCALE + 16 * SCALE  // logo + brand + url
    + CARD_PAD;                               // bottom padding

  const H = OUTER_PAD + cardH + OUTER_PAD;

  const canvas = document.createElement('canvas');
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext('2d')!;

  // Outer background (matches app bg)
  ctx.fillStyle = color.bg.sunken;
  ctx.fillRect(0, 0, W, H);

  // Card background
  ctx.fillStyle = color.bg.raised;
  roundRect(ctx, OUTER_PAD, OUTER_PAD, CARD_W, cardH, CARD_R);
  ctx.fill();
  // Card border
  ctx.strokeStyle = color.border.base;
  ctx.lineWidth = 1;
  roundRect(ctx, OUTER_PAD, OUTER_PAD, CARD_W, cardH, CARD_R);
  ctx.stroke();

  const L = OUTER_PAD + CARD_PAD;
  const R = OUTER_PAD + CARD_W - CARD_PAD;
  const contentW = R - L;
  let y = OUTER_PAD + CARD_PAD;

  const s = (px: number) => px * SCALE;

  // Header: TRANSACTION RECEIPT + chain name
  ctx.fillStyle = color.fg.base;
  ctx.font = `bold ${s(13)}px Inter, system-ui, sans-serif`;
  ctx.textAlign = 'left';
  ctx.fillText('TRANSACTION RECEIPT', L, y + s(14));
  ctx.fillStyle = color.fg.muted;
  ctx.font = `500 ${s(12)}px Inter, system-ui, sans-serif`;
  ctx.textAlign = 'right';
  ctx.fillText(chain, R, y + s(14));
  y += s(24) + s(16);

  // Divider
  ctx.fillStyle = color.border.base;
  ctx.fillRect(L, y, contentW, 1);
  y += 1 + s(20);

  // Token logo (try to load, skip if fails)
  const tokenLogoSrc = logoUrls?.[0];
  if (tokenLogoSrc) {
    try {
      const tokenImg = await loadImageRobust(tokenLogoSrc);
      const tx = (W - tokenLogoSize) / 2;
      ctx.save();
      ctx.beginPath();
      ctx.arc(tx + tokenLogoSize / 2, y + tokenLogoSize / 2, tokenLogoSize / 2, 0, Math.PI * 2);
      ctx.clip();
      ctx.drawImage(tokenImg, tx, y, tokenLogoSize, tokenLogoSize);
      ctx.restore();
    } catch {}
  }
  y += tokenLogoSize + s(10);

  // Amount
  ctx.fillStyle = color.fg.base;
  ctx.font = `bold ${s(28)}px Inter, system-ui, sans-serif`;
  ctx.textAlign = 'center';
  ctx.fillText(`${formatBalance(parseFloat(amount))} ${symbol}`, W / 2, y + s(28));
  y += s(36);
  if (hasUsd) {
    ctx.fillStyle = color.fg.muted;
    ctx.font = `500 ${s(15)}px Inter, system-ui, sans-serif`;
    ctx.fillText(formatUsd(usdValue!), W / 2, y + s(15));
    y += s(24);
  }
  y += s(16);

  // Divider
  ctx.fillStyle = color.border.base;
  ctx.fillRect(L, y, contentW, 1);
  y += 1 + s(12);

  // Detail rows
  const details: [string, string, string?][] = [
    ['From', shortAddr(from), fromName],
    ['To', shortAddr(to), displayToName ?? undefined],
    ['Network', chain],
    ['Time', formatTime(timestamp)],
    ['Tx Hash', shortAddr(txHash)],
  ];

  for (const [label, value, name] of details) {
    ctx.fillStyle = color.fg.muted;
    ctx.font = `400 ${s(13)}px Inter, system-ui, sans-serif`;
    ctx.textAlign = 'left';

    if (name) {
      ctx.fillText(label, L, y + s(18));
      ctx.textAlign = 'right';
      ctx.fillStyle = color.fg.base;
      ctx.font = `600 ${s(13)}px Inter, system-ui, sans-serif`;
      ctx.fillText(name, R, y + s(12));
      ctx.fillStyle = color.fg.muted;
      ctx.font = `400 ${s(12)}px "SF Mono", monospace`;
      ctx.fillText(value, R, y + s(30));
      y += nameRowH;
    } else {
      ctx.fillText(label, L, y + s(18));
      ctx.textAlign = 'right';
      ctx.fillStyle = color.fg.base;
      ctx.font = `600 ${s(13)}px Inter, system-ui, sans-serif`;
      ctx.fillText(value, R, y + s(18));
      y += detailRowH;
    }
  }

  // QR section
  y += s(8);
  ctx.fillStyle = color.border.base;
  ctx.fillRect(L, y, contentW, 1);
  y += 1 + s(20);

  // QR code
  const qrModules = QRCodeLib.create(explorerUrl, { errorCorrectionLevel: 'M' }).modules;
  const moduleCount = qrModules.size;
  const moduleSize = qrSize / moduleCount;
  const qrX = (W - qrSize) / 2;
  ctx.fillStyle = color.fg.base;
  for (let row = 0; row < moduleCount; row++) {
    for (let col = 0; col < moduleCount; col++) {
      if (qrModules.data[row * moduleCount + col] === 1) {
        ctx.fillRect(qrX + col * moduleSize, y + row * moduleSize, moduleSize + 0.5, moduleSize + 0.5);
      }
    }
  }
  y += qrSize + s(8);
  ctx.fillStyle = color.fg.subtle;
  ctx.font = `400 ${s(11)}px Inter, system-ui, sans-serif`;
  ctx.textAlign = 'center';
  ctx.fillText('Scan to view on explorer', W / 2, y + s(12));
  y += s(16) + s(24);

  // Footer: logo + VELA WALLET + getvela.app
  let logoImg: HTMLImageElement | null = null;
  const logoSources = [resolveAssetUri(LOGO_ASSET), '/assets/assets/images/icon.png', '/assets/images/icon.png'].filter(Boolean);
  for (const src of logoSources) {
    try { logoImg = await loadImageRobust(src); break; } catch {}
  }
  if (logoImg) {
    const lx = (W - appLogoSize) / 2;
    ctx.save();
    ctx.beginPath();
    ctx.arc(lx + appLogoSize / 2, y + appLogoSize / 2, appLogoSize / 2, 0, Math.PI * 2);
    ctx.clip();
    ctx.drawImage(logoImg, lx, y, appLogoSize, appLogoSize);
    ctx.restore();
  }
  y += appLogoSize + s(8);
  ctx.fillStyle = color.fg.base;
  ctx.font = `bold ${s(13)}px Inter, system-ui, sans-serif`;
  ctx.textAlign = 'center';
  ctx.letterSpacing = `${s(2)}px`;
  ctx.fillText('VELA WALLET', W / 2, y + s(13));
  y += s(18);
  ctx.fillStyle = color.fg.subtle;
  ctx.font = `400 ${s(11)}px Inter, system-ui, sans-serif`;
  ctx.letterSpacing = '0px';
  ctx.fillText('getvela.app', W / 2, y + s(11));

  return new Promise((resolve) => canvas.toBlob(resolve as BlobCallback, 'image/png', 1));
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function TransactionReceipt({
  from, fromName, to, toName, amount, symbol, chainId,
  txHash, logoUrls, usdValue, timestamp, recipientIdentity, onDone,
}: Props) {
  const receiptRef = useRef<View>(null);
  const chain = chainName(chainId);
  const net = getAllNetworksSync().find(n => n.chainId === chainId);
  const explorerBase = net?.explorerURL ?? 'https://etherscan.io';
  const explorerUrl = `${explorerBase}/tx/${txHash}`;
  const displayToName = recipientIdentity?.name ?? toName;

  const handleViewExplorer = () => openBrowser(explorerUrl);

  const handleShare = async () => {
    if (Platform.OS === 'web') {
      try {
        const blob = await renderReceiptToCanvas({
          from, fromName, to, toName, amount, symbol, chainId,
          txHash, logoUrls, usdValue, timestamp, recipientIdentity, onDone,
        });
        const file = new File([blob], `vela-receipt-${txHash.slice(0, 10)}.png`, { type: 'image/png' });

        if (navigator.share && navigator.canShare?.({ files: [file] })) {
          await navigator.share({ files: [file] });
          return;
        }

        // Fallback: copy image to clipboard
        try {
          await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
          hapticSuccess();
          showAlert('Copied', 'Receipt image copied to clipboard.');
          return;
        } catch {}

        // Fallback: download
        const url = URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.download = file.name;
        link.href = url;
        link.click();
        URL.revokeObjectURL(url);
        hapticSuccess();
      } catch {
        await copyToClipboard(explorerUrl);
        showAlert('Copied', 'Explorer link copied.');
      }
    } else {
      try {
        const { captureRef } = await import('react-native-view-shot');
        const Sharing = await import('expo-sharing');
        if (receiptRef.current) {
          const uri = await captureRef(receiptRef, { format: 'png', quality: 1, result: 'tmpfile' });
          await Sharing.shareAsync(uri, { mimeType: 'image/png' });
        }
      } catch (e) {
        console.warn('Share failed:', e);
        await copyToClipboard(explorerUrl);
        showAlert('Copied', 'Explorer link copied.');
      }
    }
  };

  return (
    <ScrollView style={styles.screen} contentContainerStyle={styles.screenContent} showsVerticalScrollIndicator={false}>
      {/* Capturable receipt card */}
      <View ref={receiptRef} testID="receipt-card" collapsable={false} style={styles.receipt}>
        <View style={styles.header}>
          <Text style={styles.headerTitle}>Transaction Receipt</Text>
          <View style={styles.headerNetwork}>
            {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={16} />}
            <Text style={styles.headerChain}>{chain}</Text>
          </View>
        </View>
        <View style={styles.separator} />
        <View style={styles.amountSection}>
          <TokenLogo symbol={symbol} logoUrls={logoUrls} size={44} />
          <Text style={styles.amountText}>{formatBalance(parseFloat(amount))} {symbol}</Text>
          {usdValue != null && usdValue > 0 && <Text style={styles.amountUsd}>{formatUsd(usdValue)}</Text>}
        </View>
        <View style={styles.separator} />

        <View style={styles.detailRow}><Text style={styles.detailLabel}>From</Text><View style={styles.detailValueCol}>{fromName && <Text style={styles.detailName}>{fromName}</Text>}<Text style={styles.detailAddr}>{shortAddr(from)}</Text></View></View>
        <View style={styles.detailRow}><Text style={styles.detailLabel}>To</Text><View style={styles.detailValueCol}>{displayToName && <Text style={styles.detailName}>{displayToName}</Text>}<Text style={styles.detailAddr}>{shortAddr(to)}</Text></View></View>
        <View style={styles.detailRow}><Text style={styles.detailLabel}>Network</Text><Text style={styles.detailValue}>{chain}</Text></View>
        <View style={styles.detailRow}><Text style={styles.detailLabel}>Time</Text><Text style={styles.detailValue}>{formatTime(timestamp)}</Text></View>
        <View style={styles.detailRow}><Text style={styles.detailLabel}>Tx Hash</Text><Text style={styles.detailAddr}>{shortAddr(txHash)}</Text></View>

        <View style={styles.qrSection}>
          <QRCode value={explorerUrl} size={80} />
          <Text style={styles.qrHint}>Scan to view on explorer</Text>
        </View>
        <View style={styles.footer}>
          <Image source={LOGO_ASSET} style={styles.footerLogoImg} resizeMode="contain" />
          <Text style={styles.footerLogo}>VELA WALLET</Text>
          <Text style={styles.footerUrl}>getvela.app</Text>
        </View>
      </View>

      {/* Action buttons */}
      <View style={styles.actions}>
        <Pressable style={styles.actionBtn} onPress={handleViewExplorer}>
          <ExternalLink size={18} color={color.fg.muted} strokeWidth={2} />
          <Text style={styles.actionText}>Explorer</Text>
        </Pressable>
        <Pressable style={styles.actionBtn} onPress={handleShare}>
          <Share2 size={18} color={color.fg.muted} strokeWidth={2} />
          <Text style={styles.actionText}>Share</Text>
        </Pressable>
      </View>

      <Pressable style={styles.doneBtn} onPress={onDone}>
        <Text style={styles.doneBtnText}>Done</Text>
      </Pressable>
    </ScrollView>
  );
}

const styles = createStyles(() => ({
  screen: { flex: 1, backgroundColor: color.bg.base },
  screenContent: { paddingBottom: 100 },
  receipt: {
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    padding: space['2xl'],
    borderWidth: 1,
    borderColor: color.border.base,
  },
  header: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: space.md },
  headerTitle: { fontSize: text.sm, ...inter.bold, color: color.fg.base, textTransform: 'uppercase' as const, letterSpacing: 0.5 },
  headerNetwork: { flexDirection: 'row', alignItems: 'center', gap: space.xs },
  headerChain: { fontSize: text.xs, ...inter.medium, color: color.fg.muted },
  separator: { height: 1, backgroundColor: color.border.base, marginVertical: space.lg },
  amountSection: { alignItems: 'center', gap: space.sm, paddingVertical: space.lg },
  amountText: { fontSize: text['3xl'], ...inter.bold, fontFamily: font.display, color: color.fg.base, marginTop: space.sm },
  amountUsd: { fontSize: text.base, ...inter.medium, color: color.fg.muted },
  detailRow: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'flex-start', paddingVertical: space.md },
  detailLabel: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, minWidth: 70 },
  detailValueCol: { alignItems: 'flex-end' as const, flex: 1 },
  detailName: { fontSize: text.sm, ...inter.semibold, color: color.fg.base, textAlign: 'right' as const },
  detailAddr: { fontSize: text.sm, ...inter.medium, fontFamily: font.mono, color: color.fg.muted, textAlign: 'right' as const },
  detailValue: { fontSize: text.sm, ...inter.semibold, color: color.fg.base, textAlign: 'right' as const, flex: 1 },
  qrSection: { alignItems: 'center', marginTop: space.xl, paddingTop: space.lg, borderTopWidth: 1, borderTopColor: color.border.base, gap: space.sm },
  qrHint: { fontSize: text.xs, ...inter.regular, color: color.fg.subtle },
  footer: { alignItems: 'center', marginTop: space.xl, gap: 4 },
  footerLogoImg: { width: 32, height: 32, borderRadius: 16, marginBottom: 4 },
  footerLogo: { fontSize: text.sm, ...inter.bold, color: color.fg.muted, letterSpacing: 2 },
  footerUrl: { fontSize: text.xs, ...inter.regular, color: color.fg.subtle },
  actions: { flexDirection: 'row', justifyContent: 'center', gap: space['5xl'], marginTop: space['2xl'] },
  actionBtn: { alignItems: 'center', gap: space.sm },
  actionText: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  doneBtn: { backgroundColor: color.fg.base, borderRadius: radius.xl, paddingVertical: space.xl, alignItems: 'center', marginTop: space['2xl'] },
  doneBtnText: { fontSize: text.lg, ...inter.semibold, color: color.fg.inverse },
}));
