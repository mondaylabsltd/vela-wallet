/**
 * Pure-JS image decoder for native platforms (no canvas needed).
 * Decodes JPEG/PNG base64 data into raw RGBA pixel data for jsQR.
 */

import jpegDecode from 'jpeg-js';
// @ts-ignore — pngjs has no type declarations
import { PNG } from 'pngjs';

export interface ImageData {
  data: Uint8ClampedArray;
  width: number;
  height: number;
}

/** Maximum dimension — scale down large images to avoid OOM on mobile. */
const MAX_DIM = 1024;

/**
 * Decode a base64-encoded JPEG or PNG into RGBA pixel data.
 * Returns null if the format is unrecognised or decoding fails.
 */
export function decodeBase64Image(base64: string, hintWidth?: number, hintHeight?: number): ImageData | null {
  const buf = Buffer.from(base64, 'base64');

  // Detect format by magic bytes
  const isJPEG = buf[0] === 0xFF && buf[1] === 0xD8;
  const isPNG = buf[0] === 0x89 && buf[1] === 0x50 && buf[2] === 0x4E && buf[3] === 0x47;

  try {
    if (isJPEG) {
      return decodeJPEG(buf);
    }
    if (isPNG) {
      return decodePNG(buf);
    }
  } catch (e) {
    console.warn('[image-decode] Failed to decode image:', e);
  }
  return null;
}

function decodeJPEG(buf: Buffer): ImageData {
  // Limit max memory to prevent OOM on huge photos
  const raw = jpegDecode.decode(buf, { useTArray: true, maxMemoryUsageInMB: 128 });
  return maybeScale({
    data: new Uint8ClampedArray(raw.data.buffer, raw.data.byteOffset, raw.data.byteLength),
    width: raw.width,
    height: raw.height,
  });
}

function decodePNG(buf: Buffer): ImageData {
  const png = PNG.sync.read(buf);
  return maybeScale({
    data: new Uint8ClampedArray(png.data.buffer, png.data.byteOffset, png.data.byteLength),
    width: png.width,
    height: png.height,
  });
}

/** Scale down if either dimension exceeds MAX_DIM (simple nearest-neighbour). */
function maybeScale(img: ImageData): ImageData {
  if (img.width <= MAX_DIM && img.height <= MAX_DIM) return img;

  const scale = MAX_DIM / Math.max(img.width, img.height);
  const w = Math.round(img.width * scale);
  const h = Math.round(img.height * scale);
  const out = new Uint8ClampedArray(w * h * 4);

  for (let y = 0; y < h; y++) {
    const srcY = Math.min(Math.round(y / scale), img.height - 1);
    for (let x = 0; x < w; x++) {
      const srcX = Math.min(Math.round(x / scale), img.width - 1);
      const si = (srcY * img.width + srcX) * 4;
      const di = (y * w + x) * 4;
      out[di] = img.data[si];
      out[di + 1] = img.data[si + 1];
      out[di + 2] = img.data[si + 2];
      out[di + 3] = img.data[si + 3];
    }
  }

  return { data: out, width: w, height: h };
}
