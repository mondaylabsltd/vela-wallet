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

/**
 * Try jsQR on raw image data, then retry with binarization at progressive
 * thresholds to handle dark-background screenshots (e.g. WalletPair dark theme).
 *
 * Dark backgrounds (lum ~18–30) sit very close to black QR modules (lum 0).
 * A low threshold (≤15) separates them; higher thresholds merge them and jsQR
 * loses the quiet zone boundary.
 */
function decodeQR(imageData: ImageData): string | null {
  const opts = { inversionAttempts: 'attemptBoth' as const };
  const { data, width, height } = imageData;
  // 1. Try raw image first (fast path for clean / light-background images)
  const direct = jsQR(data as any, width, height, opts);
  if (direct?.data) return direct.data;
  // 2. Retry with binarization at low thresholds (dark-bg fix)
  for (const t of [10, 40, 80]) {
    const bin = binarizeAt(data, width, height, t);
    const result = jsQR(bin.data as any, width, height, opts);
    if (result?.data) return result.data;
  }
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
// Web camera component using getUserMedia + BarcodeDetector (jsQR fallback)
// ---------------------------------------------------------------------------

/** Use native BarcodeDetector when available (Chrome Android, Safari 16.4+). */
const nativeDetector: any =
  typeof globalThis !== 'undefined' && 'BarcodeDetector' in globalThis
    ? new (globalThis as any).BarcodeDetector({ formats: ['qr_code'] })
    : null;

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

    navigator.mediaDevices.getUserMedia({
      video: { facingMode: 'environment', width: { ideal: 1920 }, height: { ideal: 1080 } },
    })
      .then(stream => {
        if (!mounted) { stream.getTracks().forEach(t => t.stop()); return; }
        streamRef.current = stream;
        if (videoRef.current) {
          videoRef.current.srcObject = stream;
          videoRef.current.play();
        }
      })
      .catch(() => {});

    // Stable interval — reads refs, never needs to be re-created
    timerRef.current = setInterval(async () => {
      if (scannedRef.current || busyRef.current) return;
      const video = videoRef.current;
      if (!video || video.readyState !== video.HAVE_ENOUGH_DATA) return;
      busyRef.current = true;

      try {
        // 1. Always try jsQR first (synchronous, never blocks)
        const canvas = canvasRef.current;
        if (canvas) {
          const ctx = canvas.getContext('2d')!;
          const w = Math.min(video.videoWidth, 1280);
          const h = Math.round(w * (video.videoHeight / video.videoWidth));
          canvas.width = w;
          canvas.height = h;
          ctx.drawImage(video, 0, 0, w, h);
          const imageData = ctx.getImageData(0, 0, w, h);
          const decoded = decodeQR(imageData);
          if (decoded) {
            onScanRef.current(decoded);
            return;
          }
        }

        // 2. Fallback: BarcodeDetector with timeout (async, may hang on dense codes)
        if (nativeDetector) {
          try {
            const barcodes: any[] = await Promise.race([
              nativeDetector.detect(video),
              new Promise<any[]>(r => setTimeout(() => r([]), 500)),
            ]);
            if (barcodes.length > 0 && barcodes[0].rawValue) {
              onScanRef.current(barcodes[0].rawValue);
            }
          } catch {}
        }
      } finally {
        busyRef.current = false;
      }
    }, 350);

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
          if (!file) return;

          // Load image via HTMLImageElement (universally supported)
          const url = URL.createObjectURL(file);
          const img = new Image();
          await new Promise<void>((resolve, reject) => {
            img.onload = () => resolve();
            img.onerror = () => reject(new Error('Failed to load image'));
            img.src = url;
          });

          const maxDim = 2048;
          const scale = Math.min(1, maxDim / Math.max(img.naturalWidth, img.naturalHeight));
          const w = Math.round(img.naturalWidth * scale);
          const h = Math.round(img.naturalHeight * scale);

          const canvas = document.createElement('canvas');
          canvas.width = w;
          canvas.height = h;
          const ctx = canvas.getContext('2d')!;
          ctx.drawImage(img, 0, 0, w, h);
          URL.revokeObjectURL(url);

          const imageData = ctx.getImageData(0, 0, w, h);
          const decoded = decodeQR(imageData);
          if (decoded) {
            hapticSuccess();
            onScan(parseAddress(decoded));
          } else {
            showAlert('No QR Found', 'Could not find a QR code in the selected image.');
          }
        } catch (e) {
          console.warn('[QRScanner] Web image scan error:', e);
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
