/**
 * On-device storage for the wallet's account list, in the browser.
 *
 * Keys and record shapes are byte-compatible with the shipping Expo client
 * (which reads the same names out of AsyncStorage), so a person who created a
 * wallet there is not stranded here.
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
	return readList<Account>(STORAGE_KEYS.accounts);
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
