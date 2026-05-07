import React, { useState } from 'react';
import { View, Text, StyleSheet, TouchableOpacity } from 'react-native';
import { AppModal } from '@/components/ui/AppModal';
import { CameraView, useCameraPermissions } from 'expo-camera';
import { color, weight, radius } from '@/constants/theme';

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
      // ethereum:0x1234...?value=1000 or ethereum:0x1234...@1
      address = data.replace('ethereum:', '').split('?')[0].split('@')[0];
    }

    onScan(address);
    // Reset after a short delay so the scanner can be reused
    setTimeout(() => setScanned(false), 2000);
  }

  if (!visible) return null;

  return (
    <AppModal visible={visible}>
      <View style={styles.container}>
        <View style={styles.header}>
          <TouchableOpacity onPress={onClose} activeOpacity={0.7}>
            <Text style={styles.closeButton}>Close</Text>
          </TouchableOpacity>
          <Text style={styles.title}>Scan QR Code</Text>
          <TouchableOpacity onPress={() => setFacing(f => f === 'back' ? 'front' : 'back')} activeOpacity={0.7}>
            <Text style={styles.flipButton}>{facing === 'back' ? 'Front' : 'Back'}</Text>
          </TouchableOpacity>
        </View>

        {!permission?.granted ? (
          <View style={styles.permissionContainer}>
            <Text style={styles.permissionText}>
              Camera access is needed to scan QR codes.
            </Text>
            <TouchableOpacity
              style={styles.permissionButton}
              onPress={requestPermission}
              activeOpacity={0.7}
            >
              <Text style={styles.permissionButtonText}>Grant Permission</Text>
            </TouchableOpacity>
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
                <View style={styles.scanFrame} />
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

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000',
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 20,
    paddingTop: 60,
    paddingBottom: 16,
  },
  closeButton: {
    fontSize: 16, fontWeight: weight.semibold,
    color: color.accent.base,
    width: 60,
  },
  title: {
    fontSize: 17, fontWeight: weight.semibold,
    color: '#FFFFFF',
  },
  flipButton: {
    fontSize: 16, fontWeight: weight.semibold,
    color: color.accent.base,
    width: 60,
    textAlign: 'right',
  },
  permissionContainer: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: 40,
    gap: 20,
  },
  permissionText: {
    fontSize: 16, fontWeight: weight.regular,
    color: '#CCCCCC',
    textAlign: 'center',
  },
  permissionButton: {
    backgroundColor: color.accent.base,
    paddingHorizontal: 28,
    paddingVertical: 14,
    borderRadius: radius.xl,
  },
  permissionButtonText: {
    fontSize: 16, fontWeight: weight.semibold,
    color: '#FFFFFF',
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
    borderWidth: 2,
    borderColor: color.accent.base,
    borderRadius: 16,
  },
  overlayBottom: {
    flex: 1,
    width: '100%',
    backgroundColor: 'rgba(0,0,0,0.55)',
  },
  hint: {
    fontSize: 14, fontWeight: weight.regular,
    color: '#999999',
    textAlign: 'center',
    paddingHorizontal: 40,
    paddingVertical: 24,
    paddingBottom: 48,
  },
});
