/**
 * Tests for the Chainlink fiat-rate plumbing that runs without network:
 *   - ENS namehash derivation for `<ccy>-usd.data.eth` (must match on-chain)
 *   - raw Chainlink answer decode (decimals vary per fiat feed)
 *   - USD→fiat rate math, incl. the 18-decimal PHP feed
 */
// Stub the heavy/native deps pulled in transitively (these tests are pure logic).
jest.mock('@/services/rpc-pool', () => ({ poolRpcCall: jest.fn() }));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    getItem: jest.fn(() => Promise.resolve(null)),
    setItem: jest.fn(() => Promise.resolve()),
  },
}));

import { namehash, feedEnsName, isChainlinkFiat, FIAT_FEED_CODES } from '@/services/fiat-rates';
import { decChainlinkAnswer } from '@/services/abi';

describe('fiat-rates: ENS naming', () => {
  it('derives the documented ENS feed name', () => {
    expect(feedEnsName('GBP')).toBe('gbp-usd.data.eth');
    expect(feedEnsName('ars')).toBe('ars-usd.data.eth');
  });

  it('namehash matches the canonical value (cast namehash gbp-usd.data.eth)', () => {
    expect(namehash('gbp-usd.data.eth')).toBe(
      '0x04bfc12ced1dbf6aaefa3549071775f566946ad4826a694b659458bed0a3f6d2',
    );
  });

  it('namehash("") is the zero node', () => {
    expect(namehash('')).toBe('0x' + '0'.repeat(64));
  });

  it('recognizes supported Chainlink currencies (case-insensitive)', () => {
    expect(isChainlinkFiat('GBP')).toBe(true);
    expect(isChainlinkFiat('php')).toBe(true);
    expect(isChainlinkFiat('VND')).toBe(false); // no mainnet feed → hosted fallback
    expect(FIAT_FEED_CODES).toContain('ARS');
  });
});

/** Build a 5-word latestRoundData() return with `answer` in the 2nd slot. */
function roundData(answer: bigint): string {
  const word = (v: bigint) => (v & ((1n << 256n) - 1n)).toString(16).padStart(64, '0');
  return '0x' + word(1n) + word(answer) + word(0n) + word(0n) + word(1n);
}

describe('fiat-rates: rate math (USD → fiat = 10**dec / answer)', () => {
  it('decodes an 8-decimal feed (GBP/USD = 1.3408 → ~0.746 GBP per USD)', () => {
    const answer = decChainlinkAnswer(roundData(134080500n));
    const usdPerUnit = Number(answer) / 1e8;
    expect(usdPerUnit).toBeCloseTo(1.340805, 6);
    expect(1 / usdPerUnit).toBeCloseTo(0.7458, 3);
  });

  it('decodes an 18-decimal feed (PHP/USD ≈ 0.01646 → ~60.7 PHP per USD)', () => {
    const answer = decChainlinkAnswer(roundData(16461447290445776n));
    const usdPerUnit = Number(answer) / 1e18;
    expect(usdPerUnit).toBeCloseTo(0.01646, 5);
    expect(1 / usdPerUnit).toBeCloseTo(60.74, 1);
  });

  it('handles a tiny rate (ARS/USD = 0.00070019 → ~1428 ARS per USD)', () => {
    const answer = decChainlinkAnswer(roundData(70019n));
    const usdPerUnit = Number(answer) / 1e8;
    expect(1 / usdPerUnit).toBeCloseTo(1428.2, 1);
  });
});
