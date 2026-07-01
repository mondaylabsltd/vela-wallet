/**
 * Transaction receipt — bank-receipt style full-screen view.
 * Web: Canvas-rendered share image (high quality, like ReceiveScreen).
 * Native: react-native-view-shot screenshot.
 */

import { ChainLogo } from '@/components/ChainLogo';
import { QRCode } from '@/components/QRCode';
import { TokenLogo } from '@/components/TokenLogo';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';
import { chainName, getAllNetworksSync, explorerTxURL } from '@/models/network';
import { formatBalance, shortAddr } from '@/models/types';
import { fetchWithTimeout, NET_TIMEOUTS } from '@/services/net';
import { pollUserOpReceipt } from '@/services/tx-reconciler';
import { formatFiat } from '@/services/currency';
import { formatDateTime, useLocalePrefs } from '@/services/locale-format';
import { copyToClipboard, hapticSuccess, openBrowser, showAlert } from '@/services/platform';
import type { RecipientIdentity } from '@/services/recipient-identity';
import { ExternalLink, Share2, BookmarkPlus, Check, CheckCircle2, Clock, XCircle } from 'lucide-react-native';
import QRCodeLib from 'qrcode';
import React, { useEffect, useRef, useState } from 'react';
import { Image, Platform, Pressable, ScrollView, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** One line item in a batch send (advanced split / multiSelect modes). */
export interface ReceiptTransfer {
  to: string;
  toName?: string | null;
  amount: string;
  symbol: string;
  logoUrls: string[];
  usdValue?: number;
}

interface Props {
  from: string;
  fromName?: string;
  to: string;
  toName?: string | null;
  amount: string;
  symbol: string;
  chainId: number;
  txHash: string;
  /**
   * Submitted UserOp hash. When the on-chain hash is still resolving (submitted),
   * the receipt polls the bundler with this and converges its own status/hash —
   * so the "confirming" stamp flips to confirmed/failed on its own, without
   * relying on the parent screen still being mounted and pushing a new txHash.
   */
  userOpHash?: string;
  logoUrls: string[];
  usdValue?: number;
  /** Display-currency conversion for the fiat line (defaults to USD). */
  rate?: number;
  currencyCode?: string;
  currencySymbol?: string;
  timestamp: Date;
  recipientIdentity?: RecipientIdentity | null;
  /**
   * Batch sends (advanced modes). With >1 entry the receipt renders a line-item
   * breakdown instead of a single amount — the `amount`/`symbol`/`to` props are
   * then ignored:
   *   split       — one token → many recipients (each row = a recipient)
   *   multiSelect — many tokens → one recipient (each row = a token)
   * A 1-element array just overrides the single-send fields (so a degenerate
   * batch still renders the right token/amount instead of stale state).
   */
  transfers?: ReceiptTransfer[];
  batchKind?: 'split' | 'multiSelect';
  /**
   * Settlement state for the prominent status stamp:
   *   submitted — accepted by the bundler, on-chain hash still resolving
   *   confirmed — landed on-chain (success)
   *   failed    — the op was dropped / reverted
   * Defaults to confirmed/submitted derived from whether a txHash is present.
   */
  status?: 'submitted' | 'confirmed' | 'failed';
  onDone: () => void;
  /** Offer a "Save to contacts" action (omitted when the recipient is already saved). */
  onSaveContact?: () => void;
}

interface BatchView {
  kind: 'split' | 'multiSelect';
  items: ReceiptTransfer[];
  totalUsd: number;
  /** split only: summed token amount (formatted) + its symbol — one token, so summable. */
  splitTotal?: string;
  splitSymbol?: string;
}

/** Effective receipt values: a >1 batch, or the single line (props or a 1-element batch). */
function normalizeReceipt(p: Pick<Props, 'transfers' | 'batchKind' | 'amount' | 'symbol' | 'logoUrls' | 'usdValue' | 'to' | 'toName'>) {
  const tr = p.transfers && p.transfers.length ? p.transfers : null;
  let batch: BatchView | null = null;
  if (tr && tr.length > 1) {
    const kind = p.batchKind ?? 'split';
    const totalUsd = tr.reduce((s, x) => s + (x.usdValue ?? 0), 0);
    batch = kind === 'split'
      ? { kind, items: tr, totalUsd, splitTotal: formatBalance(tr.reduce((s, x) => s + (parseFloat(x.amount) || 0), 0)), splitSymbol: tr[0]?.symbol }
      : { kind, items: tr, totalUsd };
  }
  const one = tr && tr.length === 1 ? tr[0] : null;
  return {
    batch,
    amount: one ? one.amount : p.amount,
    symbol: one ? one.symbol : p.symbol,
    logoUrls: one ? one.logoUrls : p.logoUrls,
    usdValue: one ? one.usdValue : p.usdValue,
    to: one ? one.to : p.to,
    toName: one ? one.toName : p.toName,
  };
}

/** Format a USD value into the receipt's display currency. */
function fiat(usd: number, p: { rate?: number; currencyCode?: string; currencySymbol?: string }): string {
  return formatFiat(usd * (p.rate ?? 1), p.currencyCode ?? 'USD', p.currencySymbol ?? '$');
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
  const resp = await fetchWithTimeout(src, { mode: 'cors' }, { timeoutMs: NET_TIMEOUTS.descriptor });
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

export interface CanvasLabels {
  canvasTitle: string;
  from: string;
  to: string;
  network: string;
  time: string;
  txHash: string;
  scanHint: string;
  footerBrand: string;
  footerUrl: string;
  recipientsSummary: string;
  assetsSummary: string;
}

/**
 * Build the canvas share-image labels from the i18n `t`. Shared by the component
 * and the dev receipt harness so the two render paths can never drift apart.
 * `itemCount` drives the "N recipients" / "N assets" summaries.
 */
export function buildCanvasLabels(t: TFunction, itemCount: number): CanvasLabels {
  return {
    canvasTitle: t('componentsTx.receipt.canvasTitle'),
    from: t('componentsTx.receipt.from'),
    to: t('componentsTx.receipt.to'),
    network: t('componentsTx.receipt.network'),
    time: t('componentsTx.receipt.time'),
    txHash: t('componentsTx.receipt.txHash'),
    scanHint: t('componentsTx.receipt.scanHint'),
    footerBrand: t('componentsTx.receipt.footerBrand'),
    footerUrl: t('componentsTx.receipt.footerUrl'),
    recipientsSummary: t('componentsTx.receipt.recipientsCount', { n: itemCount }),
    assetsSummary: t('componentsTx.receipt.assetsCount', { n: itemCount }),
  };
}

export async function renderReceiptToCanvas(props: Props, labels: CanvasLabels): Promise<Blob> {
  const { from, fromName, chainId, txHash, timestamp, recipientIdentity } = props;
  const { batch, amount, symbol, logoUrls, usdValue, to, toName } = normalizeReceipt(props);
  const chain = chainName(chainId);
  const explorerUrl = explorerTxURL(chainId, txHash);
  const displayToName = recipientIdentity?.name ?? toName;

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
  const rowH = 30 * SCALE; // batch breakdown line item

  // Pre-compute height
  const hasUsd = usdValue != null && usdValue > 0;
  // Hero "sub" line under the big number: split → fiat total (when priced),
  // multiSelect → asset count.
  const heroSub = batch ? (batch.kind === 'split' ? batch.totalUsd > 0 : true) : false;
  // "To" collapses to a count for split (plain row); multiSelect keeps a named recipient.
  const toIsName = batch ? (batch.kind === 'multiSelect' && !!displayToName) : !!displayToName;
  const nameRows = [!!fromName, toIsName].filter(Boolean).length;
  const detailRowH = 40 * SCALE;
  const nameRowH = 52 * SCALE;
  const amountBlockH = batch
    ? (batch.kind === 'split' ? tokenLogoSize + 10 * SCALE : 0) // top logo: split only
      + 36 * SCALE                            // hero number
      + (heroSub ? 24 * SCALE : 0)           // hero sub
      + 16 * SCALE                            // gap
      + 1 + 12 * SCALE                       // breakdown divider
      + batch.items.length * rowH            // line items
      + 16 * SCALE                            // trailing gap
    : tokenLogoSize + 10 * SCALE             // token logo
      + 36 * SCALE                            // amount text
      + (hasUsd ? 24 * SCALE : 0)           // usd
      + 16 * SCALE;                           // gap
  const cardH =
    CARD_PAD                                  // top padding
    + 24 * SCALE + 16 * SCALE                // header + gap
    + 1 + 20 * SCALE                         // divider
    + amountBlockH                            // amount / batch breakdown
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
  ctx.fillText(labels.canvasTitle, L, y + s(14));
  ctx.fillStyle = color.fg.muted;
  ctx.font = `500 ${s(12)}px Inter, system-ui, sans-serif`;
  ctx.textAlign = 'right';
  ctx.fillText(chain, R, y + s(14));
  y += s(24) + s(16);

  // Divider
  ctx.fillStyle = color.border.base;
  ctx.fillRect(L, y, contentW, 1);
  y += 1 + s(20);

  const drawTokenLogo = async (src: string | undefined) => {
    if (!src) return;
    try {
      const tokenImg = await loadImageRobust(src);
      const tx = (W - tokenLogoSize) / 2;
      ctx.save();
      ctx.beginPath();
      ctx.arc(tx + tokenLogoSize / 2, y + tokenLogoSize / 2, tokenLogoSize / 2, 0, Math.PI * 2);
      ctx.clip();
      ctx.drawImage(tokenImg, tx, y, tokenLogoSize, tokenLogoSize);
      ctx.restore();
    } catch {}
  };

  // Small circular logo for a breakdown line (multiSelect rows) — matches the
  // in-app receipt, where each token row leads with its logo.
  const drawInlineLogo = async (src: string | undefined, x: number, cy: number, d: number) => {
    if (!src) return;
    try {
      const img = await loadImageRobust(src);
      ctx.save();
      ctx.beginPath();
      ctx.arc(x + d / 2, cy, d / 2, 0, Math.PI * 2);
      ctx.clip();
      ctx.drawImage(img, x, cy - d / 2, d, d);
      ctx.restore();
    } catch {}
  };

  if (batch) {
    // Hero: split shows the single token's total + symbol (one token, summable);
    // multiSelect shows the fiat total (or asset count when unpriced).
    if (batch.kind === 'split') {
      await drawTokenLogo(batch.items[0]?.logoUrls?.[0]);
      y += tokenLogoSize + s(10);
    }
    ctx.fillStyle = color.fg.base;
    ctx.font = `bold ${s(28)}px Inter, system-ui, sans-serif`;
    ctx.textAlign = 'center';
    const hero = batch.kind === 'split'
      ? `${batch.splitTotal} ${batch.splitSymbol}`
      : (batch.totalUsd > 0 ? fiat(batch.totalUsd, props) : labels.assetsSummary);
    ctx.fillText(hero, W / 2, y + s(28));
    y += s(36);
    if (heroSub) {
      ctx.fillStyle = color.fg.muted;
      ctx.font = `500 ${s(15)}px Inter, system-ui, sans-serif`;
      const sub = batch.kind === 'split'
        ? `${labels.recipientsSummary} · ${fiat(batch.totalUsd, props)}`
        : labels.assetsSummary;
      ctx.fillText(sub, W / 2, y + s(15));
      y += s(24);
    }
    y += s(16);

    // Breakdown divider
    ctx.fillStyle = color.border.base;
    ctx.fillRect(L, y, contentW, 1);
    y += 1 + s(12);

    // Line items — split rows are "recipient … amount symbol", multiSelect rows
    // are "logo symbol … amount" (the logo mirrors the in-app receipt).
    for (const it of batch.items) {
      const left = batch.kind === 'split' ? (it.toName || shortAddr(it.to)) : it.symbol;
      const right = batch.kind === 'split'
        ? `${formatBalance(parseFloat(it.amount))} ${it.symbol}`
        : formatBalance(parseFloat(it.amount));
      let textLeft = L;
      if (batch.kind === 'multiSelect') {
        const d = s(20);
        await drawInlineLogo(it.logoUrls?.[0], L, y + s(9), d);
        textLeft = L + d + s(8);
      }
      ctx.fillStyle = color.fg.muted;
      ctx.font = `400 ${s(13)}px Inter, system-ui, sans-serif`;
      ctx.textAlign = 'left';
      ctx.fillText(left, textLeft, y + s(14));
      ctx.fillStyle = color.fg.base;
      ctx.font = `600 ${s(13)}px Inter, system-ui, sans-serif`;
      ctx.textAlign = 'right';
      ctx.fillText(right, R, y + s(14));
      y += rowH;
    }
    y += s(16);
  } else {
    // Token logo (try to load, skip if fails)
    await drawTokenLogo(logoUrls?.[0]);
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
      ctx.fillText(fiat(usdValue!, props), W / 2, y + s(15));
      y += s(24);
    }
    y += s(16);
  }

  // Divider
  ctx.fillStyle = color.border.base;
  ctx.fillRect(L, y, contentW, 1);
  y += 1 + s(12);

  // Detail rows — split collapses the recipient into a count (each recipient is
  // already its own line item above); single/multiSelect show the one recipient.
  const toRow: [string, string, string?] = batch?.kind === 'split'
    ? [labels.to, labels.recipientsSummary]
    : [labels.to, shortAddr(to), displayToName ?? undefined];
  const details: [string, string, string?][] = [
    [labels.from, shortAddr(from), fromName],
    toRow,
    [labels.network, chain],
    [labels.time, formatDateTime(timestamp)],
    [labels.txHash, shortAddr(txHash)],
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
  ctx.fillText(labels.scanHint, W / 2, y + s(12));
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
  ctx.fillText(labels.footerBrand, W / 2, y + s(13));
  y += s(18);
  ctx.fillStyle = color.fg.subtle;
  ctx.font = `400 ${s(11)}px Inter, system-ui, sans-serif`;
  ctx.letterSpacing = '0px';
  ctx.fillText(labels.footerUrl, W / 2, y + s(11));

  return new Promise((resolve) => canvas.toBlob(resolve as BlobCallback, 'image/png', 1));
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function TransactionReceipt(props: Props) {
  const {
    from, fromName, chainId, txHash, userOpHash, rate, currencyCode, currencySymbol,
    timestamp, recipientIdentity, onDone, onSaveContact, transfers, batchKind, status,
  } = props;
  const { batch, amount, symbol, logoUrls, usdValue, to, toName } = normalizeReceipt(props);
  const { t } = useTranslation();
  useLocalePrefs(); // re-render when number/date/time format changes
  const receiptRef = useRef<View>(null);
  const [contactSaved, setContactSaved] = useState(false);
  const fiatPrefs = { rate, currencyCode, currencySymbol };
  const chain = chainName(chainId);
  const net = getAllNetworksSync().find(n => n.chainId === chainId);
  const displayToName = recipientIdentity?.name ?? toName;

  // The send is already accepted by the bundler when this renders; the on-chain
  // hash may still be resolving. While it is, poll the bundler ourselves and
  // converge the status — otherwise the receipt could sit on "confirming" forever
  // if the parent's waitForTxHash timed out or the screen was torn down. Local
  // resolution wins over the (possibly stale) props.
  const [liveStatus, setLiveStatus] = useState<'submitted' | 'confirmed' | 'failed' | null>(null);
  const [liveTxHash, setLiveTxHash] = useState<string | null>(null);
  const effTxHash = liveTxHash || txHash || '';
  // A prominent status stamp tells the user plainly whether it's still settling,
  // confirmed, or failed — while submitted we hide the (broken) explorer link / QR
  // and show a "confirming" hint instead.
  const settle = liveStatus ?? status ?? (effTxHash ? 'confirmed' : 'submitted');
  const pending = settle === 'submitted';
  const failed = settle === 'failed';
  const explorerUrl = explorerTxURL(chainId, effTxHash);

  useEffect(() => {
    if (settle !== 'submitted' || !userOpHash) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    let attempts = 0;
    const tick = async () => {
      if (cancelled) return;
      attempts++;
      const r = await pollUserOpReceipt(userOpHash, chainId);
      if (cancelled) return;
      if (r && (r.confirmed || r.failed)) {
        setLiveStatus(r.failed ? 'failed' : 'confirmed');
        if (r.txHash) setLiveTxHash(r.txHash);
        return; // final — stop polling
      }
      if (attempts < 60) timer = setTimeout(tick, 4000);
    };
    timer = setTimeout(tick, 1500);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [settle, userOpHash, chainId]);

  const recipientsSummary = t('componentsTx.receipt.recipientsCount', { n: batch?.items.length ?? 0 });
  const assetsSummary = t('componentsTx.receipt.assetsCount', { n: batch?.items.length ?? 0 });
  const canvasLabels = buildCanvasLabels(t, batch?.items.length ?? 0);

  const handleViewExplorer = () => openBrowser(explorerUrl);

  const handleShare = async () => {
    if (Platform.OS === 'web') {
      try {
        const blob = await renderReceiptToCanvas({
          from, fromName, to, toName, amount, symbol, chainId,
          txHash: effTxHash, logoUrls, usdValue, rate, currencyCode, currencySymbol, timestamp, recipientIdentity,
          transfers, batchKind, onDone,
        }, canvasLabels);
        const file = new File([blob], `vela-receipt-${(effTxHash || userOpHash || '').slice(0, 10)}.png`, { type: 'image/png' });

        if (navigator.share && navigator.canShare?.({ files: [file] })) {
          await navigator.share({ files: [file] });
          return;
        }

        // Fallback: copy image to clipboard
        try {
          await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
          hapticSuccess();
          showAlert(t('componentsTx.receipt.copiedTitle'), t('componentsTx.receipt.copiedImageBody'));
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
        showAlert(t('componentsTx.receipt.copiedTitle'), t('componentsTx.receipt.copiedLinkBody'));
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
        showAlert(t('componentsTx.receipt.copiedTitle'), t('componentsTx.receipt.copiedLinkBody'));
      }
    }
  };

  return (
    <ScrollView style={styles.screen} contentContainerStyle={styles.screenContent} showsVerticalScrollIndicator={false}>
      {/* Capturable receipt card */}
      <View ref={receiptRef} testID="receipt-card" collapsable={false} style={styles.receipt}>
        <View style={styles.header}>
          <Text style={styles.headerTitle}>{t('componentsTx.receipt.title')}</Text>
          <View style={styles.headerNetwork}>
            {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={16} />}
            <Text style={styles.headerChain}>{chain}</Text>
          </View>
        </View>
        <View style={styles.separator} />
        {batch ? (
          <>
            <View style={styles.amountSection}>
              {batch.kind === 'split' ? (
                <>
                  <TokenLogo symbol={batch.splitSymbol ?? ''} logoUrls={batch.items[0].logoUrls} size={44} />
                  <Text style={styles.amountText}>{batch.splitTotal} {batch.splitSymbol}</Text>
                  <Text style={styles.amountUsd}>
                    {recipientsSummary}{batch.totalUsd > 0 ? ` · ${fiat(batch.totalUsd, fiatPrefs)}` : ''}
                  </Text>
                </>
              ) : (
                <>
                  <Text style={styles.amountText}>
                    {batch.totalUsd > 0 ? fiat(batch.totalUsd, fiatPrefs) : assetsSummary}
                  </Text>
                  {batch.totalUsd > 0 && <Text style={styles.amountUsd}>{assetsSummary}</Text>}
                </>
              )}
            </View>
            <View style={styles.separator} />
            <View style={styles.breakdown}>
              {batch.items.map((it, i) => (
                <View key={i} style={styles.breakdownRow}>
                  {batch.kind === 'split' ? (
                    <Text style={styles.breakdownLabel} numberOfLines={1}>{it.toName || shortAddr(it.to)}</Text>
                  ) : (
                    <View style={styles.breakdownTokenCol}>
                      <TokenLogo symbol={it.symbol} logoUrls={it.logoUrls} size={24} />
                      <Text style={styles.breakdownLabel} numberOfLines={1}>{it.symbol}</Text>
                    </View>
                  )}
                  <View style={styles.breakdownValueCol}>
                    <Text style={styles.breakdownAmount}>
                      {formatBalance(parseFloat(it.amount))}{batch.kind === 'split' ? ` ${it.symbol}` : ''}
                    </Text>
                    {it.usdValue != null && it.usdValue > 0 && <Text style={styles.breakdownUsd}>{fiat(it.usdValue, fiatPrefs)}</Text>}
                  </View>
                </View>
              ))}
            </View>
          </>
        ) : (
          <View style={styles.amountSection}>
            <TokenLogo symbol={symbol} logoUrls={logoUrls} size={44} />
            <Text style={styles.amountText}>{formatBalance(parseFloat(amount))} {symbol}</Text>
            {usdValue != null && usdValue > 0 && <Text style={styles.amountUsd}>{fiat(usdValue, fiatPrefs)}</Text>}
          </View>
        )}

        {/* Prominent settlement stamp — success / submitting / failed at a glance */}
        <View style={[styles.statusStamp, failed ? styles.statusStampFail : pending ? styles.statusStampPending : styles.statusStampOk]}>
          {failed
            ? <XCircle size={16} color={color.error.base} strokeWidth={2.4} />
            : pending
              ? <Clock size={16} color={color.warning.base} strokeWidth={2.4} />
              : <CheckCircle2 size={16} color={color.success.base} strokeWidth={2.4} />}
          <Text style={[styles.statusStampText, { color: failed ? color.error.base : pending ? color.warning.base : color.success.base }]}>
            {failed ? t('componentsTx.receipt.statusFailed') : pending ? t('componentsTx.receipt.statusSubmitted') : t('componentsTx.receipt.statusConfirmed')}
          </Text>
        </View>

        <View style={styles.separator} />

        <View style={styles.detailRow}><Text style={styles.detailLabel}>{t('componentsTx.receipt.from')}</Text><View style={styles.detailValueCol}>{fromName && <Text style={styles.detailName}>{fromName}</Text>}<Text style={styles.detailAddr}>{shortAddr(from)}</Text></View></View>
        {batch?.kind === 'split' ? (
          <View style={styles.detailRow}><Text style={styles.detailLabel}>{t('componentsTx.receipt.to')}</Text><Text style={styles.detailValue}>{recipientsSummary}</Text></View>
        ) : (
          <View style={styles.detailRow}><Text style={styles.detailLabel}>{t('componentsTx.receipt.to')}</Text><View style={styles.detailValueCol}>{displayToName && <Text style={styles.detailName}>{displayToName}</Text>}<Text style={styles.detailAddr}>{shortAddr(to)}</Text></View></View>
        )}
        <View style={styles.detailRow}><Text style={styles.detailLabel}>{t('componentsTx.receipt.network')}</Text><Text style={styles.detailValue}>{chain}</Text></View>
        <View style={styles.detailRow}><Text style={styles.detailLabel}>{t('componentsTx.receipt.time')}</Text><Text style={styles.detailValue}>{formatDateTime(timestamp)}</Text></View>
        <View style={styles.detailRow}><Text style={styles.detailLabel}>{t('componentsTx.receipt.txHash')}</Text>{pending ? <Text style={styles.detailPending}>{t('componentsTx.receipt.confirming')}</Text> : failed ? <Text style={[styles.detailPending, { color: color.error.base }]}>{t('componentsTx.receipt.statusFailed')}</Text> : <Text style={styles.detailAddr}>{shortAddr(effTxHash)}</Text>}</View>

        {failed ? (
          <View style={styles.confirmingSection}>
            <Text style={[styles.confirmingHint, { color: color.error.base }]}>{t('componentsTx.receipt.failedHint')}</Text>
          </View>
        ) : pending ? (
          <View style={styles.confirmingSection}>
            <Text style={styles.confirmingHint}>{t('componentsTx.receipt.confirmingHint')}</Text>
          </View>
        ) : (
          <View style={styles.qrSection}>
            <QRCode value={explorerUrl} size={80} />
            <Text style={styles.qrHint}>{t('componentsTx.receipt.scanHint')}</Text>
          </View>
        )}
        <View style={styles.footer}>
          <Image source={LOGO_ASSET} style={styles.footerLogoImg} resizeMode="contain" />
          <Text style={styles.footerLogo}>{t('componentsTx.receipt.footerBrand')}</Text>
          <Text style={styles.footerUrl}>{t('componentsTx.receipt.footerUrl')}</Text>
        </View>
      </View>

      {/* Action buttons — explorer link appears once the on-chain hash resolves */}
      <View style={styles.actions}>
        {settle === 'confirmed' && (
          <Pressable style={styles.actionBtn} onPress={handleViewExplorer}>
            <ExternalLink size={18} color={color.fg.muted} strokeWidth={2} />
            <Text style={styles.actionText}>{t('componentsTx.receipt.explorer')}</Text>
          </Pressable>
        )}
        <Pressable style={styles.actionBtn} onPress={handleShare}>
          <Share2 size={18} color={color.fg.muted} strokeWidth={2} />
          <Text style={styles.actionText}>{t('componentsTx.receipt.share')}</Text>
        </Pressable>
        {onSaveContact && (
          <Pressable
            style={styles.actionBtn}
            disabled={contactSaved}
            onPress={() => { onSaveContact(); setContactSaved(true); }}
          >
            {contactSaved
              ? <Check size={18} color={color.success.base} strokeWidth={2} />
              : <BookmarkPlus size={18} color={color.fg.muted} strokeWidth={2} />}
            <Text style={styles.actionText}>
              {contactSaved ? t('contacts.saved') : t('contacts.saveToContacts')}
            </Text>
          </Pressable>
        )}
      </View>

      <Pressable style={styles.doneBtn} onPress={onDone}>
        <Text style={styles.doneBtnText}>{t('componentsTx.receipt.done')}</Text>
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
  statusStamp: { flexDirection: 'row', alignItems: 'center', alignSelf: 'center', gap: space.xs, paddingHorizontal: space.lg, paddingVertical: space.sm, borderRadius: radius.full, marginTop: space.sm },
  statusStampOk: { backgroundColor: color.success.soft },
  statusStampPending: { backgroundColor: color.warning.soft },
  statusStampFail: { backgroundColor: color.error.soft },
  statusStampText: { fontSize: text.sm, ...inter.semibold },
  breakdown: { gap: space.xs, paddingVertical: space.xs },
  breakdownRow: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', paddingVertical: space.sm, gap: space.md },
  breakdownTokenCol: { flexDirection: 'row', alignItems: 'center', gap: space.sm, flex: 1, minWidth: 0 },
  breakdownLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.base, flexShrink: 1 },
  breakdownValueCol: { alignItems: 'flex-end' as const },
  breakdownAmount: { fontSize: text.sm, ...inter.semibold, color: color.fg.base, textAlign: 'right' as const },
  breakdownUsd: { fontSize: text.xs, ...inter.regular, color: color.fg.muted, textAlign: 'right' as const, marginTop: 2 },
  detailRow: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'flex-start', paddingVertical: space.md },
  detailLabel: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, minWidth: 70 },
  detailValueCol: { alignItems: 'flex-end' as const, flex: 1 },
  detailName: { fontSize: text.sm, ...inter.semibold, color: color.fg.base, textAlign: 'right' as const },
  detailAddr: { fontSize: text.sm, ...inter.medium, fontFamily: font.mono, color: color.fg.muted, textAlign: 'right' as const },
  detailValue: { fontSize: text.sm, ...inter.semibold, color: color.fg.base, textAlign: 'right' as const, flex: 1 },
  qrSection: { alignItems: 'center', marginTop: space.xl, paddingTop: space.lg, borderTopWidth: 1, borderTopColor: color.border.base, gap: space.sm },
  qrHint: { fontSize: text.xs, ...inter.regular, color: color.fg.subtle },
  detailPending: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle, textAlign: 'right' as const },
  confirmingSection: { alignItems: 'center', marginTop: space.xl, paddingTop: space.lg, borderTopWidth: 1, borderTopColor: color.border.base },
  confirmingHint: { fontSize: text.sm, ...inter.medium, color: color.fg.muted, textAlign: 'center' as const },
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
