/**
 * Contacts (address book).
 *
 * A contact is a *recipient* you've sent to, or one you save by hand — never a
 * contract you merely called. The auto-suggestion source is deliberately narrow:
 * only `type: 'send'` transactions (native/ERC-20 transfers), whose `to` is the
 * actual recipient. dApp contract calls (`type: 'dapp_tx'`) are excluded, so
 * routers/tokens/dApps never pollute the book. A code-bearing recipient is a
 * smart-contract *account* (another AA/Safe wallet), lazily labelled.
 *
 * Two tiers, unified by {@link getAllContacts}:
 *   - saved (`source: 'manual'`) — persisted in `vela.contacts`, user-named/favourited
 *   - auto  (`source: 'auto'`)   — derived live from send history, promotable by saving
 *
 * All on-device, no sync. Identity/classification are best-effort and cached.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import { loadTransactions } from '@/services/storage';
import { isAddress } from '@/models/types';
import { resolveRecipientIdentity } from '@/services/recipient-identity';
import { poolRpcCall } from '@/services/rpc-pool';

export type ContactKind = 'eoa' | 'account' | 'unknown';

export interface Contact {
  /** Lowercased address — the canonical key. */
  address: string;
  /** User-given name. Wins over `resolvedName` for display. */
  name?: string;
  /** Cached identity name (ENS / Basename / passkey), for display + search. */
  resolvedName?: string;
  resolvedSource?: string;
  /** EOA / smart-contract account / unknown — lazily filled by {@link classifyContact}. */
  kind: ContactKind;
  favorite?: boolean;
  note?: string;
  /** Count of `send` txs to this address (recency/sort signal). */
  txCount: number;
  /** ms timestamp of the most recent send. */
  lastUsed: number;
  firstSeen: number;
  /** `manual` = saved/named by the user; `auto` = a live suggestion from history. */
  source: 'manual' | 'auto';
}

/**
 * A named set of contacts (e.g. "Payroll") — a first-class registry object rather
 * than a tag, so it can be empty, renamed, and reordered independently of its
 * members. `members` are lowercased addresses; a whole group can be picked as the
 * recipients of a split send (one row per member).
 */
export interface ContactGroup {
  id: string;
  name: string;
  /** Optional accent hue for the group chip (a `color.*` token name or hex). */
  color?: string;
  /** Lowercased member addresses, in display order. */
  members: string[];
}

const KEY = 'vela.contacts';
const DISMISSED_KEY = 'vela.contacts.dismissed';
const GROUPS_KEY = 'vela.contactGroups';

let _saved: Contact[] | null = null;
/** address → ms timestamp it was deleted. A history-derived suggestion is
 *  suppressed unless the user has transacted with it *since* the deletion. */
let _dismissed: Record<string, number> | null = null;
let _groups: ContactGroup[] | null = null;

// Serialize read-modify-write mutations of the `_saved` cache. Several callers can
// fire concurrently — e.g. multiple RecipientTrust rows each resolving an identity
// and writing the name back at once — and would otherwise interleave
// loadSaved()→map→persist and silently drop each other's writes (last-writer-wins).
// Chaining every mutator through one promise makes each write atomic.
let _writeChain: Promise<unknown> = Promise.resolve();
function withWriteLock<T>(fn: () => Promise<T>): Promise<T> {
  const run = _writeChain.then(fn, fn);
  _writeChain = run.then(() => undefined, () => undefined);
  return run;
}

async function loadSaved(): Promise<Contact[]> {
  if (_saved) return _saved;
  try {
    const raw = await AsyncStorage.getItem(KEY);
    _saved = raw ? (JSON.parse(raw) as Contact[]) : [];
  } catch {
    _saved = [];
  }
  return _saved;
}

async function persist(list: Contact[]): Promise<void> {
  _saved = list;
  try {
    await AsyncStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    /* storage unavailable — keep the in-memory copy for this session */
  }
}

async function loadDismissed(): Promise<Record<string, number>> {
  if (_dismissed) return _dismissed;
  try {
    const raw = await AsyncStorage.getItem(DISMISSED_KEY);
    _dismissed = raw ? (JSON.parse(raw) as Record<string, number>) : {};
  } catch {
    _dismissed = {};
  }
  return _dismissed;
}

async function persistDismissed(map: Record<string, number>): Promise<void> {
  _dismissed = map;
  try {
    await AsyncStorage.setItem(DISMISSED_KEY, JSON.stringify(map));
  } catch {
    /* storage unavailable — keep the in-memory copy for this session */
  }
}

async function loadGroups(): Promise<ContactGroup[]> {
  if (_groups) return _groups;
  try {
    const raw = await AsyncStorage.getItem(GROUPS_KEY);
    _groups = raw ? (JSON.parse(raw) as ContactGroup[]) : [];
  } catch {
    _groups = [];
  }
  return _groups;
}

async function persistGroups(list: ContactGroup[]): Promise<void> {
  _groups = list;
  try {
    await AsyncStorage.setItem(GROUPS_KEY, JSON.stringify(list));
  } catch {
    /* storage unavailable — keep the in-memory copy for this session */
  }
}

/** Lowercase, drop invalid, de-dupe (first wins) — the canonical member shape. */
function normalizeMembers(addrs: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const a of addrs) {
    if (!isAddress(a)) continue;
    const low = a.toLowerCase();
    if (seen.has(low)) continue;
    seen.add(low);
    out.push(low);
  }
  return out;
}

/** Next stable, collision-free group id: one past the largest numeric suffix.
 *  Deterministic (no Date/random), so it survives cold reload and is test-safe. */
function nextGroupId(list: ContactGroup[]): string {
  const max = list.reduce((m, g) => {
    const n = parseInt(g.id.replace(/^grp_/, ''), 10);
    return Number.isFinite(n) && n > m ? n : m;
  }, 0);
  return `grp_${max + 1}`;
}

/** Manually-saved contacts only (no history suggestions). */
export async function getSavedContacts(): Promise<Contact[]> {
  return [...(await loadSaved())];
}

export interface SaveContactInput {
  address: string;
  name?: string;
  note?: string;
  favorite?: boolean;
  kind?: ContactKind;
  resolvedName?: string;
  resolvedSource?: string;
}

/** Create or update a saved contact (idempotent on address). */
export async function saveContact(input: SaveContactInput): Promise<Contact> {
  return withWriteLock(async () => {
    const addr = input.address.toLowerCase();
    const list = await loadSaved();
    const now = Date.now();
    const existing = list.find((x) => x.address === addr);
    const merged: Contact = existing
      ? { ...existing, ...input, address: addr, source: 'manual' }
      : {
          txCount: 0,
          lastUsed: now,
          firstSeen: now,
          ...input,
          address: addr,
          kind: input.kind ?? 'unknown',
          source: 'manual',
        };
    await persist([merged, ...list.filter((x) => x.address !== addr)]);
    // Re-adding an address clears any prior deletion tombstone.
    const dismissed = await loadDismissed();
    if (dismissed[addr] != null) {
      const { [addr]: _removed, ...rest } = dismissed;
      await persistDismissed(rest);
    }
    return merged;
  });
}

export async function deleteContact(address: string): Promise<void> {
  return withWriteLock(async () => {
    const addr = address.toLowerCase();
    await persist((await loadSaved()).filter((x) => x.address !== addr));
    // Tombstone the address so it doesn't immediately re-appear as a
    // history-derived suggestion. Cleared if the user saves it again, or
    // superseded by any later send (see getAllContacts).
    await persistDismissed({ ...(await loadDismissed()), [addr]: Date.now() });
    // Cascade: drop the address from every group so a member never dangles.
    const groups = await loadGroups();
    if (groups.some((g) => g.members.includes(addr))) {
      await persistGroups(
        groups.map((g) => (g.members.includes(addr) ? { ...g, members: g.members.filter((m) => m !== addr) } : g)),
      );
    }
  });
}

export async function updateContact(address: string, patch: Partial<Contact>): Promise<void> {
  return withWriteLock(async () => {
    const addr = address.toLowerCase();
    await persist(
      (await loadSaved()).map((x) => (x.address === addr ? { ...x, ...patch, address: addr } : x)),
    );
  });
}

/** Favourite a contact — promotes a live suggestion to a saved contact. */
export async function toggleFavorite(address: string): Promise<void> {
  const addr = address.toLowerCase();
  const list = await loadSaved();
  const c = list.find((x) => x.address === addr);
  if (c) await updateContact(addr, { favorite: !c.favorite });
  else await saveContact({ address: addr, favorite: true });
}

export async function isSavedContact(address: string): Promise<boolean> {
  return (await loadSaved()).some((x) => x.address === address.toLowerCase());
}

/** The user's saved contact for an address, or null. The anti-poisoning signal:
 *  a recipient you previously saved (and named) is one you've vouched for. */
export async function getSavedContact(address: string): Promise<Contact | null> {
  if (!isAddress(address)) return null;
  return (await loadSaved()).find((x) => x.address === address.toLowerCase()) ?? null;
}

/**
 * The unified address book: saved contacts merged with live suggestions derived
 * from send history. Saved entries win on identity/name; recency and txCount are
 * refreshed from history so a saved contact still sorts by recent use.
 */
export async function getAllContacts(myAddress?: string): Promise<Contact[]> {
  const saved = await loadSaved();
  const dismissed = await loadDismissed();
  const byAddr = new Map(saved.map((c) => [c.address, { ...c }]));
  for (const a of await deriveFromHistory(myAddress)) {
    const existing = byAddr.get(a.address);
    if (existing) {
      existing.txCount = Math.max(existing.txCount, a.txCount);
      existing.lastUsed = Math.max(existing.lastUsed, a.lastUsed);
      if (!existing.resolvedName && a.resolvedName) existing.resolvedName = a.resolvedName;
    } else {
      // Suppress a deleted recipient unless it's been used since deletion.
      const dismissedAt = dismissed[a.address];
      if (dismissedAt != null && a.lastUsed <= dismissedAt) continue;
      byAddr.set(a.address, a);
    }
  }
  return [...byAddr.values()];
}

/** Recipients from `send` history, one per address, with counts + recency. */
async function deriveFromHistory(myAddress?: string): Promise<Contact[]> {
  let txs;
  try {
    txs = await loadTransactions();
  } catch {
    return [];
  }
  const me = myAddress?.toLowerCase();
  const map = new Map<string, Contact>();
  for (const t of txs) {
    if (t.type !== 'send') continue; // transfers only — never dApp contract calls
    const to = t.to?.toLowerCase();
    if (!to || !isAddress(to) || to === me) continue;
    const ts = t.timestamp ?? 0;
    const existing = map.get(to);
    if (existing) {
      existing.txCount += 1;
      existing.lastUsed = Math.max(existing.lastUsed, ts);
      existing.firstSeen = Math.min(existing.firstSeen, ts);
      if (!existing.resolvedName && t.toName) existing.resolvedName = t.toName;
    } else {
      map.set(to, {
        address: to,
        resolvedName: t.toName,
        kind: 'unknown',
        txCount: 1,
        lastUsed: ts,
        firstSeen: ts,
        source: 'auto',
      });
    }
  }
  return [...map.values()];
}

// ── Contact groups ───────────────────────────────────────────────────────────

/** All groups (copies — safe for the caller to mutate). */
export async function getGroups(): Promise<ContactGroup[]> {
  return (await loadGroups()).map((g) => ({ ...g, members: [...g.members] }));
}

export interface SaveGroupInput {
  /** Omit to create; pass an existing id to update in place. */
  id?: string;
  name: string;
  color?: string;
  /** When updating, omitting `members` leaves the existing membership untouched. */
  members?: string[];
}

/** Create or update a group. New groups get a stable, collision-free id; members
 *  are normalised (lowercased, valid-only, de-duped). */
export async function saveGroup(input: SaveGroupInput): Promise<ContactGroup> {
  return withWriteLock(async () => {
    const list = await loadGroups();
    const existing = input.id ? list.find((g) => g.id === input.id) : undefined;
    if (existing) {
      const merged: ContactGroup = {
        ...existing,
        name: input.name.trim() || existing.name,
        color: input.color ?? existing.color,
        members: input.members ? normalizeMembers(input.members) : existing.members,
      };
      await persistGroups(list.map((g) => (g.id === existing.id ? merged : g)));
      return merged;
    }
    const group: ContactGroup = {
      id: input.id ?? nextGroupId(list),
      name: input.name.trim(),
      color: input.color,
      members: normalizeMembers(input.members ?? []),
    };
    await persistGroups([...list, group]);
    return group;
  });
}

export async function deleteGroup(id: string): Promise<void> {
  return withWriteLock(async () => {
    await persistGroups((await loadGroups()).filter((g) => g.id !== id));
  });
}

export async function setGroupMembers(id: string, addrs: string[]): Promise<void> {
  return withWriteLock(async () => {
    const members = normalizeMembers(addrs);
    await persistGroups((await loadGroups()).map((g) => (g.id === id ? { ...g, members } : g)));
  });
}

export async function addToGroup(id: string, address: string): Promise<void> {
  if (!isAddress(address)) return;
  return withWriteLock(async () => {
    const addr = address.toLowerCase();
    await persistGroups(
      (await loadGroups()).map((g) =>
        g.id === id && !g.members.includes(addr) ? { ...g, members: [...g.members, addr] } : g,
      ),
    );
  });
}

export async function removeFromGroup(id: string, address: string): Promise<void> {
  return withWriteLock(async () => {
    const addr = address.toLowerCase();
    await persistGroups(
      (await loadGroups()).map((g) => (g.id === id ? { ...g, members: g.members.filter((m) => m !== addr) } : g)),
    );
  });
}

/**
 * Resolve a group's members to contacts, in membership order. A member with a
 * saved contact carries its name/kind; a member without one (e.g. imported, or a
 * bare address) is returned as a minimal `auto` contact so send-to-group never
 * silently drops a payee.
 */
export async function getGroupMembers(id: string): Promise<Contact[]> {
  const group = (await loadGroups()).find((g) => g.id === id);
  if (!group) return [];
  const byAddr = new Map((await loadSaved()).map((c) => [c.address, c]));
  return group.members.map(
    (addr) =>
      byAddr.get(addr) ?? { address: addr, kind: 'unknown', txCount: 0, lastUsed: 0, firstSeen: 0, source: 'auto' },
  );
}

/** The ids of every group a given address belongs to (for the contact form chips). */
export async function getGroupsForAddress(address: string): Promise<string[]> {
  const addr = address.toLowerCase();
  return (await loadGroups()).filter((g) => g.members.includes(addr)).map((g) => g.id);
}

/** Best-effort identity name (ENS / Basename / passkey) for auto-naming. */
export async function enrichContactIdentity(address: string): Promise<{ name?: string; source?: string }> {
  try {
    const id = await resolveRecipientIdentity(address);
    return id ? { name: id.name, source: id.source } : {};
  } catch {
    return {};
  }
}

const kindCache = new Map<string, ContactKind>();

/** Best-effort: is this recipient an EOA or a smart-contract account? */
export async function classifyContact(chainId: number, address: string): Promise<ContactKind> {
  const addr = address.toLowerCase();
  const cached = kindCache.get(addr);
  if (cached) return cached;
  try {
    const res = await poolRpcCall('eth_getCode', [addr, 'latest'], chainId);
    if (res?.error || typeof res?.result !== 'string') return 'unknown';
    const kind: ContactKind = res.result !== '0x' && res.result.length > 2 ? 'account' : 'eoa';
    kindCache.set(addr, kind);
    return kind;
  } catch {
    return 'unknown';
  }
}

/** User name → resolved identity → '' (caller falls back to a short address). */
export function contactDisplayName(c: Pick<Contact, 'name' | 'resolvedName'>): string {
  return c.name || c.resolvedName || '';
}

/** Sort: favourites first, then most-recently-used, then name/address. */
export function sortContacts(list: Contact[]): Contact[] {
  return [...list].sort((a, b) => {
    if (!!b.favorite !== !!a.favorite) return b.favorite ? 1 : -1;
    if (b.lastUsed !== a.lastUsed) return b.lastUsed - a.lastUsed;
    return contactDisplayName(a).localeCompare(contactDisplayName(b)) || a.address.localeCompare(b.address);
  });
}

/** Match a contact against a search query (name or address). */
export function matchesQuery(c: Contact, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return (
    c.address.includes(q) ||
    (c.name?.toLowerCase().includes(q) ?? false) ||
    (c.resolvedName?.toLowerCase().includes(q) ?? false)
  );
}

/** Test hook / account switch. */
export function clearContactsCache(): void {
  _saved = null;
  _dismissed = null;
  _groups = null;
  kindCache.clear();
}
