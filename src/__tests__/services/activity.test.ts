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

import { loadActivityItems, loadConnectionEvents } from '@/services/activity';
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

describe('activity - loadConnectionEvents (issue #88: dApp tx reviewable when disconnected)', () => {
  const dappRecord = (
    id: string,
    timestamp: number,
    type: LocalTransaction['type'],
    from = ADDR,
  ): LocalTransaction => ({
    id, userOpHash: id, txHash: '', from, to: '0x' + '22'.repeat(20),
    value: '0', symbol: '', decimals: 18, chainId: 1, timestamp,
    status: 'pending', type, dappOrigin: 'app.uniswap.org',
  });

  test('returns dApp records for the address regardless of live connection status', async () => {
    seed([
      dappRecord('0xtx', 3000, 'dapp_tx'),
      dappRecord('0xconn', 2000, 'connect'),
      sendRecord('0xsend', 1500),                       // value transfer → not a connection event
      dappRecord('0xother', 1000, 'dapp_tx', '0x' + '99'.repeat(20)), // different owner
    ]);
    const events = await loadConnectionEvents(ADDR);
    expect(events.map((e) => e.id)).toEqual(['0xtx', '0xconn']); // newest-first, only my dApp records
    expect(events[0].tx.id).toBe('0xtx'); // carries the full tx for the review sheet
  });

  test('a just-signed pending dApp tx is present (nothing to review → something to review)', async () => {
    seed([dappRecord('0xpending', 5000, 'dapp_tx')]);
    const events = await loadConnectionEvents(ADDR);
    expect(events).toHaveLength(1);
    expect(events[0].status).toBe('pending');
  });
});

describe('activity - batch grouping', () => {
  const batchLine = (
    userOpHash: string,
    i: number,
    over: Partial<LocalTransaction>,
  ): LocalTransaction => ({
    ...sendRecord(`${userOpHash}-${i}`, 5000),
    userOpHash,
    ...over,
  });

  test('split (1 token → N recipients) collapses to one row + breakdown', async () => {
    const op = '0xsplit';
    seed([
      batchLine(op, 0, { to: '0x' + 'a1'.repeat(20), value: '10', symbol: 'USDC' }),
      batchLine(op, 1, { to: '0x' + 'b2'.repeat(20), value: '20', symbol: 'USDC' }),
      batchLine(op, 2, { to: '0x' + 'c3'.repeat(20), value: '30', symbol: 'USDC' }),
    ]);

    const items = await loadActivityItems(ADDR);
    expect(items).toHaveLength(1);
    const it = items[0];
    expect(it.id).toBe(op); // keyed by the shared userOpHash
    expect(it.batch?.kind).toBe('split');
    expect(it.batch?.count).toBe(3);
    expect(it.batch?.ids).toEqual([`${op}-0`, `${op}-1`, `${op}-2`]);
    expect(it.batch?.symbol).toBe('USDC');
    expect(it.address).toBeUndefined(); // split has no single recipient
  });

  test('multiSelect (N tokens → 1 recipient) collapses to one row', async () => {
    const op = '0xmulti';
    const to = '0x' + 'd4'.repeat(20);
    seed([
      batchLine(op, 0, { to, value: '10', symbol: 'USDC' }),
      batchLine(op, 1, { to, value: '5', symbol: 'DAI' }),
    ]);

    const items = await loadActivityItems(ADDR);
    expect(items).toHaveLength(1);
    expect(items[0].batch?.kind).toBe('multiSelect');
    expect(items[0].batch?.transfers.map((x) => x.symbol)).toEqual(['USDC', 'DAI']);
    expect(items[0].address).toBe(to); // single recipient drives the "to" subtitle
  });

  test('same-id duplicates in one userOp are NOT treated as a batch', async () => {
    // A resubmitted single send lands two records with the SAME id + userOpHash.
    seed([sendRecord('0xdup', 2000), sendRecord('0xdup', 1000)]);

    const items = await loadActivityItems(ADDR);
    expect(items).toHaveLength(1);
    expect(items[0].batch).toBeUndefined(); // one line, not a batch
  });
});
