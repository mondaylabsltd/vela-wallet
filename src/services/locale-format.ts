/**
 * Locale-aware number / date / time formatting.
 *
 * Format-based (not country-based): a short set of explicit presets the user
 * picks by example in Settings. Formatting uses explicit rules (separators +
 * patterns) so it's reliable on Hermes, which ships incomplete `Intl`/ICU data.
 *
 * `auto` follows the system: we best-effort detect the device's conventions via
 * `Intl.*.formatToParts` (when available) and map them to a preset; if `Intl`
 * is missing or locked to en-US, we degrade to sensible defaults.
 */
import { useSyncExternalStore } from 'react';
import type { NumberFormatKey, DateFormatKey, TimeFormatKey, LocalePrefs } from '@/models/types';
import { getLocalePrefs, subscribeLocalePrefs } from '@/services/storage';

/**
 * Subscribe a component to number/date/time format changes. Returns the current
 * prefs, but most callers ignore the value — the point is to re-render when the
 * user picks a new format in Settings, so the `format*` helpers (which read the
 * prefs cache directly) produce fresh output. Call it in any component that
 * renders a formatted number, date, or time.
 */
export function useLocalePrefs(): LocalePrefs {
  return useSyncExternalStore(subscribeLocalePrefs, getLocalePrefs, getLocalePrefs);
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

interface NumberStyle { group: string; decimal: string; indian: boolean }

const NUMBER_STYLES: Record<Exclude<NumberFormatKey, 'auto'>, NumberStyle> = {
  comma_dot:   { group: ',', decimal: '.', indian: false },
  dot_comma:   { group: '.', decimal: ',', indian: false },
  space_comma: { group: ' ', decimal: ',', indian: false }, // space grouping (fr-FR style)
  indian:      { group: ',', decimal: '.', indian: true },
};

// ---------------------------------------------------------------------------
// System detection (best-effort) for `auto`
// ---------------------------------------------------------------------------

let _autoNumber: Exclude<NumberFormatKey, 'auto'> | null = null;
let _autoDate: Exclude<DateFormatKey, 'auto'> | null = null;
let _autoTime: Exclude<TimeFormatKey, 'auto'> | null = null;

function detectNumber(): Exclude<NumberFormatKey, 'auto'> {
  if (_autoNumber) return _autoNumber;
  let key: Exclude<NumberFormatKey, 'auto'> = 'comma_dot';
  try {
    const parts = new Intl.NumberFormat(undefined, { minimumFractionDigits: 1 }).formatToParts(1234567.8);
    const group = parts.find((p) => p.type === 'group')?.value ?? ',';
    const decimal = parts.find((p) => p.type === 'decimal')?.value ?? '.';
    if (decimal === ',' && group === '.') key = 'dot_comma';
    else if (decimal === ',') key = 'space_comma'; // space / NBSP / narrow NBSP
    else {
      // decimal '.', group ',' → standard, unless Indian 2-3 grouping (…,234,567 → …,23,45,678)
      const grouped = parts.filter((p) => p.type === 'integer').map((p) => p.value);
      key = grouped.length > 1 && grouped[grouped.length - 1].length === 3 && grouped.some((g) => g.length === 2)
        ? 'indian' : 'comma_dot';
    }
  } catch { /* keep default */ }
  _autoNumber = key;
  return key;
}

function detectDate(): Exclude<DateFormatKey, 'auto'> {
  if (_autoDate) return _autoDate;
  let key: Exclude<DateFormatKey, 'auto'> = 'mdy_slash';
  try {
    const parts = new Intl.DateTimeFormat(undefined, { year: 'numeric', month: '2-digit', day: '2-digit' })
      .formatToParts(new Date(2026, 5, 13));
    const order = parts.filter((p) => ['year', 'month', 'day'].includes(p.type)).map((p) => p.type[0]).join('');
    const sep = (parts.find((p) => p.type === 'literal')?.value ?? '/').trim() || '/';
    if (order === 'ymd') key = sep === '-' ? 'iso' : 'ymd_slash';
    else if (order === 'dmy') key = sep === '.' ? 'dmy_dot' : 'dmy_slash';
    else key = 'mdy_slash';
  } catch { /* keep default */ }
  _autoDate = key;
  return key;
}

function detectTime(): Exclude<TimeFormatKey, 'auto'> {
  if (_autoTime) return _autoTime;
  let key: Exclude<TimeFormatKey, 'auto'> = 'h24';
  try {
    const parts = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: 'numeric' })
      .formatToParts(new Date(2026, 5, 13, 13, 45));
    key = parts.some((p) => p.type === 'dayPeriod') ? 'h12' : 'h24';
  } catch { /* keep default */ }
  _autoTime = key;
  return key;
}

function resolveNumber(key: NumberFormatKey): Exclude<NumberFormatKey, 'auto'> {
  return key === 'auto' ? detectNumber() : key;
}
function resolveDate(key: DateFormatKey): Exclude<DateFormatKey, 'auto'> {
  return key === 'auto' ? detectDate() : key;
}
function resolveTime(key: TimeFormatKey): Exclude<TimeFormatKey, 'auto'> {
  return key === 'auto' ? detectTime() : key;
}

// ---------------------------------------------------------------------------
// Number
// ---------------------------------------------------------------------------

const pad2 = (n: number) => (n < 10 ? '0' + n : '' + n);

function groupInteger(digits: string, group: string, indian: boolean): string {
  if (digits.length <= 3) return digits;
  if (!indian) return digits.replace(/\B(?=(\d{3})+(?!\d))/g, group);
  const head = digits.slice(0, -3);
  const tail = digits.slice(-3);
  return head.replace(/\B(?=(\d{2})+(?!\d))/g, group) + group + tail;
}

/** Group + decimal separators for the chosen (or current) number preset. */
export function numberSeparators(key?: NumberFormatKey): { group: string; decimal: string } {
  const s = NUMBER_STYLES[resolveNumber(key ?? getLocalePrefs().numberFormat)];
  return { group: s.group, decimal: s.decimal };
}

interface NumberOpts { minimumFractionDigits?: number; maximumFractionDigits?: number; key?: NumberFormatKey }

/** Format a number using the chosen (or current) preset's separators/grouping. */
export function formatNumber(value: number, opts: NumberOpts = {}): string {
  if (!isFinite(value)) return '0';
  const style = NUMBER_STYLES[resolveNumber(opts.key ?? getLocalePrefs().numberFormat)];
  const maxFrac = opts.maximumFractionDigits ?? 2;
  const minFrac = Math.min(opts.minimumFractionDigits ?? 0, maxFrac);

  const sign = value < 0 ? '-' : '';
  const fixed = Math.abs(value).toFixed(maxFrac); // stable en-notation: "1234567.80"
  let [intPart, fracPart = ''] = fixed.split('.');
  // Trim trailing zeros down to minFrac.
  while (fracPart.length > minFrac && fracPart.endsWith('0')) fracPart = fracPart.slice(0, -1);

  const grouped = groupInteger(intPart, style.group, style.indian);
  return sign + grouped + (fracPart ? style.decimal + fracPart : '');
}

// Compact-notation tiers (crypto/fintech-standard Latin suffixes). Locale-
// specific suffixes (e.g. CJK myriads) are intentionally avoided — K/M/B/T read
// universally in a wallet and don't depend on Hermes' incomplete `Intl` data.
const COMPACT_TIERS = [
  { v: 1e12, s: 'T' },
  { v: 1e9,  s: 'B' },
  { v: 1e6,  s: 'M' },
  { v: 1e3,  s: 'K' },
] as const;

/**
 * Compact ("abbreviated") form for large magnitudes, e.g.
 *   1234567.89 → "1.23M"   ·   4.5e9 → "4.5B"   ·   820 → "820"
 *
 * Used as the legibility floor for hero amounts: once shrinking a full number
 * would make it illegible, callers switch to this instead of going tiny. Keeps
 * 2–3 significant figures and honours the current preset's decimal separator.
 */
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
  // Below 1000 there's nothing to abbreviate — render normally.
  return formatNumber(value, { maximumFractionDigits: abs < 1 ? 4 : 2, key });
}

/**
 * Format a crypto/token amount with magnitude-appropriate precision:
 *   >= 1000 → 2 decimals   ·   >= 1 → 4 decimals   ·   < 1 → 6 (keep tiny tails)
 *
 * With `compact: true` (glanceable surfaces — feeds, balances) large amounts are
 * abbreviated above 1e6, e.g. 12,345,678.9 → "12.3M". Small amounts are NEVER
 * abbreviated. Detail views pass full precision (no `compact`), matching the
 * big-tech "glance = compact, detail = exact" split.
 */
export function formatTokenAmount(value: number, opts: { compact?: boolean } = {}): string {
  if (!isFinite(value) || value === 0) return '0';
  const abs = Math.abs(value);
  if (opts.compact && abs >= 1e6) return formatCompact(value);
  if (abs >= 1000) return formatNumber(value, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  if (abs >= 1) return formatNumber(value, { maximumFractionDigits: 4 });
  return formatNumber(value, { maximumFractionDigits: 6 });
}

// ---------------------------------------------------------------------------
// Date & time
// ---------------------------------------------------------------------------

/** Format a date (no time) using the chosen (or current) preset. */
export function formatDate(input: Date | number, key?: DateFormatKey): string {
  const d = typeof input === 'number' ? new Date(input) : input;
  const y = d.getFullYear();
  const m = pad2(d.getMonth() + 1);
  const day = pad2(d.getDate());
  switch (resolveDate(key ?? getLocalePrefs().dateFormat)) {
    case 'ymd_slash': return `${y}/${m}/${day}`;
    case 'iso':       return `${y}-${m}-${day}`;
    case 'dmy_slash': return `${day}/${m}/${y}`;
    case 'dmy_dot':   return `${day}.${m}.${y}`;
    case 'mdy_slash':
    default:          return `${m}/${day}/${y}`;
  }
}

/** Format a time using the chosen (or current) preset (12- or 24-hour). */
export function formatTime(input: Date | number, key?: TimeFormatKey): string {
  const d = typeof input === 'number' ? new Date(input) : input;
  const h = d.getHours();
  const min = pad2(d.getMinutes());
  if (resolveTime(key ?? getLocalePrefs().timeFormat) === 'h12') {
    const h12 = h % 12 === 0 ? 12 : h % 12;
    return `${h12}:${min} ${h < 12 ? 'AM' : 'PM'}`;
  }
  return `${pad2(h)}:${min}`;
}

/** Combined date + time, e.g. "2026/06/13, 13:45" or "06/13/2026, 1:45 PM". */
export function formatDateTime(input: Date | number): string {
  const d = typeof input === 'number' ? new Date(input) : input;
  return `${formatDate(d)}, ${formatTime(d)}`;
}

// ---------------------------------------------------------------------------
// Option metadata for the Settings picker (label = a live example)
// ---------------------------------------------------------------------------

const SAMPLE = new Date(2026, 5, 13, 13, 45); // 2026-06-13 13:45 — distinguishes all orders

// `noteKey` is a stable semantic token (not display text) so the UI can
// translate it: render via t(`settings.formatNote.${noteKey}`).
export interface FormatOption<K> { key: K; example: string; noteKey?: 'system' | 'indian' | 'h24' | 'h12' }

export function numberFormatOptions(): FormatOption<NumberFormatKey>[] {
  const keys: NumberFormatKey[] = ['auto', 'comma_dot', 'dot_comma', 'space_comma', 'indian'];
  return keys.map((key) => ({
    key,
    example: formatNumber(1234567.89, { minimumFractionDigits: 2, maximumFractionDigits: 2, key }),
    noteKey: key === 'auto' ? 'system' : key === 'indian' ? 'indian' : undefined,
  }));
}

export function dateFormatOptions(): FormatOption<DateFormatKey>[] {
  const keys: DateFormatKey[] = ['auto', 'ymd_slash', 'mdy_slash', 'dmy_slash', 'dmy_dot', 'iso'];
  return keys.map((key) => ({ key, example: formatDate(SAMPLE, key), noteKey: key === 'auto' ? 'system' : undefined }));
}

export function timeFormatOptions(): FormatOption<TimeFormatKey>[] {
  const keys: TimeFormatKey[] = ['auto', 'h24', 'h12'];
  return keys.map((key) => ({
    key,
    example: formatTime(SAMPLE, key),
    noteKey: key === 'auto' ? 'system' : key === 'h24' ? 'h24' : 'h12',
  }));
}
