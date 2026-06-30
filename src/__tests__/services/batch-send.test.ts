/**
 * Tests for batch-send — the pure call-batch builders behind the two advanced
 * send modes (① split: one token → many recipients, ② sweep: many tokens → one
 * recipient). Covers calldata/value shape, decimal scaling, totals, validation
 * guards, and the sweepable predicate.
 */
import {
  encodeErc20Transfer,
  buildTransferCall,
  buildSplitCalls,
  sumSplitBaseUnits,
  buildSweepCalls,
  isSweepable,
  toSweepTokens,
  selectAllValuable,
  BatchSendError,
  type SplitRecipient,
  type SweepToken,
} from '@/services/batch-send';
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

describe('buildSweepCalls (② many tokens → one recipient)', () => {
  it('routes every token to the same recipient', () => {
    const tokens: SweepToken[] = [
      { tokenAddress: USDC, decimals: 6, amount: '10' },
      { tokenAddress: null, decimals: 18, amount: '0.5' },
    ];
    const calls = buildSweepCalls(A, tokens);
    expect(calls).toHaveLength(2);
    expect(calls[0].to).toBe(USDC); // ERC-20 call targets the token contract
    expect(calls[1]).toEqual({ to: A, value: '0x' + (5n * 10n ** 17n).toString(16), data: '0x' });
  });

  it('throws on an empty token list', () => {
    expect(() => buildSweepCalls(A, [])).toThrow(BatchSendError);
  });
});

describe('isSweepable', () => {
  it('includes a held, non-spam token', () => {
    expect(isSweepable(token({ balance: '5' }))).toBe(true);
  });

  it('excludes spam and zero-balance tokens', () => {
    expect(isSweepable(token({ spam: true }))).toBe(false);
    expect(isSweepable(token({ balance: '0' }))).toBe(false);
  });

  it('with requireValue, excludes tokens with no known USD price', () => {
    expect(isSweepable(token({ priceUsd: null }), true)).toBe(false);
    expect(isSweepable(token({ priceUsd: 1 }), true)).toBe(true);
  });
});

describe('toSweepTokens', () => {
  it('maps each token to its full-balance sweep spec (native vs ERC-20)', () => {
    const specs = toSweepTokens([
      token({ tokenAddress: USDC, decimals: 6, balance: '12.5' }),
      token({ tokenAddress: null, decimals: 18, balance: '0.3' }),
    ]);
    expect(specs).toEqual([
      { tokenAddress: USDC, decimals: 6, amount: '12.5' },
      { tokenAddress: null, decimals: 18, amount: '0.3' },
    ]);
  });

  it('feeds straight into buildSweepCalls', () => {
    const calls = buildSweepCalls(A, toSweepTokens([token({ tokenAddress: USDC, decimals: 6, balance: '1' })]));
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
