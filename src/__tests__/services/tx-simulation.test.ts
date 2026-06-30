/**
 * Tests for the revert-reason parser used by the client-side pre-check.
 */
import {
  parseRevertReason, serializeAssetSim, deserializeAssetSim, type AssetSimResult,
} from '@/services/tx-simulation';

/** ABI-encode an Error(string) revert payload. */
function encodeErrorString(msg: string): string {
  const hex = Buffer.from(msg, 'utf8').toString('hex');
  const len = Buffer.from(msg, 'utf8').length;
  const padded = hex.padEnd(Math.ceil(hex.length / 64) * 64, '0');
  return '0x08c379a0'
    + (32).toString(16).padStart(64, '0') // offset
    + len.toString(16).padStart(64, '0')  // length
    + padded;                              // bytes
}

describe('parseRevertReason', () => {
  test('decodes Error(string) from error.data', () => {
    expect(parseRevertReason({ data: encodeErrorString('insufficient allowance') }))
      .toBe('insufficient allowance');
  });

  test('decodes Error(string) from nested error.data.data', () => {
    expect(parseRevertReason({ data: { data: encodeErrorString('STF') } })).toBe('STF');
  });

  test('recognizes Panic(uint256)', () => {
    expect(parseRevertReason({ data: '0x4e487b710000000000000000000000000000000000000000000000000000000000000011' }))
      .toMatch(/Panic/);
  });

  test('falls back to a cleaned message', () => {
    expect(parseRevertReason({ message: 'execution reverted: ERC20: transfer amount exceeds balance' }))
      .toBe('ERC20: transfer amount exceeds balance');
  });

  test('returns undefined for a bare "execution reverted"', () => {
    expect(parseRevertReason({ message: 'execution reverted' })).toBeUndefined();
  });

  test('returns undefined when there is nothing useful', () => {
    expect(parseRevertReason({})).toBeUndefined();
    expect(parseRevertReason({ data: '0x' })).toBeUndefined();
  });
});

describe('asset-sim persistence (serialize/deserialize)', () => {
  const live: AssetSimResult = {
    ok: true,
    underfundedNative: false,
    engine: 'rpc',
    changes: [
      { kind: 'erc20', token: '0xabc', delta: -123456789012345678901234567890n, symbol: 'USDC', decimals: 6 },
      { kind: 'native', delta: 5_000000000000000000n, symbol: 'ETH', decimals: 18 },
      { kind: 'erc20', token: '0xdef', delta: 7n, unverified: true },
    ],
  };

  test('round-trips bigint deltas through JSON-safe storage', () => {
    const stored = serializeAssetSim(live);
    // Persisted form must be JSON-serializable (no bigints) — AsyncStorage holds strings.
    expect(() => JSON.stringify(stored)).not.toThrow();
    expect(stored.changes![0].delta).toBe('-123456789012345678901234567890');

    const round = deserializeAssetSim(JSON.parse(JSON.stringify(stored)));
    expect(round).toEqual(live);
    expect(round.changes![0].delta).toBe(-123456789012345678901234567890n);
    expect(round.changes![2].unverified).toBe(true);
  });

  test('preserves a null changes set (degraded sim) and the revert signal', () => {
    const reverted: AssetSimResult = { ok: false, revertReason: 'STF', engine: 'none', changes: null };
    expect(deserializeAssetSim(serializeAssetSim(reverted))).toEqual(reverted);
  });

  test('a corrupt stored delta reads as 0n rather than throwing', () => {
    const round = deserializeAssetSim({ ok: true, engine: 'rpc', changes: [{ kind: 'native', delta: 'not-a-number' }] });
    expect(round.changes![0].delta).toBe(0n);
  });
});
