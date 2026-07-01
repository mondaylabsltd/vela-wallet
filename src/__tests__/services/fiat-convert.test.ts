import { tokenPriceInFiat, fiatToTokenAmount, resolveTokenAmount } from '@/services/fiat-convert';

describe('tokenPriceInFiat', () => {
  test('USDT (priceUsd=1) at a CNY rate is just the rate', () => {
    expect(tokenPriceInFiat(1, 7.1)).toBeCloseTo(7.1, 10);
  });
  test('ETH price scales by the fiat rate', () => {
    expect(tokenPriceInFiat(3000, 7.1)).toBeCloseTo(21300, 6);
  });
  test('unknown / non-positive price ⇒ 0 (the "cannot convert" sentinel)', () => {
    expect(tokenPriceInFiat(null, 7.1)).toBe(0);
    expect(tokenPriceInFiat(undefined, 7.1)).toBe(0);
    expect(tokenPriceInFiat(0, 7.1)).toBe(0);
    expect(tokenPriceInFiat(-5, 7.1)).toBe(0);
  });
  test('a non-positive fiat rate falls back to 1 (USD passthrough)', () => {
    expect(tokenPriceInFiat(2, 0)).toBe(2);
    expect(tokenPriceInFiat(2, -1)).toBe(2);
  });
});

describe('fiatToTokenAmount — the payroll conversion', () => {
  test('7100 CNY ÷ (1 USDT = 7.1 CNY) = 1000 USDT', () => {
    expect(fiatToTokenAmount(7100, 7.1, 6)).toBe('1000');
  });
  test('a fractional payroll figure truncates to token decimals', () => {
    // 1234.56 CNY / 7.1 = 173.8816901..., USDT has 6 decimals
    expect(fiatToTokenAmount(1234.56, 7.1, 6)).toBe('173.88169');
  });
  test('strips trailing zeros but keeps meaningful decimals', () => {
    expect(fiatToTokenAmount(710, 7.1, 6)).toBe('100');
    expect(fiatToTokenAmount(15, 10, 18)).toBe('1.5');
  });
  test('non-positive fiat ⇒ "0"', () => {
    expect(fiatToTokenAmount(0, 7.1, 6)).toBe('0');
    expect(fiatToTokenAmount(-100, 7.1, 6)).toBe('0');
  });
  test('unknown price ⇒ "0" (never divide by zero)', () => {
    expect(fiatToTokenAmount(1000, 0, 6)).toBe('0');
    expect(fiatToTokenAmount(1000, -1, 6)).toBe('0');
  });
  test('a 0-decimal token stays an integer (no trailing-zero mangling)', () => {
    // regression guard: the old inline /\.?0+$/ strip turned "100" into "1"
    expect(fiatToTokenAmount(300, 3, 0)).toBe('100');
  });
});

describe('resolveTokenAmount — single-send fiat toggle (behaviour preserved)', () => {
  test('token mode returns the typed amount untouched', () => {
    expect(resolveTokenAmount('1.5', false, 3000, 18, 7.1)).toBe('1.5');
  });
  test('an unpriced token in fiat mode returns the raw amount (cannot convert)', () => {
    expect(resolveTokenAmount('100', true, null, 6, 7.1)).toBe('100');
    expect(resolveTokenAmount('100', true, 0, 6, 7.1)).toBe('100');
  });
  test('fiat mode divides by the token price in display currency', () => {
    // 7100 CNY of USDT (priceUsd=1) at rate 7.1 ⇒ 1000 USDT
    expect(resolveTokenAmount('7100', true, 1, 6, 7.1)).toBe('1000');
    // 21300 CNY of ETH (priceUsd=3000) at rate 7.1 ⇒ 1 ETH
    expect(resolveTokenAmount('21300', true, 3000, 18, 7.1)).toBe('1');
  });
  test('non-positive fiat input ⇒ "0"', () => {
    expect(resolveTokenAmount('0', true, 1, 6, 7.1)).toBe('0');
    expect(resolveTokenAmount('', true, 1, 6, 7.1)).toBe('0');
  });
  test('rate defaults to 1 (USD) when omitted', () => {
    expect(resolveTokenAmount('3000', true, 3000, 18)).toBe('1');
  });
});
