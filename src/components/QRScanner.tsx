import React, { useState, useRef, useEffect, useCallback } from 'react';
import { Platform, View, Text, StyleSheet, Pressable, Modal, StatusBar, Linking } from 'react-native';
import { useTranslation } from 'react-i18next';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { GestureDetector, Gesture, GestureHandlerRootView } from 'react-native-gesture-handler';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withRepeat,
  withSpring,
  withTiming,
  runOnJS,
  Easing,
} from 'react-native-reanimated';
import { showAlert, hapticSuccess, hapticLight } from '@/services/platform';
import jsQR from 'jsqr';
import { color, text, inter, space, radius, createStyles } from '@/constants/theme';
import { X, SwitchCamera, Camera, ImagePlus, Flashlight, ZoomIn } from 'lucide-react-native';

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
  c.getContext('2d', { willReadFrequently: true })!.drawImage(src, 0, 0, c.width, c.height);
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
  const d = canvas.getContext('2d', { willReadFrequently: true })!.getImageData(0, 0, canvas.width, canvas.height);
  try { const s = await zbarScan(d); if (s.length > 0) return s[0].decode(); } catch {}
  INVERT(d.data);
  try { const s = await zbarScan(d); if (s.length > 0) return s[0].decode(); } catch {}
  return null;
}

// -- jsQR with transform -----------------------------------------------------

function tryJsQR(canvas: HTMLCanvasElement, targetW: number, transform: (d: Uint8ClampedArray) => void): string | null {
  const c = drawScaled(canvas, Math.min(canvas.width, targetW), 'B');
  const img = c.getContext('2d', { willReadFrequently: true })!.getImageData(0, 0, c.width, c.height);
  transform(img.data);
  return jsQR(img.data as any, c.width, c.height, JSQR_OPTS)?.data ?? null;
}

// -- Public decode functions -------------------------------------------------

/**
 * Camera: fast zbar call per frame. `cropFactor` (0-1] takes a centered slice of
 * the frame and upscales it to ~1000px before decoding — this is our digital
 * zoom / far-reach path for browsers with no hardware zoom (e.g. iOS Safari).
 * cropFactor = 1 decodes the whole frame.
 */
async function decodeCameraFrame(canvas: HTMLCanvasElement, cropFactor = 1): Promise<string | null> {
  if (cropFactor >= 0.999) return tryZbar(drawScaled(canvas, 1000));
  const cw = Math.max(1, Math.round(canvas.width * cropFactor));
  const ch = Math.max(1, Math.round(canvas.height * cropFactor));
  const cx = (canvas.width - cw) >> 1;
  const cy = (canvas.height - ch) >> 1;
  const out = getCanvas('A');
  out.width = 1000;
  out.height = Math.max(1, Math.round(1000 * ch / cw));
  out.getContext('2d', { willReadFrequently: true })!.drawImage(canvas, cx, cy, cw, ch, 0, 0, out.width, out.height);
  return tryZbar(out);
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
// Zoom model
// ============================================================================
//
// `zoom` throughout this component is a device-agnostic 0..1 *intent* (0 = none,
// 1 = each device's best USABLE zoom). We deliberately stop short of the device's
// absolute maximum, which on phone cameras is deep digital zoom that's too blurry
// to be useful. The stopping point is device-relative, so a more capable camera
// reaches further; none reach the mushy absolute max.
//
//  - Native: expo-camera's `zoom` prop is 0..1 = a fraction of the device's max
//    zoom (that max is huge & unreadable from JS: iOS wide lens ~16-130x digital;
//    Android 4-100x). iOS applies it exponentially (factor = deviceMax^z), Android
//    linearly (factor = z*deviceMax). `nativeCap` is the best-usable fraction.
//  - Web: getCapabilities() exposes the real hardware zoom range, so we reach a
//    fraction of that range; without hardware zoom (e.g. iOS Safari) we digitally
//    crop up to a quality ceiling (deeper crops are unreadable).
const ZOOM = {
  nativeCap: Platform.OS === 'ios' ? 0.5 : 0.55, // intent 1 -> this fraction of device max
  webHwFraction: 0.6,   // web hardware zoom: fraction of the camera's own zoom range
  webDigitalMax: 4,     // web digital-crop zoom: quality ceiling (deeper is mush)
  autoIntentCap: 0.45,  // how far auto-hunt sweeps (kept gentle)
};

// Web: map a 0..1 intent to a concrete zoom value. With hardware zoom this returns
// a value inside the camera's real [min, max] range (reaching a best-usable
// fraction of it); without it, a digital magnification factor (1..webDigitalMax).
function webZoomFactor(intent: number, hw: { min: number; max: number } | null): number {
  const t = Math.max(0, Math.min(1, intent));
  if (hw) {
    const top = hw.min + (hw.max - hw.min) * ZOOM.webHwFraction;
    return hw.min + t * (top - hw.min);
  }
  return 1 + t * (ZOOM.webDigitalMax - 1);
}

// ============================================================================
// Components
// ============================================================================

interface Props {
  visible: boolean;
  /** The raw decoded QR string (trimmed). Callers parse it (address, EIP-681, walletpair, …). */
  onScan: (data: string) => void;
  onClose: () => void;
}

// -- Scan line animation (native only) ---------------------------------------

function ScanLine() {
  // Platform.OS is constant at runtime, but branching before the hooks still
  // violates rules-of-hooks — keep the hooks in a component that only ever
  // renders on native.
  return Platform.OS === 'web' ? null : <NativeScanLine />;
}

function NativeScanLine() {
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

function WebCamera({ onScan, scanned, torch, zoom, onCapabilities }: {
  onScan: (data: string) => void;
  scanned: boolean;
  torch: boolean;
  /** Normalized 0-1 zoom, shared with the native path. */
  zoom: number;
  /** Reports which of torch/hardware-zoom the browser+camera actually support. */
  onCapabilities: (caps: { torch: boolean; hwZoom: boolean }) => void;
}) {
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const scannedRef = useRef(scanned);
  const onScanRef = useRef(onScan);
  const zoomRef = useRef(zoom);
  const torchRef = useRef(torch);
  const torchSupportedRef = useRef(false);
  const hwZoomRef = useRef<{ min: number; max: number } | null>(null);
  scannedRef.current = scanned;
  onScanRef.current = onScan;
  zoomRef.current = zoom;
  torchRef.current = torch;

  // Push torch + zoom to the live track in a SINGLE applyConstraints call.
  // applyConstraints REPLACES the track's constraint set (it doesn't merge), so
  // torch and zoom must be sent together or the last writer wipes the other.
  const applyTrackConstraints = useCallback(() => {
    const track = streamRef.current?.getVideoTracks?.()[0];
    const hw = hwZoomRef.current;
    // Torch and zoom go as SEPARATE advanced ConstraintSets in ONE call: one call
    // (so neither clobbers the other) but each set is satisfied independently (so
    // an unsatisfiable zoom can't silently drop the torch).
    const sets: Record<string, unknown>[] = [];
    if (torchSupportedRef.current) sets.push({ torch: torchRef.current });
    if (hw) {
      const val = Math.max(hw.min, Math.min(hw.max, webZoomFactor(zoomRef.current, hw)));
      sets.push({ zoom: val });
    }
    if (track && sets.length) {
      try { track.applyConstraints({ advanced: sets as any }); } catch {}
    }
    // Digital zoom (no hardware zoom, e.g. iOS Safari): scale the <video> so the
    // preview matches the center crop the decoder reads.
    const v = videoRef.current;
    if (v) {
      if (hw) { v.style.transform = ''; }
      else {
        v.style.transform = `scale(${webZoomFactor(zoomRef.current, null).toFixed(3)})`;
        (v.style as any).transformOrigin = 'center center';
      }
    }
  }, []);

  useEffect(() => {
    let mounted = true;

    loadZbar();

    navigator.mediaDevices.getUserMedia({
      video: { facingMode: 'environment', width: { ideal: 1280 }, height: { ideal: 720 } },
    }).then(stream => {
      if (!mounted) { stream.getTracks().forEach(t => t.stop()); return; }
      streamRef.current = stream;
      if (videoRef.current) { videoRef.current.srcObject = stream; videoRef.current.play(); }
      // Probe hardware capabilities. Chrome/Android exposes torch + zoom; iOS
      // Safari exposes neither, so we fall back to digital zoom and hide torch.
      try {
        const track = stream.getVideoTracks()[0];
        const caps: any = track?.getCapabilities ? track.getCapabilities() : {};
        const zc = caps.zoom;
        hwZoomRef.current = zc && typeof zc === 'object' && 'max' in zc
          ? { min: typeof zc.min === 'number' ? zc.min : 1, max: zc.max }
          : null;
        torchSupportedRef.current = !!caps.torch;
        onCapabilities({ torch: !!caps.torch, hwZoom: !!hwZoomRef.current });
      } catch { onCapabilities({ torch: false, hwZoom: false }); }
      // Apply whatever torch/zoom the user (or auto-hunt) has already set while
      // the camera was starting up — otherwise an early state is silently lost.
      applyTrackConstraints();
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
        // Hardware zoom already magnified the frame; digital zoom crops to match
        // what's shown. cropFactor 1 = full frame.
        const hw = hwZoomRef.current;
        const cropFactor = hw ? 1 : 1 / webZoomFactor(zoomRef.current, null);
        decoded = await decodeCameraFrame(c, cropFactor);
        // When digitally zoomed, also try the full frame so a large/near code
        // that no longer fits the crop still decodes.
        if (!decoded && cropFactor < 0.999) decoded = await decodeCameraFrame(c, 1);
        // Always take one tighter center pass so distant/small codes get read
        // automatically without the user having to zoom in.
        if (!decoded) decoded = await decodeCameraFrame(c, 0.6);
      }

      if (mounted && decoded && !scannedRef.current) onScanRef.current(decoded);
      if (mounted) timer = setTimeout(scan, decoded ? 2000 : 700);
    }
    timer = setTimeout(scan, 1000);

    return () => { mounted = false; clearTimeout(timer); streamRef.current?.getTracks().forEach(t => t.stop()); };
  }, [applyTrackConstraints, onCapabilities]);

  // Re-apply on any torch/zoom change (single combined constraint set).
  useEffect(() => { applyTrackConstraints(); }, [torch, zoom, applyTrackConstraints]);

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

function NativeCamera({ facing, onBarCodeScanned, active, torch, zoom, onZoomChange, onManualZoom, onReady }: {
  facing: 'back' | 'front';
  onBarCodeScanned: (result: { data: string }) => void;
  active: boolean;
  torch: boolean;
  /** 0-1 zoom intent; mapped onto the device's best-usable range below. */
  zoom: number;
  onZoomChange: (zoom: number) => void;
  /** Called when the user takes manual control, so auto-hunt backs off. */
  onManualZoom: () => void;
  /** Fires once the live camera preview is running. */
  onReady: () => void;
}) {
  const { t } = useTranslation();
  const { CameraView, useCameraPermissions } = require('expo-camera');
  const [permission, requestPermission] = useCameraPermissions();

  // Pinch-to-zoom. The gesture runs on the UI thread; we hop to JS to read the
  // latest zoom at gesture start and to push updates back into React state.
  const zoomRef = useRef(zoom);
  zoomRef.current = zoom;
  const pinchStart = useRef(0);
  const onPinchStart = () => { pinchStart.current = zoomRef.current; onManualZoom(); };
  const onPinchUpdate = (scale: number) => {
    onZoomChange(Math.max(0, Math.min(1, pinchStart.current + (scale - 1) * 0.5)));
  };
  const pinch = Gesture.Pinch()
    .onStart(() => { runOnJS(onPinchStart)(); })
    .onUpdate((e) => { runOnJS(onPinchUpdate)(e.scale); });

  if (!permission?.granted) {
    // After a permanent denial (canAskAgain === false) requestPermission() can no
    // longer re-prompt, so route the user to the OS settings instead of a dead button.
    const canAskAgain = permission?.canAskAgain ?? true;
    return (
      <View style={styles.permissionContainer}>
        <Camera size={40} color={color.fg.subtle} />
        <Text style={styles.permissionText}>{t('componentsUi.scanner.permissionText')}</Text>
        <Pressable
          style={styles.permissionButton}
          onPress={canAskAgain ? requestPermission : () => Linking.openSettings()}
        >
          <Text style={styles.permissionButtonText}>
            {canAskAgain ? t('componentsUi.scanner.grantPermission') : t('onboarding.create.openSettings')}
          </Text>
        </Pressable>
      </View>
    );
  }

  return (
    <GestureDetector gesture={pinch}>
      <View style={styles.cameraContainer}>
        <CameraView
          style={styles.camera}
          facing={facing}
          enableTorch={torch}
          // Map the 0..1 intent onto the device's best-usable fraction of its max
          // zoom — never the blurry absolute max.
          zoom={zoom * ZOOM.nativeCap}
          barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
          onBarcodeScanned={active ? onBarCodeScanned : undefined}
          onCameraReady={onReady}
        />
        <ScanOverlay />
      </View>
    </GestureDetector>
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

// -- Zoom slider (manual override, shared web + native) ----------------------

function ZoomSlider({ value, onChange, onManualStart }: {
  value: number;
  onChange: (value: number) => void;
  onManualStart: () => void;
}) {
  const THUMB = 20;
  const trackWidth = useSharedValue(0);
  const thumbX = useSharedValue(0);
  const startX = useSharedValue(0);
  const dragging = useSharedValue(false);

  // Follow external changes (auto-hunt ramp) when the user isn't dragging.
  useEffect(() => {
    if (!dragging.value && trackWidth.value > 0) {
      thumbX.value = withSpring(value * trackWidth.value, { damping: 20, stiffness: 200 });
    }
  }, [value, dragging, thumbX, trackWidth]);

  const emit = useCallback((v: number) => onChange(v), [onChange]);
  const startManual = useCallback(() => onManualStart(), [onManualStart]);

  const pan = Gesture.Pan()
    .onStart(() => { dragging.value = true; startX.value = thumbX.value; runOnJS(startManual)(); })
    .onUpdate((e) => {
      const w = trackWidth.value;
      if (w <= 0) return;
      const raw = Math.max(0, Math.min(w, startX.value + e.translationX));
      thumbX.value = raw;
      runOnJS(emit)(raw / w);
    })
    .onEnd(() => { dragging.value = false; });

  const thumbStyle = useAnimatedStyle(() => ({ transform: [{ translateX: thumbX.value - THUMB / 2 }] }));
  const fillStyle = useAnimatedStyle(() => ({ width: thumbX.value }));

  return (
    <View style={styles.zoomRow}>
      <ZoomIn size={16} color="rgba(255,255,255,0.75)" strokeWidth={2} />
      <View
        style={styles.zoomTrackOuter}
        onLayout={(e) => {
          const w = e.nativeEvent.layout.width;
          trackWidth.value = w;
          thumbX.value = value * w;
        }}
      >
        <View style={styles.zoomTrack} />
        <Animated.View style={[styles.zoomFill, fillStyle]} />
        <GestureDetector gesture={pan}>
          <Animated.View style={[styles.zoomThumb, thumbStyle]} hitSlop={16} />
        </GestureDetector>
      </View>
    </View>
  );
}

// -- Main QRScanner ----------------------------------------------------------

export function QRScanner({ visible, onScan, onClose }: Props) {
  const { t } = useTranslation();
  const [scanned, setScanned] = useState(false);
  const [facing, setFacing] = useState<'back' | 'front'>('back');
  const [torch, setTorch] = useState(false);
  const [zoom, setZoom] = useState(0);
  const [manualZoom, setManualZoom] = useState(false);
  const [cameraReady, setCameraReady] = useState(false);
  const [webCaps, setWebCaps] = useState<{ torch: boolean; hwZoom: boolean }>({ torch: false, hwZoom: false });
  const insets = useSafeAreaInsets();

  // Fresh session every time the sheet opens.
  useEffect(() => {
    if (visible) { setScanned(false); setTorch(false); setZoom(0); setManualZoom(false); setCameraReady(false); }
  }, [visible]);

  // Auto-hunt zoom: after a grace period at 1x (near codes read instantly), sweep
  // the zoom up and back down in a slow triangle wave, so BOTH a near code (needs
  // 1x) and a distant one (needs magnification) get a decodable moment each cycle.
  // Unlike a ramp-and-hold this self-recovers — it can never get stuck too-zoomed.
  // Any manual interaction (slider/pinch) or a successful scan cancels it, and it
  // only runs once the live camera is up (not on the permission/denied screen).
  useEffect(() => {
    if (!visible || !cameraReady || scanned || manualZoom) return;
    const GRACE = 1800, STEP = 0.09, INTERVAL = 650, CAP = ZOOM.autoIntentCap;
    const up = Math.max(1, Math.ceil(CAP / STEP));
    const period = up * 2; // steps for a full 0 -> CAP -> 0 cycle
    let i = 0;
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      i += 1;
      const phase = i % period;
      const frac = phase <= up ? phase / up : (period - phase) / up;
      setZoom(+(frac * CAP).toFixed(3));
      timer = setTimeout(tick, INTERVAL);
    };
    timer = setTimeout(tick, GRACE);
    return () => clearTimeout(timer);
  }, [visible, cameraReady, scanned, manualZoom]);

  const startManual = useCallback(() => setManualZoom(true), []);
  // Stable identities — WebCamera's start-once effect depends on onCapabilities,
  // and the parent re-renders every ~650ms during the auto-hunt sweep, so an
  // inline closure here would restart getUserMedia on every tick.
  const handleReady = useCallback(() => setCameraReady(true), []);
  const handleCapabilities = useCallback((caps: { torch: boolean; hwZoom: boolean }) => {
    setWebCaps(caps);
    setCameraReady(true);
  }, []);

  function handleBarCodeScanned({ data }: { data: string }) {
    if (scanned) return;
    setScanned(true);
    hapticSuccess();
    onScan(data.trim());
    setZoom(0);
    setManualZoom(false);
    setTimeout(() => setScanned(false), 2000);
  }

  function toggleTorch() {
    hapticLight();
    setTorch(v => !v);
  }

  function switchCamera() {
    setFacing(f => {
      const next = f === 'back' ? 'front' : 'back';
      if (next === 'front') setTorch(false); // torch is a back-camera feature
      return next;
    });
  }

  // Torch button visibility: only once the camera is live, and on native only for
  // the back camera; on web capability-gated (hidden on iOS Safari, no torch).
  const showTorch = cameraReady && (Platform.OS === 'web' ? webCaps.torch : facing === 'back');

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
          canvas.getContext('2d', { willReadFrequently: true })!.drawImage(img, 0, 0);
          URL.revokeObjectURL(url);

          await loadZbar();
          const decoded = await decodeUploadedImage(canvas);
          if (decoded) { hapticSuccess(); onScan(decoded.trim()); }
          else showAlert(t('componentsUi.scanner.noQrFound'), t('componentsUi.scanner.noQrFoundMsg'));
        } catch (e: any) {
          console.warn('[QR] pick error:', e?.message);
          showAlert(t('componentsUi.scanner.error'), t('componentsUi.scanner.errorImage'));
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
            hapticSuccess(); onScan(barcodes[0].data.trim()); return;
          }
        } catch {}

        if (asset.base64) {
          try {
            const { decodeBase64Image } = await import('@/services/image-decode');
            const imageData = decodeBase64Image(asset.base64);
            if (imageData) {
              const code = jsQR(imageData.data as any, imageData.width, imageData.height, JSQR_OPTS);
              if (code?.data) { hapticSuccess(); onScan(code.data.trim()); return; }
            }
          } catch {}
        }
        showAlert(t('componentsUi.scanner.noQrFound'), t('componentsUi.scanner.noQrFoundMsg'));
      } catch { showAlert(t('componentsUi.scanner.error'), t('componentsUi.scanner.errorPicker')); }
    }
  }

  if (!visible) return null;

  const content = (
    <GestureHandlerRootView style={styles.container}>
      {Platform.OS === 'web' ? (
        <WebCamera
          onScan={(data) => handleBarCodeScanned({ data })}
          scanned={scanned}
          torch={torch}
          zoom={zoom}
          onCapabilities={handleCapabilities}
        />
      ) : (
        <NativeCamera
          facing={facing}
          onBarCodeScanned={handleBarCodeScanned}
          active={!scanned}
          torch={torch}
          zoom={zoom}
          onZoomChange={setZoom}
          onManualZoom={startManual}
          onReady={handleReady}
        />
      )}

      <View style={[styles.headerOverlay, { paddingTop: Math.max(insets.top, 20) }]}>
        <Pressable onPress={onClose} hitSlop={8} style={styles.headerBtn}>
          <X size={22} color="#fff" strokeWidth={2.5} />
        </Pressable>
        <Text style={styles.title}>{t('componentsUi.scanner.title')}</Text>
        <View style={styles.headerRight}>
          {showTorch && (
            <Pressable
              onPress={toggleTorch}
              hitSlop={8}
              style={[styles.headerBtn, torch && styles.headerBtnActive]}
              accessibilityRole="button"
              accessibilityState={{ selected: torch }}
              accessibilityLabel={t('componentsUi.scanner.torchLabel')}
            >
              <Flashlight size={20} color={torch ? '#000' : '#fff'} strokeWidth={2} />
            </Pressable>
          )}
          <Pressable onPress={handlePickImage} hitSlop={8} style={styles.headerBtn}>
            <ImagePlus size={20} color="#fff" strokeWidth={2} />
          </Pressable>
          {Platform.OS !== 'web' && (
            <Pressable onPress={switchCamera} hitSlop={8} style={styles.headerBtn}>
              <SwitchCamera size={20} color="#fff" strokeWidth={2} />
            </Pressable>
          )}
        </View>
      </View>

      <View style={[styles.footerOverlay, { paddingBottom: Math.max(insets.bottom, 20) }]}>
        {cameraReady && <ZoomSlider value={zoom} onChange={setZoom} onManualStart={startManual} />}
        <Text style={styles.hint}>{t('componentsUi.scanner.hint')}</Text>
      </View>
    </GestureHandlerRootView>
  );

  if (Platform.OS !== 'web') {
    return (
      <Modal visible={visible} animationType="slide" statusBarTranslucent onRequestClose={onClose}>
        <StatusBar barStyle="light-content" backgroundColor="transparent" translucent />
        {content}
      </Modal>
    );
  }

  return <Modal visible={visible} animationType="slide" transparent onRequestClose={onClose}>{content}</Modal>;
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
  headerBtn: { width: 44, height: 44, alignItems: 'center', justifyContent: 'center', borderRadius: 22 },
  headerBtnActive: { backgroundColor: '#fff' },
  headerRight: { flexDirection: 'row', alignItems: 'center' },
  title: { fontSize: text.lg, ...inter.bold, color: '#fff' },
  footerOverlay: { position: 'absolute', bottom: 0, left: 0, right: 0, alignItems: 'center', zIndex: 10 },
  hint: { fontSize: text.sm, ...inter.medium, color: 'rgba(255,255,255,0.7)' },
  zoomRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    width: '100%', maxWidth: 320, paddingHorizontal: space['3xl'], marginBottom: space.xl,
  },
  zoomTrackOuter: { flex: 1, height: 28, justifyContent: 'center' },
  zoomTrack: { position: 'absolute', left: 0, right: 0, height: 3, borderRadius: 2, backgroundColor: 'rgba(255,255,255,0.28)' },
  zoomFill: { position: 'absolute', left: 0, height: 3, borderRadius: 2, backgroundColor: '#fff' },
  zoomThumb: {
    position: 'absolute', top: 4, width: 20, height: 20, borderRadius: 10, backgroundColor: '#fff',
    shadowColor: '#000', shadowOffset: { width: 0, height: 1 }, shadowOpacity: 0.3, shadowRadius: 2, elevation: 3,
  },
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
