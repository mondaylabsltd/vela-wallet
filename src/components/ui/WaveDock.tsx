/**
 * WaveDock — the home screen's bottom action bar.
 *
 * Flat full-bleed bar on `bg.raised` with a 1px top hairline. The circular
 * Scan FAB floats centered, overlapping the bar's top edge by half — it is the
 * composition's focal point. Send is the single accent action (it moves
 * money); Receive is its matched neutral pill.
 *
 * Full-bleed: render this as a child of a full-width container (not inside the
 * padded ScreenContainer). It positions itself absolutely at the bottom.
 *
 * Screens must reserve scroll clearance of DOCK_BAR_HEIGHT + insets.bottom
 * (+ breathing room) — the bar height here excludes the safe-area inset.
 */
import { color, createStyles, inter, motion, radius, shadow, space, text } from '@/constants/theme';
import { hapticLight } from '@/services/platform';
import { ArrowDownLeft, ArrowUpRight, ScanLine } from 'lucide-react-native';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { Pressable, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withSpring } from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

/** Visual height of the bar above the safe-area inset (excludes the FAB overhang). */
export const DOCK_BAR_HEIGHT = 86;
/** Diameter of the Scan circle. */
const SCAN_SIZE = 56;

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
      onPressIn={() => { hapticLight(); scale.value = withSpring(0.97, motion.spring); }}
      onPressOut={() => { scale.value = withSpring(1, motion.spring); }}
      accessibilityRole="button"
      accessibilityLabel={label}
    >
      {/* Icon color must match the label color — a dimmer icon reads as half-disabled. */}
      <Icon size={22} color={primary ? color.fg.inverse : color.fg.base} strokeWidth={2.2} />
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

  // The container is taller than the bar by half the FAB so the overhanging
  // Scan button stays inside its bounds (touches outside a parent are dropped
  // on some platforms). The bar itself is anchored to the bottom.
  return (
    <View
      style={[styles.dock, { height: DOCK_BAR_HEIGHT + insets.bottom + SCAN_SIZE / 2 }]}
      pointerEvents="box-none"
    >
      <View style={[styles.bar, { height: DOCK_BAR_HEIGHT + insets.bottom }]} />

      {/* Scan — floats over the bar's top edge, half in / half out (icon only, no label) */}
      <View style={styles.scanWrap} pointerEvents="box-none">
        <AnimatedPressable
          style={[styles.scan, scanStyle]}
          onPress={onScan}
          onPressIn={() => { hapticLight(); scanScale.value = withSpring(0.92, motion.spring); }}
          onPressOut={() => { scanScale.value = withSpring(1, motion.spring); }}
          accessibilityRole="button"
          accessibilityLabel={t('componentsUi.dock.scan')}
        >
          <ScanLine size={26} color={color.fg.base} strokeWidth={2} />
        </AnimatedPressable>
      </View>

      {/* Two core buttons fill the width, leaving a slot for the Scan.
          Arrow pair mirrors ActivityRow's in/out glyphs. */}
      <View style={[styles.row, { bottom: insets.bottom + space.md }]} pointerEvents="box-none">
        <DockButton label={t('componentsUi.dock.receive')} icon={ArrowDownLeft} onPress={onReceive} variant="secondary" />
        <View style={styles.scanSlot} pointerEvents="none" />
        <DockButton label={t('componentsUi.dock.send')} icon={ArrowUpRight} onPress={onSend} variant="primary" />
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
  bar: {
    position: 'absolute',
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: color.bg.raised,
    borderTopWidth: 1,
    borderTopColor: color.border.base,
  },
  // Scan
  scanWrap: {
    position: 'absolute',
    top: 0,
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
    // Fixed-dark shadow token (fg.base would become a white glow in dark mode).
    ...shadow.md,
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
    // Both variants carry a 1px border so the pills stay the same height.
    borderWidth: 1,
  },
  btnPrimary: {
    backgroundColor: color.accent.base,
    borderColor: color.accent.base,
  },
  // bg.base + strong border, NOT bg.sunken: sunken-on-raised inverts to ~1.15:1
  // in dark mode (sunken is darker than raised there).
  btnSecondary: {
    backgroundColor: color.bg.base,
    borderColor: color.border.strong,
  },
  // text.xl: white-on-accent needs large-text size to clear WCAG 3:1 in light mode.
  btnLabel: {
    fontSize: text.xl,
  },
  btnLabelPrimary: {
    color: color.fg.inverse,
    ...inter.bold,
  },
  btnLabelSecondary: {
    color: color.fg.base,
    ...inter.semibold,
  },
}));
