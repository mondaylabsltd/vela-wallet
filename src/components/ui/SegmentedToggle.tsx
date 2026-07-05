/**
 * SegmentedToggle — content-sized, horizontally scrollable text tabs
 * (e.g. Activity | Assets | Connections).
 *
 * Labels are NEVER truncated: each segment takes the width its text needs and
 * the row scrolls when the locale runs long (ja/de/ru), instead of squeezing
 * three equal columns. The active state is a soft `bg.sunken` chip — no border,
 * no shadow (design language: light controls, no chunky boxes) — that springs
 * between segments, animating position and width together since segments are
 * no longer equal. The active segment auto-scrolls into view.
 *
 * Generic over the option key type. Optional numeric badge per segment.
 * A selection haptic fires on change.
 */
import React, { useEffect, useRef, useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import Animated, { useSharedValue, useAnimatedStyle, withSpring } from 'react-native-reanimated';
import { color, createStyles, inter, motion, radius, shadow, space, text } from '@/constants/theme';
import { hapticSelection } from '@/services/platform';

export interface SegmentOption<T extends string> {
  key: T;
  label: string;
  badge?: number;
  /** Optional leading element (e.g. a theme icon or avatar preview), rendered
      by active state so it can match the label's ink. */
  icon?: (active: boolean) => React.ReactNode;
  /** Forwarded to the segment's Pressable (e.g. E2E hooks). */
  testID?: string;
}

interface Props<T extends string> {
  options: SegmentOption<T>[];
  value: T;
  onChange: (key: T) => void;
}

type Box = { x: number; width: number };

export function SegmentedToggle<T extends string>({ options, value, onChange }: Props<T>) {
  const [boxes, setBoxes] = useState<Partial<Record<T, Box>>>({});
  const scrollRef = useRef<ScrollView>(null);

  const tx = useSharedValue(0);
  const w = useSharedValue(0);
  const ready = useSharedValue(false);

  const active = boxes[value];

  // Slide + resize the chip to the active segment. First measured position is
  // set without animation; subsequent changes spring (position AND width — the
  // segments are content-sized, so both move).
  useEffect(() => {
    if (!active || active.width <= 0) return;
    if (!ready.value) {
      tx.value = active.x;
      w.value = active.width;
      ready.value = true;
    } else {
      tx.value = withSpring(active.x, motion.spring);
      w.value = withSpring(active.width, motion.spring);
    }
    // Keep the active label readable on narrow screens / long locales.
    scrollRef.current?.scrollTo({ x: Math.max(0, active.x - space['3xl']), animated: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active?.x, active?.width]);

  const chipStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: tx.value }],
    width: w.value,
    // Hide the chip until it has a measured width. A 0-width chip still paints
    // its 1px border as a thin vertical line — visible as a flash on every tab
    // switch, because this control remounts inside the list header when the
    // tab's list swaps (state resets → w returns to 0 until onLayout re-measures).
    opacity: w.value > 0 ? 1 : 0,
  }));

  return (
    <ScrollView
      ref={scrollRef}
      horizontal
      showsHorizontalScrollIndicator={false}
      style={styles.scroller}
      contentContainerStyle={styles.track}
    >
      <Animated.View style={[styles.chip, chipStyle]} pointerEvents="none" />
      {options.map((opt) => {
        const isActive = opt.key === value;
        return (
          <Pressable
            key={opt.key}
            style={styles.segment}
            onLayout={(e) => {
              const { x, width } = e.nativeEvent.layout;
              setBoxes((prev) => {
                const cur = prev[opt.key];
                if (cur && cur.x === x && cur.width === width) return prev;
                return { ...prev, [opt.key]: { x, width } };
              });
            }}
            onPress={() => {
              if (opt.key !== value) hapticSelection();
              onChange(opt.key);
            }}
            accessibilityRole="button"
            accessibilityState={{ selected: isActive }}
            accessibilityLabel={opt.label}
            testID={opt.testID}
          >
            {opt.icon?.(isActive)}
            <Text style={[styles.label, isActive && styles.labelActive]}>{opt.label}</Text>
            {opt.badge != null && opt.badge > 0 && (
              <View style={styles.badge}>
                <Text style={styles.badgeText}>{opt.badge}</Text>
              </View>
            )}
          </Pressable>
        );
      })}
    </ScrollView>
  );
}

const styles = createStyles(() => ({
  scroller: {
    flexGrow: 0,
    flexShrink: 1,
  },
  track: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
  },
  // The single sliding active indicator — a FLOATING chip (design language:
  // "transparent track + a single floating active chip"). A raised fill lifts
  // it over the transparent track; a strong hairline defines the edge; a soft
  // shadow floats it. Multiple redundant cues, because this warm low-contrast
  // palette can't carry selection on fill alone (bg.sunken was ~1.04:1 — no
  // visible chip, selection left to label color, WCAG 1.4.1). Not the old heavy
  // pill: the cramped truncated layout was the real culprit, now content-sized.
  chip: {
    position: 'absolute',
    left: 0,
    top: 0,
    bottom: 0,
    borderRadius: radius.full,
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.strong,
    ...shadow.sm,
  },
  segment: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    paddingVertical: space.md,
    paddingHorizontal: space.xl,
    minHeight: 44, // WCAG 2.5.8 touch-target floor
    borderRadius: radius.full,
  },
  label: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.muted,
  },
  // Color only — a weight change would alter the text width and make the chip
  // re-spring a second time after landing.
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
