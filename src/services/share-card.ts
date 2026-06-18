/**
 * Capture / share / save the Receive card as a branded image.
 *
 * Native: screenshot the off-screen <ReceiveShareCard> (a dedicated branded
 * layout, not the live UI) via react-native-view-shot, then share / save.
 * Web: draw an equivalent branded image on a Canvas (the RN tree doesn't
 * screenshot reliably on web). Keep the two visually in sync.
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

// -- Web canvas composition --------------------------------------------------

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
const MUTED = '#8A8A96';
const ACCENT = '#E8572A';

async function loadLogo(): Promise<HTMLImageElement | null> {
  try {
    const src = Image.resolveAssetSource(LOGO)?.uri;
    if (!src) return null;
    const img = new window.Image();
    img.crossOrigin = 'anonymous';
    await new Promise<void>((res, rej) => { img.onload = () => res(); img.onerror = () => rej(); img.src = src; });
    return img;
  } catch {
    return null;
  }
}

function shortAddr(a: string): string {
  return a ? `${a.slice(0, 10)}…${a.slice(-8)}` : '';
}

/** Greedy-wrap network chips into centered rows; returns the rows + each chip's width. */
function layoutNetworks(ctx: CanvasRenderingContext2D, nets: ShareNetwork[], contentW: number) {
  ctx.font = `600 13px ${SANS}`;
  const padX = 12, dot = 16, dotGap = 6, gap = 8;
  const chips = nets.map((n) => ({ n, w: padX + dot + dotGap + ctx.measureText(n.name).width + padX }));
  const rows: { chips: typeof chips; width: number }[] = [];
  let i = 0;
  while (i < chips.length) {
    const row: typeof chips = [];
    let w = 0;
    while (i < chips.length && (row.length === 0 || w + gap + chips[i].w <= contentW)) {
      if (row.length) w += gap;
      w += chips[i].w;
      row.push(chips[i]);
      i++;
    }
    rows.push({ chips: row, width: w });
  }
  return { rows, chipH: 30, gap };
}

async function composeCardCanvas(model: ShareCardModel): Promise<Blob> {
  const S = 3; // retina scale for crisp text
  const W = 360;
  const PAD = 28;
  const contentW = W - PAD * 2;
  const isRequest = model.variant === 'request';
  const networks = !isRequest ? (model.networks ?? []) : [];

  // Measure network rows up front (height is dynamic for the address card).
  const measure = document.createElement('canvas').getContext('2d')!;
  const net = networks.length ? layoutNetworks(measure, networks, contentW) : null;
  const netHeight = net ? 26 /*label*/ + net.rows.length * (net.chipH + net.gap) : 0;

  const qrTop = 24 + 30 /*brand*/ + 18;
  const qrBox = 196 + 36; // qr + inner padding
  const nameTop = qrTop + qrBox + 22;
  const bodyTop = nameTop + 34;
  const bodyHeight = isRequest
    ? 26 /*summary*/ + 22 /*addr*/
    : 22 /*addr*/ + 16 + netHeight;
  const H = bodyTop + bodyHeight + 22 /*gap*/ + 18 /*footer*/ + 24;

  const canvas = document.createElement('canvas');
  canvas.width = W * S;
  canvas.height = H * S;
  const ctx = canvas.getContext('2d')!;
  ctx.scale(S, S);

  // Card background
  ctx.fillStyle = '#FFFFFF';
  roundRect(ctx, 0, 0, W, H, 28);
  ctx.fill();

  // Brand header
  const cx = W / 2;
  const logo = await loadLogo();
  ctx.textBaseline = 'middle';
  const brandText = 'Vela Wallet';
  ctx.font = `700 16px ${SANS}`;
  const btw = ctx.measureText(brandText).width;
  const logoSize = 22;
  const brandW = (logo ? logoSize + 8 : 0) + btw;
  let bx = cx - brandW / 2;
  const by = 24 + 12;
  if (logo) {
    roundRect(ctx, bx, by - logoSize / 2, logoSize, logoSize, 6);
    ctx.save();
    ctx.clip();
    ctx.drawImage(logo, bx, by - logoSize / 2, logoSize, logoSize);
    ctx.restore();
    bx += logoSize + 8;
  }
  ctx.fillStyle = INK;
  ctx.textAlign = 'left';
  ctx.fillText(brandText, bx, by);

  // QR
  const qrCanvas = document.createElement('canvas');
  await QRCodeLib.toCanvas(qrCanvas, model.qrValue || model.address, { width: 196, margin: 0, errorCorrectionLevel: 'M', color: { dark: '#000000', light: '#FFFFFF' } });
  const qrX = cx - 196 / 2;
  ctx.strokeStyle = '#ECEBE4';
  ctx.lineWidth = 1;
  roundRect(ctx, qrX - 18, qrTop, 196 + 36, 196 + 36, 16);
  ctx.stroke();
  ctx.drawImage(qrCanvas, qrX, qrTop + 18, 196, 196);

  // Name
  ctx.textAlign = 'center';
  ctx.fillStyle = INK;
  ctx.font = `700 22px ${SANS}`;
  ctx.fillText(model.name, cx, nameTop + 14);

  // Body
  let y = bodyTop;
  if (isRequest) {
    if (model.summary) {
      ctx.fillStyle = ACCENT;
      ctx.font = `600 16px ${SANS}`;
      ctx.fillText(model.summary, cx, y + 6);
      y += 26;
    }
    ctx.fillStyle = MUTED;
    ctx.font = `500 13px ${MONO}`;
    ctx.fillText(shortAddr(model.address), cx, y + 6);
  } else {
    ctx.fillStyle = MUTED;
    ctx.font = `500 13px ${MONO}`;
    ctx.fillText(shortAddr(model.address), cx, y + 6);
    y += 22 + 16;
    if (net) {
      ctx.fillStyle = '#B0ADA5';
      ctx.font = `500 12px ${SANS}`;
      ctx.fillText(`${networks.length} supported networks`, cx, y);
      y += 24;
      drawNetworkRows(ctx, net, cx, y);
    }
  }

  // Footer
  ctx.fillStyle = '#B5B5BE';
  ctx.font = `600 13px ${SANS}`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'alphabetic';
  ctx.fillText('getvela.app', cx, H - 22);

  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('toBlob failed'))), 'image/png', 1);
  });
}

function drawNetworkRows(ctx: CanvasRenderingContext2D, net: ReturnType<typeof layoutNetworks>, cx: number, startY: number) {
  const { rows, chipH, gap } = net;
  const padX = 12, dot = 16, dotGap = 6;
  let y = startY;
  for (const row of rows) {
    let x = cx - row.width / 2;
    for (const c of row.chips) {
      roundRect(ctx, x, y, c.w, chipH, chipH / 2);
      ctx.fillStyle = '#F5F3EF';
      ctx.fill();
      // colored badge
      const dcx = x + padX + dot / 2;
      const dcy = y + chipH / 2;
      ctx.beginPath();
      ctx.arc(dcx, dcy, dot / 2, 0, Math.PI * 2);
      ctx.fillStyle = c.n.bg;
      ctx.fill();
      ctx.fillStyle = c.n.color;
      ctx.font = `700 7px ${SANS}`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(c.n.label.slice(0, 4), dcx, dcy + 0.5);
      // name
      ctx.fillStyle = INK;
      ctx.font = `600 13px ${SANS}`;
      ctx.textAlign = 'left';
      ctx.fillText(c.n.name, x + padX + dot + dotGap, dcy);
      x += c.w + gap;
    }
    y += chipH + gap;
  }
  ctx.textAlign = 'center';
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
