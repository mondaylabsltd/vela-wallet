/**
 * Locale-aware formatting presets (number grouping / decimal mark, date order +
 * separator, 12- vs 24-hour). Explicit-key paths are deterministic and don't
 * depend on the runner's system locale.
 */
jest.mock('@/services/storage', () => ({
  // Fixed prefs so the `current preset` paths are deterministic in CI.
  getLocalePrefs: () => ({ numberFormat: 'comma_dot', dateFormat: 'ymd_slash', timeFormat: 'h24' }),
}));

import {
  formatNumber, formatDate, formatTime, formatDateTime, numberSeparators,
  formatCompact, formatTokenAmount,
} from '@/services/locale-format';

describe('locale-format: numbers', () => {
  const f2 = { minimumFractionDigits: 2, maximumFractionDigits: 2 } as const;

  it('groups + marks decimals per preset', () => {
    expect(formatNumber(1234567.89, { ...f2, key: 'comma_dot' })).toBe('1,234,567.89');
    expect(formatNumber(1234567.89, { ...f2, key: 'dot_comma' })).toBe('1.234.567,89');
    // space_comma uses a space group char — derive it so the assertion is exact.
    const sc = numberSeparators('space_comma');
    expect(formatNumber(1234567.89, { ...f2, key: 'space_comma' })).toBe(`1${sc.group}234${sc.group}567${sc.decimal}89`);
    expect(formatNumber(1234567.89, { ...f2, key: 'indian' })).toBe('12,34,567.89');
  });

  it('drops trailing zeros down to the minimum', () => {
    expect(formatNumber(1, { minimumFractionDigits: 0, maximumFractionDigits: 2, key: 'comma_dot' })).toBe('1');
    expect(formatNumber(1.5, { minimumFractionDigits: 0, maximumFractionDigits: 2, key: 'comma_dot' })).toBe('1.5');
  });

  it('rounds when no fraction digits are shown', () => {
    expect(formatNumber(2460539.4, { maximumFractionDigits: 0, key: 'comma_dot' })).toBe('2,460,539');
  });

  it('handles negatives', () => {
    expect(formatNumber(-1234.5, { ...f2, key: 'comma_dot' })).toBe('-1,234.50');
  });

  it('exposes the current preset separators', () => {
    expect(numberSeparators()).toEqual({ group: ',', decimal: '.' });
    expect(numberSeparators('dot_comma')).toEqual({ group: '.', decimal: ',' });
  });
});

describe('locale-format: compact notation', () => {
  it('abbreviates with K/M/B/T and 2–3 significant figures', () => {
    expect(formatCompact(1234567.89)).toBe('1.23M');
    expect(formatCompact(12345678.9)).toBe('12.3M');
    expect(formatCompact(4.5e9)).toBe('4.5B');
    expect(formatCompact(123456789012)).toBe('123B');
    expect(formatCompact(-1500000)).toBe('-1.5M');
  });

  it('leaves sub-1000 values un-abbreviated', () => {
    expect(formatCompact(820)).toBe('820');
    expect(formatCompact(0.5)).toBe('0.5');
  });
});

describe('locale-format: token amounts', () => {
  it('scales precision by magnitude', () => {
    expect(formatTokenAmount(0)).toBe('0');
    expect(formatTokenAmount(1234.5678)).toBe('1,234.57'); // >=1000 → 2dp
    expect(formatTokenAmount(12.3456)).toBe('12.3456');     // >=1   → 4dp
    expect(formatTokenAmount(0.00004212)).toBe('0.000042'); // <1    → keep tiny tail
  });

  it('abbreviates only large amounts when compact is requested', () => {
    expect(formatTokenAmount(12345678.9, { compact: true })).toBe('12.3M');
    expect(formatTokenAmount(999999, { compact: true })).toBe('999,999.00'); // < 1e6 → full
    expect(formatTokenAmount(0.5, { compact: true })).toBe('0.5');           // tiny never compacts
  });

  it('keeps full precision without compact (detail / confirm surfaces)', () => {
    expect(formatTokenAmount(12345678.9)).toBe('12,345,678.90');
  });
});

describe('locale-format: dates & times', () => {
  const d = new Date(2026, 5, 13, 13, 45); // Sat 2026-06-13 13:45 local

  it('orders date fields + separator per preset', () => {
    expect(formatDate(d, 'ymd_slash')).toBe('2026/06/13');
    expect(formatDate(d, 'mdy_slash')).toBe('06/13/2026');
    expect(formatDate(d, 'dmy_slash')).toBe('13/06/2026');
    expect(formatDate(d, 'dmy_dot')).toBe('13.06.2026');
    expect(formatDate(d, 'iso')).toBe('2026-06-13');
  });

  it('formats 12- and 24-hour clocks', () => {
    expect(formatTime(d, 'h24')).toBe('13:45');
    expect(formatTime(d, 'h12')).toBe('1:45 PM');
    expect(formatTime(new Date(2026, 5, 13, 0, 5), 'h12')).toBe('12:05 AM');
    expect(formatTime(new Date(2026, 5, 13, 0, 5), 'h24')).toBe('00:05');
  });

  it('combines date + time using the current prefs', () => {
    expect(formatDateTime(d)).toBe('2026/06/13, 13:45');
  });
});
