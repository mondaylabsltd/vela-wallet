/**
 * RPC simulation engine (eth_simulateV1). The RPC layer is mocked; we assert how
 * the engine shapes responses into ok/revert + deltas, and when it returns null
 * to signal "degrade".
 */
import { TRANSFER_TOPIC, NATIVE_TRANSFER_SENTINEL } from '@/services/sim-assets';

jest.mock('@/services/rpc-adapter', () => ({ rpcCall: jest.fn() }));
import { rpcCall } from '@/services/rpc-adapter';
import { rpcSimulate } from '@/services/sim-engine-rpc';

const mockRpc = rpcCall as jest.Mock;

const USER = '0x' + '11'.repeat(20);
const PEER = '0x' + '22'.repeat(20);
const USDC = '0x' + 'a0'.repeat(20);

const topic = (addr: string) => '0x' + addr.replace(/^0x/, '').toLowerCase().padStart(64, '0');
const word = (v: bigint) => '0x' + v.toString(16).padStart(64, '0');
const transfer = (token: string, from: string, to: string, v: bigint) => ({
  address: token, topics: [TRANSFER_TOPIC, topic(from), topic(to)], data: word(v),
});

/** Error(string) revert payload, e.g. "STF". */
function errorString(msg: string): string {
  const hex = Buffer.from(msg, 'utf8').toString('hex');
  const len = Buffer.from(msg, 'utf8').length;
  return '0x08c379a0'
    + (32).toString(16).padStart(64, '0')
    + len.toString(16).padStart(64, '0')
    + hex.padEnd(Math.ceil(hex.length / 64) * 64, '0');
}

beforeEach(() => mockRpc.mockReset());

describe('rpcSimulate', () => {
  test('success: parses logs into net deltas', async () => {
    mockRpc.mockResolvedValue({
      result: [{
        calls: [{
          status: '0x1',
          logs: [
            transfer(USDC, USER, PEER, 1_000_000n),
            transfer(NATIVE_TRANSFER_SENTINEL, PEER, USER, 10n ** 18n),
          ],
        }],
      }],
    });

    const r = await rpcSimulate(USER, [{ to: USDC, data: '0xabcd' }], 1);
    expect(r).not.toBeNull();
    expect(r!.ok).toBe(true);
    expect(r!.deltas).toContainEqual({ kind: 'erc20', token: USDC, delta: -1_000_000n });
    expect(r!.deltas).toContainEqual({ kind: 'native', token: undefined, delta: 10n ** 18n });
  });

  test('sends the right payload shape (traceTransfers, validation:false, value default)', async () => {
    mockRpc.mockResolvedValue({ result: [{ calls: [{ status: '0x1', logs: [] }] }] });
    await rpcSimulate(USER, [{ to: USDC }], 137);

    expect(mockRpc).toHaveBeenCalledWith('eth_simulateV1', expect.any(Array), 137);
    const [, params] = mockRpc.mock.calls[0];
    const [payload, block] = params;
    expect(block).toBe('latest');
    expect(payload.traceTransfers).toBe(true);
    expect(payload.validation).toBe(false);
    expect(payload.blockStateCalls[0].calls[0]).toMatchObject({ from: USER, to: USDC, value: '0x0' });
    // no data key when calldata is absent
    expect(payload.blockStateCalls[0].calls[0]).not.toHaveProperty('data');
  });

  test('revert: ok=false, reason decoded, no deltas', async () => {
    mockRpc.mockResolvedValue({
      result: [{ calls: [{ status: '0x0', error: { data: errorString('STF') }, logs: [] }] }],
    });
    const r = await rpcSimulate(USER, [{ to: USDC, data: '0x1234' }], 1);
    expect(r).toEqual({ ok: false, revertReason: 'STF', deltas: [] });
  });

  test('batch: nets logs across sequential calls', async () => {
    mockRpc.mockResolvedValue({
      result: [{
        calls: [
          { status: '0x1', logs: [transfer(USDC, USER, PEER, 5n)] },
          { status: '0x1', logs: [transfer(USDC, PEER, USER, 2n)] },
        ],
      }],
    });
    const r = await rpcSimulate(USER, [{ to: USDC, data: '0x01' }, { to: USDC, data: '0x02' }], 1);
    expect(r!.ok).toBe(true);
    expect(r!.deltas).toEqual([{ kind: 'erc20', token: USDC, delta: -3n }]);
  });

  test('method unsupported (top-level error) → null to degrade', async () => {
    mockRpc.mockResolvedValue({ error: { code: -32601, message: 'method eth_simulateV1 does not exist' } });
    expect(await rpcSimulate(USER, [{ to: USDC, data: '0x1' }], 1)).toBeNull();
  });

  test('network failure (throw) → null', async () => {
    mockRpc.mockRejectedValue(new Error('all endpoints down'));
    expect(await rpcSimulate(USER, [{ to: USDC, data: '0x1' }], 1)).toBeNull();
  });

  test('empty / malformed result → null', async () => {
    mockRpc.mockResolvedValue({ result: [] });
    expect(await rpcSimulate(USER, [{ to: USDC, data: '0x1' }], 1)).toBeNull();

    mockRpc.mockResolvedValue({ result: [{ calls: [] }] });
    expect(await rpcSimulate(USER, [{ to: USDC, data: '0x1' }], 1)).toBeNull();
  });

  test('no `to` → null without calling RPC', async () => {
    expect(await rpcSimulate(USER, [{ to: '' }], 1)).toBeNull();
    expect(await rpcSimulate(USER, [], 1)).toBeNull();
    expect(mockRpc).not.toHaveBeenCalled();
  });
});
