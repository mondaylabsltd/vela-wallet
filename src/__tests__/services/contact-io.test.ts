/**
 * contact-io: JSON/CSV serialize+parse round-trips (pure) and the existing-wins
 * importContacts path (run against the real contacts store, storage mocked).
 */
const mem = new Map<string, string>();
jest.mock('@react-native-async-storage/async-storage', () => ({
  getItem: jest.fn(async (k: string) => mem.get(k) ?? null),
  setItem: jest.fn(async (k: string, v: string) => { mem.set(k, v); }),
}));
jest.mock('@/services/storage', () => ({ loadTransactions: jest.fn(async () => []) }));
jest.mock('@/services/recipient-identity', () => ({ resolveRecipientIdentity: jest.fn(async () => null) }));
jest.mock('@/services/rpc-pool', () => ({ poolRpcCall: jest.fn() }));

import {
  serializeContactsJson, serializeContactsCsv, parseContactsFile, importContacts,
  type ExportedContact,
} from '@/services/contact-io';
import {
  saveContact, saveGroup, getSavedContacts, getGroups, getSavedContact, getGroupMembers,
  clearContactsCache, type Contact, type ContactGroup,
} from '@/services/contacts';

const A = '0x' + 'aa'.repeat(20);
const B = '0x' + 'bb'.repeat(20);
const C = '0x' + 'cc'.repeat(20);

const realNow = Date.now;
beforeEach(() => {
  mem.clear();
  clearContactsCache();
  Date.now = () => 1000;
});
afterEach(() => { Date.now = realNow; });

const contact = (address: string, name?: string, extra: Partial<Contact> = {}): Contact => ({
  address, name, kind: 'unknown', txCount: 0, lastUsed: 0, firstSeen: 0, source: 'manual', ...extra,
});
const group = (id: string, name: string, members: string[], color?: string): ContactGroup => ({ id, name, members, color });

describe('JSON serialize/parse round-trip', () => {
  test('carries contacts + groups (members by address), stamps version/date', () => {
    const json = serializeContactsJson(
      [contact(A, 'Alice', { favorite: true, note: 'lead' }), contact(B, 'Bob')],
      [group('grp_1', 'Payroll', [A, B], 'blue')],
      '2026-07-01T00:00:00.000Z',
    );
    const parsed = JSON.parse(json);
    expect(parsed).toMatchObject({ version: 1, exportedAt: '2026-07-01T00:00:00.000Z' });

    const back = parseContactsFile(json, 'backup.json');
    expect(back.contacts).toEqual([
      { address: A, name: 'Alice', note: 'lead', favorite: true },
      { address: B, name: 'Bob' },
    ]);
    expect(back.groups).toEqual([{ name: 'Payroll', color: 'blue', members: [A, B] }]);
  });

  test('detects JSON by shape even without a filename', () => {
    const json = serializeContactsJson([contact(A, 'Alice')], []);
    expect(parseContactsFile(json).contacts[0].address).toBe(A);
  });

  test('malformed JSON ⇒ empty result, never throws', () => {
    expect(parseContactsFile('{ not json', 'x.json')).toEqual({ contacts: [], groups: [] });
  });
});

describe('CSV serialize/parse round-trip', () => {
  test('per-row group names invert back into groups; favorite + note preserved', () => {
    const csv = serializeContactsCsv(
      [contact(A, 'Alice', { favorite: true, note: 'lead' }), contact(B, 'Bob')],
      [group('grp_1', 'Payroll', [A, B]), group('grp_2', 'Friends', [A])],
    );
    expect(csv.split('\n')[0]).toBe('address,name,note,favorite,groups');

    const back = parseContactsFile(csv, 'contacts.csv');
    expect(back.contacts).toEqual([
      { address: A, name: 'Alice', note: 'lead', favorite: true },
      { address: B, name: 'Bob' },
    ]);
    // Alice is in both groups; Bob only in Payroll
    const payroll = back.groups.find((g) => g.name === 'Payroll');
    const friends = back.groups.find((g) => g.name === 'Friends');
    expect(payroll?.members.sort()).toEqual([A, B].sort());
    expect(friends?.members).toEqual([A]);
  });

  test('quotes a name containing a comma and round-trips it intact', () => {
    const csv = serializeContactsCsv([contact(A, 'Doe, Jane')], []);
    expect(csv).toContain('"Doe, Jane"');
    expect(parseContactsFile(csv, 'c.csv').contacts[0].name).toBe('Doe, Jane');
  });

  test('a headerless CSV still parses by position', () => {
    const back = parseContactsFile(`${A},Alice,,true,`, 'c.csv');
    expect(back.contacts[0]).toEqual({ address: A, name: 'Alice', favorite: true });
  });
});

describe('importContacts — existing-wins', () => {
  test('adds only new addresses; never clobbers a local contact', async () => {
    await saveContact({ address: A, name: 'Local Alice', favorite: true });

    const file: ExportedContact[] = [
      { address: A, name: 'Imported Alice' }, // exists → skipped, local wins
      { address: B, name: 'Bob' },            // new → added
      { address: '0xnope', name: 'Bad' },     // invalid → dropped
    ];
    const report = await importContacts({ contacts: file, groups: [] });

    expect(report).toEqual({ added: 1, skipped: 1, invalid: 1, groupsCreated: 0 });
    expect((await getSavedContact(A))?.name).toBe('Local Alice'); // untouched
    expect((await getSavedContact(A))?.favorite).toBe(true);
    expect((await getSavedContact(B))?.name).toBe('Bob');
  });

  test('a duplicate address within the same file is added once', async () => {
    const report = await importContacts({
      contacts: [{ address: A, name: 'One' }, { address: A, name: 'Two' }],
      groups: [],
    });
    expect(report).toMatchObject({ added: 1, skipped: 1 });
    expect((await getSavedContacts()).filter((c) => c.address === A)).toHaveLength(1);
  });

  test('creates missing groups and attaches only the newly-added members', async () => {
    await saveContact({ address: A, name: 'Local Alice' }); // pre-existing
    const report = await importContacts({
      contacts: [{ address: A }, { address: B }, { address: C }],
      groups: [{ name: 'Payroll', members: [A, B, C] }],
    });
    expect(report).toMatchObject({ added: 2, groupsCreated: 1 });
    const payroll = (await getGroups()).find((g) => g.name === 'Payroll')!;
    // A was pre-existing (skipped), so only B + C get attached
    expect(payroll.members.sort()).toEqual([B, C].sort());
  });

  test('unions new members into an existing same-named group, leaving current members', async () => {
    await saveContact({ address: A, name: 'Alice' });
    const g = await saveGroup({ name: 'Payroll', members: [A] });
    await importContacts({ contacts: [{ address: B }], groups: [{ name: 'payroll', members: [B] }] });
    const payroll = (await getGroups()).find((x) => x.id === g.id)!;
    expect(payroll.members.sort()).toEqual([A, B].sort()); // A kept, B unioned in
  });

  test('a full JSON backup restores into a fresh, empty store', async () => {
    // simulate an export from one device…
    await saveContact({ address: A, name: 'Alice', favorite: true });
    await saveContact({ address: B, name: 'Bob' });
    await saveGroup({ name: 'Payroll', members: [A, B] });
    const json = serializeContactsJson(await getSavedContacts(), await getGroups());

    // …restored on a fresh device
    clearContactsCache();
    mem.clear();
    const report = await importContacts(parseContactsFile(json, 'backup.json'));
    expect(report).toMatchObject({ added: 2, groupsCreated: 1 });
    expect(await getSavedContacts()).toHaveLength(2);
    const members = await getGroupMembers((await getGroups())[0].id);
    expect(members.map((m) => m.address).sort()).toEqual([A, B].sort());
  });
});
