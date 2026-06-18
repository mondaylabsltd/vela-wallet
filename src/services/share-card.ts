/**
 * Capture / share / save the Receive card as an image.
 *
 * Native: screenshot the on-screen card via react-native-view-shot, then share
 * through the OS sheet (expo-sharing) or save to the photo library
 * (expo-media-library). Web: compose a standalone "long image" on a Canvas
 * (the RN view tree doesn't screenshot reliably on web) and share / download it.
 *
 * Mirrors the proven pattern in components/ui/TransactionReceipt.tsx.
 */
import { hapticSuccess } from '@/services/platform';
import QRCodeLib from 'qrcode';
import type { RefObject } from 'react';
import { Platform } from 'react-native';

export interface ShareCardData {
  /** Account / wallet display name shown under the QR. */
  name: string;
  /** The exact string encoded in the QR (address or EIP-681 URI). */
  qrValue: string;
  /** Human-readable one-liner, e.g. "Request 1.5 USDC · Polygon". */
  summary: string;
  /** Base file name (without extension). */
  fileName: string;
}

export type SaveResult = 'saved' | 'downloaded' | 'denied' | 'unsupported';

async function captureNative(ref: RefObject<unknown>): Promise<string | null> {
  const { captureRef } = await import('react-native-view-shot');
  if (!ref.current) return null;
  return captureRef(ref as never, { format: 'png', quality: 1, result: 'tmpfile' });
}

/** Share the card image through the OS share sheet (native) or Web Share / download (web). */
export async function shareReceiveCard(ref: RefObject<unknown>, data: ShareCardData): Promise<void> {
  if (Platform.OS === 'web') {
    const blob = await composeCardCanvas(data);
    const file = new File([blob], `${data.fileName}.png`, { type: 'image/png' });
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
    await Sharing.shareAsync(uri, { mimeType: 'image/png', dialogTitle: data.summary });
  }
}

/** Save the card image to the device photo library (native) or download it (web). */
export async function saveReceiveCard(ref: RefObject<unknown>, data: ShareCardData): Promise<SaveResult> {
  if (Platform.OS === 'web') {
    const blob = await composeCardCanvas(data);
    downloadBlob(blob, `${data.fileName}.png`);
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

/**
 * Compose a clean, share-ready "long image": white rounded card on a soft
 * backdrop, centered QR, name, summary, and the encoded value in mono, with a
 * Vela footer. Pixel-doubled for retina.
 */
async function composeCardCanvas(data: ShareCardData): Promise<Blob> {
  const S = 2; // retina scale
  const W = 540;
  const H = 760;
  const canvas = document.createElement('canvas');
  canvas.width = W * S;
  canvas.height = H * S;
  const ctx = canvas.getContext('2d')!;
  ctx.scale(S, S);

  // Backdrop
  ctx.fillStyle = '#F4F4F2';
  ctx.fillRect(0, 0, W, H);

  // Card
  const pad = 28;
  roundRect(ctx, pad, pad, W - pad * 2, H - pad * 2, 28);
  ctx.fillStyle = '#FFFFFF';
  ctx.fill();

  // QR — render to an offscreen canvas, then draw centered.
  const qrSize = 320;
  const qrCanvas = document.createElement('canvas');
  await QRCodeLib.toCanvas(qrCanvas, data.qrValue, {
    width: qrSize,
    margin: 1,
    color: { dark: '#000000', light: '#FFFFFF' },
    errorCorrectionLevel: 'M',
  });
  const qrX = (W - qrSize) / 2;
  const qrY = 72;
  ctx.drawImage(qrCanvas, qrX, qrY, qrSize, qrSize);

  // Name
  ctx.textAlign = 'center';
  ctx.fillStyle = '#16161A';
  ctx.font = '700 26px Inter, system-ui, sans-serif';
  ctx.fillText(data.name, W / 2, qrY + qrSize + 56);

  // Summary
  ctx.fillStyle = '#5B5B66';
  ctx.font = '500 17px Inter, system-ui, sans-serif';
  wrapText(ctx, data.summary, W / 2, qrY + qrSize + 92, W - pad * 4, 24);

  // Encoded value (mono, wrapped)
  ctx.fillStyle = '#8A8A96';
  ctx.font = '400 13px ui-monospace, Menlo, monospace';
  wrapText(ctx, data.qrValue, W / 2, qrY + qrSize + 150, W - pad * 3, 18);

  // Footer
  ctx.fillStyle = '#B5B5BE';
  ctx.font = '600 13px Inter, system-ui, sans-serif';
  ctx.fillText('Vela Wallet', W / 2, H - pad - 18);

  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('toBlob failed'))), 'image/png', 1);
  });
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

/** Draw centered, wrapped text; returns the y after the last line. */
function wrapText(ctx: CanvasRenderingContext2D, textStr: string, cx: number, y: number, maxW: number, lh: number): number {
  const words = textStr.split(/(\s+|(?<=.{12}))/).filter(Boolean); // break long unbroken URIs too
  let line = '';
  let yy = y;
  for (const word of words) {
    const test = line + word;
    if (ctx.measureText(test).width > maxW && line) {
      ctx.fillText(line.trim(), cx, yy);
      line = word;
      yy += lh;
    } else {
      line = test;
    }
  }
  if (line) ctx.fillText(line.trim(), cx, yy);
  return yy + lh;
}
