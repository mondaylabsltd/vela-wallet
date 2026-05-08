import React, { useState } from 'react';
import { View, Text, StyleSheet, Pressable } from 'react-native';
import { AppModal } from '@/components/ui/AppModal';
import { CameraView, useCameraPermissions } from 'expo-camera';
import { color, text, weight, space, radius, createStyles } from '@/constants/theme';
import { X, SwitchCamera, Camera } from 'lucide-react-native';

interface Props {
  visible: boolean;
  onScan: (data: string) => void;
  onClose: () => void;
}

export function QRScanner({ visible, onScan, onClose }: Props) {
  const [permission, requestPermission] = useCameraPermissions();
  const [scanned, setScanned] = useState(false);
  const [facing, setFacing] = useState<'back' | 'front'>('back');

  function handleBarCodeScanned({ data }: { data: string }) {
    if (scanned) return;
    setScanned(true);

    // Parse ethereum: URI scheme if present
    let address = data;
    if (data.startsWith('ethereum:')) {
      address = data.replace('ethereum:', '').split('?')[0].split('@')[0];
    }

    onScan(address);
    setTimeout(() => setScanned(false), 2000);
  }

  if (!visible) return null;

  return (
    <AppModal visible={visible}>
      <View style={styles.container}>
        <View style={styles.header}>
          <Pressable onPress={onClose} hitSlop={8} style={styles.headerBtn}>
            <X size={22} color={color.accent.base} strokeWidth={2.5} />
          </Pressable>
          <Text style={styles.title}>Scan QR</Text>
          <Pressable
            onPress={() => setFacing(f => f === 'back' ? 'front' : 'back')}
            hitSlop={8}
            style={styles.headerBtn}
          >
            <SwitchCamera size={22} color={color.accent.base} strokeWidth={2} />
          </Pressable>
        </View>

        {!permission?.granted ? (
          <View style={styles.permissionContainer}>
            <Camera size={40} color={color.fg.subtle} />
            <Text style={styles.permissionText}>
              Camera access is needed to scan QR codes.
            </Text>
            <Pressable style={styles.permissionButton} onPress={requestPermission}>
              <Text style={styles.permissionButtonText}>Grant Permission</Text>
            </Pressable>
          </View>
        ) : (
          <View style={styles.cameraContainer}>
            <CameraView
              style={styles.camera}
              facing={facing}
              barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
              onBarcodeScanned={handleBarCodeScanned}
            />
            {/* Overlay with scanning frame */}
            <View style={styles.overlay}>
              <View style={styles.overlayTop} />
              <View style={styles.overlayMiddle}>
                <View style={styles.overlaySide} />
                <View style={styles.scanFrame}>
                  {/* Corner accents */}
                  <View style={[styles.corner, styles.cornerTL]} />
                  <View style={[styles.corner, styles.cornerTR]} />
                  <View style={[styles.corner, styles.cornerBL]} />
                  <View style={[styles.corner, styles.cornerBR]} />
                </View>
                <View style={styles.overlaySide} />
              </View>
              <View style={styles.overlayBottom} />
            </View>
          </View>
        )}

        <Text style={styles.hint}>
          Point the camera at a QR code containing a wallet address
        </Text>
      </View>
    </AppModal>
  );
}

const FRAME_SIZE = 250;
const CORNER_SIZE = 24;

const styles = createStyles(() => ({
  container: {
    flex: 1,
    backgroundColor: '#000',
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: space['2xl'],
    paddingTop: 60,
    paddingBottom: space.xl,
  },
  headerBtn: {
    width: 44,
    height: 44,
    alignItems: 'center',
    justifyContent: 'center',
  },
  title: {
    fontSize: text.xl,
    fontWeight: weight.bold,
    color: color.fg.inverse,
  },
  permissionContainer: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: space['5xl'],
    gap: space['2xl'],
  },
  permissionText: {
    fontSize: text.lg,
    fontWeight: weight.regular,
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
    fontWeight: weight.semibold,
    color: color.fg.inverse,
  },
  cameraContainer: {
    flex: 1,
    position: 'relative',
  },
  camera: {
    flex: 1,
  },
  overlay: {
    ...StyleSheet.absoluteFillObject,
    justifyContent: 'center',
    alignItems: 'center',
  },
  overlayTop: {
    flex: 1,
    width: '100%',
    backgroundColor: 'rgba(0,0,0,0.55)',
  },
  overlayMiddle: {
    flexDirection: 'row',
    height: FRAME_SIZE,
  },
  overlaySide: {
    flex: 1,
    backgroundColor: 'rgba(0,0,0,0.55)',
  },
  scanFrame: {
    width: FRAME_SIZE,
    height: FRAME_SIZE,
    position: 'relative',
  },
  corner: {
    position: 'absolute',
    width: CORNER_SIZE,
    height: CORNER_SIZE,
    borderColor: color.accent.base,
  },
  cornerTL: {
    top: 0, left: 0,
    borderTopWidth: 3, borderLeftWidth: 3,
    borderTopLeftRadius: 8,
  },
  cornerTR: {
    top: 0, right: 0,
    borderTopWidth: 3, borderRightWidth: 3,
    borderTopRightRadius: 8,
  },
  cornerBL: {
    bottom: 0, left: 0,
    borderBottomWidth: 3, borderLeftWidth: 3,
    borderBottomLeftRadius: 8,
  },
  cornerBR: {
    bottom: 0, right: 0,
    borderBottomWidth: 3, borderRightWidth: 3,
    borderBottomRightRadius: 8,
  },
  overlayBottom: {
    flex: 1,
    width: '100%',
    backgroundColor: 'rgba(0,0,0,0.55)',
  },
  hint: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    paddingHorizontal: space['5xl'],
    paddingVertical: space['3xl'],
    paddingBottom: space['5xl'],
  },
}));
