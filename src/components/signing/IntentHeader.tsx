/**
 * Intent header — the action word above a signing surface (hero or eyebrow).
 */
import React from 'react';
import { View, Text } from 'react-native';
import { styles } from './signing-core';

export function IntentHeader({ intent, color: intentColor, variant = 'hero', colorEyebrow = false }: {
  intent: string;
  color: string;
  /** 'hero' = the big headline verb (opaque/risky actions own the screen).
   *  'eyebrow' = a small kicker above an asset-flow hero (benign, decoded
   *  value transfers — the money movement is the headline, not the verb). */
  variant?: 'hero' | 'eyebrow';
  /** Tint the eyebrow with `color` (red danger / green safe) instead of neutral
   *  grey. Keeps a risky verb readable as risk without blowing it up into a giant
   *  headline that competes with the summary for focus. */
  colorEyebrow?: boolean;
}) {
  if (variant === 'eyebrow') {
    // A clean uppercase kicker (no dot) — a quiet label that names the action and
    // cedes the headline to the content below. Neutral grey by default; a
    // danger/safe verb keeps its hue but not its size (colorEyebrow).
    return (
      <View style={styles.intentEyebrow}>
        <Text style={[styles.intentEyebrowText, colorEyebrow && { color: intentColor }]}>{intent}</Text>
      </View>
    );
  }
  return (
    <View style={styles.intentHeader}>
      <Text style={[styles.intentText, { color: intentColor }]}>
        {intent}
      </Text>
    </View>
  );
}
