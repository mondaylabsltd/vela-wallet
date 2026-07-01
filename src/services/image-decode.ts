/**
 * Pure-JS JPEG decoder for native platforms (no canvas needed).
 *
 * PNG is deliberately not decoded here. The gallery flow first asks
 * expo-camera's native `scanFromURLAsync` to scan both JPEG and PNG files. This
 * module is only the jsQR fallback, and importing the Node-oriented `pngjs`
 * package here pulls `util`/`stream` into the Android bundle and breaks Metro.
 */

import jpegDecode from 'jpeg-js';

export interface ImageData {
  data: Uint8ClampedArray;
  width: number;
  height: number;
}

/** Maximum dimension — scale down large images to avoid OOM on mobile. */
const MAX_DIM = 1024;

/**
 * Decode a base64-encoded JPEG into RGBA pixel data.
 * Returns null for PNG/unknown formats or when decoding fails.
 */
export function decodeBase64Image(base64: string): ImageData | null {
  const buf = Buffer.from(base64, 'base64');

  // Detect format by magic bytes
  const isJPEG = buf[0] === 0xFF && buf[1] === 0xD8;

  try {
    if (isJPEG) {
      return decodeJPEG(buf);
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
