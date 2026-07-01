/**
 * Behavioural tests for transfer-monitor's bounded recent-window scanner.
 *
 * `scanRecentTransfers` queries the last N blocks in a single `eth_getLogs`, and
 * when an endpoint caps the span ("…limited to a 100 range") it retries EXACTLY
 * ONCE for the most-recent `cap` blocks — never fanning out into many chunks
 * (which is what trips RPC rate limits). Genuine (non-range) errors propagate so
 * the caller can mark the chain failed. `poolRpcCall` is mocked but the real
 * `getLogsRangeCap` is exercised.
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

import { scanRecentTransfers } from '@/services/transfer-monitor';

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

function getLogsParams(): any[] {
  return mockPoolRpcCall.mock.calls
    .filter((c: any) => c[0] === 'eth_getLogs')
    .map((c: any) => c[1][0]);
}
const toBlockOf = (p: any) => parseInt(p.toBlock, 16);
const spanOfFilter = (p: any) => parseInt(p.toBlock, 16) - parseInt(p.fromBlock, 16) + 1;

describe('transfer-monitor scanRecentTransfers — bounded recent-window scan', () => {
  test('scans the window in a single getLogs when the endpoint accepts the span', async () => {
    // Generous cap (≥ the ~1000-block window) → one call, no retry.
    mockPoolRpcCall.mockImplementation(logsMock(5000, 'unused'));

    const out = await scanRecentTransfers(ADDR, 42161, 1000);

    expect(getLogsSpans()).toHaveLength(1);
    expect(out).toHaveLength(1);
    expect(out.every((t) => t.from.toLowerCase() === SENDER.toLowerCase())).toBe(true);
  });

  test('retries ONCE for the most-recent cap blocks when the span is capped — no fan-out', async () => {
    // Monad-style 100-block cap. The probe is rejected; we must do exactly one
    // more call covering only the last 100 blocks (ending at latest), never 11.
    mockPoolRpcCall.mockImplementation(logsMock(100, 'eth_getLogs is limited to a 100 range'));

    const out = await scanRecentTransfers(ADDR, 42161, 1000);

    const calls = getLogsParams();
    expect(calls).toHaveLength(2); // probe + one capped retry, nothing more
    expect(spanOfFilter(calls[0])).toBe(1001); // probed the full window
    expect(spanOfFilter(calls[1])).toBe(100); // retried at exactly the cap
    expect(toBlockOf(calls[1])).toBe(LATEST); // most-recent blocks, not the oldest
    expect(out).toHaveLength(1);
  });

  test('propagates a genuine non-range error so the caller can fail the chain', async () => {
    // The first getLogs returns a real (non-range) error → surfaced, not retried.
    mockPoolRpcCall.mockImplementation(async (method: string) => {
      if (method === 'eth_blockNumber') return { result: hex(LATEST) };
      if (method === 'eth_getLogs') return { error: { code: -32603, message: 'internal error' } };
      return { result: null };
    });

    await expect(scanRecentTransfers(ADDR, 1, 1000)).rejects.toThrow('internal error');
    expect(getLogsSpans()).toHaveLength(1); // no retry on a non-range error
  });

  test('restricts getLogs to the trusted-contract allowlist (anti-scam)', async () => {
    // The address filter is what keeps scam/airdrop Transfer spam out of the feed.
    mockPoolRpcCall.mockImplementation(logsMock(5000, 'unused'));
    const allow = [
      '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      '0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
    ];

    await scanRecentTransfers(ADDR, 42161, 1000, allow);

    expect(getLogsParams()[0].address).toEqual(allow);
  });

  test('honours a k-suffixed cap ("up to a 1K block range") when retrying', async () => {
    // Proves "1K" is parsed as 1000, not 1: the retry span must be 1000.
    mockPoolRpcCall.mockImplementation(
      logsMock(1000, 'You can make eth_getLogs requests with up to a 1K block range'),
    );

    const out = await scanRecentTransfers(ADDR, 42161, 1000);

    const calls = getLogsParams();
    expect(calls).toHaveLength(2);
    expect(spanOfFilter(calls[1])).toBe(1000); // retried at 1000, not 1
    expect(out).toHaveLength(1);
  });
});

describe('transfer-monitor decode — anti-spoofing + spam filter (money-in correctness)', () => {
  /** A crafted eth_getLogs batch (span within cap → one call) + block timestamps. */
  function batchMock(logs: any[]) {
    return async (method: string) => {
      if (method === 'eth_blockNumber') return { result: hex(LATEST) };
      if (method === 'eth_getBlockByNumber') return { result: { timestamp: '0x60000000' } };
      if (method === 'eth_getLogs') return { result: logs };
      return { result: null };
    };
  }

  function log(opts: {
    to?: string; from?: string; value?: bigint; topics?: string[]; address?: string; block?: number;
  } = {}) {
    const block = opts.block ?? 999_990;
    const topics = opts.topics ?? [
      TRANSFER_TOPIC,
      recipientTopic(opts.from ?? SENDER),
      recipientTopic(opts.to ?? ADDR),
    ];
    return {
      address: opts.address ?? '0x' + 'a'.repeat(40),
      topics,
      data: '0x' + (opts.value ?? 1000n).toString(16).padStart(64, '0'),
      transactionHash: '0x' + block.toString(16).padStart(64, '0'),
      blockNumber: hex(block),
      logIndex: '0x0',
    };
  }

  test('rejects a log whose recipient is NOT this wallet (RPC spoof / cache bleed)', async () => {
    // A failover endpoint returns someone else's Transfer — topics[2] ≠ us. The
    // decoder must drop it even though the RPC "matched" our topic filter.
    mockPoolRpcCall.mockImplementation(batchMock([
      log(),                 // → ADDR, valid
      log({ to: SENDER }),   // → someone else, must be dropped
    ]));
    const out = await scanRecentTransfers(ADDR, 42161, 100);
    expect(out).toHaveLength(1);
    expect(out[0].from.toLowerCase()).toBe(SENDER.toLowerCase());
  });

  test('skips malformed (<3 topics) and zero-value spam', async () => {
    mockPoolRpcCall.mockImplementation(batchMock([
      log(),                             // valid
      log({ topics: [TRANSFER_TOPIC] }), // <3 topics — malformed
      log({ value: 0n }),                // zero-value dust
    ]));
    const out = await scanRecentTransfers(ADDR, 42161, 100);
    expect(out).toHaveLength(1);
  });

  test('extracts sender, value and the ERC-20 token address', async () => {
    mockPoolRpcCall.mockImplementation(batchMock([
      log({ from: SENDER, value: 123456n, address: '0x' + 'c'.repeat(40) }),
    ]));
    const [t] = await scanRecentTransfers(ADDR, 42161, 100);
    expect(t.from.toLowerCase()).toBe(SENDER.toLowerCase());
    expect(t.value).toBe(123456n);
    expect(t.isNative).toBe(false);
    expect(t.token).toBe('0x' + 'c'.repeat(40));
  });

  test('classifies an EIP-7708 native-transfer log as native (token = null)', async () => {
    mockPoolRpcCall.mockImplementation(batchMock([
      log({ address: '0xfffffffffffffffffffffffffffffffffffffffe' }),
    ]));
    const [t] = await scanRecentTransfers(ADDR, 42161, 100);
    expect(t.isNative).toBe(true);
    expect(t.token).toBeNull();
  });
});
