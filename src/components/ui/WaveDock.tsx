/**
 * WaveDock — the home screen's bottom action bar.
 *
 * One continuous full-bleed surface with a soft wave crest in the centre that
 * the circular Scan button emerges from. Two core actions sit on the bar:
 * Receive (primary / brand) and Send (secondary). Payment-first: Receive is the
 * single brand-accent control by default.
 *
 * Full-bleed: render this as a child of a full-width container (not inside the
 * padded ScreenContainer). It positions itself absolutely at the bottom.
 *
 * Theme-driven (light/dark via token reads) and follows the design system:
 * spring scale on press, Lucide icons, Pressable.
 */
import { color, createStyles, inter, motion, radius, space, text } from '@/constants/theme';
import { Download, ScanLine, Send as SendIcon } from 'lucide-react-native';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { Pressable, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withSpring } from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import Svg, { Path } from 'react-native-svg';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

/** Visual height of the bar above the safe-area inset (compact). */
const BAR_HEIGHT = 86;
/** Height of the wavy top strip (viewBox is 1:1 with this). */
const WAVE_H = 40;
/** Diameter of the Scan circle. */
const SCAN_SIZE = 56;

// Tight concave cradle that hugs the Scan FAB — shallow + narrow so it doesn't
// waste space. Shoulders y=12 (bar opaque almost full width), cradle bottom y=24.
const WAVE_FILL = 'M0,12 L140,12 C152,12 158,24 180,24 C202,24 208,12 220,12 L360,12 L360,40 L0,40 Z';

interface DockButtonProps {
  label: string;
  icon: React.ComponentType<any>;
  onPress: () => void;
  variant: 'primary' | 'secondary';
}

function DockButton({ label, icon: Icon, onPress, variant }: DockButtonProps) {
  const scale = useSharedValue(1);
  const animatedStyle = useAnimatedStyle(() => ({ transform: [{ scale: scale.value }] }));
  const primary = variant === 'primary';
  return (
    <AnimatedPressable
      style={[styles.btn, primary ? styles.btnPrimary : styles.btnSecondary, animatedStyle]}
      onPress={onPress}
      onPressIn={() => { scale.value = withSpring(0.97, motion.spring); }}
      onPressOut={() => { scale.value = withSpring(1, motion.spring); }}
    >
      <Icon size={22} color={primary ? color.fg.inverse : color.fg.muted} strokeWidth={2.2} />
      <Text style={[styles.btnLabel, primary ? styles.btnLabelPrimary : styles.btnLabelSecondary]}>{label}</Text>
    </AnimatedPressable>
  );
}

interface WaveDockProps {
  onReceive: () => void;
  onScan: () => void;
  onSend: () => void;
}

export function WaveDock({ onReceive, onScan, onSend }: WaveDockProps) {
  const { t } = useTranslation();
  const insets = useSafeAreaInsets();
  const scanScale = useSharedValue(1);
  const scanStyle = useAnimatedStyle(() => ({ transform: [{ scale: scanScale.value }] }));

  return (
    <View style={[styles.dock, { height: BAR_HEIGHT + insets.bottom }]} pointerEvents="box-none">
      {/* Continuous surface: wavy top strip + solid fill down to the bottom.
          No stroke — a soft drop-shadow defines the edge so there's no hard line. */}
      <Svg width="100%" height={WAVE_H} viewBox="0 0 360 40" preserveAspectRatio="none" style={styles.wave}>
        <Path d={WAVE_FILL} fill={color.bg.raised} />
      </Svg>
      <View style={[styles.fill, { top: WAVE_H - 0.5 }]} />

      {/* Scan — floats in the cradle (icon only, no label) */}
      <View style={styles.scanWrap} pointerEvents="box-none">
        <AnimatedPressable
          style={[styles.scan, scanStyle]}
          onPress={onScan}
          onPressIn={() => { scanScale.value = withSpring(0.92, motion.spring); }}
          onPressOut={() => { scanScale.value = withSpring(1, motion.spring); }}
        >
          <ScanLine size={26} color={color.fg.base} strokeWidth={2} />
        </AnimatedPressable>
      </View>

      {/* Two core buttons fill the width, leaving a slot for the Scan */}
      <View style={[styles.row, { bottom: insets.bottom + space.md }]} pointerEvents="box-none">
        <DockButton label={t('componentsUi.dock.receive')} icon={Download} onPress={onReceive} variant="primary" />
        <View style={styles.scanSlot} pointerEvents="none" />
        <DockButton label={t('componentsUi.dock.send')} icon={SendIcon} onPress={onSend} variant="secondary" />
      </View>
    </View>
  );
}

const styles = createStyles(() => ({
  dock: {
    position: 'absolute',
    left: 0,
    right: 0,
    bottom: 0,
  },
  wave: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    // ...shadow.lg,
    // shadowOffset: { width: 0, height: -6 },
  },
  fill: {
    position: 'absolute',
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: color.bg.raised,
  },
  // Scan
  scanWrap: {
    position: 'absolute',
    top: -16,
    left: 0,
    right: 0,
    alignItems: 'center',
  },
  scan: {
    width: SCAN_SIZE,
    height: SCAN_SIZE,
    borderRadius: SCAN_SIZE / 2,
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.base,
    alignItems: 'center',
    justifyContent: 'center',
    // Fixed dark shadow (NOT fg.base — that becomes a white glow in dark mode).
    // Gentle, so Scan reads as a quiet utility button rather than an eye-catcher.
    shadowColor: '#000000',
    shadowOffset: { width: 0, height: 3 },
    shadowOpacity: 0.10,
    shadowRadius: 8,
    elevation: 4,
  },
  // Buttons fill the width; a small fixed slot leaves room for the Scan FAB.
  row: {
    position: 'absolute',
    left: 0,
    right: 0,
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingHorizontal: space.lg,
  },
  scanSlot: { width: SCAN_SIZE + space.sm * 2 },
  btn: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.md,
    paddingVertical: space.xl,
    paddingHorizontal: space.md,
    borderRadius: radius.xl,
  },
  btnPrimary: {
    backgroundColor: color.accent.base,
  },
  btnSecondary: {
    backgroundColor: color.bg.sunken,
  },
  btnLabel: {
    fontSize: text.lg,
    ...inter.semibold,
  },
  btnLabelPrimary: {
    color: color.fg.inverse,
  },
  btnLabelSecondary: {
    color: color.fg.base,
  },
}));
