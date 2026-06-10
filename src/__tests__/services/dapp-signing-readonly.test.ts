/**
 * Tests for handleReadOnlyRPC — the wallet-side read/forward path:
 *   - eth_getCode counterfactual smart-account override
 *   - wallet_getCallsStatus (EIP-5792)
 *   - EIP-2255 compatibility shims
 *   - plain forwarding to RPC
 */

// Mock react-native + heavy/native transitive dependencies so the module imports
// cleanly under jsdom. rpc-adapter is mocked so we can drive RPC responses.
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/modules/cloud-sync', () => ({
  get: jest.fn(), save: jest.fn(), remove: jest.fn(), syncNow: jest.fn(),
}));
jest.mock('@/modules/passkey', () => ({}));
jest.mock('@/services/storage', () => ({ findAccountByCredentialId: jest.fn() }));
jest.mock('@/services/public-key-index', () => ({ queryRecord: jest.fn() }));
jest.mock('@/services/safe-transaction', () => ({
  sendContractCall: jest.fn(), sendNative: jest.fn(), buildEip1271Signature: jest.fn(),
  extractClientDataFields: jest.fn(), computeSafeMessageHash: jest.fn(), sendBatchCalls: jest.fn(),
}));
jest.mock('@/models/network', () => ({
  getAllNetworksSync: () => [{ chainId: 1 }, { chainId: 137 }],
  DEFAULT_NETWORKS: [{ chainId: 1 }, { chainId: 137 }],
}));

const rpcCallMock = jest.fn();
jest.mock('@/services/rpc-adapter', () => ({
  rpcCall: (...args: any[]) => rpcCallMock(...args),
}));

import {
  handleReadOnlyRPC,
  __resetReadOnlyCache,
  INSTANT_READONLY_METHODS,
  isSigningMethod,
} from '@/hooks/use-dapp-signing';
import { SAFE_PROXY_RUNTIME_CODE } from '@/services/safe-address';

const OWN = '0xAccount0000000000000000000000000000000001';
const CHAIN = 137;

function unwrap(res: { handled: boolean; result?: any }): any {
  expect(res.handled).toBe(true);
  return (res as any).result;
}

beforeEach(() => {
  rpcCallMock.mockReset();
  __resetReadOnlyCache();
});

describe('handleReadOnlyRPC — eth_getCode counterfactual override', () => {
  test('returns Safe proxy runtime code when own account is undeployed (real code 0x)', async () => {
    rpcCallMock.mockResolvedValue({ result: '0x' });
    const res = await handleReadOnlyRPC('eth_getCode', [OWN, 'latest'], OWN, CHAIN);
    expect(unwrap(res)).toBe(SAFE_PROXY_RUNTIME_CODE);
  });

  test('returns real on-chain code when own account is already deployed', async () => {
    rpcCallMock.mockResolvedValue({ result: '0x6080604052deadbeef' });
    const res = await handleReadOnlyRPC('eth_getCode', [OWN, 'latest'], OWN, CHAIN);
    expect(unwrap(res)).toBe('0x6080604052deadbeef');
  });

  test('falls back to runtime code if the RPC lookup throws', async () => {
    rpcCallMock.mockRejectedValue(new Error('rpc down'));
    const res = await handleReadOnlyRPC('eth_getCode', [OWN, 'latest'], OWN, CHAIN);
    expect(unwrap(res)).toBe(SAFE_PROXY_RUNTIME_CODE);
  });

  test('matches the own address case-insensitively', async () => {
    rpcCallMock.mockResolvedValue({ result: '0x' });
    const res = await handleReadOnlyRPC('eth_getCode', [OWN.toUpperCase(), 'latest'], OWN.toLowerCase(), CHAIN);
    expect(unwrap(res)).toBe(SAFE_PROXY_RUNTIME_CODE);
  });

  test('does NOT override eth_getCode for a different address (plain forward)', async () => {
    rpcCallMock.mockResolvedValue({ result: '0xfeedface' });
    const res = await handleReadOnlyRPC('eth_getCode', ['0xSomeOtherContract', 'latest'], OWN, CHAIN);
    expect(unwrap(res)).toBe('0xfeedface');
  });

  test('caches deployed self code and skips the RPC on subsequent self queries', async () => {
    rpcCallMock.mockResolvedValue({ result: '0x6080604052deadbeef' });
    const first = await handleReadOnlyRPC('eth_getCode', [OWN, 'latest'], OWN, CHAIN);
    expect(unwrap(first)).toBe('0x6080604052deadbeef');

    rpcCallMock.mockClear();
    const second = await handleReadOnlyRPC('eth_getCode', [OWN, 'latest'], OWN, CHAIN);
    expect(unwrap(second)).toBe('0x6080604052deadbeef');
    expect(rpcCallMock).not.toHaveBeenCalled(); // served from cache
  });

  test('does NOT cache an undeployed (0x) self result — keeps re-checking', async () => {
    rpcCallMock.mockResolvedValue({ result: '0x' });
    await handleReadOnlyRPC('eth_getCode', [OWN, 'latest'], OWN, CHAIN);
    rpcCallMock.mockClear();
    rpcCallMock.mockResolvedValue({ result: '0x' });
    await handleReadOnlyRPC('eth_getCode', [OWN, 'latest'], OWN, CHAIN);
    expect(rpcCallMock).toHaveBeenCalledTimes(1); // re-queried, not cached
  });
});

describe('handleReadOnlyRPC — wallet_getCallsStatus (EIP-5792)', () => {
  test('status 200 (confirmed) with a receipt when the batch tx succeeded', async () => {
    // ERC-4337 eth_getUserOperationReceipt: { success, receipt: TransactionReceipt }
    rpcCallMock.mockResolvedValue({
      result: {
        success: true,
        receipt: {
          status: '0x1', logs: [], blockHash: '0xbh', blockNumber: '0x10',
          gasUsed: '0x5208', transactionHash: '0xbatch',
        },
      },
    });
    const out = unwrap(await handleReadOnlyRPC('wallet_getCallsStatus', ['0xbatch'], OWN, CHAIN));
    expect(out.status).toBe(200);
    expect(out.id).toBe('0xbatch');
    expect(out.chainId).toBe('0x89');
    expect(out.receipts).toHaveLength(1);
    expect(out.receipts[0].transactionHash).toBe('0xbatch');
  });

  test('status 500 (reverted) when the batch tx reverted on-chain', async () => {
    rpcCallMock.mockResolvedValue({
      result: { success: true, receipt: { status: '0x0', logs: [], transactionHash: '0xbatch' } },
    });
    const out = unwrap(await handleReadOnlyRPC('wallet_getCallsStatus', ['0xbatch'], OWN, CHAIN));
    expect(out.status).toBe(500);
  });

  test('status 500 when the UserOp was mined but the inner call failed (success:false)', async () => {
    // ERC-4337 nuance: the bundle tx can succeed (receipt.status 0x1) while the
    // UserOp's inner call reverted — opReceipt.success === false must map to 500.
    rpcCallMock.mockResolvedValue({
      result: { success: false, receipt: { status: '0x1', logs: [], transactionHash: '0xbatch' } },
    });
    const out = unwrap(await handleReadOnlyRPC('wallet_getCallsStatus', ['0xbatch'], OWN, CHAIN));
    expect(out.status).toBe(500);
  });

  test('status 100 (pending) when no receipt yet', async () => {
    rpcCallMock.mockResolvedValue({ result: null });
    const out = unwrap(await handleReadOnlyRPC('wallet_getCallsStatus', ['0xbatch'], OWN, CHAIN));
    expect(out.status).toBe(100);
    expect(out.receipts).toEqual([]);
  });

  test('status 100 (pending) when no id is supplied', async () => {
    const out = unwrap(await handleReadOnlyRPC('wallet_getCallsStatus', [], OWN, CHAIN));
    expect(out.status).toBe(100);
    expect(rpcCallMock).not.toHaveBeenCalled();
  });

  test('status 100 (pending) if the receipt lookup throws', async () => {
    rpcCallMock.mockRejectedValue(new Error('rpc down'));
    const out = unwrap(await handleReadOnlyRPC('wallet_getCallsStatus', ['0xbatch'], OWN, CHAIN));
    expect(out.status).toBe(100);
  });
});

describe('handleReadOnlyRPC — local methods & EIP-2255 compat shims', () => {
  test('eth_chainId returns hex chain id', async () => {
    expect(unwrap(await handleReadOnlyRPC('eth_chainId', [], OWN, CHAIN))).toBe('0x89');
  });

  test('net_version returns decimal chain id', async () => {
    expect(unwrap(await handleReadOnlyRPC('net_version', [], OWN, CHAIN))).toBe('137');
  });

  test('eth_accounts returns the wallet address', async () => {
    expect(unwrap(await handleReadOnlyRPC('eth_accounts', [], OWN, CHAIN))).toEqual([OWN]);
  });

  test('wallet_getPermissions / wallet_requestPermissions return the eth_accounts grant', async () => {
    const a = unwrap(await handleReadOnlyRPC('wallet_getPermissions', [], OWN, CHAIN));
    const b = unwrap(await handleReadOnlyRPC('wallet_requestPermissions', [], OWN, CHAIN));
    expect(a).toEqual([{ parentCapability: 'eth_accounts' }]);
    expect(b).toEqual([{ parentCapability: 'eth_accounts' }]);
  });

  test('forwards an unknown read-only method to RPC', async () => {
    rpcCallMock.mockResolvedValue({ result: '0xbalance' });
    const res = await handleReadOnlyRPC('eth_getBalance', [OWN, 'latest'], OWN, CHAIN);
    expect(unwrap(res)).toBe('0xbalance');
    expect(rpcCallMock).toHaveBeenCalledWith('eth_getBalance', [OWN, 'latest'], CHAIN);
  });

  test('returns handled:false for a signing method (not read-only)', async () => {
    const res = await handleReadOnlyRPC('personal_sign', ['0x', OWN], OWN, CHAIN);
    expect(res.handled).toBe(false);
  });
});

describe('read-only dispatch classification', () => {
  test('instant methods are local and never signing methods', () => {
    for (const m of INSTANT_READONLY_METHODS) {
      expect(isSigningMethod(m)).toBe(false);
    }
  });

  test('hot network reads are NOT instant (so they pass through the gate)', () => {
    for (const m of ['eth_call', 'eth_getBalance', 'eth_getLogs', 'eth_estimateGas', 'eth_getCode']) {
      expect(INSTANT_READONLY_METHODS.has(m)).toBe(false);
    }
  });

  test('signing methods are never instant (so they never get throttled by the gate)', () => {
    for (const m of ['eth_sendTransaction', 'wallet_sendCalls', 'personal_sign', 'eth_signTypedData_v4']) {
      expect(INSTANT_READONLY_METHODS.has(m)).toBe(false);
    }
  });
});
