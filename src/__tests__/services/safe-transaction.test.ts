/**
 * Tests for safe-transaction service.
 *
 * Tests the pure functions that build calldata, compute hashes, and format values.
 * RPC-dependent functions are not tested here (require network mocking).
 */

// Mock react-native transitive dependencies
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
import {
  formatWeiToEth, calcMaxFeePerGas, GAS_TIER_MULTIPLIERS,
  encodeErc20Transfer, buildExecuteCallData, buildMultiSendExecuteCallData, buildInitCode,
  parseHexUInt64, parseExistingUserOpHash,
} from '@/services/safe-transaction';
import type { GasTier } from '@/services/safe-transaction';
import { functionSelector } from '@/services/eth-crypto';

/** Uint8Array → lowercase hex (no 0x), for golden-vector assertions. */
const hex = (u: Uint8Array) => Array.from(u).map((b) => b.toString(16).padStart(2, '0')).join('');
/** The i-th 32-byte ABI word after a 4-byte selector, as hex. */
const word = (u: Uint8Array, i: number) => hex(u).slice(8 + i * 64, 8 + (i + 1) * 64);
const zeroWord = '0'.repeat(64);

describe('safe-transaction', () => {
  describe('formatWeiToEth', () => {
    test('formats zero', () => {
      expect(formatWeiToEth(0n)).toBe('0');
    });

    test('formats very small amounts', () => {
      expect(formatWeiToEth(1n)).toBe('< 0.000001');
      expect(formatWeiToEth(999n)).toBe('< 0.000001');
    });

    test('formats small amounts with 6 decimals', () => {
      // 0.001 ETH = 1e15 wei
      const result = formatWeiToEth(1_000_000_000_000n); // 0.000001 ETH
      expect(result).toBe('0.000001');
    });

    test('formats medium amounts with 4 decimals', () => {
      // 0.5 ETH = 5e17 wei
      const result = formatWeiToEth(500_000_000_000_000_000n);
      expect(result).toBe('0.5000');
    });

    test('formats amounts >= 1 with 3 decimals', () => {
      // 1.5 ETH
      const result = formatWeiToEth(1_500_000_000_000_000_000n);
      expect(result).toBe('1.500');
    });

    test('formats large amounts', () => {
      // 100 ETH
      const result = formatWeiToEth(100_000_000_000_000_000_000n);
      expect(result).toBe('100.000');
    });

    test('handles typical gas fees (0.001-0.01 ETH)', () => {
      // 0.005 ETH = 5e15 wei
      const result = formatWeiToEth(5_000_000_000_000_000n);
      expect(result).toBe('0.0050');
    });
  });

  // --- calcMaxFeePerGas: maxFee = gasPrice × speedTier × BUNDLER_MARGIN (2.0×) ---
  // BUNDLER_MARGIN_PERCENT = 100 → 2× markup → relayer fee ≈ network fee, i.e. the
  // margin over the outer gas price (gasPrice × tier) is a constant 100% for every tier.

  describe('calcMaxFeePerGas', () => {
    const gasPrice = 10_000_000_000n; // 10 gwei

    test('standard tier: gasPrice × 1.2 × 2.0 = 24 gwei', () => {
      expect(calcMaxFeePerGas(gasPrice, 'standard')).toBe(24_000_000_000n);
    });

    test('slow tier: gasPrice × 1.1 × 2.0 = 22 gwei', () => {
      expect(calcMaxFeePerGas(gasPrice, 'slow')).toBe(22_000_000_000n);
    });

    test('rapid tier: gasPrice × 1.5 × 2.0 = 30 gwei', () => {
      expect(calcMaxFeePerGas(gasPrice, 'rapid')).toBe(30_000_000_000n);
    });

    test('fast tier: gasPrice × 2.0 × 2.0 = 40 gwei', () => {
      expect(calcMaxFeePerGas(gasPrice, 'fast')).toBe(40_000_000_000n);
    });

    test('default tier is standard', () => {
      expect(calcMaxFeePerGas(gasPrice)).toBe(calcMaxFeePerGas(gasPrice, 'standard'));
    });

    test('floor: gasPrice=0 → 1 wei', () => {
      expect(calcMaxFeePerGas(0n)).toBe(1n);
    });

    test('margin is constant 100% across all tiers', () => {
      // maxFee = outerGasPrice × 2.0, so margin = 2.0 - 1 = 1.0, for ALL tiers
      const tiers: GasTier[] = ['slow', 'standard', 'rapid', 'fast'];
      for (const tier of tiers) {
        const m = GAS_TIER_MULTIPLIERS[tier];
        const outerGasPrice = (gasPrice * m.num) / m.den; // gasPrice × speedTier
        const maxFee = calcMaxFeePerGas(gasPrice, tier);
        const margin = Number(maxFee - outerGasPrice) / Number(outerGasPrice);
        expect(margin).toBeCloseTo(1.0, 5); // always 100%
      }
    });

    test('user cost scales with tier (faster = more expensive)', () => {
      const slow = calcMaxFeePerGas(gasPrice, 'slow');
      const std = calcMaxFeePerGas(gasPrice, 'standard');
      const rapid = calcMaxFeePerGas(gasPrice, 'rapid');
      const fast = calcMaxFeePerGas(gasPrice, 'fast');
      expect(slow < std).toBe(true);
      expect(std < rapid).toBe(true);
      expect(rapid < fast).toBe(true);
    });
  });

  // --- calldata encoders (the load-bearing money-path ABI encoding) ----------
  // These assert the selector + argument layout, locking the composition so a
  // re-ordered field or wrong selector can't slip through silently.
  describe('encodeErc20Transfer', () => {
    const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';

    test('matches the canonical transfer(address,uint256) layout', () => {
      const out = encodeErc20Transfer(USDC, 1_000_000n);
      expect(out.length).toBe(4 + 32 + 32);
      expect(hex(out).slice(0, 8)).toBe('a9059cbb'); // well-known ERC-20 selector
      expect(hex(out).slice(0, 8)).toBe(hex(functionSelector('transfer(address,uint256)')));
      expect(word(out, 0)).toBe(USDC.slice(2).toLowerCase().padStart(64, '0')); // recipient (lowercased, left-padded)
      expect(word(out, 1)).toBe((1_000_000).toString(16).padStart(64, '0')); // amount
    });

    test('zero amount encodes a zero word', () => {
      expect(word(encodeErc20Transfer(USDC, 0n), 1)).toBe(zeroWord);
    });
  });

  describe('buildExecuteCallData (Safe executeUserOp)', () => {
    const TO = '0x1111111111111111111111111111111111111111';

    test('empty data → selector + 5 words, CALL operation, 0x80 data offset', () => {
      const out = buildExecuteCallData(TO, '0x0', new Uint8Array(0));
      expect(out.length).toBe(4 + 32 * 5);
      expect(hex(out).slice(0, 8)).toBe(hex(functionSelector('executeUserOp(address,uint256,bytes,uint8)')));
      expect(word(out, 0)).toBe(TO.slice(2).padStart(64, '0')); // to
      expect(word(out, 1)).toBe(zeroWord); // value
      expect(word(out, 2)).toBe((128).toString(16).padStart(64, '0')); // bytes offset = 4*32
      expect(word(out, 3)).toBe(zeroWord); // operation = CALL
      expect(word(out, 4)).toBe(zeroWord); // data length = 0
    });

    test('non-empty data is length-prefixed and zero-padded to 32 bytes', () => {
      const out = buildExecuteCallData(TO, '0x0', new Uint8Array([0xde, 0xad]));
      expect(word(out, 4)).toBe((2).toString(16).padStart(64, '0')); // data length = 2
      expect(out.length).toBe(4 + 32 * 5 + 32); // 2 bytes padded to a full word
      expect(hex(out).slice(8 + 5 * 64, 8 + 5 * 64 + 4)).toBe('dead'); // payload then padding
    });
  });

  describe('buildMultiSendExecuteCallData', () => {
    test('wraps the batch as a DELEGATECALL into MultiSend', () => {
      const out = buildMultiSendExecuteCallData([
        { to: '0x1111111111111111111111111111111111111111', value: '0x0', data: new Uint8Array(0) },
        { to: '0x2222222222222222222222222222222222222222', value: '0x0', data: new Uint8Array([0xaa]) },
      ]);
      // Outer call is still executeUserOp, but operation MUST be DELEGATECALL (1).
      expect(hex(out).slice(0, 8)).toBe(hex(functionSelector('executeUserOp(address,uint256,bytes,uint8)')));
      expect(word(out, 2)).toBe((128).toString(16).padStart(64, '0')); // data offset
      expect(word(out, 3)).toBe((1).toString(16).padStart(64, '0')); // DELEGATECALL — the load-bearing bit
      // Inner payload carries the multiSend(bytes) selector somewhere after the header.
      expect(hex(out)).toContain(hex(functionSelector('multiSend(bytes)')));
    });
  });

  describe('buildInitCode (createProxyWithNonce)', () => {
    const PUBKEY = '0x04' + 'aa'.repeat(32) + 'bb'.repeat(32); // 04 ++ 32-byte x ++ 32-byte y

    test('is factory-address ++ createProxyWithNonce(...) and is deterministic', () => {
      const out = buildInitCode(PUBKEY);
      // 4337 initCode = 20-byte factory address, then the factory calldata.
      const selectorAfterFactory = hex(out).slice(40, 48); // skip 20-byte (40 hex) factory
      expect(selectorAfterFactory).toBe(hex(functionSelector('createProxyWithNonce(address,bytes,uint256)')));
      // Same passkey must always derive the same wallet (salt = keccak(x,y)).
      expect(hex(buildInitCode(PUBKEY))).toBe(hex(out));
    });
  });

  // --- pure parsers ----------------------------------------------------------
  describe('parseExistingUserOpHash', () => {
    test('extracts the in-flight hash from the bundler marker', () => {
      expect(parseExistingUserOpHash('AA25 invalid nonce [existingHash:0xabc123]')).toBe('0xabc123');
      expect(parseExistingUserOpHash('[existingHash:0xDEADbeef] and more')).toBe('0xDEADbeef');
    });
    test('returns null when the marker is absent or malformed', () => {
      expect(parseExistingUserOpHash('some unrelated bundler error')).toBeNull();
      expect(parseExistingUserOpHash('[existingHash:nothex]')).toBeNull();
      expect(parseExistingUserOpHash('')).toBeNull();
    });
  });

  describe('parseHexUInt64', () => {
    test('coerces hex (with/without 0x) to bigint, empty/undefined → 0n', () => {
      expect(parseHexUInt64(undefined)).toBe(0n);
      expect(parseHexUInt64('')).toBe(0n);
      expect(parseHexUInt64('0x')).toBe(0n);
      expect(parseHexUInt64('0x0')).toBe(0n);
      expect(parseHexUInt64('0xff')).toBe(255n);
      expect(parseHexUInt64('ff')).toBe(255n);
    });
  });
});
