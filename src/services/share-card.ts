/**
 * Capture / share / save the Receive card as a branded image.
 *
 * Native: screenshot the off-screen <ReceiveShareCard> (a dedicated branded
 * layout, not the live UI) via react-native-view-shot, then share / save.
 * Web: draw an equivalent branded image on a Canvas (the RN tree doesn't
 * screenshot reliably on web) — with the real Vela mark and real chain logos.
 * Keep the two visually in sync.
 */
import type { ShareCardModel, ShareNetwork } from '@/components/ReceiveShareCard';
import { hapticSuccess } from '@/services/platform';
import QRCodeLib from 'qrcode';
import type { RefObject } from 'react';
import { Image, Platform } from 'react-native';

const LOGO = require('../../assets/images/icon.png');

export type SaveResult = 'saved' | 'downloaded' | 'denied' | 'unsupported';

async function captureNative(ref: RefObject<unknown>): Promise<string | null> {
  const { captureRef } = await import('react-native-view-shot');
  if (!ref.current) return null;
  return captureRef(ref as never, { format: 'png', quality: 1, result: 'tmpfile' });
}

export async function shareReceiveCard(ref: RefObject<unknown>, model: ShareCardModel, fileName: string): Promise<void> {
  if (Platform.OS === 'web') {
    const blob = await composeCardCanvas(model);
    const file = new File([blob], `${fileName}.png`, { type: 'image/png' });
    if (navigator.share && navigator.canShare?.({ files: [file] })) {
      await navigator.share({ files: [file] });
      return;
    }
    downloadBlob(blob, file.name);
    hapticSuccess();
    return;
  }

  const uri = await captureNative(ref);
  if (!uri) return;
  const Sharing = await import('expo-sharing');
  if (await Sharing.isAvailableAsync()) {
    await Sharing.shareAsync(uri, { mimeType: 'image/png', dialogTitle: model.summary ?? model.name });
  }
}

export async function saveReceiveCard(ref: RefObject<unknown>, model: ShareCardModel, fileName: string): Promise<SaveResult> {
  if (Platform.OS === 'web') {
    const blob = await composeCardCanvas(model);
    downloadBlob(blob, `${fileName}.png`);
    hapticSuccess();
    return 'downloaded';
  }

  const uri = await captureNative(ref);
  if (!uri) return 'unsupported';
  const MediaLibrary = await import('expo-media-library');
  const perm = await MediaLibrary.requestPermissionsAsync(true /* writeOnly */);
  if (!perm.granted) return 'denied';
  await MediaLibrary.saveToLibraryAsync(uri);
  hapticSuccess();
  return 'saved';
}

// ===========================================================================
// Web canvas composition
// ===========================================================================

function downloadBlob(blob: Blob, fileName: string) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.download = fileName;
  link.href = url;
  link.click();
  URL.revokeObjectURL(url);
}

const SANS = "Inter, -apple-system, system-ui, sans-serif";
const MONO = "ui-monospace, Menlo, monospace";
const INK = '#16161A';
const SUBTLE = '#9A968D';
const ACCENT = '#E8572A';
const CHIP_BG = '#F4F2EE';
const BORDER = '#ECEBE4';

function loadImageEl(src: string, cors: boolean): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new window.Image();
    if (cors) img.crossOrigin = 'anonymous';
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

/** Fetch a (possibly cross-origin) image as a blob first, so drawing it never taints the canvas. */
async function loadViaFetch(url: string): Promise<HTMLImageElement | null> {
  try {
    const r = await fetch(url, { mode: 'cors' });
    if (!r.ok) return null;
    const obj = URL.createObjectURL(await r.blob());
    const img = await loadImageEl(obj, false);
    URL.revokeObjectURL(obj);
    return img;
  } catch {
    return null;
  }
}

async function composeCardCanvas(model: ShareCardModel): Promise<Blob> {
  const S = 3; // retina
  const M = 24; // outer margin (so the card casts a soft shadow)
  const W = 384; // card width
  const PAD = 32;
  const contentW = W - PAD * 2;
  const isRequest = model.variant === 'request';
  const networks = !isRequest ? (model.networks ?? []) : [];

  // Preload imagery in parallel.
  const logoSrc = Image.resolveAssetSource(LOGO)?.uri ?? '';
  const [logo, ...netImgs] = await Promise.all([
    logoSrc ? loadImageEl(logoSrc, false) : Promise.resolve(null),
    ...networks.map((n) => (n.logoURL ? loadViaFetch(n.logoURL) : Promise.resolve(null))),
  ]);
  if (document.fonts?.ready) { try { await document.fonts.ready; } catch { /* ignore */ } }

  // --- layout metrics ---
  const QR = 206;
  const qrBox = QR + 36;
  const headerH = 30;
  const gapHeaderQr = 24;
  const gapQrName = 22;
  const nameH = 30;

  const cols = 2, colGap = 10, chipH = 42, rowGap = 8;
  const rows = Math.ceil(networks.length / cols);
  const gridH = networks.length ? rows * chipH + (rows - 1) * rowGap : 0;

  const bodyH = isRequest
    ? 28 /*summary*/ + 22 /*addr*/
    : 22 /*addr*/ + 24 /*divider+label*/ + 14 + gridH;

  const cardH = PAD + headerH + gapHeaderQr + qrBox + gapQrName + nameH + 14 + bodyH + 28 /*footer*/ + PAD;
  const W2 = W + M * 2;
  const H2 = cardH + M * 2;

  const canvas = document.createElement('canvas');
  canvas.width = W2 * S;
  canvas.height = H2 * S;
  const ctx = canvas.getContext('2d')!;
  ctx.scale(S, S);

  // Soft neutral backdrop
  ctx.fillStyle = '#F2F1ED';
  ctx.fillRect(0, 0, W2, H2);

  // Card with a soft drop shadow
  ctx.save();
  ctx.shadowColor = 'rgba(20,18,12,0.10)';
  ctx.shadowBlur = 30;
  ctx.shadowOffsetY = 10;
  roundRect(ctx, M, M, W, cardH, 30);
  ctx.fillStyle = '#FFFFFF';
  ctx.fill();
  ctx.restore();

  const cx = M + W / 2;
  let y = M + PAD;

  // --- brand header ---
  ctx.textBaseline = 'middle';
  ctx.font = `800 17px ${SANS}`;
  const brand = 'Vela Wallet';
  const btw = ctx.measureText(brand).width;
  const lsz = 26;
  const brandW = lsz + 9 + btw;
  let bx = cx - brandW / 2;
  const by = y + headerH / 2;
  if (logo) drawRoundedImage(ctx, logo, bx, by - lsz / 2, lsz, 7);
  else { roundRect(ctx, bx, by - lsz / 2, lsz, lsz, 7); ctx.fillStyle = INK; ctx.fill(); }
  ctx.fillStyle = INK;
  ctx.textAlign = 'left';
  ctx.fillText(brand, bx + lsz + 9, by + 0.5);
  y += headerH + gapHeaderQr;

  // --- QR ---
  const qrCanvas = document.createElement('canvas');
  await QRCodeLib.toCanvas(qrCanvas, model.qrValue || model.address, { width: QR, margin: 0, errorCorrectionLevel: 'M', color: { dark: '#16161A', light: '#FFFFFF' } });
  const qrX = cx - QR / 2;
  ctx.strokeStyle = BORDER;
  ctx.lineWidth = 1;
  roundRect(ctx, qrX - 18, y, qrBox, qrBox, 20);
  ctx.stroke();
  ctx.drawImage(qrCanvas, qrX, y + 18, QR, QR);
  y += qrBox + gapQrName;

  // --- name ---
  ctx.textAlign = 'center';
  ctx.fillStyle = INK;
  ctx.font = `700 23px ${SANS}`;
  ctx.fillText(model.name, cx, y + nameH / 2);
  y += nameH + 14;

  // --- body ---
  if (isRequest) {
    if (model.summary) {
      ctx.fillStyle = ACCENT;
      ctx.font = `600 16px ${SANS}`;
      ctx.fillText(model.summary, cx, y + 8);
      y += 28;
    }
    ctx.fillStyle = SUBTLE;
    ctx.font = `500 13px ${MONO}`;
    ctx.fillText(shortAddr(model.address), cx, y + 6);
  } else {
    ctx.fillStyle = SUBTLE;
    ctx.font = `500 13px ${MONO}`;
    ctx.fillText(shortAddr(model.address), cx, y + 6);
    y += 22;
    // label
    ctx.fillStyle = SUBTLE;
    ctx.font = `500 12px ${SANS}`;
    ctx.fillText(`${networks.length} supported networks`, cx, y + 12);
    y += 24 + 6;
    // 2-column grid
    const colW = (contentW - colGap) / 2;
    networks.forEach((n, i) => {
      const col = i % cols;
      const row = Math.floor(i / cols);
      const x = M + PAD + col * (colW + colGap);
      const cy = y + row * (chipH + rowGap);
      roundRect(ctx, x, cy, colW, chipH, chipH / 2);
      ctx.fillStyle = CHIP_BG;
      ctx.fill();
      const iconSize = 22;
      const ix = x + 12;
      const iy = cy + (chipH - iconSize) / 2;
      const img = netImgs[i];
      if (img) drawCircleImage(ctx, img, ix, iy, iconSize);
      else drawBadge(ctx, n, ix, iy, iconSize);
      ctx.fillStyle = INK;
      ctx.font = `600 13.5px ${SANS}`;
      ctx.textAlign = 'left';
      ctx.textBaseline = 'middle';
      ctx.fillText(clip(ctx, n.name, colW - iconSize - 28), ix + iconSize + 9, cy + chipH / 2 + 0.5);
    });
    y += gridH;
  }

  // --- footer ---
  ctx.fillStyle = '#B7B4AC';
  ctx.font = `600 13px ${SANS}`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'alphabetic';
  ctx.fillText('getvela.app', cx, M + cardH - PAD + 6);

  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('toBlob failed'))), 'image/png', 1);
  });
}

function shortAddr(a: string): string {
  return a ? `${a.slice(0, 10)}…${a.slice(-8)}` : '';
}

function clip(ctx: CanvasRenderingContext2D, s: string, maxW: number): string {
  if (ctx.measureText(s).width <= maxW) return s;
  let out = s;
  while (out.length > 1 && ctx.measureText(out + '…').width > maxW) out = out.slice(0, -1);
  return out + '…';
}

function drawBadge(ctx: CanvasRenderingContext2D, n: ShareNetwork, x: number, y: number, size: number) {
  ctx.beginPath();
  ctx.arc(x + size / 2, y + size / 2, size / 2, 0, Math.PI * 2);
  ctx.fillStyle = n.bg;
  ctx.fill();
  ctx.fillStyle = n.color;
  ctx.font = `700 ${Math.round(size * 0.36)}px ${SANS}`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(n.label.slice(0, 4), x + size / 2, y + size / 2 + 0.5);
}

function drawCircleImage(ctx: CanvasRenderingContext2D, img: HTMLImageElement, x: number, y: number, size: number) {
  ctx.save();
  ctx.beginPath();
  ctx.arc(x + size / 2, y + size / 2, size / 2, 0, Math.PI * 2);
  ctx.clip();
  ctx.drawImage(img, x, y, size, size);
  ctx.restore();
}

function drawRoundedImage(ctx: CanvasRenderingContext2D, img: HTMLImageElement, x: number, y: number, size: number, r: number) {
  ctx.save();
  roundRect(ctx, x, y, size, size, r);
  ctx.clip();
  ctx.drawImage(img, x, y, size, size);
  ctx.restore();
}

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}
