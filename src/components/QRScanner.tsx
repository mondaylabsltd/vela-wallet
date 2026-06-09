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

const JSQR_OPTS = { inversionAttempts: 'attemptBoth' as const };

/**
 * Try jsQR with a pixel transform (invert / binarize) at a target size.
 * Browser canvas drawImage handles the downscale (high quality).
 */
function tryJsQR(
  canvas: HTMLCanvasElement,
  targetW: number,
  transform: (d: Uint8ClampedArray) => void,
): string | null {
  const tw = Math.min(canvas.width, targetW);
  const th = Math.round(tw * canvas.height / canvas.width);
  const small = document.createElement('canvas');
  small.width = tw;
  small.height = th;
  small.getContext('2d')!.drawImage(canvas, 0, 0, tw, th);
  const imageData = small.getContext('2d')!.getImageData(0, 0, tw, th);
  transform(imageData.data);
  return jsQR(imageData.data as any, tw, th, JSQR_OPTS)?.data ?? null;
}

const INVERT = (d: Uint8ClampedArray) => {
  for (let i = 0; i < d.length; i += 4) { d[i] = 255 - d[i]; d[i+1] = 255 - d[i+1]; d[i+2] = 255 - d[i+2]; }
};
const BINARIZE = (t: number) => (d: Uint8ClampedArray) => {
  for (let i = 0; i < d.length; i += 4) {
    const lum = (d[i] * 77 + d[i+1] * 150 + d[i+2] * 29) >> 8;
    const v = lum <= t ? 0 : 255;
    d[i] = v; d[i+1] = v; d[i+2] = v;
  }
};

/**
 * Decode QR from a canvas using multiple strategies.
 * Proven on iPhone Safari with real WalletPair images:
 *   - JPEG photo (772×1154): invert@400
 *   - PNG screenshot (998×1298): bin160@600
 */
function decodeFromCanvas(canvas: HTMLCanvasElement, label = 'image'): string | null {
  const w = canvas.width;
  // Build target widths: original, then progressively smaller
  // Full image first (QR could be anywhere), then smaller for speed
  const targets = new Set<number>();
  targets.add(w); // original size
  for (const s of [1200, 1000, 800, 600, 400]) {
    if (s < w) targets.add(s);
  }
  const sizes = [...targets].sort((a, b) => b - a); // largest first

  const strategies: [string, () => string | null][] = [];
  for (const s of sizes) {
    strategies.push(
      [`invert@${s}`, () => tryJsQR(canvas, s, INVERT)],
      [`bin160@${s}`, () => tryJsQR(canvas, s, BINARIZE(160))],
    );
  }

  for (const [name, fn] of strategies) {
    const r = fn();
    if (r) {
      console.log(`[QR] ${label}: ${name} OK`);
      return r;
    }
  }
  console.log(`[QR] ${label}: all strategies failed ${canvas.width}×${canvas.height}`);
  return null;
}

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
// Web camera component using qr-scanner (WASM-based, works on all browsers)
// ---------------------------------------------------------------------------

function WebCamera({ onScan, scanned }: { onScan: (data: string) => void; scanned: boolean }) {
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const scannedRef = useRef(scanned);
  const onScanRef = useRef(onScan);
  scannedRef.current = scanned;
  onScanRef.current = onScan;

  useEffect(() => {
    let mounted = true;
    let frameCount = 0;
    let QrScannerLib: any = null;

    // Load qr-scanner WASM decoder
    import('qr-scanner').then(m => { QrScannerLib = m.default; console.log('[QR] WASM decoder loaded'); })
      .catch(e => console.warn('[QR] WASM decoder failed to load:', e?.message));

    // Start camera
    navigator.mediaDevices.getUserMedia({
      video: { facingMode: 'environment', width: { ideal: 1920 }, height: { ideal: 1080 } },
    })
      .then(stream => {
        if (!mounted) { stream.getTracks().forEach(t => t.stop()); return; }
        streamRef.current = stream;
        const s = stream.getVideoTracks()[0]?.getSettings?.();
        console.log('[QR] camera: stream', s?.width ?? '?', '×', s?.height ?? '?');
        if (videoRef.current) {
          videoRef.current.srcObject = stream;
          videoRef.current.play();
        }
      })
      .catch(e => console.warn('[QR] camera failed:', e?.message));

    // Grab-and-analyze loop
    let timer: ReturnType<typeof setTimeout>;
    async function scanFrame() {
      if (!mounted || scannedRef.current) { timer = setTimeout(scanFrame, 500); return; }
      const video = videoRef.current;
      if (!video || video.readyState !== video.HAVE_ENOUGH_DATA) { timer = setTimeout(scanFrame, 300); return; }
      frameCount++;

      let decoded: string | null = null;

      // Strategy 1: qr-scanner WASM on the video element directly
      if (QrScannerLib && !decoded) {
        try {
          const result = await QrScannerLib.scanImage(video, { returnDetailedScanResult: true });
          decoded = result?.data ?? null;
          if (decoded) console.log('[QR] camera: WASM decoded', decoded.substring(0, 50) + '...');
        } catch {}
      }

      // Strategy 2: jsQR inverted on canvas (handles dark-themed QR codes)
      if (!decoded) {
        const canvas = canvasRef.current;
        if (canvas) {
          const vw = video.videoWidth, vh = video.videoHeight;
          canvas.width = vw;
          canvas.height = vh;
          canvas.getContext('2d', { willReadFrequently: true })!.drawImage(video, 0, 0);
          decoded = decodeFromCanvas(canvas, 'camera');
        }
      }

      if (frameCount % 5 === 1) {
        console.log(`[QR] camera: frame#${frameCount} ${decoded ? 'OK' : 'no QR'}`);
      }

      if (decoded && !scannedRef.current) {
        onScanRef.current(decoded);
      }

      if (mounted) timer = setTimeout(scanFrame, decoded ? 2000 : 500);
    }
    timer = setTimeout(scanFrame, 800);

    return () => {
      mounted = false;
      clearTimeout(timer);
      streamRef.current?.getTracks().forEach(t => t.stop());
    };
  }, []);

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
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = 'image/*';
      input.onchange = async () => {
        try {
          const file = input.files?.[0];
          if (!file) return;
          console.log(`[QR] pick: file ${file.name} (${file.type}, ${(file.size / 1024).toFixed(0)}KB)`);

          let decoded: string | null = null;

          // Strategy 1: qr-scanner WASM (handles most QR codes)
          try {
            const QrScanner = (await import('qr-scanner')).default;
            const result = await QrScanner.scanImage(file, { returnDetailedScanResult: true });
            decoded = result?.data ?? null;
            if (decoded) console.log('[QR] pick: WASM decoded', decoded.substring(0, 50) + '...');
          } catch {
            console.log('[QR] pick: WASM failed, trying inverted jsQR...');
          }

          // Strategy 2: jsQR multi-strategy (invert, binarize at various sizes)
          if (!decoded) {
            const url = URL.createObjectURL(file);
            const img = new Image();
            await new Promise<void>((res, rej) => { img.onload = () => res(); img.onerror = rej; img.src = url; });
            const canvas = document.createElement('canvas');
            canvas.width = img.naturalWidth;
            canvas.height = img.naturalHeight;
            canvas.getContext('2d')!.drawImage(img, 0, 0);
            URL.revokeObjectURL(url);
            decoded = decodeFromCanvas(canvas, 'pick');
          }

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
      // Native: expo-image-picker + expo-camera scanFromURLAsync
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

        // 2. Fallback: decode base64 image → jsQR
        if (asset.base64) {
          try {
            const jsQR = (await import('jsqr')).default;
            const { decodeBase64Image } = await import('@/services/image-decode');
            const imageData = decodeBase64Image(asset.base64, asset.width, asset.height);
            if (imageData) {
              const code = jsQR(imageData.data as any, imageData.width, imageData.height, {
                inversionAttempts: 'attemptBoth',
              });
              if (code?.data) {
                hapticSuccess();
                onScan(parseAddress(code.data));
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
