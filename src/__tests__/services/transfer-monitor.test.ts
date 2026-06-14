/**
 * Behavioural tests for transfer-monitor's adaptive getLogs range-splitting.
 *
 * When an endpoint rejects a wide block span ("…limited to a 100 range"),
 * `deepScanChain` must transparently split into chunks within the stated cap
 * (or halve when no number is given) and still return every decoded transfer —
 * while surfacing genuine (non-range) errors so the caller can mark the chain
 * failed. `poolRpcCall` is mocked but the real `getLogsRangeCap` is exercised.
 */

// Mocks required for the real rpc-pool module tree to load under jest (same set
// as rpc-pool.test.ts: react-native is ESM and crashes the transform otherwise).
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    getItem: jest.fn(async () => null),
    setItem: jest.fn(async () => {}),
    removeItem: jest.fn(async () => {}),
  },
}));
jest.mock('@/modules/cloud-sync', () => ({
  get: jest.fn(async () => null),
  save: jest.fn(async () => {}),
  remove: jest.fn(async () => {}),
  syncNow: jest.fn(async () => {}),
}));
jest.mock('@/services/chain-registry', () => ({
  fetchChainInfo: jest.fn(async () => null),
}));

// Mock poolRpcCall but keep the real getLogsRangeCap so the message-parsing path
// is covered end-to-end.
const mockPoolRpcCall = jest.fn();
jest.mock('@/services/rpc-pool', () => ({
  ...jest.requireActual('@/services/rpc-pool'),
  poolRpcCall: (...args: any[]) => mockPoolRpcCall(...args),
}));

import { deepScanChain } from '@/services/transfer-monitor';

const TRANSFER_TOPIC = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';
const ADDR = '0x1111111111111111111111111111111111111111';
const SENDER = '0x2222222222222222222222222222222222222222';

function recipientTopic(addr: string): string {
  return '0x' + '0'.repeat(24) + addr.slice(2).toLowerCase();
}

/** A well-formed ERC-20 Transfer log addressed to ADDR, in `blockNum`. */
function makeLog(blockNum: number) {
  return {
    address: '0x' + 'a'.repeat(40),
    topics: [TRANSFER_TOPIC, recipientTopic(SENDER), recipientTopic(ADDR)],
    data: '0x' + (1000).toString(16).padStart(64, '0'),
    transactionHash: '0x' + blockNum.toString(16).padStart(64, '0'),
    blockNumber: '0x' + blockNum.toString(16),
    logIndex: '0x0',
  };
}

const LATEST = 1_000_000;
const hex = (n: number) => '0x' + n.toString(16);
const spanOf = (params: any) => parseInt(params[0].toBlock, 16) - parseInt(params[0].fromBlock, 16) + 1;

/** Build a poolRpcCall mock whose eth_getLogs rejects spans over `cap` with `errMsg`. */
function logsMock(cap: number, errMsg: string) {
  return async (method: string, params: any[]) => {
    if (method === 'eth_blockNumber') return { result: hex(LATEST) };
    if (method === 'eth_getBlockByNumber') return { result: { timestamp: '0x60000000' } };
    if (method === 'eth_getLogs') {
      if (spanOf(params) > cap) return { error: { code: -32000, message: errMsg } };
      // Within cap → one transfer in the chunk's first block.
      return { result: [makeLog(parseInt(params[0].fromBlock, 16))] };
    }
    return { result: null };
  };
}

function getLogsSpans(): number[] {
  return mockPoolRpcCall.mock.calls
    .filter((c) => c[0] === 'eth_getLogs')
    .map((c) => spanOf(c[1]));
}

beforeEach(() => {
  jest.clearAllMocks();
});

describe('transfer-monitor deepScanChain — adaptive getLogs splitting', () => {
  test('splits a too-wide span into <= stated-cap chunks and returns all transfers', async () => {
    // Arbitrum (250ms blocks) × 10min → ~2400-block window in one outer chunk,
    // far above the 100 cap, forcing a split.
    mockPoolRpcCall.mockImplementation(logsMock(100, 'eth_getLogs is limited to a 100 range'));

    const out = await deepScanChain(ADDR, 42161, 10);

    const spans = getLogsSpans();
    // One wide call errored; it was split into 25 sub-calls each within the cap.
    expect(spans.filter((s) => s > 100)).toHaveLength(1);
    expect(spans.filter((s) => s <= 100)).toHaveLength(25);
    expect(Math.max(...spans.filter((s) => s <= 100))).toBeLessThanOrEqual(100);
    // Every chunk yielded one transfer → all recovered, none dropped.
    expect(out).toHaveLength(25);
    expect(out.every((t) => t.from.toLowerCase() === SENDER.toLowerCase())).toBe(true);
  });

  test('halves repeatedly when the error states no number, until under the real cap', async () => {
    // Server gives a range error with no usable number → caller halves. Real cap
    // is 100, so halving must converge (every successful call within 100 blocks).
    mockPoolRpcCall.mockImplementation(logsMock(100, 'block range is too wide'));

    const out = await deepScanChain(ADDR, 42161, 10);

    const okSpans = getLogsSpans().filter((s) => s <= 100);
    expect(out.length).toBeGreaterThan(0);
    expect(okSpans.length).toBeGreaterThan(0);
    expect(Math.max(...okSpans)).toBeLessThanOrEqual(100);
    // Splitting actually happened (some calls exceeded the cap and were rejected).
    expect(getLogsSpans().some((s) => s > 100)).toBe(true);
  });

  test('propagates a genuine non-range error so the caller can fail the chain', async () => {
    // Ethereum (12s blocks) × 10min → ~50-block window, a single sub-cap call
    // that returns a real error which must NOT be swallowed by the split path.
    mockPoolRpcCall.mockImplementation(async (method: string) => {
      if (method === 'eth_blockNumber') return { result: hex(LATEST) };
      if (method === 'eth_getLogs') return { error: { code: -32603, message: 'internal error' } };
      return { result: null };
    });

    await expect(deepScanChain(ADDR, 1, 10)).rejects.toThrow('internal error');
  });

  test('honours a k-suffixed cap ("up to a 2K block range") without over-splitting', async () => {
    // 2K cap ≥ the ~2400 window after one halve-free split, so we expect a small
    // number of chunks, not hundreds — proves the suffix is parsed as 2000.
    mockPoolRpcCall.mockImplementation(
      logsMock(2000, 'You can make eth_getLogs requests with up to a 2K block range'),
    );

    const out = await deepScanChain(ADDR, 42161, 10);

    const okSpans = getLogsSpans().filter((s) => s <= 2000);
    expect(out.length).toBeGreaterThan(0);
    expect(Math.max(...okSpans)).toBeLessThanOrEqual(2000);
    // 2400-block window at a 2000 cap → 2 chunks, not 24 (which a cap of 2 implies).
    expect(okSpans.length).toBeLessThanOrEqual(3);
  });
});
