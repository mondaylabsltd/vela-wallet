/**
 * AmountText — "atomic number" display, the way fintech apps do it.
 *
 * A monetary amount is read as ONE unit, so we never wrap it mid-number.
 * Instead we apply the standard cascade (Apple Wallet / Cash App / Coinbase /
 * Robinhood), in priority order:
 *
 *   1. Fit-to-width on one line   — shrink the font from `size` to fit the box.
 *   2. Compact notation as floor  — once shrinking would cross `minScale`,
 *      switch representation ($1,234,567.89 → $1.23M) instead of rendering an
 *      illegible 10px number. Full precision stays available in detail views.
 *   3. Two-tier typography        — integer large, decimal/unit tail subordinated.
 *
 * The box width is measured via onLayout, the font size is estimated from it,
 * and `adjustsFontSizeToFit` does the final sub-pixel correction (covers font-
 * metric error + Android, where our estimate is the load-bearing part).
 *
 * `maxLines` defaults to 1 (atomic). Raise it ONLY for input / full-precision
 * detail surfaces, where wrapping at the decimal boundary is acceptable — for a
 * hero balance, keep it at 1.
 */
import React, { useState } from 'react';
import { LayoutChangeEvent, StyleProp, Text, TextStyle, View, ViewStyle } from 'react-native';

import { formatCompact, formatNumber, numberSeparators, useLocalePrefs } from '@/services/locale-format';

// Average glyph advance for the bold display face, in em. Deliberately a touch
// generous so the width estimate never overflows; adjustsFontSizeToFit trims any
// residual. Tabular figures sit ~0.55em; symbols/separators pull the mean up.
const CHAR_EM = 0.6;

export interface AmountTextProps {
  /**
   * Numeric value (fiat path): formatted to the locale preset, with two-tier
   * decimals and the compact-notation floor. Provide this OR `text`.
   */
  value?: number;
  /**
   * Pre-formatted string (e.g. a crypto amount from `formatBalance`). Rendered
   * as-is, fit-to-width only — no re-formatting, no compact fallback. Use when
   * the caller already owns precision/grouping. Provide this OR `value`.
   */
  text?: string;
  /** Prefix glyph, e.g. a currency symbol "$" (kept attached to the integer). */
  symbol?: string;
  /** Suffix unit, e.g. a token ticker "ETH" (subordinated like the decimals). */
  unit?: string;
  /** Largest/ideal font size; the amount shrinks from here. */
  size: number;
  /** Render the decimal fraction. Caller decides (e.g. via shouldShowDecimals). */
  showDecimals?: boolean;
  /** Shrink floor (fraction of `size`) before switching to compact notation. */
  minScale?: number;
  /** Allow the compact-notation fallback for very large values. Default true. */
  compact?: boolean;
  /** Relative size of the subordinated tail (decimal + unit). Default 0.56. */
  tailScale?: number;
  /** Max lines. 1 (default) = atomic single line; >1 wraps at the decimal. */
  maxLines?: number;
  /** Style for the integer (color / weight / fontFamily — fontSize is managed). */
  style?: StyleProp<TextStyle>;
  /** Style override for the subordinated tail. */
  tailStyle?: StyleProp<TextStyle>;
  /** Style for the measured box (e.g. `flex: 1`). */
  containerStyle?: StyleProp<ViewStyle>;
}

interface Resolved { head: string; tail: string; unit: string; size: number }

/** Largest font that fits `effLen` "full-size" chars in `width`, floored at `min`. */
function fit(effLen: number, width: number, size: number, min: number): number {
  if (!width || effLen <= 0) return size; // pre-measure: render at full, let RN guard
  const ideal = width / (effLen * CHAR_EM);
  return Math.max(min, Math.min(size, ideal));
}

function resolve(p: AmountTextProps, width: number): Resolved {
  const { value, text, symbol = '', unit = '', size, showDecimals = true } = p;
  const minScale = p.minScale ?? 0.6;
  const tailScale = p.tailScale ?? 0.56;
  const compact = p.compact ?? true;
  const floor = size * minScale;
  const unitStr = unit ? ' ' + unit : '';

  // Pre-formatted path: the caller owns the string; we only fit it to width.
  if (text != null) {
    const effLen = text.length + unitStr.length * tailScale;
    return { head: text, tail: '', unit: unitStr, size: fit(effLen, width, size, floor) };
  }

  const sign = (value ?? 0) < 0 ? '-' : '';
  const abs = Math.abs(value ?? 0);

  // Full-precision split into a large head (symbol + integer) and a small tail.
  let head: string, tail: string;
  if (!showDecimals) {
    head = sign + symbol + formatNumber(abs, { maximumFractionDigits: 0 });
    tail = '';
  } else {
    const { decimal } = numberSeparators();
    const full = formatNumber(abs, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    const idx = full.lastIndexOf(decimal);
    head = sign + symbol + (idx === -1 ? full : full.slice(0, idx));
    tail = idx === -1 ? '' : full.slice(idx);
  }

  // Effective char count: the tail + unit count less because they render smaller.
  const effLen = head.length + (tail.length + unitStr.length) * tailScale;
  const ideal = width ? width / (effLen * CHAR_EM) : size;

  // Full number fits above the floor → keep it (two-tier).
  if (!compact || ideal >= floor) {
    return { head, tail, unit: unitStr, size: Math.max(floor, Math.min(size, ideal)) };
  }

  // Below the floor → switch representation rather than shrink into illegibility.
  const cHead = sign + symbol + formatCompact(abs);
  const cEff = cHead.length + unitStr.length * tailScale;
  return { head: cHead, tail: '', unit: unitStr, size: fit(cEff, width, size, floor) };
}

export function AmountText(props: AmountTextProps) {
  useLocalePrefs(); // re-render when the number format changes (value-mode formatting)
  const { style, tailStyle, containerStyle, maxLines = 1, minScale = 0.6 } = props;
  const [width, setWidth] = useState(0);

  const onLayout = (e: LayoutChangeEvent) => {
    const w = e.nativeEvent.layout.width;
    if (Math.abs(w - width) > 0.5) setWidth(w);
  };

  const { head, tail, unit, size } = resolve(props, width);
  const tailSize = Math.round(size * (props.tailScale ?? 0.56));
  const tailFont: StyleProp<TextStyle> = [style, tailStyle, { fontSize: tailSize }];

  return (
    <View style={containerStyle} onLayout={onLayout}>
      <Text
        numberOfLines={maxLines}
        adjustsFontSizeToFit
        minimumFontScale={minScale}
        style={[style, { fontSize: Math.round(size), lineHeight: Math.round(size * 1.12) }]}
      >
        {head}
        {tail ? <Text style={tailFont}>{tail}</Text> : null}
        {unit ? <Text style={tailFont}>{unit}</Text> : null}
      </Text>
    </View>
  );
}
