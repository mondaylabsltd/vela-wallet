/**
 * Regression tests for the Activity-feed de-dup logic.
 *
 * The home FlatList keys rows by `item.id` (= userOpHash for sends). A
 * resubmitted UserOp sharing a userOpHash could land two records with the same
 * id in the store, which rendered as React "two children with the same key"
 * warnings. loadActivityItems must collapse same-id records to one feed row,
 * so the feed stays clean even if the store already holds a legacy duplicate.
 */

// Mock AsyncStorage (same shape as storage.test.ts) — loadActivityItems reads
// the local transaction store through it.
const mockStorage = new Map<string, string>();
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    getItem: jest.fn(async (key: string) => mockStorage.get(key) ?? null),
    setItem: jest.fn(async (key: string, value: string) => { mockStorage.set(key, value); }),
    removeItem: jest.fn(async (key: string) => { mockStorage.delete(key); }),
  },
}));

// Stub i18n — it pulls in expo-localization (ESM, untransformed in jsdom/node)
// and loadActivityItems never calls it (only relativeTime/labels do).
jest.mock('@/i18n', () => ({
  __esModule: true,
  default: { t: (k: string) => k, language: 'en' },
}));

import { loadActivityItems } from '@/services/activity';
import type { LocalTransaction } from '@/services/storage';

const ADDR = '0x' + '11'.repeat(20);

const sendRecord = (id: string, timestamp: number): LocalTransaction => ({
  id,
  userOpHash: id,
  txHash: '',
  from: ADDR,
  to: '0x' + '22'.repeat(20),
  value: '1',
  symbol: 'USDC.e',
  decimals: 6,
  chainId: 1,
  timestamp,
  status: 'confirmed',
  type: 'send',
});

const seed = (txs: LocalTransaction[]) =>
  mockStorage.set('vela.transactionHistory', JSON.stringify(txs));

beforeEach(() => {
  mockStorage.clear();
  jest.clearAllMocks();
});

describe('activity - loadActivityItems de-dup', () => {
  test('collapses records sharing an id to a single feed row', async () => {
    const dupId = '0x88fe569cebb551c0c51b5ba3bab67b80fddfc52b9a18ec9618b19c69b4b7a2e4';
    seed([sendRecord(dupId, 2000), sendRecord(dupId, 1000)]);

    const items = await loadActivityItems(ADDR);
    expect(items).toHaveLength(1);
    expect(items[0].id).toBe(dupId);
  });

  test('produces unique FlatList keys across a mixed feed', async () => {
    seed([
      sendRecord('0xaaa', 3000),
      sendRecord('0xaaa', 2000), // duplicate of the above
      sendRecord('0xbbb', 1000),
    ]);

    const items = await loadActivityItems(ADDR);
    const ids = items.map((i) => i.id);
    expect(ids).toHaveLength(2);
    expect(new Set(ids).size).toBe(ids.length); // all keys unique
  });

  test('leaves a feed with already-unique ids untouched', async () => {
    seed([sendRecord('0xaaa', 2000), sendRecord('0xbbb', 1000)]);

    const items = await loadActivityItems(ADDR);
    expect(items.map((i) => i.id)).toEqual(['0xaaa', '0xbbb']); // newest first, both kept
  });
});
