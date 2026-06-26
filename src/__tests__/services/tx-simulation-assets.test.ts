/**
 * Orchestrator: simulateAssetChanges. Engines + metadata + the degrade-path RPC
 * are mocked so we can assert engine selection, enrichment, and graceful
 * fallback without any network.
 */
jest.mock('@/services/sim-engine-rpc', () => ({ rpcSimulate: jest.fn() }));
jest.mock('@/services/sim-engine-tevm', () => ({ tevmSimulate: jest.fn() }));
jest.mock('@/services/token-metadata', () => ({ resolveTokenMetadata: jest.fn() }));
jest.mock('@/services/chain-tokens', () => ({ fetchChainTokens: jest.fn() }));
jest.mock('@/services/wallet-api', () => ({ getCachedHeldTokens: jest.fn(() => []) }));
jest.mock('@/services/rpc-pool', () => ({ poolRpcCall: jest.fn() }));

import { rpcSimulate } from '@/services/sim-engine-rpc';
import { tevmSimulate } from '@/services/sim-engine-tevm';
import { resolveTokenMetadata } from '@/services/token-metadata';
import { fetchChainTokens } from '@/services/chain-tokens';
import { getCachedHeldTokens } from '@/services/wallet-api';
import { poolRpcCall } from '@/services/rpc-pool';
import { nativeSymbol } from '@/models/network';
import { simulateAssetChanges } from '@/services/tx-simulation';

const mockRpcSim = rpcSimulate as jest.Mock;
const mockTevmSim = tevmSimulate as jest.Mock;
const mockMeta = resolveTokenMetadata as jest.Mock;
const mockChainTokens = fetchChainTokens as jest.Mock;
const mockHeld = getCachedHeldTokens as jest.Mock;
const mockPool = poolRpcCall as jest.Mock;

const USER = '0x' + '11'.repeat(20);
const PEER = '0x' + '22'.repeat(20);
const USDC = '0x' + 'a0'.repeat(20);
const UNKNOWN = '0x' + 'cc'.repeat(20);
const REAL_USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'; // in the curated KNOWN_TOKENS

/** Make `addr` a trusted (stablecoin) token on the chain for the trust-gate. */
const trust = (addr: string) => ({ stables: [{ symbol: 'USDC', type: 'native', contract: addr }], wrappedNativeToken: null });
/** wei hex for a whole-ETH amount. */
const eth = (n: bigint) => '0x' + (n * 10n ** 18n).toString(16);

beforeEach(() => {
  mockRpcSim.mockReset();
  mockTevmSim.mockReset().mockResolvedValue(null);
  mockMeta.mockReset().mockResolvedValue(new Map());
  mockChainTokens.mockReset().mockResolvedValue(null); // default: nothing trusted
  mockHeld.mockReset().mockReturnValue([]); // default: no holdings cached
  mockPool.mockReset();
});

describe('simulateAssetChanges', () => {
  test('rpc engine: enriches native + known + unknown ERC-20', async () => {
    mockRpcSim.mockResolvedValue({
      ok: true,
      deltas: [
        { kind: 'native', token: undefined, delta: 10n ** 18n },
        { kind: 'erc20', token: USDC, delta: -1_000_000n },
        { kind: 'erc20', token: UNKNOWN, delta: 50n },
      ],
    });
    mockMeta.mockResolvedValue(new Map([[USDC, { symbol: 'USDC', decimals: 6 }]]));

    const r = await simulateAssetChanges(USER, [{ to: USDC, data: '0xabcd' }], 1);

    expect(r).not.toBeNull();
    expect(r!.engine).toBe('rpc');
    expect(r!.ok).toBe(true);
    expect(r!.changes).toContainEqual({ kind: 'native', delta: 10n ** 18n, symbol: nativeSymbol(1), decimals: 18 });
    expect(r!.changes).toContainEqual({ kind: 'erc20', token: USDC, delta: -1_000_000n, symbol: 'USDC', decimals: 6 });
    expect(r!.changes).toContainEqual({ kind: 'erc20', token: UNKNOWN, delta: 50n, unverified: true });
    // Tevm fallback must not be consulted when the primary engine answered.
    expect(mockTevmSim).not.toHaveBeenCalled();
  });

  test('only ERC-20 addresses are sent to the metadata resolver', async () => {
    mockRpcSim.mockResolvedValue({
      ok: true,
      deltas: [
        { kind: 'native', token: undefined, delta: 1n },
        { kind: 'erc20', token: USDC, delta: 2n },
      ],
    });
    await simulateAssetChanges(USER, [{ to: USDC, data: '0x01' }], 1);
    expect(mockMeta).toHaveBeenCalledWith(1, [USDC]);
  });

  test('falls back to the Tevm engine when rpc returns null', async () => {
    mockRpcSim.mockResolvedValue(null);
    mockTevmSim.mockResolvedValue({ ok: true, deltas: [{ kind: 'erc20', token: USDC, delta: 7n }] });
    mockMeta.mockResolvedValue(new Map([[USDC, { symbol: 'USDC', decimals: 6 }]]));
    mockChainTokens.mockResolvedValue(trust(USDC)); // trusted received token → amount shown

    const r = await simulateAssetChanges(USER, [{ to: USDC, data: '0x01' }], 1);
    expect(r!.engine).toBe('tevm');
    expect(r!.changes).toEqual([{ kind: 'erc20', token: USDC, delta: 7n, symbol: 'USDC', decimals: 6 }]);
  });

  test('anti-spoof: a RECEIVED token with metadata but NOT trusted is unverified', async () => {
    // A hostile contract emits a fake inbound Transfer and answers symbol()/decimals()
    // to look like USDC. It must NOT render as a confident "+1,000,000 USDC".
    const SPOOF = '0x' + 'de'.repeat(20);
    mockRpcSim.mockResolvedValue({ ok: true, deltas: [{ kind: 'erc20', token: SPOOF, delta: 1_000_000n }] });
    mockMeta.mockResolvedValue(new Map([[SPOOF, { symbol: 'USDC', decimals: 6 }]]));
    mockChainTokens.mockResolvedValue(null); // not a trusted token on this chain

    const r = await simulateAssetChanges(USER, [{ to: SPOOF, data: '0x01' }], 1);
    expect(r!.changes).toEqual([{ kind: 'erc20', token: SPOOF, delta: 1_000_000n, unverified: true }]);
  });

  test('a SENT token renders its amount even when not in the trusted set', async () => {
    // Outflows can't be spoofed to understate, so sent amounts show with metadata.
    mockRpcSim.mockResolvedValue({ ok: true, deltas: [{ kind: 'erc20', token: UNKNOWN, delta: -42n }] });
    mockMeta.mockResolvedValue(new Map([[UNKNOWN, { symbol: 'TKN', decimals: 18 }]]));
    mockChainTokens.mockResolvedValue(null);

    const r = await simulateAssetChanges(USER, [{ to: UNKNOWN, data: '0x01' }], 1);
    expect(r!.changes).toEqual([{ kind: 'erc20', token: UNKNOWN, delta: -42n, symbol: 'TKN', decimals: 18 }]);
  });

  test('a RECEIVED token the user already holds is trusted (amount shown)', async () => {
    const HELD = '0x' + 'ab'.repeat(20);
    mockRpcSim.mockResolvedValue({ ok: true, deltas: [{ kind: 'erc20', token: HELD, delta: 500n }] });
    mockMeta.mockResolvedValue(new Map([[HELD, { symbol: 'TKN', decimals: 18 }]]));
    mockChainTokens.mockResolvedValue(null); // not a stable
    mockHeld.mockReturnValue([HELD]); // but the user holds it

    const r = await simulateAssetChanges(USER, [{ to: HELD, data: '0x01' }], 1);
    expect(r!.changes).toEqual([{ kind: 'erc20', token: HELD, delta: 500n, symbol: 'TKN', decimals: 18 }]);
  });

  test('a RECEIVED curated known token is trusted without holdings or registry', async () => {
    mockRpcSim.mockResolvedValue({ ok: true, deltas: [{ kind: 'erc20', token: REAL_USDC, delta: 1_000_000n }] });
    mockMeta.mockResolvedValue(new Map([[REAL_USDC, { symbol: 'USDC', decimals: 6 }]]));
    mockChainTokens.mockResolvedValue(null);
    mockHeld.mockReturnValue([]);

    const r = await simulateAssetChanges(USER, [{ to: REAL_USDC, data: '0x01' }], 1);
    expect(r!.changes).toEqual([{ kind: 'erc20', token: REAL_USDC, delta: 1_000_000n, symbol: 'USDC', decimals: 6 }]);
  });

  test('flags a native outflow larger than the wallet balance as underfunded', async () => {
    mockRpcSim.mockResolvedValue({ ok: true, deltas: [{ kind: 'native', token: undefined, delta: -(10n ** 18n) }] });
    mockPool.mockResolvedValue({ result: '0x16345785d8a0000' }); // 0.1 ETH < 1 ETH out

    const r = await simulateAssetChanges(USER, [{ to: PEER, value: eth(1n) }], 1);
    expect(r!.underfundedNative).toBe(true);
    expect(mockPool).toHaveBeenCalledWith('eth_getBalance', [USER, 'latest'], 1);
  });

  test('does not flag underfunded when the balance covers the native outflow', async () => {
    mockRpcSim.mockResolvedValue({ ok: true, deltas: [{ kind: 'native', token: undefined, delta: -(10n ** 17n) }] });
    mockPool.mockResolvedValue({ result: eth(1n) }); // 1 ETH covers 0.1 ETH

    const r = await simulateAssetChanges(USER, [{ to: PEER, value: '0x16345785d8a0000' }], 1);
    expect(r!.underfundedNative).toBeUndefined();
  });

  test('does not flag underfunded when the balance is unknown', async () => {
    mockRpcSim.mockResolvedValue({ ok: true, deltas: [{ kind: 'native', token: undefined, delta: -(10n ** 18n) }] });
    mockPool.mockResolvedValue({ error: { message: 'down' } });

    const r = await simulateAssetChanges(USER, [{ to: PEER, value: eth(1n) }], 1);
    expect(r!.underfundedNative).toBeUndefined();
  });

  test('degrades to the revert pre-check when no engine can compute changes', async () => {
    mockRpcSim.mockResolvedValue(null);
    mockTevmSim.mockResolvedValue(null);
    mockPool.mockResolvedValue({ result: '0x' }); // eth_call succeeds

    const r = await simulateAssetChanges(USER, [{ to: USDC, data: '0x01' }], 1);
    expect(r).toEqual({ ok: true, revertReason: undefined, changes: null, engine: 'none' });
    expect(mockPool).toHaveBeenCalledWith('eth_call', expect.any(Array), 1);
  });

  test('degrade path surfaces a revert reason', async () => {
    mockRpcSim.mockResolvedValue(null);
    mockTevmSim.mockResolvedValue(null);
    mockPool.mockResolvedValue({ error: { message: 'execution reverted: NOPE' } });

    const r = await simulateAssetChanges(USER, [{ to: USDC, data: '0x01' }], 1);
    expect(r).toMatchObject({ ok: false, revertReason: 'NOPE', changes: null, engine: 'none' });
  });

  test('returns null when everything is unreachable', async () => {
    mockRpcSim.mockResolvedValue(null);
    mockTevmSim.mockResolvedValue(null);
    mockPool.mockRejectedValue(new Error('down'));

    expect(await simulateAssetChanges(USER, [{ to: USDC, data: '0x01' }], 1)).toBeNull();
  });

  test('metadata lookup failure → ERC-20 flagged unverified, never throws', async () => {
    mockRpcSim.mockResolvedValue({ ok: true, deltas: [{ kind: 'erc20', token: USDC, delta: 9n }] });
    mockMeta.mockRejectedValue(new Error('rpc down'));

    const r = await simulateAssetChanges(USER, [{ to: USDC, data: '0x01' }], 1);
    expect(r!.changes).toEqual([{ kind: 'erc20', token: USDC, delta: 9n, unverified: true }]);
  });

  test('empty calls / missing target → null, no engine consulted', async () => {
    expect(await simulateAssetChanges(USER, [], 1)).toBeNull();
    expect(await simulateAssetChanges(USER, [{ to: '' }], 1)).toBeNull();
    expect(mockRpcSim).not.toHaveBeenCalled();
  });
});
