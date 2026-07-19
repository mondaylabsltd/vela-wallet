/**
 * Tests for batch-send — the pure call-batch builders behind the two advanced
 * send modes (① split: one token → many recipients, ② multiSelect: many tokens → one
 * recipient). Covers calldata/value shape, decimal scaling, totals, validation
 * guards, and the selectable predicate.
 */
import {
  encodeErc20Transfer,
  buildTransferCall,
  buildSplitCalls,
  sumSplitBaseUnits,
  buildMultiTokenCalls,
  isMultiSelectable,
  toMultiTokenSpecs,
  selectAllValuable,
  reserveNativeGas,
  reserveFeeToken,
  maxNativeSendable,
  BatchSendError,
  type SplitRecipient,
  type MultiTokenSpec,
} from '@/services/batch-send';
import { toBaseUnits, fromBaseUnits } from '@/services/eip681';
import type { APIToken } from '@/models/types';

const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';
const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';

function token(over: Partial<APIToken> = {}): APIToken {
  return {
    network: 'polygon', chainName: 'Polygon', symbol: 'USDT', balance: '10',
    decimals: 6, logo: null, name: 'Tether', tokenAddress: USDC, priceUsd: 1, spam: false,
    ...over,
  };
}

describe('encodeErc20Transfer', () => {
  it('encodes selector + padded address (lowercased) + amount', () => {
    const data = encodeErc20Transfer(A, 1_000_000n);
    expect(data).toBe(
      '0xa9059cbb' +
      '000000000000000000000000' + '1111111111111111111111111111111111111111' +
      '00000000000000000000000000000000000000000000000000000000000f4240',
    );
  });

  it('lowercases a checksummed recipient', () => {
    const data = encodeErc20Transfer(USDC, 1n);
    expect(data).toContain('a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48');
  });

  it('rejects a malformed recipient', () => {
    expect(() => encodeErc20Transfer('0x123', 1n)).toThrow(BatchSendError);
  });

  it('rejects an amount that overflows uint256', () => {
    expect(() => encodeErc20Transfer(A, 2n ** 256n)).toThrow(/uint256/);
  });
});

describe('buildTransferCall', () => {
  it('builds a native transfer (value carries the amount, no calldata)', () => {
    expect(buildTransferCall(null, A, 5n)).toEqual({ to: A, value: '0x5', data: '0x' });
  });

  it('builds an ERC-20 transfer (zero value, calldata carries it)', () => {
    const call = buildTransferCall(USDC, A, 1_000_000n);
    expect(call.to).toBe(USDC);
    expect(call.value).toBe('0x0');
    expect(call.data.startsWith('0xa9059cbb')).toBe(true);
  });

  it('rejects a zero or negative amount', () => {
    expect(() => buildTransferCall(null, A, 0n)).toThrow(BatchSendError);
    expect(() => buildTransferCall(USDC, A, -1n)).toThrow(BatchSendError);
  });

  it('rejects malformed recipient / token addresses', () => {
    expect(() => buildTransferCall(null, 'nope', 1n)).toThrow(BatchSendError);
    expect(() => buildTransferCall('0xbad', A, 1n)).toThrow(BatchSendError);
  });
});

describe('buildSplitCalls (① one token → many recipients)', () => {
  it('builds one transfer per recipient, scaling by decimals', () => {
    const recipients: SplitRecipient[] = [
      { address: A, amount: '1.5' },
      { address: B, amount: '2' },
    ];
    const calls = buildSplitCalls({ tokenAddress: USDC, decimals: 6 }, recipients);
    expect(calls).toHaveLength(2);
    // 1.5 USDT @ 6dp = 1_500_000 = 0x16e360
    expect(calls[0].data.endsWith((1_500_000).toString(16).padStart(64, '0'))).toBe(true);
    expect(calls[1].data.endsWith((2_000_000).toString(16).padStart(64, '0'))).toBe(true);
  });

  it('supports a native token split', () => {
    const calls = buildSplitCalls({ tokenAddress: null, decimals: 18 }, [{ address: A, amount: '1' }]);
    expect(calls[0]).toEqual({ to: A, value: '0x' + (10n ** 18n).toString(16), data: '0x' });
  });

  it('throws on an empty recipient list', () => {
    expect(() => buildSplitCalls({ tokenAddress: USDC, decimals: 6 }, [])).toThrow(BatchSendError);
  });
});

describe('sumSplitBaseUnits', () => {
  it('sums human amounts in base units', () => {
    const recipients: SplitRecipient[] = [
      { address: A, amount: '1.5' },
      { address: B, amount: '0.25' },
    ];
    expect(sumSplitBaseUnits(recipients, 6)).toBe(1_750_000n);
  });
});

describe('buildMultiTokenCalls (② many tokens → one recipient)', () => {
  it('routes every token to the same recipient', () => {
    const tokens: MultiTokenSpec[] = [
      { tokenAddress: USDC, decimals: 6, amount: '10' },
      { tokenAddress: null, decimals: 18, amount: '0.5' },
    ];
    const calls = buildMultiTokenCalls(A, tokens);
    expect(calls).toHaveLength(2);
    expect(calls[0].to).toBe(USDC); // ERC-20 call targets the token contract
    expect(calls[1]).toEqual({ to: A, value: '0x' + (5n * 10n ** 17n).toString(16), data: '0x' });
  });

  it('throws on an empty token list', () => {
    expect(() => buildMultiTokenCalls(A, [])).toThrow(BatchSendError);
  });
});

describe('isMultiSelectable', () => {
  it('includes a held, non-spam token', () => {
    expect(isMultiSelectable(token({ balance: '5' }))).toBe(true);
  });

  it('excludes spam and zero-balance tokens', () => {
    expect(isMultiSelectable(token({ spam: true }))).toBe(false);
    expect(isMultiSelectable(token({ balance: '0' }))).toBe(false);
  });

  it('with requireValue, excludes tokens with no known USD price', () => {
    expect(isMultiSelectable(token({ priceUsd: null }), true)).toBe(false);
    expect(isMultiSelectable(token({ priceUsd: 1 }), true)).toBe(true);
  });
});

describe('toMultiTokenSpecs', () => {
  it('maps each token to its full-balance multiSelect spec (native vs ERC-20)', () => {
    const specs = toMultiTokenSpecs([
      token({ tokenAddress: USDC, decimals: 6, balance: '12.5' }),
      token({ tokenAddress: null, decimals: 18, balance: '0.3' }),
    ]);
    expect(specs).toEqual([
      { tokenAddress: USDC, decimals: 6, amount: '12.5' },
      { tokenAddress: null, decimals: 18, amount: '0.3' },
    ]);
  });

  it('feeds straight into buildMultiTokenCalls', () => {
    const calls = buildMultiTokenCalls(A, toMultiTokenSpecs([token({ tokenAddress: USDC, decimals: 6, balance: '1' })]));
    expect(calls[0].to).toBe(USDC);
    expect(calls[0].data.endsWith((1_000_000).toString(16).padStart(64, '0'))).toBe(true);
  });
});

describe('selectAllValuable', () => {
  it('keeps only held, non-spam, priced tokens', () => {
    const list = [
      token({ symbol: 'USDT', balance: '5', priceUsd: 1 }),    // kept
      token({ symbol: 'SPAM', balance: '999', spam: true }),   // dropped: spam
      token({ symbol: 'ZERO', balance: '0', priceUsd: 1 }),    // dropped: no balance
      token({ symbol: 'NOPX', balance: '5', priceUsd: null }), // dropped: no price
    ];
    expect(selectAllValuable(list).map((t) => t.symbol)).toEqual(['USDT']);
  });
});

describe('reserveNativeGas (native multiSelect keeps gas for the EntryPoint prefund)', () => {
  const native: MultiTokenSpec = { tokenAddress: null, decimals: 18, amount: '1' };       // 1e18
  const erc20: MultiTokenSpec = { tokenAddress: USDC, decimals: 6, amount: '10' };

  it('trims only the native line by the reserve', () => {
    const out = reserveNativeGas([erc20, native], 2n * 10n ** 17n); // reserve 0.2
    expect(out[0]).toEqual(erc20);                                   // ERC-20 untouched
    expect(out[1]).toEqual({ tokenAddress: null, decimals: 18, amount: '0.8' });
  });

  it('drops the native line if the balance cannot cover the reserve', () => {
    const out = reserveNativeGas([erc20, native], 5n * 10n ** 18n); // reserve 5 > 1
    expect(out).toEqual([erc20]);
  });

  it('is a no-op for a non-positive reserve (e.g. Tempo)', () => {
    expect(reserveNativeGas([erc20, native], 0n)).toEqual([erc20, native]);
  });
});

describe('maxNativeSendable (single-send "Max" = balance − reserve, string-exact)', () => {
  const DEC = 18;
  const wei = (s: string) => toBaseUnits(s, DEC);

  // The reported bug: the old `Number(maxWei)/1e18 + toFixed(18)` path produced
  // values like 2500.546999999999570719 / 1.497000000000000108 that overshoot
  // balance−reserve and trip the "insufficient for gas" guard. The invariant
  // below is what makes the guard pass: toBaseUnits(max) + reserve === balance.
  const reserve = 3_000_000_000_000_000n; // 0.003 (≈ 3× a typical gas estimate)
  for (const bal of ['1.5', '2500.55', '12345.6789', '0.5', '100.123456789012345678']) {
    it(`round-trips exactly for balance ${bal} (no float garbage)`, () => {
      const max = maxNativeSendable(wei(bal), reserve, DEC);
      // Exact: the sent amount plus the reserve equals the whole balance.
      expect(toBaseUnits(max, DEC) + reserve).toBe(wei(bal));
      // Never overshoots balance − reserve (the old lossy path did).
      expect(toBaseUnits(max, DEC)).toBe(wei(bal) - reserve);
      // No trailing float-garbage run of zeros+digits.
      expect(max).not.toMatch(/0{6,}\d+$/);
    });
  }

  it('returns "0" when the balance cannot even cover the reserve', () => {
    expect(maxNativeSendable(wei('0.001'), reserve, DEC)).toBe('0');
    expect(maxNativeSendable(reserve, reserve, DEC)).toBe('0'); // exactly equal → nothing left
  });

  it('respects non-18-decimal tokens (no hardcoded 1e18)', () => {
    // 6-decimal coin, balance 5.0, reserve 1.25 → 3.75
    expect(maxNativeSendable(5_000_000n, 1_250_000n, 6)).toBe('3.75');
  });
});

describe('reserveFeeToken (an in-band ERC-20 fee asset remains available for reimbursement)', () => {
  const PATHUSD = '0x20c0000000000000000000000000000000000000';
  const pathUsd: MultiTokenSpec = { tokenAddress: PATHUSD, decimals: 6, amount: '1' }; // 1e6 units
  const otherTip20: MultiTokenSpec = { tokenAddress: USDC, decimals: 6, amount: '10' };

  it('trims only the fee-token (pathUSD) line — other TIP-20s pay no gas and pass through', () => {
    const out = reserveFeeToken([otherTip20, pathUsd], PATHUSD, 200_000n); // reserve 0.2 pathUSD
    expect(out[0]).toEqual(otherTip20);
    expect(out[1]).toEqual({ tokenAddress: PATHUSD, decimals: 6, amount: '0.8' });
  });

  it('matches the fee token case-insensitively', () => {
    const out = reserveFeeToken([pathUsd], PATHUSD.toUpperCase(), 200_000n);
    expect(out[0].amount).toBe('0.8');
  });

  it('drops the pathUSD line if its whole balance is needed for gas', () => {
    const out = reserveFeeToken([otherTip20, pathUsd], PATHUSD, 5_000_000n); // reserve 5 > 1
    expect(out).toEqual([otherTip20]);
  });

  it('is a no-op when pathUSD is not in the selection', () => {
    expect(reserveFeeToken([otherTip20], PATHUSD, 200_000n)).toEqual([otherTip20]);
  });

  it('is a no-op for a non-positive reserve', () => {
    expect(reserveFeeToken([otherTip20, pathUsd], PATHUSD, 0n)).toEqual([otherTip20, pathUsd]);
  });
});

describe('full-balance multiSelect precision (round-trip)', () => {
  // A swept "full balance" is the token's exact raw amount fed through
  // fromBaseUnits (what produces APIToken.balance) → toBaseUnits. It must be
  // lossless, or a multiSelect would leave dust / over-send.
  it.each([
    [1n, 18],
    [31_743_219_870_000_000_000n, 18],
    [123_456n, 6],
    [10n ** 30n + 7n, 18],
    [999_999_999n, 0],
  ])('round-trips raw %s @ %i dp', (raw, dec) => {
    expect(toBaseUnits(fromBaseUnits(raw as bigint, dec as number), dec as number)).toBe(raw);
  });
});
