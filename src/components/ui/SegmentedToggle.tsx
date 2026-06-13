/**
 * SegmentedToggle — a compact two-or-more segment switch (e.g. Activity | Connections).
 *
 * Generic over the option key type. Optional numeric badge per segment.
 * Theme-driven (light/dark). The active segment lifts onto a raised surface.
 */
import React from 'react';
import { Pressable, Text, View } from 'react-native';
import { color, createStyles, inter, radius, shadow, space, text } from '@/constants/theme';

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

export function SegmentedToggle<T extends string>({ options, value, onChange }: Props<T>) {
  return (
    <View style={styles.track}>
      {options.map((opt) => {
        const active = opt.key === value;
        return (
          <Pressable
            key={opt.key}
            style={[styles.segment, active && styles.segmentActive]}
            onPress={() => onChange(opt.key)}
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
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.sm,
    gap: space.sm,
    flex: 1,
  },
  segment: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    paddingVertical: space.md,
    borderRadius: radius.md,
  },
  segmentActive: {
    backgroundColor: color.bg.raised,
    ...shadow.sm,
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
