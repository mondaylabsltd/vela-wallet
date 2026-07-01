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

const KEY = 'vela.contacts';

let _saved: Contact[] | null = null;

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
    return merged;
  });
}

export async function deleteContact(address: string): Promise<void> {
  return withWriteLock(async () => {
    const addr = address.toLowerCase();
    await persist((await loadSaved()).filter((x) => x.address !== addr));
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
  const byAddr = new Map(saved.map((c) => [c.address, { ...c }]));
  for (const a of await deriveFromHistory(myAddress)) {
    const existing = byAddr.get(a.address);
    if (existing) {
      existing.txCount = Math.max(existing.txCount, a.txCount);
      existing.lastUsed = Math.max(existing.lastUsed, a.lastUsed);
      if (!existing.resolvedName && a.resolvedName) existing.resolvedName = a.resolvedName;
    } else {
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
  kindCache.clear();
}
