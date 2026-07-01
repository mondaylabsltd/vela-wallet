/**
 * Contacts service: persistence, history derivation, merge, sort/search, classify.
 * Storage / history / identity / RPC are mocked — no real I/O.
 */
const mem = new Map<string, string>();
jest.mock('@react-native-async-storage/async-storage', () => ({
  getItem: jest.fn(async (k: string) => mem.get(k) ?? null),
  setItem: jest.fn(async (k: string, v: string) => { mem.set(k, v); }),
}));
jest.mock('@/services/storage', () => ({ loadTransactions: jest.fn(async () => []) }));
jest.mock('@/services/recipient-identity', () => ({ resolveRecipientIdentity: jest.fn(async () => null) }));
jest.mock('@/services/rpc-pool', () => ({ poolRpcCall: jest.fn() }));

import { loadTransactions } from '@/services/storage';
import { poolRpcCall } from '@/services/rpc-pool';
import {
  saveContact, deleteContact, toggleFavorite, getSavedContacts, getAllContacts,
  isSavedContact, sortContacts, matchesQuery, classifyContact, clearContactsCache,
  type Contact,
} from '@/services/contacts';

const mockTxs = loadTransactions as jest.Mock;
const mockRpc = poolRpcCall as jest.Mock;

const A = '0x' + 'aa'.repeat(20);
const B = '0x' + 'bb'.repeat(20);
const ME = '0x' + '11'.repeat(20);

const sendTx = (to: string, timestamp: number, toName?: string) => ({ type: 'send', to, timestamp, toName, from: ME });

const realNow = Date.now;
beforeEach(() => {
  mem.clear();
  clearContactsCache();
  mockTxs.mockReset().mockResolvedValue([]);
  mockRpc.mockReset();
  Date.now = () => 1000; // deterministic save timestamps
});
afterEach(() => { Date.now = realNow; });

describe('saved contacts CRUD', () => {
  test('save, read, idempotent update by address', async () => {
    await saveContact({ address: A.toUpperCase(), name: 'Alice' });
    let saved = await getSavedContacts();
    expect(saved).toHaveLength(1);
    expect(saved[0]).toMatchObject({ address: A, name: 'Alice', source: 'manual' });

    await saveContact({ address: A, name: 'Alice 2', note: 'hi' });
    saved = await getSavedContacts();
    expect(saved).toHaveLength(1); // still one — keyed by address
    expect(saved[0]).toMatchObject({ name: 'Alice 2', note: 'hi' });
  });

  test('delete removes', async () => {
    await saveContact({ address: A, name: 'Alice' });
    await deleteContact(A);
    expect(await getSavedContacts()).toHaveLength(0);
  });

  test('isSavedContact reflects state', async () => {
    expect(await isSavedContact(A)).toBe(false);
    await saveContact({ address: A });
    expect(await isSavedContact(A.toUpperCase())).toBe(true);
  });

  test('toggleFavorite flips a saved contact and promotes a suggestion', async () => {
    await saveContact({ address: A, name: 'Alice' });
    await toggleFavorite(A);
    expect((await getSavedContacts()).find((c) => c.address === A)?.favorite).toBe(true);

    // B isn't saved yet → favoriting it promotes it to a saved contact
    await toggleFavorite(B);
    const b = (await getSavedContacts()).find((c) => c.address === B);
    expect(b).toMatchObject({ favorite: true, source: 'manual' });
  });

  test('persists across a cold load (cache cleared)', async () => {
    await saveContact({ address: A, name: 'Alice' });
    clearContactsCache(); // simulate app restart (storage persists)
    expect(await isSavedContact(A)).toBe(true);
  });
});

describe('derivation from send history', () => {
  test('only `send` txs become suggestions; dApp calls excluded', async () => {
    mockTxs.mockResolvedValue([
      sendTx(A, 100),
      { type: 'dapp_tx', to: B, timestamp: 200, from: ME }, // contract call — excluded
      { type: 'receive', to: ME, timestamp: 300, from: A },  // not a send — excluded
    ]);
    const all = await getAllContacts(ME);
    expect(all.map((c) => c.address)).toEqual([A]);
    expect(all[0]).toMatchObject({ source: 'auto', txCount: 1 });
  });

  test('dedups a repeated recipient, counts and tracks recency', async () => {
    mockTxs.mockResolvedValue([sendTx(A, 100), sendTx(A, 300), sendTx(A, 200)]);
    const all = await getAllContacts(ME);
    expect(all).toHaveLength(1);
    expect(all[0]).toMatchObject({ txCount: 3, lastUsed: 300, firstSeen: 100 });
  });

  test('skips sends to self', async () => {
    mockTxs.mockResolvedValue([sendTx(ME, 100), sendTx(A, 200)]);
    const all = await getAllContacts(ME);
    expect(all.map((c) => c.address)).toEqual([A]);
  });

  test('ignores malformed recipients', async () => {
    mockTxs.mockResolvedValue([sendTx('0x123', 100), sendTx(A, 200)]);
    const all = await getAllContacts(ME);
    expect(all.map((c) => c.address)).toEqual([A]);
  });

  test('carries toName from history as resolvedName', async () => {
    mockTxs.mockResolvedValue([sendTx(A, 100, 'vitalik.eth')]);
    const all = await getAllContacts(ME);
    expect(all[0].resolvedName).toBe('vitalik.eth');
  });
});

describe('merge of saved + history', () => {
  test('saved contact wins on name; recency refreshed from history', async () => {
    await saveContact({ address: A, name: 'Alice' }); // saved at t=1000
    mockTxs.mockResolvedValue([sendTx(A, 2000), sendTx(A, 1500)]); // later sends
    const all = await getAllContacts(ME);
    expect(all).toHaveLength(1);
    expect(all[0]).toMatchObject({ name: 'Alice', source: 'manual', txCount: 2, lastUsed: 2000 });
  });

  test('a saved-only contact (never sent to) still appears', async () => {
    await saveContact({ address: B, name: 'Bob' });
    mockTxs.mockResolvedValue([sendTx(A, 100)]);
    const all = await getAllContacts(ME);
    expect(all.map((c) => c.address).sort()).toEqual([A, B].sort());
  });
});

describe('delete tombstone (history suggestions)', () => {
  test('deleting a saved contact with send history removes it for good, not just un-favorited', async () => {
    mockTxs.mockResolvedValue([sendTx(A, 500)]); // sent before deletion (delete stamps t=1000)
    await saveContact({ address: A, name: 'Alice', favorite: true });
    expect((await getAllContacts(ME)).map((c) => c.address)).toEqual([A]);

    await deleteContact(A);
    // Regression: used to re-appear as an `auto` suggestion derived from history.
    expect(await getAllContacts(ME)).toHaveLength(0);
  });

  test('a send after deletion re-surfaces the recipient', async () => {
    mockTxs.mockResolvedValue([sendTx(A, 500)]);
    await deleteContact(A); // tombstone at t=1000
    expect(await getAllContacts(ME)).toHaveLength(0);

    mockTxs.mockResolvedValue([sendTx(A, 500), sendTx(A, 2000)]); // transacted again, after deletion
    expect((await getAllContacts(ME)).map((c) => c.address)).toEqual([A]);
  });

  test('re-saving a deleted address clears the tombstone', async () => {
    mockTxs.mockResolvedValue([sendTx(A, 500)]);
    await deleteContact(A);
    expect(await getAllContacts(ME)).toHaveLength(0);

    await saveContact({ address: A, name: 'Alice' });
    const all = await getAllContacts(ME);
    expect(all.map((c) => c.address)).toEqual([A]);
    expect(all[0]).toMatchObject({ name: 'Alice', source: 'manual' });
  });

  test('tombstone persists across a cold load', async () => {
    mockTxs.mockResolvedValue([sendTx(A, 500)]);
    await deleteContact(A);
    clearContactsCache(); // simulate app restart (storage persists)
    expect(await getAllContacts(ME)).toHaveLength(0);
  });
});

describe('sort + search', () => {
  test('favorites first, then most-recent', () => {
    const list: Contact[] = [
      { address: A, kind: 'unknown', txCount: 1, lastUsed: 100, firstSeen: 0, source: 'auto' },
      { address: B, kind: 'unknown', txCount: 1, lastUsed: 200, firstSeen: 0, source: 'auto' },
      { address: '0x' + 'cc'.repeat(20), kind: 'unknown', txCount: 1, lastUsed: 50, firstSeen: 0, source: 'manual', favorite: true },
    ];
    const sorted = sortContacts(list);
    expect(sorted[0].favorite).toBe(true);        // favourite wins despite older lastUsed
    expect(sorted[1].address).toBe(B);            // then most-recent
    expect(sorted[2].address).toBe(A);
  });

  test('matchesQuery on address, name, resolvedName', () => {
    const c: Contact = { address: A, name: 'Alice', resolvedName: 'alice.eth', kind: 'eoa', txCount: 0, lastUsed: 0, firstSeen: 0, source: 'manual' };
    expect(matchesQuery(c, '')).toBe(true);
    expect(matchesQuery(c, 'ali')).toBe(true);
    expect(matchesQuery(c, '.eth')).toBe(true);
    expect(matchesQuery(c, 'aaaa')).toBe(true);   // address prefix
    expect(matchesQuery(c, 'zzz')).toBe(false);
  });
});

describe('classifyContact', () => {
  test('code present → account, empty → eoa, error → unknown', async () => {
    mockRpc.mockResolvedValueOnce({ result: '0x60016002' });
    expect(await classifyContact(1, A)).toBe('account');

    mockRpc.mockResolvedValueOnce({ result: '0x' });
    expect(await classifyContact(1, B)).toBe('eoa');

    mockRpc.mockResolvedValueOnce({ error: { message: 'down' } });
    expect(await classifyContact(1, '0x' + 'cc'.repeat(20))).toBe('unknown');
  });

  test('caches a resolved verdict', async () => {
    mockRpc.mockResolvedValue({ result: '0x' });
    await classifyContact(1, A);
    await classifyContact(1, A);
    expect(mockRpc).toHaveBeenCalledTimes(1);
  });
});
