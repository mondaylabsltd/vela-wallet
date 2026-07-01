/**
 * SegmentedToggle — a compact two-or-more segment switch (e.g. Activity | Connections).
 *
 * Generic over the option key type. Optional numeric badge per segment.
 * Theme-driven (light/dark). A single raised pill slides between segments with a
 * spring (iOS-segmented-control feel) and a selection haptic fires on change.
 */
import React, { useEffect, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated, { useSharedValue, useAnimatedStyle, withSpring } from 'react-native-reanimated';
import { color, createStyles, inter, motion, radius, shadow, space, text } from '@/constants/theme';
import { hapticSelection } from '@/services/platform';

export interface SegmentOption<T extends string> {
  key: T;
  label: string;
  badge?: number;
}

interface Props<T extends string> {
  options: SegmentOption<T>[];
  value: T;
  onChange: (key: T) => void;
}

/** Track padding + inter-segment gap (kept in sync with the styles below). */
const PAD = space.sm;
const GAP = space.sm;

export function SegmentedToggle<T extends string>({ options, value, onChange }: Props<T>) {
  const [trackW, setTrackW] = useState(0);
  const n = options.length;
  const segW = trackW > 0 ? (trackW - PAD * 2 - GAP * (n - 1)) / n : 0;
  const activeIndex = Math.max(0, options.findIndex((o) => o.key === value));

  const tx = useSharedValue(0);
  const ready = useSharedValue(false);

  // Slide the pill to the active segment. First measured position is set without
  // animation; subsequent changes spring.
  useEffect(() => {
    if (segW <= 0) return;
    const target = PAD + activeIndex * (segW + GAP);
    if (!ready.value) {
      tx.value = target;
      ready.value = true;
    } else {
      tx.value = withSpring(target, motion.spring);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeIndex, segW]);

  const pillStyle = useAnimatedStyle(() => ({ transform: [{ translateX: tx.value }] }));

  return (
    <View style={styles.track} onLayout={(e) => setTrackW(e.nativeEvent.layout.width)}>
      {segW > 0 && (
        <Animated.View style={[styles.pill, { width: segW }, pillStyle]} pointerEvents="none" />
      )}
      {options.map((opt) => {
        const active = opt.key === value;
        return (
          <Pressable
            key={opt.key}
            style={styles.segment}
            onPress={() => {
              if (opt.key !== value) hapticSelection();
              onChange(opt.key);
            }}
            accessibilityRole="button"
            accessibilityState={{ selected: active }}
            accessibilityLabel={opt.label}
          >
            <Text style={[styles.label, active && styles.labelActive]} numberOfLines={1}>{opt.label}</Text>
            {opt.badge != null && opt.badge > 0 && (
              <View style={styles.badge}>
                <Text style={styles.badgeText}>{opt.badge}</Text>
              </View>
            )}
          </Pressable>
        );
      })}
    </View>
  );
}

const styles = createStyles(() => ({
  track: {
    flexDirection: 'row',
    // Transparent track (was a heavy filled bg.sunken box) — reads as light tabs
    // with a single floating active chip, matching the de-boxed page.
    padding: PAD,
    gap: GAP,
    flex: 1,
  },
  // The single sliding indicator behind the labels (replaces per-segment bg).
  pill: {
    position: 'absolute',
    left: 0,
    top: PAD,
    bottom: PAD,
    borderRadius: radius.md,
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.base,
    ...shadow.sm,
  },
  segment: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    paddingVertical: space.md,
    minHeight: 44, // WCAG 2.5.8 touch-target floor (was ~29px)
    borderRadius: radius.md,
  },
  label: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.muted,
  },
  labelActive: {
    color: color.fg.base,
  },
  badge: {
    minWidth: 18,
    height: 18,
    borderRadius: 9,
    paddingHorizontal: 5,
    backgroundColor: color.fg.base,
    alignItems: 'center',
    justifyContent: 'center',
  },
  badgeText: {
    fontSize: text.xs,
    ...inter.bold,
    color: color.fg.inverse,
  },
}));
