/**
 * token-reads — point-in-time ERC-20 state reads for the signing sheet.
 * readErc20Balance powers issue #86's one-tap finite "Balance" approval cap.
 */
const poolRpcCall = jest.fn();
jest.mock('@/services/rpc-pool', () => ({ poolRpcCall }));

import { readErc20Balance, readErc20Allowance } from '@/services/token-reads';

const TOKEN = '0xdAC17F958D2ee523a2206206994597C13D831ec7'; // USDT
const OWNER = '0x' + '11'.repeat(20);
const SPENDER = '0x' + '22'.repeat(20);

beforeEach(() => poolRpcCall.mockReset());

describe('readErc20Balance', () => {
  it('calls balanceOf(owner) with the right selector + padded owner, and decodes the result', async () => {
    poolRpcCall.mockResolvedValue({ result: '0x0000000000000000000000000000000000000000000000000000000005f5e100' }); // 100_000_000
    const bal = await readErc20Balance(1, TOKEN, OWNER);
    expect(bal).toBe(100_000_000n);
    const [method, params, chainId] = poolRpcCall.mock.calls[0];
    expect(method).toBe('eth_call');
    expect(chainId).toBe(1);
    expect(params[0].to).toBe(TOKEN);
    expect(params[0].data).toBe('0x70a08231' + '11'.repeat(20).padStart(64, '0'));
  });

  it('returns null on a reverted/empty read (graceful degrade → no preset)', async () => {
    poolRpcCall.mockResolvedValue({ result: '0x' });
    expect(await readErc20Balance(1, TOKEN, OWNER)).toBeNull();
  });

  it('returns null on an RPC error and on a thrown call', async () => {
    poolRpcCall.mockResolvedValue({ error: { message: 'boom' } });
    expect(await readErc20Balance(1, TOKEN, OWNER)).toBeNull();
    poolRpcCall.mockRejectedValue(new Error('network'));
    expect(await readErc20Balance(1, TOKEN, OWNER)).toBeNull();
  });

  it('rejects malformed addresses without an RPC call', async () => {
    expect(await readErc20Balance(1, '0x123', OWNER)).toBeNull();
    expect(await readErc20Balance(1, TOKEN, 'nope')).toBeNull();
    expect(poolRpcCall).not.toHaveBeenCalled();
  });

  it('readErc20Allowance still works (regression: shared module)', async () => {
    poolRpcCall.mockResolvedValue({ result: '0x' + (500n).toString(16).padStart(64, '0') });
    expect(await readErc20Allowance(1, TOKEN, OWNER, SPENDER)).toBe(500n);
  });
});
