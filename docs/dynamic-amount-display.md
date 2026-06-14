# Dynamic Amount Display — the "atomic number" cascade

A portable pattern for rendering money/number values that stay legible at any
magnitude. Lifted from Vela Wallet; the logic is framework-agnostic, the code
samples are React Native (Expo) but port cleanly to web/CSS and SwiftUI.

> **TL;DR** — A number is read as *one unit*. Never wrap it. Instead:
> **fit-to-width → compact-notation floor → two-tier typography**, and apply the
> rule **"glance = compact, detail = exact."**

---

## 1. The principle

A monetary amount (`$1,234,567.89`, `12.3M PEPE`) is a single semantic unit. The
moment it wraps to a second line, the eye asks "is this one number or two?" So
mainstream fintech (Apple Wallet, Cash App, Coinbase, Robinhood) **never wraps a
pure number**. Instead they apply a *priority cascade* — these are ordered
fallbacks, not alternatives:

| # | Step | What it does |
|---|------|--------------|
| 1 | **Fit-to-width** | Shrink the font from a max size so the number fits on one line. |
| 2 | **Compact floor** | Once shrinking would cross a legibility floor, switch *representation* (`$1,234,567.89 → $1.23M`) instead of rendering an illegible 10px number. |
| 3 | **Two-tier type** | Integer large, decimals / unit ticker subordinated (smaller + muted). |
| 4 | **(escape hatch)** | Multi-line only for *input* or *full-precision detail*, wrapping at the decimal boundary — never mid-number. |

Compact notation sits *between* shrinking and wrapping: shrink until the floor,
then abbreviate, and only wrap as a last resort for special surfaces.

---

## 2. The rule: "glance = compact, detail = exact"

The single most important product decision. The *same* value renders differently
by context:

| Context | Behavior | Why |
|---|---|---|
| **Input** (user typing) | smooth shrink, single line, **never** abbreviate | you must see the exact digits you typed |
| **Glanceable feed / balance list** | abbreviate large (`≥1e6 → 12.3M`) | a feed/list is scanned, not studied |
| **Detail / confirm view** | **full** precision | you tapped in (or are signing) to see the exact number |
| **Small amounts (`<1`)** | **never** abbreviate | `0.00004212` compacted is nonsense |

So an activity feed shows `+12.3M PEPE`; tapping into the detail shows
`+12,345,678.90 PEPE`. A send-confirm screen always shows the exact amount.

### Where to apply vs. leave alone

| Apply the cascade | Leave as plain text |
|---|---|
| Hero balance, account total | `≈ $…` secondary/conversion lines |
| Activity / history feed amounts | Gas fees (`≈ $0.02`) |
| Token-list balances, token-detail hero | Tiny inline labels, chart tooltips |
| Send available-balance + amount input | Share images / canvas-rendered receipts |

Rule of thumb: **the prominent number gets the cascade; secondary `≈$` text does
not.** Compacting a gas fee is pointless.

---

## 3. Building block A — `formatCompact`

Abbreviates large magnitudes with Latin suffixes (`K/M/B/T`). Locale-specific
suffixes (CJK myriads, etc.) are intentionally avoided — `K/M/B/T` read
universally and don't depend on a runtime's (often incomplete) `Intl` compact data.

```ts
const COMPACT_TIERS = [
  { v: 1e12, s: 'T' },
  { v: 1e9,  s: 'B' },
  { v: 1e6,  s: 'M' },
  { v: 1e3,  s: 'K' },
] as const;

/** 1234567.89 → "1.23M" · 4.5e9 → "4.5B" · 820 → "820" */
export function formatCompact(value: number, key?: NumberFormatKey): string {
  if (!isFinite(value)) return '0';
  const sign = value < 0 ? '-' : '';
  const abs = Math.abs(value);
  for (const t of COMPACT_TIERS) {
    if (abs >= t.v) {
      const scaled = abs / t.v;
      const frac = scaled < 10 ? 2 : scaled < 100 ? 1 : 0; // 1.23 / 12.3 / 123
      return sign + formatNumber(scaled, { maximumFractionDigits: frac, key }) + t.s;
    }
  }
  return formatNumber(value, { maximumFractionDigits: abs < 1 ? 4 : 2, key });
}
```

> **Dependency:** `formatNumber` is your locale-aware grouping/decimal formatter.
> If you don't have one, `new Intl.NumberFormat(locale, { maximumFractionDigits })`
> is a drop-in. `key`/`NumberFormatKey` is Vela's locale-preset selector — delete
> it if you just use the system locale.

---

## 4. Building block B — `formatTokenAmount`

Magnitude-scaled precision for crypto/token amounts, with the **compact threshold
(`≥1e6`)** as an opt-in. This encodes the "glance vs detail" rule at the data layer:
pass `{ compact: true }` on feeds/lists, omit it on detail/confirm.

```ts
export function formatTokenAmount(value: number, opts: { compact?: boolean } = {}): string {
  if (!isFinite(value) || value === 0) return '0';
  const abs = Math.abs(value);
  if (opts.compact && abs >= 1e6) return formatCompact(value);   // glance
  if (abs >= 1000) return formatNumber(value, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  if (abs >= 1)    return formatNumber(value, { maximumFractionDigits: 4 });
  return formatNumber(value, { maximumFractionDigits: 6 });       // keep tiny tails
}
```

| Input | `{compact:true}` | default (exact) |
|---|---|---|
| `0.5` | `0.5` | `0.5` |
| `1234.5678` | `1,234.57` | `1,234.57` |
| `0.00004212` | `0.000042` | `0.000042` |
| `12,345,678.9` | **`12.3M`** | `12,345,678.90` |
| `4.5e9` | **`4.5B`** | `4,500,000,000.00` |

For **fiat**, the equivalent decision (2 decimals, drop cents above ~100k) lives
in your currency layer; the *compact* fallback is handled width-aware by the
component below.

---

## 5. Building block C — `AmountText` component

The renderer. Two modes:

- **Numeric (`value`)** — fiat path: formats, two-tier decimals, **width-aware**
  compact fallback.
- **Pre-formatted (`text`)** — caller already owns the string (e.g. a crypto
  amount from `formatTokenAmount`): fit-to-width only, optional subordinated unit.

```tsx
import React, { useState } from 'react';
import { LayoutChangeEvent, StyleProp, Text, TextStyle, View, ViewStyle } from 'react-native';
import { formatCompact, formatNumber, numberSeparators } from '@/services/locale-format';

// Average glyph advance for the bold display face, in em. A touch generous so the
// width estimate never overflows; adjustsFontSizeToFit trims any residual.
const CHAR_EM = 0.6;

export interface AmountTextProps {
  value?: number;          // numeric (fiat) path — OR text
  text?: string;           // pre-formatted path — OR value
  symbol?: string;         // prefix glyph, e.g. "$"
  unit?: string;           // suffix ticker, e.g. "ETH" (subordinated)
  size: number;            // largest/ideal font size
  showDecimals?: boolean;  // render the cents/decimals
  minScale?: number;       // shrink floor (fraction of size) before going compact
  compact?: boolean;       // allow compact fallback (default true)
  tailScale?: number;      // relative size of the subordinated tail (default 0.56)
  maxLines?: number;       // 1 = atomic (default); >1 wraps at the decimal
  style?: StyleProp<TextStyle>;
  tailStyle?: StyleProp<TextStyle>;
  containerStyle?: StyleProp<ViewStyle>;
}

interface Resolved { head: string; tail: string; unit: string; size: number }

/** Largest font that fits `effLen` full-size chars in `width`, floored at `min`. */
function fit(effLen: number, width: number, size: number, min: number): number {
  if (!width || effLen <= 0) return size; // pre-measure: render at full, RN guards
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

  // Pre-formatted path: caller owns the string; we only fit it to width.
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

  // Tail + unit count less toward width because they render smaller.
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
```

### How it works

1. The wrapping `<View>` measures its own width via `onLayout`.
2. `fit()` estimates the largest font that fits `effLen` chars (`width / (effLen ·
   CHAR_EM)`), clamped to `[floor, size]`. `CHAR_EM ≈ 0.6` for a bold display face.
3. If the full number can't fit above the floor, switch to `formatCompact`.
4. `adjustsFontSizeToFit` is the *residual* safety net for font-metric error — our
   computed size is the load-bearing part (see gotchas).

---

## 6. ⚠️ The layout requirement (the #1 gotcha)

**Fit-to-width only works inside a width-bounded container.** If the amount sits in
a content-sized column (e.g. `alignItems: flex-end` with no width/flex), the box
measures its *own content* → it never shrinks, it just pushes its neighbour.

The fix — reserve a capped column and let the number stretch + shrink inside it:

```ts
// the right-hand amount column
right:      { alignItems: 'flex-end', flexShrink: 1, maxWidth: '52%' },
// the AmountText container — fills the (capped) column so it can measure + shrink
amountBox:  { alignSelf: 'stretch' },
// the text — right-align the digits to the column edge
amount:     { textAlign: 'right' /* + your font/color */ },
```

```tsx
<View style={styles.right}>
  <AmountText text={amount} size={17} minScale={0.7}
              style={styles.amount} containerStyle={styles.amountBox} />
  {/* fiat / time below … */}
</View>
```

Short amounts sit at full size (column = content width); long amounts shrink to the
`maxWidth` cap instead of squeezing the title. In a list, every row shares the same
column width, so the same magnitude renders consistently.

---

## 7. React-Native gotchas

- **`adjustsFontSizeToFit` needs `numberOfLines` *and* a bounded width.** Without
  both it does nothing.
- **`adjustsFontSizeToFit` + nested `<Text>` is unreliable on Android** — the child
  (decimal/unit) font size may not scale with the parent. That's *why* this
  component measures and computes the size itself, using `adjustsFontSizeToFit`
  only as a sub-pixel cushion rather than the primary mechanism.
- **Inputs can't use `adjustsFontSizeToFit`** (it's a `Text` prop; `TextInput`
  support is iOS-only/buggy). Drive an input's font from a smooth length→size
  function instead, and **never abbreviate an input**:
  ```ts
  function amountFontSize(value: string): number {
    const len = Math.max(value.length, 1);
    const size = Math.round(230 / Math.max(len, 5.75)); // ~5.75 chars at the 40px max
    return Math.max(17, Math.min(40, size));            // smooth, no step jumps
  }
  ```
- **`onLayout` fires on react-native-web too** (via `ResizeObserver`), so the same
  component works on the web build and is screenshot-testable (see §10).

---

## 8. Usage cookbook

```tsx
// Fiat hero balance — width-aware compact, two-tier cents
<AmountText value={totalUsd} symbol="$" size={52} minScale={0.55}
            showDecimals={shouldShowDecimals(totalUsd, code)}
            style={s.balanceInt} tailStyle={s.balanceDec} containerStyle={{ flex: 1 }} />

// Activity feed amount — pre-formatted + compact at the data layer
<AmountText text={`+${formatTokenAmount(v, { compact: true })}`} unit="PEPE"
            size={17} minScale={0.7} style={s.amount} containerStyle={s.amountBox} />

// Transaction detail hero — exact (no compact), subordinated ticker
<AmountText text={`+ ${formatTokenAmount(v)}`} unit="ETH"
            size={20} minScale={0.55} tailScale={0.62}
            style={s.heroAmount} tailStyle={s.heroUnit} />
```

## 9. Tuning knobs

| Knob | Typical | Effect |
|---|---|---|
| `size` | 17–52 | the max/ideal font size |
| `minScale` | 0.55–0.7 | how small before compacting (lower = more shrink before abbreviating) |
| `tailScale` | 0.56–0.7 | how subordinated the decimals/ticker are |
| `CHAR_EM` | 0.55–0.62 | width estimate per char; raise if your face is wide / text clips |
| compact threshold | `1e6` | magnitude at which `formatTokenAmount` abbreviates |
| `maxWidth` on the column | 48–58% | how much row width the amount may claim before shrinking |

---

## 10. Porting

- **Web / CSS** — same cascade. Fit-to-width via `clamp()` + container-query units
  (`font-size: clamp(1rem, 8cqi, 3.25rem)`) or a measuring lib (fitty). Compact via
  `Intl.NumberFormat(locale, { notation: 'compact' })`. Wrap-at-boundary via
  `text-wrap: balance`. Same "glance = compact, detail = exact" rule.
- **SwiftUI** — `Text(...).lineLimit(1).minimumScaleFactor(0.55)` is the fit-to-width
  primitive; `ViewThatFits` for representation swaps; `.monospacedDigit()` for
  stable widths.

## 11. Verifying it

Because `onLayout` runs on react-native-web, drive the web build with Playwright and
screenshot a harness route that renders the component across magnitudes + container
widths (tiny → huge, wide card vs narrow column, crypto tickers). Confirm visually:
small amounts identical across feed/detail, large amounts abbreviate in feeds, tiny
amounts never abbreviate, nothing overflows. Delete the harness route after review.
```
