/**
 * SectionLabel — the one section heading used across the app's minimal, de-boxed
 * layout (Apple Wallet / Wise style): an uppercase, letter-spaced, muted label that
 * groups open content by TYPE + SPACE instead of by boxing each section in a card.
 *
 *   <SectionLabel>Account</SectionLabel>
 *   … open rows separated by <Divider/> …
 */
import React from 'react';
import { Text, type TextStyle } from 'react-native';
import { color, createStyles, inter, space, text } from '@/constants/theme';

export function SectionLabel({ children, style }: { children: string; style?: TextStyle }) {
  return <Text style={[styles.label, style]}>{children}</Text>;
}

const styles = createStyles(() => ({
  label: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.subtle,
    letterSpacing: 0.6,
    textTransform: 'uppercase',
    marginTop: space['2xl'],
    marginBottom: space.md,
  },
}));
