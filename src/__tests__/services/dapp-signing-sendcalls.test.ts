/**
 * Tests for handleSendCalls — the wallet-side EIP-5792 wallet_sendCalls path:
 *   - single call → sendNative / sendContractCall, returns the userOpHash (batch id)
 *   - multiple calls → Safe MultiSend batch, returns the userOpHash
 *   - rejects empty batches
 *   - rejects required (non-optional) capabilities with code 5700, ignores optional ones
 *   - remembers which chain a batch was submitted on, so a later
 *     wallet_getCallsStatus poll queries that chain even after a network switch
 */

// Mock react-native + heavy/native transitive dependencies so the module imports
// cleanly under jsdom. Service modules are mocked so we can drive their results.
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/modules/passkey', () => ({}));

const findAccountMock = jest.fn();
jest.mock('@/services/storage', () => ({
  findAccountByCredentialId: (...args: any[]) => findAccountMock(...args),
}));

const queryRecordMock = jest.fn();
jest.mock('@/services/public-key-index', () => ({
  queryRecord: (...args: any[]) => queryRecordMock(...args),
}));

const sendNativeMock = jest.fn();
const sendContractCallMock = jest.fn();
const sendBatchCallsMock = jest.fn();
jest.mock('@/services/safe-transaction', () => ({
  sendNative: (...args: any[]) => sendNativeMock(...args),
  sendContractCall: (...args: any[]) => sendContractCallMock(...args),
  sendBatchCalls: (...args: any[]) => sendBatchCallsMock(...args),
  buildEip1271Signature: jest.fn(),
  extractClientDataFields: jest.fn(),
  computeSafeMessageHash: jest.fn(),
}));

jest.mock('@/models/network', () => ({
  getAllNetworksSync: () => [{ chainId: 1 }, { chainId: 137 }],
  DEFAULT_NETWORKS: [{ chainId: 1 }, { chainId: 137 }],
}));

const rpcCallMock = jest.fn();
jest.mock('@/services/rpc-adapter', () => ({
  rpcCall: (...args: any[]) => rpcCallMock(...args),
}));

import { handleSendCalls, handleReadOnlyRPC } from '@/hooks/use-dapp-signing';

const ACCOUNT = { id: 'cred-1' } as any;
const SAFE = '0xSafe000000000000000000000000000000000001';

function req(params: any[]): any {
  return { id: 'req-1', method: 'wallet_sendCalls', params };
}

beforeEach(() => {
  findAccountMock.mockReset();
  queryRecordMock.mockReset();
  sendNativeMock.mockReset();
  sendContractCallMock.mockReset();
  sendBatchCallsMock.mockReset();
  rpcCallMock.mockReset();
  // Default: a deployed/known account with a public key on file.
  findAccountMock.mockResolvedValue({ publicKeyHex: '0xpub' });
});

describe('handleSendCalls (EIP-5792 wallet_sendCalls)', () => {
  test('single native call → returns the userOpHash as the batch id', async () => {
    sendNativeMock.mockResolvedValue({ userOpHash: '0xhash-native' });
    const id = await handleSendCalls(
      req([{ chainId: '0x1', calls: [{ to: '0xdead', value: '0x1', data: '0x' }] }]),
      ACCOUNT,
      SAFE,
      1,
    );
    expect(id).toBe('0xhash-native');
    expect(sendNativeMock).toHaveBeenCalledTimes(1);
    expect(sendBatchCallsMock).not.toHaveBeenCalled();
  });

  test('single call with calldata → routes through sendContractCall', async () => {
    sendContractCallMock.mockResolvedValue({ userOpHash: '0xhash-contract' });
    const id = await handleSendCalls(
      req([{ chainId: '0x1', calls: [{ to: '0xdead', value: '0x0', data: '0xabcd' }] }]),
      ACCOUNT,
      SAFE,
      1,
    );
    expect(id).toBe('0xhash-contract');
    expect(sendContractCallMock).toHaveBeenCalledTimes(1);
    expect(sendNativeMock).not.toHaveBeenCalled();
  });

  test('multiple calls → batches via Safe MultiSend, returns the userOpHash', async () => {
    sendBatchCallsMock.mockResolvedValue({ userOpHash: '0xhash-batch' });
    const id = await handleSendCalls(
      req([{
        chainId: '0x1',
        calls: [
          { to: '0xaaa', value: '0x1', data: '0x' },
          { to: '0xbbb', value: '0x0', data: '0xbeef' },
        ],
      }]),
      ACCOUNT,
      SAFE,
      1,
    );
    expect(id).toBe('0xhash-batch');
    expect(sendBatchCallsMock).toHaveBeenCalledTimes(1);
    // The batch is forwarded with normalized value/data fields.
    const forwardedCalls = sendBatchCallsMock.mock.calls[0][1];
    expect(forwardedCalls).toHaveLength(2);
    expect(forwardedCalls[1]).toEqual({ to: '0xbbb', value: '0', data: '0xbeef' });
  });

  test('rejects an empty batch', async () => {
    await expect(
      handleSendCalls(req([{ chainId: '0x1', calls: [] }]), ACCOUNT, SAFE, 1),
    ).rejects.toThrow('No calls provided');
  });

  test('uses the embedded payload chainId over the fallback', async () => {
    sendNativeMock.mockResolvedValue({ userOpHash: '0xhash' });
    await handleSendCalls(
      req([{ chainId: '0x89', calls: [{ to: '0xdead', value: '0x0', data: '0x' }] }]),
      ACCOUNT,
      SAFE,
      1, // fallback chain 1, but payload says 137
    );
    // sendNative(safeAddress, to, value, chainId, ...)
    expect(sendNativeMock.mock.calls[0][3]).toBe(137);
  });

  test('rejects an unsupported chain (EIP-3085 code 4902)', async () => {
    await expect(
      handleSendCalls(req([{ chainId: '0x2105', calls: [{ to: '0xdead' }] }]), ACCOUNT, SAFE, 1),
    ).rejects.toMatchObject({ code: 4902 });
  });

  // ── EIP-5792 capability negotiation ───────────────────────────────────────

  test('rejects a required (non-optional) top-level capability with code 5700', async () => {
    sendNativeMock.mockResolvedValue({ userOpHash: '0xhash' });
    await expect(
      handleSendCalls(
        req([{
          chainId: '0x1',
          calls: [{ to: '0xdead', value: '0x0', data: '0x' }],
          capabilities: { paymasterService: { url: 'https://paymaster.example' } },
        }]),
        ACCOUNT,
        SAFE,
        1,
      ),
    ).rejects.toMatchObject({ code: 5700 });
    expect(sendNativeMock).not.toHaveBeenCalled();
  });

  test('rejects a required per-call capability with code 5700', async () => {
    await expect(
      handleSendCalls(
        req([{
          chainId: '0x1',
          calls: [{ to: '0xdead', value: '0x0', data: '0x', capabilities: { auxiliaryFunds: {} } }],
        }]),
        ACCOUNT,
        SAFE,
        1,
      ),
    ).rejects.toMatchObject({ code: 5700 });
  });

  test('ignores optional capabilities and proceeds', async () => {
    sendNativeMock.mockResolvedValue({ userOpHash: '0xhash-optional' });
    const id = await handleSendCalls(
      req([{
        chainId: '0x1',
        calls: [{ to: '0xdead', value: '0x0', data: '0x' }],
        capabilities: { paymasterService: { optional: true, url: 'https://paymaster.example' } },
      }]),
      ACCOUNT,
      SAFE,
      1,
    );
    expect(id).toBe('0xhash-optional');
  });

  // ── Batch → chain tracking, consumed by wallet_getCallsStatus ─────────────

  test('remembers the batch chain so getCallsStatus polls the right chain after a network switch', async () => {
    // Submit a batch on chain 1.
    sendNativeMock.mockResolvedValue({ userOpHash: '0xtracked-batch' });
    const id = await handleSendCalls(
      req([{ chainId: '0x1', calls: [{ to: '0xdead', value: '0x0', data: '0x' }] }]),
      ACCOUNT,
      SAFE,
      1,
    );
    expect(id).toBe('0xtracked-batch');

    // Wallet has since switched to chain 137; poll status with current chain = 137.
    rpcCallMock.mockResolvedValue({
      result: { success: true, receipt: { status: '0x1', logs: [], transactionHash: id } },
    });
    const res = await handleReadOnlyRPC('wallet_getCallsStatus', [id], SAFE, 137);
    expect(res.handled).toBe(true);
    const status = (res as any).result;

    // The receipt lookup must target chain 1 (where the batch was submitted)…
    expect(rpcCallMock).toHaveBeenCalledWith('eth_getUserOperationReceipt', [id], 1);
    // …and the reported chainId must reflect the batch's chain, not the current one.
    expect(status.chainId).toBe('0x1');
    expect(status.status).toBe(200);
  });

  test('falls back to the current chain for an unknown batch id', async () => {
    rpcCallMock.mockResolvedValue({ result: null });
    const res = await handleReadOnlyRPC('wallet_getCallsStatus', ['0xunknown-id'], SAFE, 137);
    expect(rpcCallMock).toHaveBeenCalledWith('eth_getUserOperationReceipt', ['0xunknown-id'], 137);
    expect((res as any).result.chainId).toBe('0x89');
  });
});
