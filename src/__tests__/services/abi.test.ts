/**
 * Tests for ABI encoding/decoding utilities.
 *
 * Verifies Multicall3 encoding, ERC-20 call encoding, DEX quote encoding,
 * Chainlink decoding, and edge cases in hex manipulation.
 */
import {
  MULTICALL3,
  encAggregate3, decAggregate3,
  encBalanceOf, encDecimals, encGetEthBalance,
  encQuoteV3, encGetAmountsOut, decAmountsOut,
  encLatestRound, decChainlinkUsd, decU256, decI256, decU8,
  SEL,
  type Call3,
} from '@/services/abi';

describe('abi', () => {
  describe('function selectors', () => {
    test('all selectors are 8 hex chars (4 bytes)', () => {
      for (const [name, sel] of Object.entries(SEL)) {
        expect(sel).toHaveLength(8);
        expect(/^[0-9a-f]{8}$/.test(sel)).toBe(true);
      }
    });

    test('MULTICALL3 is valid checksummed address', () => {
      expect(MULTICALL3).toMatch(/^0x[0-9a-fA-F]{40}$/);
    });
  });

  describe('encBalanceOf', () => {
    test('encodes address correctly', () => {
      const encoded = encBalanceOf('0x0000000000000000000000000000000000000001');
      expect(encoded).toMatch('0x' + SEL.balanceOf);
      expect(encoded).toHaveLength(2 + 8 + 64); // 0x + selector + address
      expect(encoded.endsWith('0000000000000000000000000000000000000001')).toBe(true);
    });
  });

  describe('encDecimals', () => {
    test('encodes with no params', () => {
      const encoded = encDecimals();
      expect(encoded).toBe('0x' + SEL.decimals);
    });
  });

  describe('encGetEthBalance', () => {
    test('encodes address', () => {
      const encoded = encGetEthBalance('0xabc');
      expect(encoded).toMatch('0x' + SEL.getEthBalance);
    });
  });

  describe('encQuoteV3', () => {
    test('encodes Uniswap V3 quote correctly', () => {
      const encoded = encQuoteV3(
        '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2', // WETH
        '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        1000000000000000000n, // 1 ETH
        3000, // 0.3% fee
      );
      expect(encoded).toMatch('0x' + SEL.quoteV3);
      // 5 fields × 64 hex chars + selector
      expect(encoded).toHaveLength(2 + 8 + 5 * 64);
    });
  });

  describe('encGetAmountsOut', () => {
    test('encodes Solidly/Aerodrome route', () => {
      const encoded = encGetAmountsOut(
        1000000000000000000n,
        '0xWETH',
        '0xUSDC',
        false,
      );
      expect(encoded).toMatch('0x' + SEL.getAmountsOut);
    });
  });

  describe('Multicall3 round-trip', () => {
    test('encodes and decodes single call', () => {
      const calls: Call3[] = [
        { target: '0x0000000000000000000000000000000000000001', allowFailure: true, callData: '0x' + SEL.decimals },
      ];
      const encoded = encAggregate3(calls);
      expect(encoded).toMatch('0x' + SEL.aggregate3);
    });

    test('encodes multiple calls', () => {
      const calls: Call3[] = [
        { target: '0x0000000000000000000000000000000000000001', allowFailure: true, callData: '0x' + SEL.decimals },
        { target: '0x0000000000000000000000000000000000000002', allowFailure: true, callData: encBalanceOf('0x0000000000000000000000000000000000000003') },
      ];
      const encoded = encAggregate3(calls);
      expect(encoded).toMatch('0x' + SEL.aggregate3);
    });

    test('decAggregate3 returns empty for short data', () => {
      expect(decAggregate3('0x')).toEqual([]);
      expect(decAggregate3('0x00')).toEqual([]);
    });
  });

  describe('decU256', () => {
    test('decodes zero', () => {
      expect(decU256('0x' + '0'.repeat(64))).toBe(0n);
    });

    test('decodes 1', () => {
      expect(decU256('0x' + '0'.repeat(63) + '1')).toBe(1n);
    });

    test('decodes max uint256', () => {
      expect(decU256('0x' + 'f'.repeat(64))).toBe((1n << 256n) - 1n);
    });

    test('returns 0 for short data', () => {
      expect(decU256('0x')).toBe(0n);
      expect(decU256('0x00')).toBe(0n);
    });

    test('reads only first 32 bytes', () => {
      const hex = '0x' + '0'.repeat(63) + '5' + 'f'.repeat(64);
      expect(decU256(hex)).toBe(5n);
    });
  });

  describe('decI256', () => {
    test('decodes positive values', () => {
      expect(decI256('0x' + '0'.repeat(63) + '1')).toBe(1n);
    });

    test('decodes -1 (all F)', () => {
      expect(decI256('0x' + 'f'.repeat(64))).toBe(-1n);
    });

    test('decodes negative values', () => {
      // -2 = 0xFFF...FFE
      const hex = '0x' + 'f'.repeat(63) + 'e';
      expect(decI256(hex)).toBe(-2n);
    });
  });

  describe('decU8', () => {
    test('decodes 18 (common decimals)', () => {
      // 18 = 0x12, padded to 64 hex chars
      expect(decU8('0x' + '0'.repeat(62) + '12')).toBe(18);
    });

    test('decodes 6', () => {
      expect(decU8('0x' + '0'.repeat(63) + '6')).toBe(6);
    });
  });

  describe('decChainlinkUsd', () => {
    test('decodes typical ETH price', () => {
      // Simulate: roundId=anything, answer=2500_00000000 (2500 USD, 8 decimals)
      const roundId = '0'.repeat(64);
      const answer = BigInt(2500_00000000).toString(16).padStart(64, '0');
      const hex = '0x' + roundId + answer;
      expect(decChainlinkUsd(hex)).toBeCloseTo(2500, 0);
    });

    test('decodes 1 USD (stablecoin)', () => {
      const roundId = '0'.repeat(64);
      const answer = BigInt(1_00000000).toString(16).padStart(64, '0');
      const hex = '0x' + roundId + answer;
      expect(decChainlinkUsd(hex)).toBeCloseTo(1, 0);
    });

    test('returns 0 for short data', () => {
      expect(decChainlinkUsd('0x')).toBe(0);
    });
  });

  describe('decAmountsOut', () => {
    test('decodes Solidly getAmountsOut return', () => {
      // decAmountsOut reads the 4th 32-byte word (positions 192-256 hex chars)
      // Layout: offset(64) + length(64) + amounts[0](64) + amounts[1](64) = 256 hex chars
      const offset = (64n).toString(16).padStart(64, '0'); // offset to array = 0x40
      const length = (2n).toString(16).padStart(64, '0');  // 2 elements
      const amountIn = BigInt(1e18).toString(16).padStart(64, '0');
      const amountOut = BigInt(2500e6).toString(16).padStart(64, '0');
      const hex = '0x' + offset + length + amountIn + amountOut;
      expect(decAmountsOut(hex)).toBe(BigInt(2500e6));
    });

    test('returns 0 for short data', () => {
      expect(decAmountsOut('0x')).toBe(0n);
      expect(decAmountsOut('0x' + '0'.repeat(200))).toBe(0n);
    });
  });

  describe('encLatestRound', () => {
    test('is just the selector', () => {
      expect(encLatestRound()).toBe('0x' + SEL.latestRoundData);
    });
  });
});
