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
  saveCustomToken,
  loadCustomTokens,
  removeCustomToken,
  clearAll,
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
