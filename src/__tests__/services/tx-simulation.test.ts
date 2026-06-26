/**
 * Tests for the revert-reason parser used by the client-side pre-check.
 */
import { parseRevertReason } from '@/services/tx-simulation';

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
