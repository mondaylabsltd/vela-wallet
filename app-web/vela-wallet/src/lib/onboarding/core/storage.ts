/**
 * On-device storage for the wallet's account list, in the browser.
 *
 * Keys are the retired Expo client's, so a person who created a wallet there
 * is not stranded here — but that client wrote its records in camelCase
 * (`publicKeyHex`, `createdAt`, `keys[].credentialId`), and the core reads
 * snake_case. Spec 048: `loadAccounts` normalises either spelling and, the
 * first time it meets the old one, writes the list back in the current one.
 * (Device-found 2026-09-12: after the hostname moved to this shell, sign-in
 * signed and then did nothing, because the core refused the old records.)
 *
 * ONE invariant governs every function below. `Account` carries both the legacy
 * scalar key fields and the full `keys` array, and the core derives the address
 * from **all** keys. A mapper that copies an account field by field and drops
 * `keys` does not merely lose data — it silently "repairs" a multi-key account
 * into a different, wrong, single-key Safe on the next restore, at an address
 * nothing can deploy. So nothing here reshapes an account: records go in and
 * come out whole.
 */

import type { Account } from '../generated/Account';
import type { PendingUpload } from '../generated/PendingUpload';

export const STORAGE_KEYS = {
	accounts: 'vela.accounts',
	activeAccountIndex: 'vela.activeAccountIndex',
	pendingUploads: 'vela.pendingUploads',
	serviceEndpoints: 'vela.serviceEndpoints'
} as const;

/** Thrown when the browser refuses storage (private mode quota, disabled
 *  cookies). The core answers with `storage_failed`, never a crash. */
export class StorageError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'StorageError';
	}
}

function store(): Storage {
	if (typeof localStorage === 'undefined') {
		throw new StorageError('Local storage is unavailable in this context');
	}
	return localStorage;
}

function readList<T>(key: string): T[] {
	let raw: string | null;
	try {
		raw = store().getItem(key);
	} catch (error) {
		throw new StorageError(describe(error));
	}
	if (!raw) return [];
	try {
		const parsed = JSON.parse(raw);
		return Array.isArray(parsed) ? (parsed as T[]) : [];
	} catch {
		// Corrupt JSON reads as empty rather than throwing: a damaged list must
		// not make the wallet permanently unopenable, and every write below
		// replaces the whole list anyway.
		return [];
	}
}

function writeList(key: string, value: unknown): void {
	try {
		store().setItem(key, JSON.stringify(value));
	} catch (error) {
		throw new StorageError(describe(error));
	}
}

export function loadAccounts(): Account[] {
	const raw = readList<Record<string, unknown>>(STORAGE_KEYS.accounts);
	let rewrite = false;
	const accounts: Account[] = [];
	for (const record of raw) {
		const normalised = normaliseAccount(record);
		if (normalised === null) continue;
		if (normalised !== record) rewrite = true;
		accounts.push(normalised);
	}
	if (rewrite) {
		try {
			writeList(STORAGE_KEYS.accounts, accounts);
		} catch {
			// A read-only store still gets the normalised list in memory.
		}
	}
	return accounts;
}

/**
 * One record in the current spelling, whatever spelling it was written in.
 * Returns the SAME object when nothing had to change (so the caller can tell a
 * rewrite is due), `null` for a record that is not an account at all.
 */
export function normaliseAccount(record: unknown): Account | null {
	if (!record || typeof record !== 'object') return null;
	const r = record as Record<string, unknown>;
	const str = (a: unknown, b?: unknown): string | undefined =>
		typeof a === 'string' ? a : typeof b === 'string' ? b : undefined;
	const id = str(r.id);
	const address = str(r.address);
	if (id === undefined || address === undefined) return null;
	const old =
		'publicKeyHex' in r ||
		'createdAt' in r ||
		(Array.isArray(r.keys) &&
			r.keys.some((k) => k && typeof k === 'object' && 'credentialId' in k)) ||
		!Array.isArray(r.keys);
	if (!old) return record as Account;
	const keys = Array.isArray(r.keys) ? r.keys : [];
	return {
		id,
		name: str(r.name) ?? '',
		address,
		public_key_hex: str(r.public_key_hex, r.publicKeyHex) ?? '',
		created_at_iso: str(r.created_at_iso, r.createdAt) ?? '',
		keys: keys
			.filter((k): k is Record<string, unknown> => !!k && typeof k === 'object')
			.map((k) => {
				const key: Account['keys'][number] = {
					credential_id: str(k.credential_id, k.credentialId) ?? '',
					public_key_hex: str(k.public_key_hex, k.publicKeyHex) ?? '',
					name: str(k.name) ?? '',
					// Where the credential lives; the old client never recorded it.
					transports: str(k.transports) ?? ''
				};
				// Spec 075: the Clear Signer page a key lives behind. It is the
				// ONLY way that key can ever be reached, so normalising a record
				// must carry it through — dropping it here would make the key
				// unsignable and the wallet unopenable, silently. Absent stays
				// absent (the field is optional on the wire).
				const origin = str(k.signer_origin, k.signerOrigin);
				if (origin !== undefined && origin !== '') key.signer_origin = origin;
				return key;
			})
	};
}

/** Upsert by id. The whole record is written — see the invariant above. */
export function saveAccount(account: Account): void {
	const accounts = loadAccounts();
	const at = accounts.findIndex((existing) => existing.id === account.id);
	if (at >= 0) accounts[at] = account;
	else accounts.push(account);
	writeList(STORAGE_KEYS.accounts, accounts);
}

export function loadActiveIndex(): number {
	let raw: string | null;
	try {
		raw = store().getItem(STORAGE_KEYS.activeAccountIndex);
	} catch {
		return 0;
	}
	const parsed = Number.parseInt(raw ?? '', 10);
	// Missing, garbage and negative all read as 0. A negative index would make
	// the session render an empty address with a wallet present, which the
	// core forbids — so it fails closed here rather than at the wire.
	return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
}

export function saveActiveIndex(index: number): void {
	writeList(STORAGE_KEYS.activeAccountIndex, index);
}

export function loadPendingUploads(): PendingUpload[] {
	return readList<PendingUpload>(STORAGE_KEYS.pendingUploads);
}

/** Keyed by `id`, which for a pending upload IS the credential id of its first
 *  founding key — the scalar fields mirror `members[0]`. */
export function savePendingUpload(record: PendingUpload): void {
	const pending = loadPendingUploads().filter((existing) => existing.id !== record.id);
	pending.push(record);
	writeList(STORAGE_KEYS.pendingUploads, pending);
}

export function removePendingUpload(credentialId: string): void {
	writeList(
		STORAGE_KEYS.pendingUploads,
		loadPendingUploads().filter((existing) => existing.id !== credentialId)
	);
}

/**
 * Forget which wallet this browser is signed into — the account list and the
 * active index, and NOTHING else.
 *
 * The scope is the decision, not an implementation detail. Contacts, history,
 * custom tokens and networks, endpoints and preferences belong to the account
 * rather than to the session, and the account comes back intact because its
 * address derives from the passkey rather than from disk. The pending-upload
 * outbox is excluded for a second, independent reason: a record there is a
 * public key the registry never confirmed, and the next launch can still retry
 * it — but a deleted record can never be retried, and that credential becomes
 * unfindable at sign-in.
 */
export function clearSignedInWallet(): void {
	try {
		store().removeItem(STORAGE_KEYS.accounts);
		store().removeItem(STORAGE_KEYS.activeAccountIndex);
	} catch (error) {
		throw new StorageError(describe(error));
	}
}

/**
 * The stored `vela.serviceEndpoints` record, camelCase as the Expo client
 * writes it. Onboarding reads only `passkeyIndexURL`; the settings screen's
 * network_admin executor (spec 024, research D3a) writes the whole record
 * THROUGH this module so the shared key keeps exactly one storage home.
 * Absent fields stay absent — the cores apply their own defaults merge.
 */
export type ServiceEndpoints = {
	ethereumDataURL?: string;
	passkeyIndexURL?: string;
	bundlerServiceURL?: string;
	fiatRatesURL?: string;
	/** Spec 038 #E4 — where an unknown AAGUID is looked up; absent = the default node. */
	aaguidDirectoryURL?: string;
};

export function loadServiceEndpoints(): ServiceEndpoints {
	try {
		const raw = store().getItem(STORAGE_KEYS.serviceEndpoints);
		return raw ? (JSON.parse(raw) as ServiceEndpoints) : {};
	} catch {
		return {};
	}
}

export function saveServiceEndpoints(endpoints: ServiceEndpoints): void {
	writeList(STORAGE_KEYS.serviceEndpoints, endpoints);
}

function describe(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}
