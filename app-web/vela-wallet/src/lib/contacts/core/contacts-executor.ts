/**
 * The only place the `contacts` core touches the outside world — WEB.
 *
 * Ported from src/services/wallet-state-core/contacts-executor.ts @ e78afdfa
 * (spec 024). The three storage keys keep the camelCase stored shapes and the
 * address→ms tombstone map byte-for-byte — the stored format is the
 * compatibility contract across clients — and the defensive coercion rides
 * along verbatim: serde does not trust, so every row is normalised with the
 * same defaults the TS read sites applied, and anything unsalvageable answers
 * empty exactly as the old `catch { [] }` did.
 *
 * Three web differences, all recorded in contracts/shell-operations.md:
 * - `load_send_history` reads the local tx store 025 built (`records.ts`,
 *   the Expo `vela.transactionHistory` bytes) — since spec 028 US5; before
 *   that it answered truthfully empty.
 * - `resolve_identity` runs the identity waterfall (passkey index → name
 *   services); `classify_recipient` hands the raw `eth_getCode` answer back
 *   — the core owns both projections (address-book kind, risk badge). A
 *   transport-level non-answer is `null` (unknown), never a verdict. Live
 *   since spec 025 Phase 5.
 * - There is no lazy TS contacts cache to invalidate after writes (the Expo
 *   `clearContactsCache()`); the core's ledger is this app's only reader.
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with.
 */

import { resolveRecipientIdentity } from '$lib/services/recipient-identity';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { loadTransactions } from '$lib/services/records';
import { getItem, setItem } from '$lib/services/storage';
import type { LocalTransaction } from '$lib/services/transactions-model';

import type { Contact } from '$lib/core/generated/Contact';
import type { ContactGroup } from '$lib/core/generated/ContactGroup';
import type { ContactKind } from '$lib/core/generated/ContactKind';
import type { ContactShellResult } from '$lib/core/generated/ContactShellResult';
import type { ContactHistoryTx } from '$lib/core/generated/ContactHistoryTx';
import type { ContactTombstone } from '$lib/core/generated/ContactTombstone';
import type { ContactEffect } from './contacts-types';

// The three keys `services/contacts.ts` owns — value formats unchanged.
const CONTACTS_KEY = 'vela.contacts';
const DISMISSED_KEY = 'vela.contacts.dismissed';
const GROUPS_KEY = 'vela.contactGroups';

// ---------------------------------------------------------------------------
// Stored shapes (camelCase, optionals) — the compatibility contract.
// ---------------------------------------------------------------------------

type StoredContact = {
	address: string;
	name?: string;
	resolvedName?: string;
	resolvedSource?: string;
	kind: ContactKind;
	favorite?: boolean;
	note?: string;
	txCount: number;
	lastUsed: number;
	firstSeen: number;
	source: 'manual' | 'auto';
};

type StoredGroup = { id: string; name: string; color?: string; members: string[] };

/** `vela.contacts.dismissed` is an `address → epoch ms` map, not a list. */
type StoredDismissed = Record<string, number>;

// ---------------------------------------------------------------------------
// Coercion helpers — deserialization hygiene, never policy.
// ---------------------------------------------------------------------------

const KINDS: ContactKind[] = ['eoa', 'account', 'unknown'];

function str(value: unknown): string | null {
	return typeof value === 'string' && value !== '' ? value : null;
}

function num(value: unknown, fallback: number): number {
	return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function kind(value: unknown): ContactKind {
	return KINDS.includes(value as ContactKind) ? (value as ContactKind) : 'unknown';
}

async function readJson<T>(key: string): Promise<T | null> {
	try {
		const raw = await getItem(key);
		return raw ? (JSON.parse(raw) as T) : null;
	} catch {
		// Unreadable or corrupt — the TS loaders' `catch { [] }`.
		return null;
	}
}

async function writeJson(key: string, value: unknown): Promise<void> {
	await setItem(key, JSON.stringify(value));
}

// ---------------------------------------------------------------------------
// Stored ⇄ wire
// ---------------------------------------------------------------------------

function toWireContact(stored: StoredContact): Contact {
	return {
		address: String(stored.address ?? '').toLowerCase(),
		name: str(stored.name),
		resolved_name: str(stored.resolvedName),
		resolved_source: str(stored.resolvedSource),
		kind: kind(stored.kind),
		favorite: !!stored.favorite,
		note: str(stored.note),
		tx_count: Math.max(0, Math.trunc(num(stored.txCount, 0))),
		last_used_ms: num(stored.lastUsed, 0),
		first_seen_ms: num(stored.firstSeen, 0),
		source: stored.source === 'auto' ? 'auto' : 'manual'
	};
}

function toStoredContact(contact: Contact): StoredContact {
	// Optional fields are omitted rather than written as `null` so the stored
	// JSON stays identical to what `services/contacts.ts` produces.
	return {
		address: contact.address,
		...(contact.name != null ? { name: contact.name } : {}),
		...(contact.resolved_name != null ? { resolvedName: contact.resolved_name } : {}),
		...(contact.resolved_source != null ? { resolvedSource: contact.resolved_source } : {}),
		kind: contact.kind,
		favorite: contact.favorite,
		...(contact.note != null ? { note: contact.note } : {}),
		txCount: contact.tx_count,
		lastUsed: contact.last_used_ms,
		firstSeen: contact.first_seen_ms,
		source: contact.source
	};
}

function toWireGroup(stored: StoredGroup): ContactGroup {
	return {
		id: String(stored.id ?? ''),
		name: String(stored.name ?? ''),
		color: str(stored.color),
		members: Array.isArray(stored.members)
			? stored.members.filter((m): m is string => typeof m === 'string')
			: []
	};
}

function toStoredGroup(group: ContactGroup): StoredGroup {
	return {
		id: group.id,
		name: group.name,
		...(group.color != null ? { color: group.color } : {}),
		members: group.members
	};
}

async function readContacts(): Promise<Contact[]> {
	const raw = await readJson<StoredContact[]>(CONTACTS_KEY);
	if (!Array.isArray(raw)) return [];
	return raw
		.filter(
			(c): c is StoredContact => !!c && typeof c === 'object' && typeof c.address === 'string'
		)
		.map(toWireContact);
}

async function readTombstones(): Promise<ContactTombstone[]> {
	const raw = await readJson<StoredDismissed>(DISMISSED_KEY);
	if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return [];
	return Object.entries(raw)
		.filter(([, at]) => typeof at === 'number' && Number.isFinite(at))
		.map(([address, at]) => ({ address: address.toLowerCase(), dismissed_at_ms: at }));
}

function toStoredDismissed(tombstones: ContactTombstone[]): StoredDismissed {
	const out: StoredDismissed = {};
	for (const entry of tombstones) out[entry.address] = entry.dismissed_at_ms;
	return out;
}

async function readGroups(): Promise<ContactGroup[]> {
	const raw = await readJson<StoredGroup[]>(GROUPS_KEY);
	if (!Array.isArray(raw)) return [];
	return raw
		.filter((g): g is StoredGroup => !!g && typeof g === 'object' && typeof g.id === 'string')
		.map(toWireGroup);
}

/**
 * One stored record → the four fields the book reads. `type` absent is a
 * legacy row and stays `null` (the core keeps those OUT of suggestions and IN
 * prior-interaction, verbatim); `timestamp` is stored in seconds, the core
 * reads ms. Nothing here decides — a `receive` row goes through too, and the
 * core is what declines it.
 */
export function toHistoryTx(tx: LocalTransaction): ContactHistoryTx {
	return {
		kind: tx.type ?? null,
		to: typeof tx.to === 'string' && tx.to !== '' ? tx.to : null,
		to_name: typeof tx.toName === 'string' && tx.toName !== '' ? tx.toName : null,
		timestamp_ms:
			typeof tx.timestamp === 'number' && Number.isFinite(tx.timestamp) ? tx.timestamp * 1000 : null
	};
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

export async function executeContactOperation(effect: ContactEffect): Promise<ContactShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_store': {
			const [contacts, tombstones, groups] = await Promise.all([
				readContacts(),
				readTombstones(),
				readGroups()
			]);
			return { type: 'store_loaded', contacts, tombstones, groups };
		}
		case 'write_contacts':
			await writeJson(CONTACTS_KEY, operation.contacts.map(toStoredContact));
			return { type: 'written' };
		case 'write_dismissed':
			await writeJson(DISMISSED_KEY, toStoredDismissed(operation.tombstones));
			return { type: 'written' };
		case 'write_groups':
			await writeJson(GROUPS_KEY, operation.groups.map(toStoredGroup));
			return { type: 'written' };
		case 'load_send_history':
			// The local tx store 025 built and 026 writes to (spec 028 US5): the
			// core derives recent recipients from its `send` rows and judges
			// first-interaction risk by every outgoing one. Shape only — which
			// rows count is `contacts.rs`'s `derive_from_history`.
			return { type: 'history_loaded', txs: (await loadTransactions()).map(toHistoryTx) };
		case 'resolve_identity': {
			// `null` = no identity anywhere; only `Some` is ever cached by the core.
			const identity = await resolveRecipientIdentity(operation.address);
			return {
				type: 'identity_resolved',
				address: operation.address,
				identity: identity ? { name: identity.name, source: identity.source } : null
			};
		}
		case 'classify_recipient': {
			// The raw `eth_getCode` answer goes back untouched. A transport-level
			// non-answer is `null` — unknown, NOT a verdict.
			const response = await poolRpcCall(
				'eth_getCode',
				[operation.address, 'latest'],
				operation.chain_id
			);
			const code =
				response.error != null || typeof response.result !== 'string' ? null : response.result;
			return {
				type: 'recipient_classified',
				chain_id: operation.chain_id,
				address: operation.address,
				code
			};
		}
		default: {
			const never: never = operation;
			throw new Error(`unhandled contacts operation: ${JSON.stringify(never)}`);
		}
	}
}

export function contactOperationFailure(effect: ContactEffect): ContactShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_store':
			// An unreadable store means "no address book yet" (today's catch).
			return { type: 'store_loaded', contacts: [], tombstones: [], groups: [] };
		case 'write_contacts':
		case 'write_dismissed':
		case 'write_groups':
			// Best-effort, as every `persist*` swallows storage errors today;
			// the in-memory ledger stays authoritative.
			return { type: 'written' };
		case 'load_send_history':
			return { type: 'history_failed' };
		case 'resolve_identity':
			return { type: 'identity_resolved', address: operation.address, identity: null };
		case 'classify_recipient':
			return {
				type: 'recipient_classified',
				chain_id: operation.chain_id,
				address: operation.address,
				code: null
			};
		default: {
			const never: never = operation;
			throw new Error(`unhandled contacts operation: ${JSON.stringify(never)}`);
		}
	}
}
