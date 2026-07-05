import React from 'react';
import { Platform, StyleSheet as RNStyleSheet } from 'react-native';
import { getTextScaleFactor } from './text-scale';

// =============================================================================
// Design Tokens — single source of truth
//
// Naming follows Simple Design conventions:
//   Spacing:    space.xs … space.5xl  (4px base)
//   Typography: text.xs … text.5xl    (size, scaled) + weight.regular … weight.bold
//   Radius:     radius.sm … radius.full
//   Colors:     fg (foreground hierarchy), bg (background layers), accent, semantic
//   Shadows:    shadow.sm … shadow.lg
//   Motion:     motion.fast … motion.slow
// =============================================================================

// ---------------------------------------------------------------------------
// 1. Spacing (4px base grid)
// ---------------------------------------------------------------------------

export const space = {
  '0':   0,
  'xs':  2,
  'sm':  4,
  'md':  8,
  'lg':  12,
  'xl':  16,
  '2xl': 20,
  '3xl': 24,
  '4xl': 32,
  '5xl': 48,
} as const;

// ---------------------------------------------------------------------------
// 2. Typography — sizes (scaled by user preference), weights, line-heights
// ---------------------------------------------------------------------------

// Base sizes before scaling
const TEXT_BASE = {
  'xs':  10,
  'sm':  11,
  'base': 13,
  'lg':  15,
  'xl':  17,
  '2xl': 20,
  '3xl': 26,
  '4xl': 32,
  '5xl': 40,
};

type TextKey = keyof typeof TEXT_BASE;
const TEXT_KEYS = Object.keys(TEXT_BASE) as TextKey[];

// Web-only baseline boost. On native, RN keeps applying the device's OS text
// size on top of our own factor (allowFontScaling defaults to true), so each
// user's system magnification is preserved — that's why native reads
// comfortably. The browser exposes no OS text-size to react-native-web, so web
// text is stuck at the raw factor and reads noticeably small. This fixed
// multiplier stands in for "an average user's system magnification" so web isn't
// cramped. It does NOT touch native (multiplier is 1 there).
const WEB_TEXT_BOOST = Platform.OS === 'web' ? 1.2 : 1;

function buildTextScale(): Record<TextKey, number> {
  const s = getTextScaleFactor();
  const result = {} as Record<TextKey, number>;
  for (const key of TEXT_KEYS) {
    result[key] = Math.round(TEXT_BASE[key] * s * WEB_TEXT_BOOST);
  }
  return result;
}

/** Scaled text sizes — multiply base × user scale factor (loaded at app start) */
export const text = buildTextScale();

/**
 * Apply the same web boost the `text` tokens get to a RAW (non-token) font size,
 * so hardcoded `fontSize: N` values aren't left small on web while everything
 * around them grew. No-op on native (WEB_TEXT_BOOST is 1 there), so native is
 * byte-for-byte unchanged. Use as `fontSize: scaleFont(10)`.
 */
export function scaleFont(size: number): number {
  return Math.round(size * WEB_TEXT_BOOST);
}

/** Style version — increments when text scale changes, invalidating createStyles caches. */
let _styleVersion = 0;
export function getStyleVersion() { return _styleVersion; }

/** Rebuild text scale values. Call after loadTextScale() or when user changes scale. */
export function rebuildTextScale(): void {
  const s = getTextScaleFactor();
  for (const key of TEXT_KEYS) {
    text[key] = Math.round(TEXT_BASE[key] * s * WEB_TEXT_BOOST);
  }
  _styleVersion++;
}

/**
 * Drop-in replacement for StyleSheet.create that re-evaluates when text scale changes.
 *
 * Usage: replace `const styles = StyleSheet.create({...})`
 *   with: `const styles = createStyles(() => ({...}))`
 *
 * Component code stays the same — `styles.title` works as before.
 */
export function createStyles<T extends RNStyleSheet.NamedStyles<T>>(
  factory: () => T | RNStyleSheet.NamedStyles<T>,
): T {
  let cache: T | null = null;
  let ver = -1;

  return new Proxy({} as T, {
    get(_, prop: string | symbol) {
      if (cache === null || ver !== _styleVersion) {
        cache = RNStyleSheet.create(factory() as T);
        ver = _styleVersion;
      }
      return (cache as any)[prop];
    },
  });
}

/**
 * Hook version of createStyles — guaranteed instant update.
 *
 * Unlike the Proxy-based createStyles (module-level, lazily invalidated),
 * this hook recomputes styles inside the component via useMemo when
 * the text scale version changes.  Use this in screens where the user
 * directly adjusts text scale and expects to see the result immediately.
 *
 * Usage:
 *   const styleFactory = () => ({ title: { fontSize: text.xl } });
 *   function MyScreen() {
 *     const styles = useStyles(styleFactory);
 *     return <Text style={styles.title}>Hello</Text>;
 *   }
 */
export function useStyles<T extends RNStyleSheet.NamedStyles<T>>(
  factory: () => T | RNStyleSheet.NamedStyles<T>,
): T {
  const { version } = require('@/constants/text-scale').useTextScale();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  return React.useMemo(() => RNStyleSheet.create(factory() as T) as T, [version]);
}

export const leading = {
  'none':   1,
  'tight':  1.2,
  'normal': 1.4,
  'relaxed': 1.6,
} as const;

export const weight = {
  'regular': '400' as const,
  'medium':  '500' as const,
  'semibold': '600' as const,
  'bold':    '700' as const,
};

/**
 * Shorthand for applying Inter with the correct weight file.
 *
 * On Android, fontWeight is ignored when fontFamily is set —
 * you must reference the weight-specific font file directly.
 *
 * Usage: `style={{ ...inter.semibold, fontSize: text.lg }}`
 */
// Typeface = Plus Jakarta Sans (loaded via useFonts in app/_layout). The export
// is still named `inter` to avoid churn across every screen; the families point
// to the Plus Jakarta weights.
export const inter = {
  regular:  { fontFamily: 'PlusJakartaSans_400Regular',  fontWeight: weight.regular  },
  medium:   { fontFamily: 'PlusJakartaSans_500Medium',   fontWeight: weight.medium   },
  semibold: { fontFamily: 'PlusJakartaSans_600SemiBold', fontWeight: weight.semibold },
  bold:     { fontFamily: 'PlusJakartaSans_700Bold',     fontWeight: weight.bold     },
} as const;

/**
 * Font zones — bundled Inter font ensures identical rendering on iOS and Android.
 *
 *   sans:    Primary UI text — labels, buttons, body copy.
 *   display: Large headings & hero numbers (balance, token name on detail).
 *   mono:    Addresses, hashes, technical values. Keeps platform monospace
 *            because Inter has no mono variant.
 *   numeric: Balance/USD columns. Inter has excellent tabular figures built-in.
 *
 * Weight mapping (React Native requires separate font files per weight):
 *   400 → Inter-Regular
 *   500 → Inter-Medium
 *   600 → Inter-SemiBold
 *   700 → Inter-Bold
 */
export const font = {
  sans:    'PlusJakartaSans_400Regular',
  display: 'PlusJakartaSans_700Bold',
  mono:    Platform.select({ ios: 'Menlo', default: 'monospace' }),
  numeric: 'PlusJakartaSans_400Regular',
};

/**
 * Map fontWeight to the correct Inter font file.
 * Use this when you need both fontFamily and fontWeight on the same Text:
 *   style={{ fontFamily: interWeight('600'), fontWeight: '600' }}
 *
 * React Native Android ignores fontWeight when a custom fontFamily is set —
 * you must point to the correct file.
 */
export function interWeight(w: '400' | '500' | '600' | '700'): string {
  switch (w) {
    case '400': return 'PlusJakartaSans_400Regular';
    case '500': return 'PlusJakartaSans_500Medium';
    case '600': return 'PlusJakartaSans_600SemiBold';
    case '700': return 'PlusJakartaSans_700Bold';
  }
}

// ---------------------------------------------------------------------------
// 3. Border Radius
// ---------------------------------------------------------------------------

export const radius = {
  'none': 0,
  'sm':   4,
  'md':   8,
  'lg':   12,
  'xl':   16,
  '2xl':  20,
  'full': 9999,
} as const;

// ---------------------------------------------------------------------------
// 4. Colors — keep Vela palette, organize semantically
//    Supports light + dark mode via rebuildColors()
// ---------------------------------------------------------------------------

const LIGHT_COLORS = {
  // muted/subtle darkened to clear WCAG: muted #6E6B62 ≥4.5:1 body text on bg.base,
  // subtle #8C887E ≥3:1 for placeholders/timestamps/disabled (was #7A776E/#B0ADA5, both failing).
  fg:      { base: '#1A1A18', muted: '#6E6B62', subtle: '#8C887E', inverse: '#FFFFFF' },
  bg:      { base: '#FAFAF8', raised: '#FFFFFF', sunken: '#F5F3EF' },
  accent:  { base: '#E8572A', soft: '#FFF0EB' },
  success: { base: '#2D8E5F', soft: '#EDFAF2' },
  warning: { base: '#92600A', soft: '#FFF8F0', border: '#F0DCC8' },
  // base deepened #EF4444 → #C62828 so danger TEXT clears WCAG AA (4.5:1) on error.soft
  // (#FEF2F2) and on white — the old bright red measured 3.44:1, and this is the most
  // safety-critical copy on the signing surface. Deep red also reads warmer/on-brand and
  // stays clearly distinct from the accent orange. White-on-base is 5.6:1 (destructive btns).
  error:   { base: '#C62828', soft: '#FEF2F2' },
  info:    { base: '#4267F4', soft: '#EDF0FF' },
  border:  { base: '#ECEBE4', strong: '#D8D6CE' },
};

const DARK_COLORS = {
  // subtle lightened to #85827A so tertiary/placeholder text clears the 3:1 floor on dark bg (was #6A6760 ≈2.96:1).
  fg:      { base: '#E8E6E1', muted: '#9A9790', subtle: '#85827A', inverse: '#1A1A18' },
  bg:      { base: '#141412', raised: '#1E1E1B', sunken: '#0F0F0D' },
  accent:  { base: '#E8572A', soft: '#2C1A12' },
  success: { base: '#3DA872', soft: '#132A1E' },
  warning: { base: '#D4A54A', soft: '#2A2010', border: '#3D3020' },
  error:   { base: '#F87171', soft: '#2D1515' },
  info:    { base: '#5A7CF6', soft: '#131B33' },
  border:  { base: '#2C2C28', strong: '#3E3E38' },
};

/** Mutable color tokens — mutated in place by rebuildColors(). */
export const color = {
  fg:      { ...LIGHT_COLORS.fg },
  bg:      { ...LIGHT_COLORS.bg },
  accent:  { ...LIGHT_COLORS.accent },
  success: { ...LIGHT_COLORS.success },
  warning: { ...LIGHT_COLORS.warning },
  error:   { ...LIGHT_COLORS.error },
  info:    { ...LIGHT_COLORS.info },
  border:  { ...LIGHT_COLORS.border },
};

let _isDark = false;
export function isDarkMode(): boolean { return _isDark; }

/** Rebuild color tokens for the given mode. Bumps style version. */
export function rebuildColors(isDark: boolean): void {
  if (_isDark === isDark && _styleVersion > 0) return; // no-op if unchanged
  _isDark = isDark;
  const palette = isDark ? DARK_COLORS : LIGHT_COLORS;
  for (const group of Object.keys(palette) as (keyof typeof palette)[]) {
    for (const key of Object.keys(palette[group])) {
      (color as any)[group][key] = (palette as any)[group][key];
    }
  }
  _styleVersion++;
}

// ---------------------------------------------------------------------------
// 5. Shadows
// ---------------------------------------------------------------------------

export const shadow = {
  sm: {
    shadowColor: '#1A1A18',
    shadowOffset: { width: 0, height: 1 },
    shadowOpacity: 0.04,
    shadowRadius: 3,
    elevation: 1,
  },
  md: {
    shadowColor: '#1A1A18',
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.06,
    shadowRadius: 8,
    elevation: 3,
  },
  lg: {
    shadowColor: '#1A1A18',
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.08,
    shadowRadius: 16,
    elevation: 6,
  },
} as const;

// ---------------------------------------------------------------------------
// 6. Motion
// ---------------------------------------------------------------------------

export const motion = {
  fast: 150,
  normal: 250,
  slow: 400,
  spring: { damping: 15, stiffness: 150, mass: 0.8 },
  springGentle: { damping: 20, stiffness: 120, mass: 1 },
} as const;

// Legacy themed color accessor — reads from mutable `color` tokens.
export function getThemeColors() {
  return {
    text: color.fg.base,
    background: color.bg.base,
    backgroundElement: _isDark ? '#212225' : '#F0F0F3',
    backgroundSelected: _isDark ? '#2E3135' : '#E0E1E6',
    textSecondary: color.fg.muted,
  };
}

export type ThemeColor = 'text' | 'background' | 'backgroundElement' | 'backgroundSelected' | 'textSecondary';

export const Fonts = Platform.select({
  ios: { sans: 'system-ui', serif: 'ui-serif', rounded: 'ui-rounded', mono: 'ui-monospace' },
  default: { sans: 'normal', serif: 'serif', rounded: 'normal', mono: 'monospace' },
});

export const Spacing = {
  half: 2, one: 4, two: 8, three: 16, four: 24, five: 32, six: 64,
} as const;

export const BottomTabInset = Platform.select({ ios: 50, android: 80 }) ?? 0;
export const MaxContentWidth = 800;
