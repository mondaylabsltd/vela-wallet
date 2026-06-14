/**
 * The FX response normalizer accepts both provider shapes (so endpoints are
 * swappable): Frankfurter v2's array and the `{rates}` object (open.er-api / v1).
 */
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(() => Promise.resolve(null)), setItem: jest.fn(() => Promise.resolve()) },
}));
jest.mock('@/services/storage', () => ({ getFiatRatesURL: () => 'https://example.test' }));

import { normalizeRates } from '@/services/fiat-fx';

describe('fiat-fx: normalizeRates', () => {
  it('parses Frankfurter v2 array (base USD)', () => {
    const data = [
      { date: '2026-06-13', base: 'USD', quote: 'EUR', rate: 0.8646 },
      { date: '2026-06-13', base: 'USD', quote: 'VND', rate: 26208 },
      { date: '2026-06-13', base: 'USD', quote: 'cny', rate: 6.767 }, // lower-case code
    ];
    expect(normalizeRates(data)).toEqual({ USD: 1, EUR: 0.8646, VND: 26208, CNY: 6.767 });
  });

  it('parses the object shape (open.er-api / Frankfurter v1)', () => {
    expect(normalizeRates({ rates: { EUR: 0.92, JPY: 155.3 } })).toEqual({ USD: 1, EUR: 0.92, JPY: 155.3 });
  });

  it('drops non-positive / non-finite rates', () => {
    const data = [
      { base: 'USD', quote: 'EUR', rate: 0.9 },
      { base: 'USD', quote: 'BAD', rate: 0 },
      { base: 'USD', quote: 'NAN', rate: 'x' },
    ];
    expect(normalizeRates(data)).toEqual({ USD: 1, EUR: 0.9 });
  });

  it('returns null when nothing usable is present', () => {
    expect(normalizeRates([])).toBeNull();
    expect(normalizeRates({ rates: {} })).toBeNull();
    expect(normalizeRates({ error: 'nope' })).toBeNull();
    expect(normalizeRates(null)).toBeNull();
  });
});
