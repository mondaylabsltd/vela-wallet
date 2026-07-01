/**
 * Tests for tokens.ts — the single source of truth for well-known ERC-20 static
 * metadata (symbol + decimals) keyed by lowercased address. Consolidated to stop
 * the old drift across three copies; a wrong decimals here mis-scales every amount.
 */
import { knownToken, knownTokenSymbol, knownTokenDecimals, KNOWN_TOKENS } from '@/services/tokens';

const USDC_ETH = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const DAI_ETH = '0x6b175474e89094c44da98b954eedeac495271d0f';

describe('knownToken lookup', () => {
  test('resolves a well-known token to symbol + decimals', () => {
    expect(knownToken(USDC_ETH)).toEqual({ symbol: 'USDC', decimals: 6 });
    expect(knownToken(DAI_ETH)).toEqual({ symbol: 'DAI', decimals: 18 });
  });

  test('is case-insensitive on the address (checksummed input still matches)', () => {
    expect(knownTokenSymbol(USDC_ETH.toUpperCase())).toBe('USDC');
    expect(knownTokenDecimals('0xA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48')).toBe(6);
  });

  test('unknown address → undefined (never a wrong guess)', () => {
    expect(knownToken('0x000000000000000000000000000000000000dead')).toBeUndefined();
    expect(knownTokenSymbol('0xdead')).toBeUndefined();
  });

  test('undefined/empty input → undefined, no throw', () => {
    expect(knownToken(undefined)).toBeUndefined();
    expect(knownTokenSymbol(undefined)).toBeUndefined();
    expect(knownTokenDecimals('')).toBeUndefined();
  });

  test('every table entry is keyed by a lowercased address (lookup relies on it)', () => {
    for (const key of Object.keys(KNOWN_TOKENS)) {
      expect(key).toBe(key.toLowerCase());
    }
  });
});
