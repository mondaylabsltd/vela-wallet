/**
 * Pure asset-change core: deriveAssetDeltas + formatTokenAmount.
 * No network, no mocks — just log-in, delta-out.
 */
import {
  deriveAssetDeltas, formatTokenAmount,
  TRANSFER_TOPIC, NATIVE_TRANSFER_SENTINEL, type SimLog,
} from '@/services/sim-assets';

const USER = '0x' + '11'.repeat(20);
const PEER = '0x' + '22'.repeat(20);
const USDC = '0x' + 'a0'.repeat(20);
const DAI = '0x' + 'b1'.repeat(20);

/** 20-byte address → 32-byte indexed topic. */
const topic = (addr: string) => '0x' + addr.replace(/^0x/, '').toLowerCase().padStart(64, '0');
/** uint256 → 32-byte data word. */
const word = (v: bigint) => '0x' + v.toString(16).padStart(64, '0');

function transfer(token: string, from: string, to: string, value: bigint): SimLog {
  return { address: token, topics: [TRANSFER_TOPIC, topic(from), topic(to)], data: word(value) };
}

describe('deriveAssetDeltas', () => {
  test('ERC-20 received → positive delta', () => {
    const d = deriveAssetDeltas([transfer(USDC, PEER, USER, 1_000_000n)], USER);
    expect(d).toEqual([{ kind: 'erc20', token: USDC, delta: 1_000_000n }]);
  });

  test('ERC-20 sent → negative delta', () => {
    const d = deriveAssetDeltas([transfer(USDC, USER, PEER, 1_000_000n)], USER);
    expect(d).toEqual([{ kind: 'erc20', token: USDC, delta: -1_000_000n }]);
  });

  test('native value move (sentinel sender) → native delta, no token', () => {
    const recv = deriveAssetDeltas([transfer(NATIVE_TRANSFER_SENTINEL, PEER, USER, 5n * 10n ** 17n)], USER);
    expect(recv).toEqual([{ kind: 'native', token: undefined, delta: 5n * 10n ** 17n }]);

    const send = deriveAssetDeltas([transfer(NATIVE_TRANSFER_SENTINEL, USER, PEER, 10n ** 18n)], USER);
    expect(send).toEqual([{ kind: 'native', token: undefined, delta: -(10n ** 18n) }]);
  });

  test('swap: token out (−) and token in (+) become two deltas', () => {
    const logs = [
      transfer(USDC, USER, PEER, 1_000_000n), // pay 1 USDC
      transfer(DAI, PEER, USER, 999_000_000_000_000_000n), // get ~1 DAI
    ];
    const d = deriveAssetDeltas(logs, USER);
    expect(d).toContainEqual({ kind: 'erc20', token: USDC, delta: -1_000_000n });
    expect(d).toContainEqual({ kind: 'erc20', token: DAI, delta: 999_000_000_000_000_000n });
    expect(d).toHaveLength(2);
  });

  test('multiple moves of the same token net together', () => {
    const logs = [
      transfer(USDC, PEER, USER, 3n),
      transfer(USDC, USER, PEER, 1n),
    ];
    expect(deriveAssetDeltas(logs, USER)).toEqual([{ kind: 'erc20', token: USDC, delta: 2n }]);
  });

  test('self-transfer (from === to === user) nets to zero and is dropped', () => {
    expect(deriveAssetDeltas([transfer(USDC, USER, USER, 500n)], USER)).toEqual([]);
  });

  test('logs that do not touch the user are ignored', () => {
    expect(deriveAssetDeltas([transfer(USDC, PEER, PEER, 999n)], USER)).toEqual([]);
  });

  test('is case-insensitive on the user address and topics', () => {
    const d = deriveAssetDeltas([transfer(USDC, PEER, USER.toUpperCase(), 7n)], USER.toLowerCase());
    expect(d).toEqual([{ kind: 'erc20', token: USDC, delta: 7n }]);
  });

  test('ERC-721 Transfer (4 topics, tokenId indexed) is excluded', () => {
    const nft: SimLog = {
      address: USDC,
      topics: [TRANSFER_TOPIC, topic(PEER), topic(USER), topic('0x' + '00'.repeat(19) + '07')],
      data: '0x',
    };
    expect(deriveAssetDeltas([nft], USER)).toEqual([]);
  });

  test('non-Transfer topic is ignored', () => {
    const other: SimLog = { address: USDC, topics: ['0xdeadbeef', topic(PEER), topic(USER)], data: word(1n) };
    expect(deriveAssetDeltas([other], USER)).toEqual([]);
  });

  test('zero-value transfers are ignored', () => {
    expect(deriveAssetDeltas([transfer(USDC, PEER, USER, 0n)], USER)).toEqual([]);
  });

  test('empty / malformed input is safe', () => {
    expect(deriveAssetDeltas([], USER)).toEqual([]);
    expect(deriveAssetDeltas([{ address: USDC, topics: [], data: '0x' }], USER)).toEqual([]);
    expect(deriveAssetDeltas([{ address: USDC, topics: [TRANSFER_TOPIC], data: '0x' }], USER)).toEqual([]);
  });
});

describe('formatTokenAmount', () => {
  test('groups whole numbers with commas', () => {
    expect(formatTokenAmount(1_500_000n, 0)).toBe('1,500,000');
  });

  test('formats 6-decimal token amounts', () => {
    expect(formatTokenAmount(1_500_250_000n, 6)).toBe('1,500.25');
  });

  test('trims trailing fractional zeros', () => {
    expect(formatTokenAmount(2_000_000n, 6)).toBe('2');
  });

  test('takes the absolute value (sign is the caller\'s job)', () => {
    expect(formatTokenAmount(-123n, 0)).toBe('123');
  });

  test('shows a marker for amounts below display precision', () => {
    expect(formatTokenAmount(1n, 18)).toBe('<0.000001');
  });

  test('zero is "0"', () => {
    expect(formatTokenAmount(0n, 18)).toBe('0');
  });
});
