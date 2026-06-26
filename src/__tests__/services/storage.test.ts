/**
 * Tests for storage layer logic.
 *
 * Mocks AsyncStorage to test the local persistence layer in isolation.
 */

// Mock AsyncStorage
const mockStorage = new Map<string, string>();
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    getItem: jest.fn(async (key: string) => mockStorage.get(key) ?? null),
    setItem: jest.fn(async (key: string, value: string) => { mockStorage.set(key, value); }),
    removeItem: jest.fn(async (key: string) => { mockStorage.delete(key); }),
  },
}));

import {
  saveAccount,
  loadAccounts,
  findAccountByCredentialId,
  saveTransaction,
  loadTransactions,
  deleteTransaction,
  deleteConnectionEvents,
  saveCustomToken,
  loadCustomTokens,
  removeCustomToken,
  clearAll,
  type LocalTransaction,
} from '@/services/storage';
import type { StoredAccount } from '@/models/types';

const makeAccount = (id: string, name: string = 'Test'): StoredAccount => ({
  id,
  name,
  address: `0x${id.padStart(40, '0')}`,
  createdAt: new Date().toISOString(),
  publicKeyHex: '04' + '00'.repeat(64),
});

beforeEach(() => {
  mockStorage.clear();
  jest.clearAllMocks();
});

describe('storage - accounts', () => {
  test('save and load single account', async () => {
    const acct = makeAccount('abc123');
    await saveAccount(acct);
    const loaded = await loadAccounts();
    expect(loaded).toHaveLength(1);
    expect(loaded[0].id).toBe('abc123');
  });

  test('save overwrites existing account with same id', async () => {
    await saveAccount(makeAccount('a', 'First'));
    await saveAccount(makeAccount('a', 'Updated'));
    const loaded = await loadAccounts();
    expect(loaded).toHaveLength(1);
    expect(loaded[0].name).toBe('Updated');
  });

  test('save preserves different accounts', async () => {
    await saveAccount(makeAccount('a'));
    await saveAccount(makeAccount('b'));
    const loaded = await loadAccounts();
    expect(loaded).toHaveLength(2);
  });

  test('findAccountByCredentialId returns correct account', async () => {
    await saveAccount(makeAccount('cred1', 'Wallet 1'));
    await saveAccount(makeAccount('cred2', 'Wallet 2'));
    const found = await findAccountByCredentialId('cred2');
    expect(found?.name).toBe('Wallet 2');
  });

  test('findAccountByCredentialId returns undefined for missing id', async () => {
    await saveAccount(makeAccount('cred1'));
    const found = await findAccountByCredentialId('nonexistent');
    expect(found).toBeUndefined();
  });
});

describe('storage - load', () => {
  test('returns stored local data', async () => {
    const acct = makeAccount('local-only');
    mockStorage.set('vela.accounts', JSON.stringify([acct]));

    const loaded = await loadAccounts();
    expect(loaded).toHaveLength(1);
    expect(loaded[0].id).toBe('local-only');
  });

  test('returns empty array when nothing is stored', async () => {
    const loaded = await loadAccounts();
    expect(loaded).toEqual([]);
  });

  test('handles corrupted local JSON gracefully', async () => {
    mockStorage.set('vela.accounts', '{{invalid json');
    const loaded = await loadAccounts();
    expect(loaded).toEqual([]);
  });
});

describe('storage - transactions', () => {
  test('saves and loads transactions', async () => {
    await saveTransaction({
      id: 'tx1',
      userOpHash: '0xabc',
      txHash: '0xdef',
      from: '0x1',
      to: '0x2',
      value: '1.0',
      symbol: 'ETH',
      decimals: 18,
      chainId: 1,
      timestamp: 1000,
      status: 'confirmed',
    });
    const txs = await loadTransactions();
    expect(txs).toHaveLength(1);
    expect(txs[0].id).toBe('tx1');
  });

  test('newest transaction is first', async () => {
    await saveTransaction({
      id: 'tx1', userOpHash: '0x1', txHash: '0x1',
      from: '0x1', to: '0x2', value: '1', symbol: 'ETH',
      decimals: 18, chainId: 1, timestamp: 1000, status: 'confirmed',
    });
    await saveTransaction({
      id: 'tx2', userOpHash: '0x2', txHash: '0x2',
      from: '0x1', to: '0x2', value: '2', symbol: 'ETH',
      decimals: 18, chainId: 1, timestamp: 2000, status: 'confirmed',
    });
    const txs = await loadTransactions();
    expect(txs[0].id).toBe('tx2');
    expect(txs[1].id).toBe('tx1');
  });

  test('de-dupes by id: a resubmitted UserOp keeps a single record (latest wins)', async () => {
    // Two identical sends sharing a nonce yield the same userOpHash (= record
    // id). Without de-dup this persisted twice and surfaced as a React
    // duplicate-key warning in the Activity feed.
    const base = {
      userOpHash: '0xhash', txHash: '',
      from: '0x1', to: '0x2', value: '1', symbol: 'USDC.e',
      decimals: 6, chainId: 1, timestamp: 1000, type: 'send' as const,
    };
    await saveTransaction({ ...base, id: '0xhash', status: 'pending' });
    await saveTransaction({ ...base, id: '0xhash', status: 'confirmed', txHash: '0xreceipt' });

    const txs = await loadTransactions();
    expect(txs).toHaveLength(1);
    expect(txs[0].status).toBe('confirmed'); // newest write wins
    expect(txs[0].txHash).toBe('0xreceipt');
  });

  test('enforces max 200 transactions', async () => {
    // Pre-fill with 200 transactions
    const existing = Array.from({ length: 200 }, (_, i) => ({
      id: `old-${i}`, userOpHash: `0x${i}`, txHash: `0x${i}`,
      from: '0x1', to: '0x2', value: '1', symbol: 'ETH',
      decimals: 18, chainId: 1, timestamp: i, status: 'confirmed' as const,
    }));
    mockStorage.set('vela.transactionHistory', JSON.stringify(existing));

    await saveTransaction({
      id: 'newest', userOpHash: '0xnew', txHash: '0xnew',
      from: '0x1', to: '0x2', value: '1', symbol: 'ETH',
      decimals: 18, chainId: 1, timestamp: 9999, status: 'confirmed',
    });

    const txs = await loadTransactions();
    expect(txs.length).toBeLessThanOrEqual(200);
    expect(txs[0].id).toBe('newest');
  });
});

describe('storage - connection-activity deletion', () => {
  const ME = '0xme';
  const OTHER = '0xother';
  const dappTx = (id: string, from: string, type: LocalTransaction['type']): LocalTransaction => ({
    id, userOpHash: '', txHash: '', from, to: '', value: '0', symbol: '',
    decimals: 0, chainId: 1, timestamp: 1000, status: 'confirmed', type,
  });

  test('deleteTransaction removes one record by id, leaves the rest', async () => {
    await saveTransaction(dappTx('a', ME, 'dapp_tx'));
    await saveTransaction(dappTx('b', ME, 'sign_message'));
    await deleteTransaction('a');
    const txs = await loadTransactions();
    expect(txs.map((t) => t.id)).toEqual(['b']);
  });

  test('deleteTransaction is a no-op for an unknown id', async () => {
    await saveTransaction(dappTx('a', ME, 'dapp_tx'));
    await deleteTransaction('missing');
    expect(await loadTransactions()).toHaveLength(1);
  });

  test('deleteConnectionEvents clears only this address’ dApp records', async () => {
    await saveTransaction(dappTx('mine-tx', ME, 'dapp_tx'));
    await saveTransaction(dappTx('mine-sig', ME, 'sign_typed_data'));
    await saveTransaction(dappTx('theirs', OTHER, 'dapp_tx'));
    // A value transfer for ME must survive — it isn't connection activity.
    await saveTransaction({ ...dappTx('mine-send', ME, 'send'), to: '0x2', value: '1', symbol: 'ETH', decimals: 18 });

    await deleteConnectionEvents(ME);

    const ids = (await loadTransactions()).map((t) => t.id).sort();
    expect(ids).toEqual(['mine-send', 'theirs']);
  });
});

describe('storage - custom tokens', () => {
  test('save, load, and remove custom tokens', async () => {
    await saveCustomToken({
      id: '1_0xtoken', chainId: 1, contractAddress: '0xtoken',
      symbol: 'TKN', name: 'Token', decimals: 18, networkName: 'eth-mainnet',
    });
    let tokens = await loadCustomTokens();
    expect(tokens).toHaveLength(1);

    await removeCustomToken('1_0xtoken');
    tokens = await loadCustomTokens();
    expect(tokens).toHaveLength(0);
  });
});

describe('storage - clearAll', () => {
  test('clears all local storage', async () => {
    await saveAccount(makeAccount('test'));
    await saveTransaction({
      id: 'tx1', userOpHash: '0x1', txHash: '0x1',
      from: '0x1', to: '0x2', value: '1', symbol: 'ETH',
      decimals: 18, chainId: 1, timestamp: 1000, status: 'confirmed',
    });

    await clearAll();

    const accounts = await loadAccounts();
    const txs = await loadTransactions();
    expect(accounts).toHaveLength(0);
    expect(txs).toHaveLength(0);
  });
});
