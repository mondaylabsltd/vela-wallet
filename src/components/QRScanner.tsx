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

// ============================================================================
// Web QR decode engine (zbar WASM primary, jsQR fallback)
// ============================================================================

const JSQR_OPTS = { inversionAttempts: 'attemptBoth' as const };

// -- Pixel transforms --------------------------------------------------------

const INVERT = (d: Uint8ClampedArray) => {
  for (let i = 0; i < d.length; i += 4) { d[i] = 255 - d[i]; d[i+1] = 255 - d[i+1]; d[i+2] = 255 - d[i+2]; }
};
const BINARIZE = (t: number) => (d: Uint8ClampedArray) => {
  for (let i = 0; i < d.length; i += 4) {
    const lum = (d[i] * 77 + d[i+1] * 150 + d[i+2] * 29) >> 8;
    const v = lum <= t ? 0 : 255; d[i] = v; d[i+1] = v; d[i+2] = v;
  }
};
const BIN_INVERT = (t: number) => (d: Uint8ClampedArray) => {
  for (let i = 0; i < d.length; i += 4) {
    const lum = (d[i] * 77 + d[i+1] * 150 + d[i+2] * 29) >> 8;
    const v = lum <= t ? 255 : 0; d[i] = v; d[i+1] = v; d[i+2] = v;
  }
};

// -- Reusable canvases (avoid GC pressure) -----------------------------------

let _canvasA: HTMLCanvasElement | null = null;
let _canvasB: HTMLCanvasElement | null = null;
function getCanvas(slot: 'A' | 'B'): HTMLCanvasElement {
  if (slot === 'A') return (_canvasA ??= document.createElement('canvas'));
  return (_canvasB ??= document.createElement('canvas'));
}

function drawScaled(src: HTMLCanvasElement, targetW: number, slot: 'A' | 'B' = 'A'): HTMLCanvasElement {
  const c = getCanvas(slot);
  c.width = targetW;
  c.height = Math.round(targetW * src.height / src.width);
  c.getContext('2d')!.drawImage(src, 0, 0, c.width, c.height);
  return c;
}

// -- zbar WASM ---------------------------------------------------------------

let zbarScan: ((d: ImageData) => Promise<any[]>) | null = null;
let zbarLoading: Promise<void> | null = null;

function loadZbar(): Promise<void> {
  if (zbarScan) return Promise.resolve();
  if (zbarLoading) return zbarLoading;
  zbarLoading = (async () => {
    // Metro can't import @undecaf/zbar-wasm (uses import.meta).
    // Load from CDN; fall back to local copy in public/.
    const urls = [
      'https://cdn.jsdelivr.net/npm/@undecaf/zbar-wasm@0.11.0/dist/main.mjs',
      '/zbar-wasm.mjs',
    ];
    const imp = new Function('url', 'return import(url)');
    for (const url of urls) {
      try {
        const m = await imp(url);
        zbarScan = m.scanImageData;
        console.log('[QR] zbar loaded');
        return;
      } catch {}
    }
    console.warn('[QR] zbar failed to load from all sources');
  })();
  return zbarLoading;
}

async function tryZbar(canvas: HTMLCanvasElement): Promise<string | null> {
  if (!zbarScan) return null;
  const d = canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height);
  try { const s = await zbarScan(d); if (s.length > 0) return s[0].decode(); } catch {}
  INVERT(d.data);
  try { const s = await zbarScan(d); if (s.length > 0) return s[0].decode(); } catch {}
  return null;
}

// -- jsQR with transform -----------------------------------------------------

function tryJsQR(canvas: HTMLCanvasElement, targetW: number, transform: (d: Uint8ClampedArray) => void): string | null {
  const c = drawScaled(canvas, Math.min(canvas.width, targetW), 'B');
  const img = c.getContext('2d')!.getImageData(0, 0, c.width, c.height);
  transform(img.data);
  return jsQR(img.data as any, c.width, c.height, JSQR_OPTS)?.data ?? null;
}

// -- Public decode functions -------------------------------------------------

/** Camera: fast, single zbar@1000 call per frame. */
async function decodeCameraFrame(canvas: HTMLCanvasElement): Promise<string | null> {
  return tryZbar(drawScaled(canvas, 1000));
}

/** Upload: thorough, tries zbar then jsQR at multiple sizes. */
async function decodeUploadedImage(canvas: HTMLCanvasElement): Promise<string | null> {
  const w = canvas.width;
  const sizes = [1200, 1000, 800, 600, 400].filter(s => s < w);

  for (const s of sizes) {
    const r = await tryZbar(drawScaled(canvas, s));
    if (r) { console.log(`[QR] pick: zbar@${s}`); return r; }
  }

  for (const s of [w, ...sizes]) {
    for (const [n, fn] of [['binInv160', BIN_INVERT(160)], ['invert', INVERT], ['bin160', BINARIZE(160)]] as [string, (d: Uint8ClampedArray) => void][]) {
      const r = tryJsQR(canvas, s, fn);
      if (r) { console.log(`[QR] pick: ${n}@${s}`); return r; }
    }
  }
  return null;
}

// ============================================================================
// Components
// ============================================================================

interface Props {
  visible: boolean;
  onScan: (data: string) => void;
  onClose: () => void;
}

function parseAddress(data: string): string {
  let a = data.trim();
  if (a.startsWith('ethereum:')) a = a.replace('ethereum:', '').split('?')[0].split('@')[0];
  return a;
}

// -- Scan line animation (native only) ---------------------------------------

function ScanLine() {
  if (Platform.OS === 'web') return null;
  const translateY = useSharedValue(0);
  useEffect(() => {
    translateY.value = withRepeat(
      withTiming(FRAME_SIZE - 2, { duration: 2000, easing: Easing.inOut(Easing.ease) }),
      -1, true,
    );
  }, [translateY]);
  const style = useAnimatedStyle(() => ({ transform: [{ translateY: translateY.value }] }));
  return <Animated.View style={[styles.scanLine, style]} />;
}

// -- Web camera --------------------------------------------------------------

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

    loadZbar();

    navigator.mediaDevices.getUserMedia({
      video: { facingMode: 'environment', width: { ideal: 1280 }, height: { ideal: 720 } },
    }).then(stream => {
      if (!mounted) { stream.getTracks().forEach(t => t.stop()); return; }
      streamRef.current = stream;
      if (videoRef.current) { videoRef.current.srcObject = stream; videoRef.current.play(); }
    }).catch(() => {});

    let timer: ReturnType<typeof setTimeout>;
    async function scan() {
      if (!mounted || scannedRef.current) { timer = setTimeout(scan, 500); return; }
      const v = videoRef.current;
      if (!v || v.readyState !== v.HAVE_ENOUGH_DATA) { timer = setTimeout(scan, 300); return; }

      const c = canvasRef.current;
      let decoded: string | null = null;
      if (c) {
        c.width = v.videoWidth; c.height = v.videoHeight;
        c.getContext('2d', { willReadFrequently: true })!.drawImage(v, 0, 0);
        decoded = await decodeCameraFrame(c);
      }

      if (decoded && !scannedRef.current) onScanRef.current(decoded);
      if (mounted) timer = setTimeout(scan, decoded ? 2000 : 800);
    }
    timer = setTimeout(scan, 1000);

    return () => { mounted = false; clearTimeout(timer); streamRef.current?.getTracks().forEach(t => t.stop()); };
  }, []);

  return (
    <View style={styles.cameraContainer}>
      <video
        ref={videoRef as any}
        style={{ position: 'absolute', top: 0, left: 0, width: '100%', height: '100%', objectFit: 'cover', pointerEvents: 'none' } as any}
        playsInline muted autoPlay
      />
      <canvas ref={canvasRef as any} style={{ display: 'none' } as any} />
      <ScanOverlay />
    </View>
  );
}

// -- Native camera -----------------------------------------------------------

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
        <Text style={styles.permissionText}>Camera access is needed to scan QR codes.</Text>
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

// -- Scan overlay ------------------------------------------------------------

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

// -- Main QRScanner ----------------------------------------------------------

export function QRScanner({ visible, onScan, onClose }: Props) {
  const [scanned, setScanned] = useState(false);
  const [facing, setFacing] = useState<'back' | 'front'>('back');
  const insets = useSafeAreaInsets();

  useEffect(() => { if (visible) setScanned(false); }, [visible]);

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
      input.style.display = 'none';
      document.body.appendChild(input);
      input.onchange = async () => {
        document.body.removeChild(input);
        try {
          const file = input.files?.[0];
          if (!file) return;

          const url = URL.createObjectURL(file);
          const img = new Image();
          await new Promise<void>((res, rej) => { img.onload = () => res(); img.onerror = rej; img.src = url; });
          const canvas = document.createElement('canvas');
          canvas.width = img.naturalWidth;
          canvas.height = img.naturalHeight;
          canvas.getContext('2d')!.drawImage(img, 0, 0);
          URL.revokeObjectURL(url);

          await loadZbar();
          const decoded = await decodeUploadedImage(canvas);
          if (decoded) { hapticSuccess(); onScan(parseAddress(decoded)); }
          else showAlert('No QR Found', 'Could not find a QR code in the selected image.');
        } catch (e: any) {
          console.warn('[QR] pick error:', e?.message);
          showAlert('Error', 'Failed to process the image.');
        }
      };
      input.click();
    } else {
      try {
        const ImagePicker = await import('expo-image-picker');
        const result = await ImagePicker.launchImageLibraryAsync({ mediaTypes: ['images'], quality: 1, base64: true });
        if (result.canceled || !result.assets?.[0]) return;
        const asset = result.assets[0];

        try {
          const ExpoCamera = await import('expo-camera');
          const barcodes = await ExpoCamera.scanFromURLAsync(asset.uri, ['qr']);
          if (barcodes.length > 0 && barcodes[0].data) {
            hapticSuccess(); onScan(parseAddress(barcodes[0].data)); return;
          }
        } catch {}

        if (asset.base64) {
          try {
            const { decodeBase64Image } = await import('@/services/image-decode');
            const imageData = decodeBase64Image(asset.base64, asset.width, asset.height);
            if (imageData) {
              const code = jsQR(imageData.data as any, imageData.width, imageData.height, JSQR_OPTS);
              if (code?.data) { hapticSuccess(); onScan(parseAddress(code.data)); return; }
            }
          } catch {}
        }
        showAlert('No QR Found', 'Could not find a QR code in the selected image.');
      } catch { showAlert('Error', 'Failed to open image picker.'); }
    }
  }

  if (!visible) return null;

  const content = (
    <View style={styles.container}>
      {Platform.OS === 'web' ? (
        <WebCamera onScan={(data) => handleBarCodeScanned({ data })} scanned={scanned} />
      ) : (
        <NativeCamera facing={facing} onBarCodeScanned={handleBarCodeScanned} active={!scanned} />
      )}

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
            <Pressable onPress={() => setFacing(f => f === 'back' ? 'front' : 'back')} hitSlop={8} style={styles.headerBtn}>
              <SwitchCamera size={20} color="#fff" strokeWidth={2} />
            </Pressable>
          )}
        </View>
      </View>

      <View style={[styles.footerOverlay, { paddingBottom: Math.max(insets.bottom, 20) }]}>
        <Text style={styles.hint}>Point camera at a QR code</Text>
      </View>
    </View>
  );

  if (Platform.OS !== 'web') {
    return (
      <Modal visible={visible} animationType="slide" statusBarTranslucent>
        <StatusBar barStyle="light-content" backgroundColor="transparent" translucent />
        {content}
      </Modal>
    );
  }

  return <Modal visible={visible} animationType="slide" transparent>{content}</Modal>;
}

// ============================================================================
// Styles
// ============================================================================

const FRAME_SIZE = 240;
const CORNER_SIZE = 28;
const CORNER_WIDTH = 3;
const CORNER_RADIUS = 12;

const styles = createStyles(() => ({
  container: { flex: 1, backgroundColor: '#000' },
  headerOverlay: {
    position: 'absolute', top: 0, left: 0, right: 0,
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    paddingHorizontal: space.xl, paddingBottom: space.md, zIndex: 10,
  },
  headerBtn: { width: 44, height: 44, alignItems: 'center', justifyContent: 'center' },
  headerRight: { flexDirection: 'row', alignItems: 'center' },
  title: { fontSize: text.lg, ...inter.bold, color: '#fff' },
  footerOverlay: { position: 'absolute', bottom: 0, left: 0, right: 0, alignItems: 'center', zIndex: 10 },
  hint: { fontSize: text.sm, ...inter.medium, color: 'rgba(255,255,255,0.7)' },
  permissionContainer: { flex: 1, alignItems: 'center', justifyContent: 'center', paddingHorizontal: space['5xl'], gap: space['2xl'] },
  permissionText: { fontSize: text.lg, ...inter.regular, color: color.fg.subtle, textAlign: 'center', lineHeight: 22 },
  permissionButton: { backgroundColor: color.accent.base, paddingHorizontal: space['3xl'], paddingVertical: space.xl, borderRadius: radius.xl },
  permissionButtonText: { fontSize: text.lg, ...inter.semibold, color: color.fg.inverse },
  cameraContainer: { flex: 1, position: 'relative' },
  camera: { flex: 1 },
  overlay: { ...StyleSheet.absoluteFillObject, justifyContent: 'center', alignItems: 'center' },
  scanFrame: { width: FRAME_SIZE, height: FRAME_SIZE, position: 'relative' },
  corner: { position: 'absolute', width: CORNER_SIZE, height: CORNER_SIZE, borderColor: '#fff' },
  cornerTL: { top: 0, left: 0, borderTopWidth: CORNER_WIDTH, borderLeftWidth: CORNER_WIDTH, borderTopLeftRadius: CORNER_RADIUS },
  cornerTR: { top: 0, right: 0, borderTopWidth: CORNER_WIDTH, borderRightWidth: CORNER_WIDTH, borderTopRightRadius: CORNER_RADIUS },
  cornerBL: { bottom: 0, left: 0, borderBottomWidth: CORNER_WIDTH, borderLeftWidth: CORNER_WIDTH, borderBottomLeftRadius: CORNER_RADIUS },
  cornerBR: { bottom: 0, right: 0, borderBottomWidth: CORNER_WIDTH, borderRightWidth: CORNER_WIDTH, borderBottomRightRadius: CORNER_RADIUS },
  scanLine: { position: 'absolute', left: 8, right: 8, height: 2, backgroundColor: 'rgba(255,255,255,0.6)', borderRadius: 1 },
}));
