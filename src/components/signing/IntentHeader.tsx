/**
 * Intent header — the action word above a signing surface (hero or eyebrow).
 */
import React from 'react';
import { View, Text } from 'react-native';
import { styles } from './signing-core';

export function IntentHeader({ intent, color: intentColor, variant = 'hero' }: {
  intent: string;
  color: string;
  /** 'hero' = the big headline verb (opaque/risky actions own the screen).
   *  'eyebrow' = a small kicker above an asset-flow hero (benign, decoded
   *  value transfers — the money movement is the headline, not the verb). */
  variant?: 'hero' | 'eyebrow';
}) {
  if (variant === 'eyebrow') {
    // A clean uppercase kicker (no dot) — a quiet label that names the action and
    // cedes the headline to the content below. Benign only, so it's always neutral.
    return (
      <View style={styles.intentEyebrow}>
        <Text style={styles.intentEyebrowText}>{intent}</Text>
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
