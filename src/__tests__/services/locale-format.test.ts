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
