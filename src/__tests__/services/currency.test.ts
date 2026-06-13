/**
 * Tests for display-currency formatting + the priceable-source guarantee:
 *   - large balances drop their (noisy) decimals; zero-decimal currencies never show them
 *   - every offered currency is priceable by Chainlink and/or Frankfurter
 */
const CHAINLINK = [
  'EUR', 'GBP', 'JPY', 'CNY', 'AUD', 'CAD', 'CHF', 'KRW',
  'BRL', 'MXN', 'PHP', 'SGD', 'NZD', 'TRY', 'IDR', 'ARS',
];

jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(() => Promise.resolve(null)), setItem: jest.fn(() => Promise.resolve()) },
}));
jest.mock('@/services/fiat-fx', () => ({
  getFxRate: jest.fn(),
  getSupportedFxCodes: jest.fn(() => Promise.resolve(['USD'])),
}));
// Deterministic number formatting regardless of the CI runner's system locale.
jest.mock('@/services/locale-format', () => ({
  formatNumber: (v: number, o: { minimumFractionDigits?: number; maximumFractionDigits?: number } = {}) =>
    v.toLocaleString('en-US', {
      minimumFractionDigits: o.minimumFractionDigits ?? 0,
      maximumFractionDigits: o.maximumFractionDigits ?? 2,
    }),
}));
jest.mock('@/services/fiat-rates', () => ({
  getChainlinkRate: jest.fn(),
  isChainlinkFiat: (c: string) => CHAINLINK.includes(c.toUpperCase()),
  FIAT_FEED_CODES: CHAINLINK,
}));

import {
  CURRENCIES, formatFiat, shouldShowDecimals, isSupportedCurrency, ZERO_DECIMAL_CODES,
} from '@/services/currency';

describe('currency: decimal display', () => {
  it('shows cents for small amounts in minor-unit currencies', () => {
    expect(shouldShowDecimals(1620.56, 'USD')).toBe(true);
    expect(formatFiat(1620.56, 'USD', '$')).toBe('$1,620.56');
  });

  it('drops cents once the amount is large (visual noise)', () => {
    expect(shouldShowDecimals(2_460_539, 'USD')).toBe(false);
    expect(formatFiat(2_460_539, 'USD', '$')).toBe('$2,460,539');
  });

  it('never shows decimals for zero-decimal currencies, even when small', () => {
    expect(ZERO_DECIMAL_CODES.has('JPY')).toBe(true);
    expect(shouldShowDecimals(999, 'JPY')).toBe(false);
    expect(formatFiat(259_770, 'JPY', '¥')).toBe('¥259,770');
    expect(formatFiat(2_460_539, 'KRW', '₩')).toBe('₩2,460,539');
  });
});

describe('currency: offered list is priceable', () => {
  it('every offered currency is priceable by some source', () => {
    const unpriceable = CURRENCIES.map((c) => c.code).filter((c) => !isSupportedCurrency(c));
    expect(unpriceable).toEqual([]);
  });

  it('offers ARS (Chainlink-only) and HKD (Frankfurter-only)', () => {
    const codes = CURRENCIES.map((c) => c.code);
    expect(codes).toContain('ARS');
    expect(codes).toContain('HKD');
    expect(isSupportedCurrency('ARS')).toBe(true);
    expect(isSupportedCurrency('HKD')).toBe(true);
    expect(isSupportedCurrency('XYZ')).toBe(false);
  });

  it('has no duplicate currency codes', () => {
    const codes = CURRENCIES.map((c) => c.code);
    expect(new Set(codes).size).toBe(codes.length);
  });
});
