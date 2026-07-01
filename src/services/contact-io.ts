/**
 * contact-io — serialize, parse, and import the address book (contacts + groups).
 *
 * Two formats:
 *   - JSON  — full-fidelity backup/restore ({version, exportedAt, contacts, groups}).
 *   - CSV   — interop (`address,name,note,favorite,groups`); groups `;`-joined per row.
 *
 * Import policy is **existing-wins** (the user's choice): a row whose address is
 * already a saved contact is skipped untouched — import only ADDS new addresses,
 * so a re-import or a restore can never clobber a local rename/favorite/note.
 * Groups are additive: a newly-added contact is attached to a same-named group
 * (created if missing); existing contacts and their memberships are never altered.
 *
 * Serialize/parse are pure; only {@link importContacts} and the `export*` helpers
 * touch the contacts service.
 */
import { isAddress } from '@/models/types';
import { splitCsvLine } from '@/services/recipient-table';
import {
  type Contact,
  type ContactGroup,
  getSavedContacts,
  getGroups,
  isSavedContact,
  saveContact,
  saveGroup,
} from '@/services/contacts';

export const CONTACTS_BACKUP_VERSION = 1;

export interface ExportedContact {
  address: string;
  name?: string;
  note?: string;
  favorite?: boolean;
}

/** A group in a backup — members are addresses (not ids), so import maps by name. */
export interface ExportedGroup {
  name: string;
  color?: string;
  members: string[];
}

export interface ContactsBackup {
  version: number;
  exportedAt: string;
  contacts: ExportedContact[];
  groups: ExportedGroup[];
}

export interface ParsedContactsImport {
  contacts: ExportedContact[];
  groups: ExportedGroup[];
}

export interface ImportReport {
  /** New addresses saved. */
  added: number;
  /** Rows skipped because the address already exists (existing-wins) or repeats in the file. */
  skipped: number;
  /** Rows dropped for a malformed address. */
  invalid: number;
  /** New groups created from the import. */
  groupsCreated: number;
}

// ── Serialize ────────────────────────────────────────────────────────────────

function toExportedContact(c: Contact | ExportedContact): ExportedContact {
  const out: ExportedContact = { address: c.address };
  if (c.name) out.name = c.name;
  if ('note' in c && c.note) out.note = c.note;
  if (c.favorite) out.favorite = true;
  return out;
}

export function serializeContactsJson(contacts: (Contact | ExportedContact)[], groups: ContactGroup[], exportedAt?: string): string {
  const backup: ContactsBackup = {
    version: CONTACTS_BACKUP_VERSION,
    exportedAt: exportedAt ?? new Date().toISOString(),
    contacts: contacts.map(toExportedContact),
    groups: groups.map((g) => ({ name: g.name, ...(g.color ? { color: g.color } : {}), members: [...g.members] })),
  };
  return JSON.stringify(backup, null, 2);
}

/** Quote a CSV cell only when it needs it (comma, quote, or newline). */
function csvCell(v: string): string {
  return /[",\n\r]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v;
}

export function serializeContactsCsv(contacts: (Contact | ExportedContact)[], groups: ContactGroup[]): string {
  const groupsByAddr = new Map<string, string[]>();
  for (const g of groups) {
    for (const m of g.members) {
      const arr = groupsByAddr.get(m) ?? [];
      arr.push(g.name);
      groupsByAddr.set(m, arr);
    }
  }
  const header = 'address,name,note,favorite,groups';
  const lines = contacts.map((c) =>
    [
      c.address,
      c.name ?? '',
      ('note' in c && c.note) || '',
      c.favorite ? 'true' : '',
      (groupsByAddr.get(c.address) ?? []).join(';'),
    ]
      .map(csvCell)
      .join(','),
  );
  return [header, ...lines].join('\n');
}

// ── Parse ────────────────────────────────────────────────────────────────────

/** Detect JSON vs CSV by extension/shape, then parse into a normalised shape. */
export function parseContactsFile(content: string, filename?: string): ParsedContactsImport {
  const trimmed = content.replace(/^﻿/, '').trim();
  const looksJson = (!!filename && /\.json$/i.test(filename)) || trimmed.startsWith('{');
  return looksJson ? parseJson(trimmed) : parseCsv(trimmed);
}

function cleanImportedContact(c: Record<string, unknown>): ExportedContact | null {
  if (typeof c.address !== 'string') return null;
  const out: ExportedContact = { address: c.address };
  if (typeof c.name === 'string' && c.name) out.name = c.name;
  if (typeof c.note === 'string' && c.note) out.note = c.note;
  if (c.favorite === true || c.favorite === 'true') out.favorite = true;
  return out;
}

function parseJson(text: string): ParsedContactsImport {
  let data: any;
  try {
    data = JSON.parse(text);
  } catch {
    return { contacts: [], groups: [] };
  }
  const contacts: ExportedContact[] = Array.isArray(data?.contacts)
    ? data.contacts.map(cleanImportedContact).filter((c: ExportedContact | null): c is ExportedContact => !!c)
    : [];
  const groups: ExportedGroup[] = Array.isArray(data?.groups)
    ? data.groups
        .filter((g: any) => g && typeof g.name === 'string')
        .map((g: any) => ({
          name: String(g.name),
          ...(typeof g.color === 'string' ? { color: g.color } : {}),
          members: Array.isArray(g.members) ? g.members.filter((m: any) => typeof m === 'string') : [],
        }))
    : [];
  return { contacts, groups };
}

const nz = (i: number) => (i === -1 ? undefined : i);

function indexColumns(header: string[]): { address: number; name?: number; note?: number; favorite?: number; groups?: number } {
  const find = (kw: string) => header.findIndex((h) => h.toLowerCase() === kw);
  const address = find('address');
  return { address: address === -1 ? 0 : address, name: nz(find('name')), note: nz(find('note')), favorite: nz(find('favorite')), groups: nz(find('groups')) };
}

function parseCsv(text: string): ParsedContactsImport {
  const lines = text.split(/\r\n|\r|\n/).filter((l) => l.trim().length > 0);
  if (lines.length === 0) return { contacts: [], groups: [] };
  const first = splitCsvLine(lines[0], ',').map((c) => c.trim());
  const hasHeader = !first.some((c) => isAddress(c));
  const cols = hasHeader ? indexColumns(first) : { address: 0, name: 1, note: 2, favorite: 3, groups: 4 };
  const dataLines = hasHeader ? lines.slice(1) : lines;

  const contacts: ExportedContact[] = [];
  const groupMap = new Map<string, string[]>();
  for (const line of dataLines) {
    const cells = splitCsvLine(line, ',').map((c) => c.trim());
    const address = cells[cols.address] ?? '';
    if (!isAddress(address)) continue;
    const c: ExportedContact = { address };
    const name = cols.name != null ? cells[cols.name] : undefined;
    const note = cols.note != null ? cells[cols.note] : undefined;
    const fav = cols.favorite != null ? cells[cols.favorite] : undefined;
    if (name) c.name = name;
    if (note) c.note = note;
    if (fav && /^(true|1|yes)$/i.test(fav)) c.favorite = true;
    contacts.push(c);

    const grpCell = cols.groups != null ? cells[cols.groups] : undefined;
    if (grpCell) {
      for (const gname of grpCell.split(';').map((s) => s.trim()).filter(Boolean)) {
        const arr = groupMap.get(gname) ?? [];
        arr.push(address.toLowerCase());
        groupMap.set(gname, arr);
      }
    }
  }
  const groups: ExportedGroup[] = [...groupMap.entries()].map(([name, members]) => ({ name, members }));
  return { contacts, groups };
}

// ── Import (existing-wins) + export helpers ────────────────────────────────────

export async function importContacts(parsed: ParsedContactsImport): Promise<ImportReport> {
  let added = 0;
  let skipped = 0;
  let invalid = 0;
  let groupsCreated = 0;
  const newlyAdded = new Set<string>();

  for (const c of parsed.contacts) {
    if (!isAddress(c.address)) {
      invalid += 1;
      continue;
    }
    const addr = c.address.toLowerCase();
    if (newlyAdded.has(addr) || (await isSavedContact(addr))) {
      skipped += 1; // existing-wins: never overwrite a local contact
      continue;
    }
    await saveContact({ address: addr, name: c.name, note: c.note, favorite: c.favorite });
    newlyAdded.add(addr);
    added += 1;
  }

  if (parsed.groups.length && newlyAdded.size) {
    const existing = await getGroups();
    const byName = new Map(existing.map((g) => [g.name.toLowerCase(), g]));
    for (const g of parsed.groups) {
      const membersToAdd = g.members.map((a) => a.toLowerCase()).filter((a) => newlyAdded.has(a));
      if (membersToAdd.length === 0) continue; // nothing new — leave existing groups alone
      const found = byName.get(g.name.toLowerCase());
      if (found) {
        const union = [...found.members];
        for (const m of membersToAdd) if (!union.includes(m)) union.push(m);
        await saveGroup({ id: found.id, name: found.name, members: union });
      } else {
        const created = await saveGroup({ name: g.name, color: g.color, members: membersToAdd });
        byName.set(g.name.toLowerCase(), created);
        groupsCreated += 1;
      }
    }
  }

  return { added, skipped, invalid, groupsCreated };
}

/** Current address book serialized for a backup file. */
export async function exportContactsJson(exportedAt?: string): Promise<string> {
  return serializeContactsJson(await getSavedContacts(), await getGroups(), exportedAt);
}

export async function exportContactsCsv(): Promise<string> {
  return serializeContactsCsv(await getSavedContacts(), await getGroups());
}
