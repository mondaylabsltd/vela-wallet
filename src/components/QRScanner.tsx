import React, { useState, useRef, useEffect } from 'react';
import { Platform, View, Text, StyleSheet, Pressable, Modal, StatusBar } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withRepeat,
  withTiming,
  Easing,
} from 'react-native-reanimated';
import { showAlert, hapticSuccess } from '@/services/platform';
import jsQR from 'jsqr';
import { color, text, inter, space, radius, createStyles } from '@/constants/theme';
import { X, SwitchCamera, Camera, ImagePlus } from 'lucide-react-native';

interface Props {
  visible: boolean;
  onScan: (data: string) => void;
  onClose: () => void;
}

/** Parse ethereum: URI or raw address from scanned data. */
function parseAddress(data: string): string {
  let address = data.trim();
  if (address.startsWith('ethereum:')) {
    address = address.replace('ethereum:', '').split('?')[0].split('@')[0];
  }
  return address;
}

/**
 * Downscale RGBA pixel data using bilinear interpolation.
 * Smooths JPEG artifacts that confuse jsQR at full resolution.
 */
function downscalePixels(
  src: Uint8ClampedArray, srcW: number, srcH: number, dstW: number,
): { data: Uint8ClampedArray; width: number; height: number } {
  const dstH = Math.round(dstW * srcH / srcW);
  const out = new Uint8ClampedArray(dstW * dstH * 4);
  const xr = srcW / dstW, yr = srcH / dstH;
  for (let y = 0; y < dstH; y++) {
    for (let x = 0; x < dstW; x++) {
      const sx = x * xr, sy = y * yr;
      const x0 = Math.floor(sx), y0 = Math.floor(sy);
      const x1 = Math.min(x0 + 1, srcW - 1), y1 = Math.min(y0 + 1, srcH - 1);
      const xf = sx - x0, yf = sy - y0;
      const i00 = (y0 * srcW + x0) * 4, i10 = (y0 * srcW + x1) * 4;
      const i01 = (y1 * srcW + x0) * 4, i11 = (y1 * srcW + x1) * 4;
      const di = (y * dstW + x) * 4;
      for (let c = 0; c < 3; c++) {
        out[di + c] = Math.round(
          src[i00 + c] * (1 - xf) * (1 - yf) + src[i10 + c] * xf * (1 - yf) +
          src[i01 + c] * (1 - xf) * yf + src[i11 + c] * xf * yf,
        );
      }
      out[di + 3] = 255;
    }
  }
  return { data: out, width: dstW, height: dstH };
}

/**
 * Binarize image data at a fixed luminance threshold.
 * Pixels darker than threshold → black, everything else → white.
 */
function binarizeAt(data: Uint8ClampedArray, width: number, height: number, threshold: number): ImageData {
  const out = new Uint8ClampedArray(data.length);
  for (let i = 0; i < data.length; i += 4) {
    const lum = (data[i] * 77 + data[i + 1] * 150 + data[i + 2] * 29) >> 8;
    const val = lum <= threshold ? 0 : 255;
    out[i] = val; out[i + 1] = val; out[i + 2] = val; out[i + 3] = 255;
  }
  return new ImageData(out, width, height);
}

const JSQR_OPTS = { inversionAttempts: 'attemptBoth' as const };

/**
 * Decode QR from image data. Strategy:
 * 1. Raw jsQR (handles clean / light-bg images)
 * 2. Downscale to ~600px + binarize at multiple thresholds
 *    (handles JPEG photos of dark-themed screens — smooths artifacts
 *     and separates dark-gray backgrounds from true-black QR modules)
 *
 * Threshold 160 works best for real JPEG photos of dark-themed pages;
 * 120 handles cleaner dark screenshots; 80 is a low-threshold fallback.
 */
function decodeQR(imageData: ImageData, label = 'image'): string | null {
  const { data, width, height } = imageData;
  // 1. Fast path: raw image
  const direct = jsQR(data as any, width, height, JSQR_OPTS);
  if (direct?.data) {
    console.log(`[QR] ${label}: decoded raw ${width}×${height}`);
    return direct.data;
  }

  // 2. Downscale (smooths JPEG noise) + binarize at progressive thresholds
  const targetW = Math.min(width, 600);
  const small = targetW < width
    ? downscalePixels(data, width, height, targetW)
    : { data, width, height };

  for (const t of [160, 120, 80]) {
    const bin = binarizeAt(small.data, small.width, small.height, t);
    const result = jsQR(bin.data as any, small.width, small.height, JSQR_OPTS);
    if (result?.data) {
      console.log(`[QR] ${label}: decoded at ${small.width}×${small.height} threshold=${t}`);
      return result.data;
    }
  }
  console.log(`[QR] ${label}: failed ${width}×${height} (tried raw + 3 thresholds)`);
  return null;
}

// ---------------------------------------------------------------------------
// Scan line animation (native only — web reanimated causes ghost artifacts)
// ---------------------------------------------------------------------------

function ScanLine() {
  if (Platform.OS === 'web') return null;

  const translateY = useSharedValue(0);

  useEffect(() => {
    translateY.value = withRepeat(
      withTiming(FRAME_SIZE - 2, { duration: 2000, easing: Easing.inOut(Easing.ease) }),
      -1,
      true,
    );
  }, [translateY]);

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ translateY: translateY.value }],
  }));

  return (
    <Animated.View style={[styles.scanLine, animatedStyle]} />
  );
}

// ---------------------------------------------------------------------------
// Web camera component using getUserMedia + jsQR
// ---------------------------------------------------------------------------

function WebCamera({ onScan, scanned }: { onScan: (data: string) => void; scanned: boolean }) {
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const timerRef = useRef<ReturnType<typeof setInterval>>(0 as any);
  // Use refs so the interval callback always sees latest values without re-creating
  const scannedRef = useRef(scanned);
  const onScanRef = useRef(onScan);
  scannedRef.current = scanned;
  onScanRef.current = onScan;

  useEffect(() => {
    let mounted = true;
    const busyRef = { current: false };
    let frameCount = 0;

    navigator.mediaDevices.getUserMedia({
      video: { facingMode: 'environment', width: { ideal: 1280 }, height: { ideal: 720 } },
    })
      .then(stream => {
        if (!mounted) { stream.getTracks().forEach(t => t.stop()); return; }
        streamRef.current = stream;
        const track = stream.getVideoTracks()[0];
        const settings = track?.getSettings?.();
        console.log('[QR] camera: stream ready', settings?.width ?? '?', '×', settings?.height ?? '?');
        if (videoRef.current) {
          videoRef.current.srcObject = stream;
          videoRef.current.play();
        }
      })
      .catch((e) => { console.warn('[QR] camera: getUserMedia failed', e?.message); });

    // Stable interval — reads refs, never needs to be re-created
    timerRef.current = setInterval(async () => {
      if (scannedRef.current || busyRef.current) return;
      const video = videoRef.current;
      if (!video || video.readyState !== video.HAVE_ENOUGH_DATA) return;
      busyRef.current = true;
      frameCount++;

      try {
        const canvas = canvasRef.current;
        if (canvas) {
          const vw = video.videoWidth;
          const vh = video.videoHeight;
          // Crop to 85% of the shorter side — large enough that dense QR codes
          // (walletpair 57×57) get ~5px/module at the 600px canvas size
          const side = Math.round(Math.min(vw, vh) * 0.85);
          const sx = Math.round((vw - side) / 2);
          const sy = Math.round((vh - side) / 2);
          // Render cropped region at 600px — dense QR codes (walletpair, 57×57
          // modules) need ~5px/module minimum for reliable jsQR decoding
          const size = Math.min(side, 600);
          canvas.width = size;
          canvas.height = size;
          const ctx = canvas.getContext('2d', { willReadFrequently: true })!;
          ctx.drawImage(video, sx, sy, side, side, 0, 0, size, size);
          const imageData = ctx.getImageData(0, 0, size, size);
          // Log every 10th frame so we know scanning is alive
          if (frameCount % 10 === 1) {
            console.log(`[QR] camera: frame#${frameCount} video=${vw}×${vh} crop=${side} canvas=${size}`);
          }
          // Use full decodeQR: raw + 3 binarize thresholds
          // 400×400 = 160K pixels → 4 jsQR calls ≈ 60ms, well within 500ms budget
          const decoded = decodeQR(imageData, 'camera');
          if (decoded) {
            onScanRef.current(decoded);
          }
        }
      } finally {
        busyRef.current = false;
      }
    }, 500);

    return () => {
      mounted = false;
      clearInterval(timerRef.current);
      streamRef.current?.getTracks().forEach(t => t.stop());
    };
  }, []); // stable — no deps, runs once

  return (
    <View style={styles.cameraContainer}>
      <video
        ref={videoRef as any}
        style={{ position: 'absolute', top: 0, left: 0, width: '100%', height: '100%', objectFit: 'cover' } as any}
        playsInline
        muted
        autoPlay
      />
      <canvas ref={canvasRef as any} style={{ display: 'none' } as any} />
      <ScanOverlay />
    </View>
  );
}

// ---------------------------------------------------------------------------
// Native camera component using expo-camera
// ---------------------------------------------------------------------------

function NativeCamera({ facing, onBarCodeScanned, active }: {
  facing: 'back' | 'front';
  onBarCodeScanned: (result: { data: string }) => void;
  active: boolean;
}) {
  const { CameraView, useCameraPermissions } = require('expo-camera');
  const [permission, requestPermission] = useCameraPermissions();

  if (!permission?.granted) {
    return (
      <View style={styles.permissionContainer}>
        <Camera size={40} color={color.fg.subtle} />
        <Text style={styles.permissionText}>
          Camera access is needed to scan QR codes.
        </Text>
        <Pressable style={styles.permissionButton} onPress={requestPermission}>
          <Text style={styles.permissionButtonText}>Grant Permission</Text>
        </Pressable>
      </View>
    );
  }

  return (
    <View style={styles.cameraContainer}>
      <CameraView
        style={styles.camera}
        facing={facing}
        barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
        onBarcodeScanned={active ? onBarCodeScanned : undefined}
      />
      <ScanOverlay />
    </View>
  );
}

// ---------------------------------------------------------------------------
// Scan overlay with corners + animated line
// ---------------------------------------------------------------------------

function ScanOverlay() {
  return (
    <View style={styles.overlay}>
      <View style={styles.scanFrame}>
        <View style={[styles.corner, styles.cornerTL]} />
        <View style={[styles.corner, styles.cornerTR]} />
        <View style={[styles.corner, styles.cornerBL]} />
        <View style={[styles.corner, styles.cornerBR]} />
        <ScanLine />
      </View>
    </View>
  );
}

// ---------------------------------------------------------------------------
// Main QRScanner
// ---------------------------------------------------------------------------

export function QRScanner({ visible, onScan, onClose }: Props) {
  const [scanned, setScanned] = useState(false);
  const [facing, setFacing] = useState<'back' | 'front'>('back');
  const insets = useSafeAreaInsets();

  useEffect(() => {
    if (visible) setScanned(false);
  }, [visible]);

  function handleBarCodeScanned({ data }: { data: string }) {
    if (scanned) return;
    setScanned(true);
    hapticSuccess();
    onScan(parseAddress(data));
    setTimeout(() => setScanned(false), 2000);
  }

  async function handlePickImage() {
    if (Platform.OS === 'web') {
      // Web: use native file input + jsQR
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = 'image/*';
      input.onchange = async () => {
        try {
          const file = input.files?.[0];
          if (!file) { console.log('[QR] pick: no file selected'); return; }
          console.log(`[QR] pick: file ${file.name} (${file.type}, ${(file.size / 1024).toFixed(0)}KB)`);

          // Load image — keep blob URL alive until canvas draw completes
          // (iOS Safari may evict decoded pixels under memory pressure)
          const url = URL.createObjectURL(file);
          const img = new Image();
          await new Promise<void>((resolve, reject) => {
            img.onload = () => resolve();
            img.onerror = () => reject(new Error('Failed to load image'));
            img.src = url;
          });

          console.log(`[QR] pick: loaded ${img.naturalWidth}×${img.naturalHeight}`);

          // Let the browser do high-quality downscaling (much faster than JS).
          // 800px is enough for jsQR; decodeQR may further shrink to 600px.
          const maxDim = 800;
          const scale = Math.min(1, maxDim / Math.max(img.naturalWidth, img.naturalHeight));
          const w = Math.round(img.naturalWidth * scale);
          const h = Math.round(img.naturalHeight * scale);

          const canvas = document.createElement('canvas');
          canvas.width = w;
          canvas.height = h;
          const ctx = canvas.getContext('2d')!;
          ctx.drawImage(img, 0, 0, w, h);
          // Only revoke AFTER drawImage — iOS Safari may need the URL alive
          URL.revokeObjectURL(url);

          const imageData = ctx.getImageData(0, 0, w, h);
          console.log(`[QR] pick: canvas ${w}×${h}, pixels[0..3]=${imageData.data[0]},${imageData.data[1]},${imageData.data[2]},${imageData.data[3]}`);
          const decoded = decodeQR(imageData, 'pick');
          if (decoded) {
            hapticSuccess();
            onScan(parseAddress(decoded));
          } else {
            showAlert('No QR Found', 'Could not find a QR code in the selected image.');
          }
        } catch (e: any) {
          console.warn('[QR] pick error:', e?.message ?? e);
          showAlert('Error', 'Failed to process the image.');
        }
      };
      input.click();
    } else {
      // Native: expo-image-picker + expo-camera scanFromURLAsync (with jsQR fallback)
      try {
        const ImagePicker = await import('expo-image-picker');
        const result = await ImagePicker.launchImageLibraryAsync({
          mediaTypes: ['images'],
          quality: 1,
          base64: true,
        });
        if (result.canceled || !result.assets?.[0]) return;
        const asset = result.assets[0];

        // 1. Try expo-camera scanFromURLAsync (uses ML Kit on Android, Vision on iOS)
        try {
          const ExpoCamera = await import('expo-camera');
          const barcodes = await ExpoCamera.scanFromURLAsync(asset.uri, ['qr']);
          if (barcodes.length > 0 && barcodes[0].data) {
            hapticSuccess();
            onScan(parseAddress(barcodes[0].data));
            return;
          }
        } catch (e) {
          console.warn('[QRScanner] scanFromURLAsync failed:', e);
        }

        // 2. Fallback: decode base64 image → jsQR (works without ML Kit)
        if (asset.base64) {
          try {
            const { decodeBase64Image } = await import('@/services/image-decode');
            const imageData = decodeBase64Image(asset.base64, asset.width, asset.height);
            if (imageData) {
              const decoded = decodeQR(imageData as ImageData);
              if (decoded) {
                hapticSuccess();
                onScan(parseAddress(decoded));
                return;
              }
            }
          } catch (e) {
            console.warn('[QRScanner] jsQR fallback error:', e);
          }
        }

        showAlert('No QR Found', 'Could not find a QR code in the selected image.');
      } catch (e) {
        console.warn('[QRScanner] Native image pick error:', e);
        showAlert('Error', 'Failed to open image picker.');
      }
    }
  }

  if (!visible) return null;

  const content = (
    <View style={styles.container}>
      {/* Camera */}
      {Platform.OS === 'web' ? (
        <WebCamera onScan={(data) => handleBarCodeScanned({ data })} scanned={scanned} />
      ) : (
        <NativeCamera facing={facing} onBarCodeScanned={handleBarCodeScanned} active={!scanned} />
      )}

      {/* Header overlay — manual safe area padding */}
      <View style={[styles.headerOverlay, { paddingTop: Math.max(insets.top, 20) }]}>
        <Pressable onPress={onClose} hitSlop={8} style={styles.headerBtn}>
          <X size={22} color="#fff" strokeWidth={2.5} />
        </Pressable>
        <Text style={styles.title}>Scan QR</Text>
        <View style={styles.headerRight}>
          <Pressable onPress={handlePickImage} hitSlop={8} style={styles.headerBtn}>
            <ImagePlus size={20} color="#fff" strokeWidth={2} />
          </Pressable>
          {Platform.OS !== 'web' && (
            <Pressable
              onPress={() => setFacing(f => f === 'back' ? 'front' : 'back')}
              hitSlop={8}
              style={styles.headerBtn}
            >
              <SwitchCamera size={20} color="#fff" strokeWidth={2} />
            </Pressable>
          )}
        </View>
      </View>

      {/* Footer hint */}
      <View style={[styles.footerOverlay, { paddingBottom: Math.max(insets.bottom, 20) }]}>
        <Text style={styles.hint}>Point camera at a QR code</Text>
      </View>
    </View>
  );

  // Native: fullscreen modal with dark status bar
  if (Platform.OS !== 'web') {
    return (
      <Modal visible={visible} animationType="slide" statusBarTranslucent>
        <StatusBar barStyle="light-content" backgroundColor="transparent" translucent />
        {content}
      </Modal>
    );
  }

  // Web: use RN Modal — works on web and handles z-index/overlay correctly
  return (
    <Modal visible={visible} animationType="slide" transparent>
      {content}
    </Modal>
  );
}

const FRAME_SIZE = 240;
const CORNER_SIZE = 28;
const CORNER_WIDTH = 3;
const CORNER_RADIUS = 12;

const styles = createStyles(() => ({
  container: {
    flex: 1,
    backgroundColor: '#000',
  },

  // Header + footer float on top of camera
  headerOverlay: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: space.xl,
    paddingBottom: space.md,
    zIndex: 10,
  },
  headerBtn: {
    width: 44,
    height: 44,
    alignItems: 'center',
    justifyContent: 'center',
  },
  headerRight: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  title: {
    fontSize: text.lg,
    ...inter.bold,
    color: '#fff',
  },
  footerOverlay: {
    position: 'absolute',
    bottom: 0,
    left: 0,
    right: 0,
    alignItems: 'center',
    zIndex: 10,
  },
  hint: {
    fontSize: text.sm,
    ...inter.medium,
    color: 'rgba(255,255,255,0.7)',
  },

  // Permission screen
  permissionContainer: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: space['5xl'],
    gap: space['2xl'],
  },
  permissionText: {
    fontSize: text.lg,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: 22,
  },
  permissionButton: {
    backgroundColor: color.accent.base,
    paddingHorizontal: space['3xl'],
    paddingVertical: space.xl,
    borderRadius: radius.xl,
  },
  permissionButtonText: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.inverse,
  },

  // Camera
  cameraContainer: {
    flex: 1,
    position: 'relative',
  },
  camera: {
    flex: 1,
  },

  // Overlay — centers the scan frame
  overlay: {
    ...StyleSheet.absoluteFillObject,
    justifyContent: 'center',
    alignItems: 'center',
  },

  // Scan frame — fixed square with corner brackets
  scanFrame: {
    width: FRAME_SIZE,
    height: FRAME_SIZE,
    position: 'relative',
  },
  corner: {
    position: 'absolute',
    width: CORNER_SIZE,
    height: CORNER_SIZE,
    borderColor: '#fff',
  },
  cornerTL: {
    top: 0, left: 0,
    borderTopWidth: CORNER_WIDTH, borderLeftWidth: CORNER_WIDTH,
    borderTopLeftRadius: CORNER_RADIUS,
  },
  cornerTR: {
    top: 0, right: 0,
    borderTopWidth: CORNER_WIDTH, borderRightWidth: CORNER_WIDTH,
    borderTopRightRadius: CORNER_RADIUS,
  },
  cornerBL: {
    bottom: 0, left: 0,
    borderBottomWidth: CORNER_WIDTH, borderLeftWidth: CORNER_WIDTH,
    borderBottomLeftRadius: CORNER_RADIUS,
  },
  cornerBR: {
    bottom: 0, right: 0,
    borderBottomWidth: CORNER_WIDTH, borderRightWidth: CORNER_WIDTH,
    borderBottomRightRadius: CORNER_RADIUS,
  },

  // Animated scan line
  scanLine: {
    position: 'absolute',
    left: 8,
    right: 8,
    height: 2,
    backgroundColor: 'rgba(255,255,255,0.6)',
    borderRadius: 1,
  },
}));
