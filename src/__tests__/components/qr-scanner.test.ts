/**
 * Tests for QR scanner decoding — verifies jsQR can handle both simple
 * Ethereum addresses and dense WalletPair URIs at various resolutions.
 */
import jsQR from 'jsqr';
import QRCode from 'qrcode';

/** Reproduces parseAddress from QRScanner.tsx */
function parseAddress(data: string): string {
  let address = data.trim();
  if (address.startsWith('ethereum:')) {
    address = address.replace('ethereum:', '').split('?')[0].split('@')[0];
  }
  return address;
}

/**
 * Generate a synthetic camera frame: QR code rendered on a dark background,
 * occupying ~50% of the frame width (simulating a phone scanning a screen).
 */
function buildFrame(
  qrData: string,
  frameW: number,
  frameH: number,
  qrFraction = 0.5,
): Uint8ClampedArray {
  const qr = QRCode.create(qrData, { errorCorrectionLevel: 'L' });
  const modules = qr.modules;
  const mSize = modules.size;
  const qrPx = Math.floor(frameW * qrFraction);
  const scale = qrPx / mSize;
  const offsetX = Math.floor((frameW - qrPx) / 2);
  const offsetY = Math.floor((frameH - qrPx) / 2);

  const data = new Uint8ClampedArray(frameW * frameH * 4);
  // Dark background
  for (let i = 0; i < data.length; i += 4) {
    data[i] = 20; data[i + 1] = 20; data[i + 2] = 28; data[i + 3] = 255;
  }
  // White quiet zone + modules
  const pad = Math.ceil(scale * 2);
  for (let y = -pad; y < qrPx + pad; y++) {
    for (let x = -pad; x < qrPx + pad; x++) {
      const px = offsetX + x, py = offsetY + y;
      if (px < 0 || px >= frameW || py < 0 || py >= frameH) continue;
      const mx = Math.floor(x / scale), my = Math.floor(y / scale);
      const isDark = mx >= 0 && mx < mSize && my >= 0 && my < mSize && modules.get(mx, my);
      const val = isDark ? 0 : 255;
      const idx = (py * frameW + px) * 4;
      data[idx] = val; data[idx + 1] = val; data[idx + 2] = val;
    }
  }
  return data;
}

/** Nearest-neighbor downscale (matches browser canvas drawImage behavior). */
function downscale(
  src: Uint8ClampedArray, srcW: number, srcH: number, dstW: number,
): { data: Uint8ClampedArray; w: number; h: number } {
  const dstH = Math.round(dstW * srcH / srcW);
  const out = new Uint8ClampedArray(dstW * dstH * 4);
  const sx = srcW / dstW, sy = srcH / dstH;
  for (let y = 0; y < dstH; y++) {
    for (let x = 0; x < dstW; x++) {
      const si = (Math.floor(y * sy) * srcW + Math.floor(x * sx)) * 4;
      const di = (y * dstW + x) * 4;
      out[di] = src[si]; out[di + 1] = src[si + 1]; out[di + 2] = src[si + 2]; out[di + 3] = 255;
    }
  }
  return { data: out, w: dstW, h: dstH };
}

// ---------------------------------------------------------------------------

const SIMPLE_ADDR = '0x1234567890abcdef1234567890abcdef12345678';
const ETHEREUM_URI = 'ethereum:0xABCDef1234567890abcdef1234567890abcdef12@1?value=1000000';

// Realistic walletpair URI (269 chars, QR version 10, 57×57 modules)
const WALLETPAIR_URI =
  'walletpair:?ch=e909cb64f8a639479fd18ecf0ce4f62149a8df63542f66aa042008d463ac2b8d' +
  '&pubkey=KcnzxAsEIMywpGOww%2FqKgeTEI26G87DSOfCD8S5UkwA%3D' +
  '&relay=wss%3A%2F%2Frelay.walletpair.xyz%2Fws' +
  '&name=WalletPair&url=https%3A%2F%2Fwalletpair.xyz' +
  '&icon=https%3A%2F%2Fwalletpair.xyz%2Ficon.png';

// ---------------------------------------------------------------------------
// parseAddress
// ---------------------------------------------------------------------------

describe('parseAddress', () => {
  test('passes through plain address', () => {
    expect(parseAddress(SIMPLE_ADDR)).toBe(SIMPLE_ADDR);
  });

  test('strips ethereum: prefix and query', () => {
    expect(parseAddress(ETHEREUM_URI)).toBe('0xABCDef1234567890abcdef1234567890abcdef12');
  });

  test('passes walletpair URI through unchanged', () => {
    expect(parseAddress(WALLETPAIR_URI)).toBe(WALLETPAIR_URI);
  });

  test('passes walletpair URI with leading whitespace', () => {
    expect(parseAddress('  ' + WALLETPAIR_URI)).toBe(WALLETPAIR_URI);
  });
});

// ---------------------------------------------------------------------------
// jsQR decoding
// ---------------------------------------------------------------------------

describe('jsQR walletpair decoding', () => {
  const FRAME_W = 1920, FRAME_H = 1080; // simulated camera resolution

  test('decodes address QR at 640px canvas', () => {
    const frame = buildFrame(SIMPLE_ADDR, FRAME_W, FRAME_H);
    const { data, w, h } = downscale(frame, FRAME_W, FRAME_H, 640);
    const result = jsQR(data as any, w, h, { inversionAttempts: 'attemptBoth' });
    expect(result).not.toBeNull();
    expect(result!.data).toBe(SIMPLE_ADDR);
  });

  test('decodes walletpair QR at 640px canvas', () => {
    const frame = buildFrame(WALLETPAIR_URI, FRAME_W, FRAME_H);
    const { data, w, h } = downscale(frame, FRAME_W, FRAME_H, 640);
    const result = jsQR(data as any, w, h, { inversionAttempts: 'attemptBoth' });
    expect(result).not.toBeNull();
    expect(result!.data).toBe(WALLETPAIR_URI);
  });

  test('decodes walletpair QR at 1280px canvas', () => {
    const frame = buildFrame(WALLETPAIR_URI, FRAME_W, FRAME_H);
    const { data, w, h } = downscale(frame, FRAME_W, FRAME_H, 1280);
    const result = jsQR(data as any, w, h, { inversionAttempts: 'attemptBoth' });
    expect(result).not.toBeNull();
    expect(result!.data).toBe(WALLETPAIR_URI);
  });

  test('decodes walletpair QR occupying only 35% of frame', () => {
    const frame = buildFrame(WALLETPAIR_URI, FRAME_W, FRAME_H, 0.35);
    const { data, w, h } = downscale(frame, FRAME_W, FRAME_H, 640);
    const result = jsQR(data as any, w, h, { inversionAttempts: 'attemptBoth' });
    expect(result).not.toBeNull();
    expect(result!.data).toBe(WALLETPAIR_URI);
  });

  test('full pipeline: scan → parseAddress → walletpair URI intact', () => {
    const frame = buildFrame(WALLETPAIR_URI, FRAME_W, FRAME_H);
    const { data, w, h } = downscale(frame, FRAME_W, FRAME_H, 1280);
    const result = jsQR(data as any, w, h, { inversionAttempts: 'attemptBoth' });
    expect(result).not.toBeNull();
    const processed = parseAddress(result!.data);
    expect(processed).toBe(WALLETPAIR_URI);
    expect(processed.startsWith('walletpair:')).toBe(true);
  });
});
