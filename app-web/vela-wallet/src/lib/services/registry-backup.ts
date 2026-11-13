/**
 * Copying the wallet's record to Ethereum — WEB transport (spec 062 §5a).
 *
 * The walk is `vela_core::registry_backup`: five `eth_call`s against the
 * registry contract, on Gnosis (where the record lives) and Ethereum (where the
 * backup goes). No index service, no passkey — the registry stored the original
 * calldata and froze its signature domain, so the Gnosis bytes verify on
 * Ethereum as they are. What this file adds is nothing but the carrying
 * (`core-walk.ts`).
 *
 * The ANSWER is never cached: the question is asked each time a person opens
 * the screen, and a tap on the row asks it again before any sheet opens
 * (`startEthereumBackup`) — a stale "not backed up" next to a fee is a worse
 * lie than five cheap reads. What is kept is the last ROW the core drew for
 * the account (`vela.backupCheck`, a cache in the core's storage catalog), so
 * the block opens on it while the walk runs instead of on "Checking…" and
 * then jumping when the answer is no row at all (perf-8).
 */
import { loadCore, registryBackupStep } from '$lib/core/client';
import { runWalk } from './core-walk';
import { storageSealed } from './storage';

/** `registry_backup::BackupState`. */
export type EthereumBackupState =
	/** The registry is not deployed on Ethereum: draw nothing. */
	| 'unavailable'
	/** Gnosis has no unit naming this address for this key (a v1-era wallet). */
	| 'not_registered'
	| 'backed_up'
	| 'not_backed_up'
	/** Somebody did not answer. Never drawn as either verdict. Asking again may answer it. */
	| 'could_not_check'
	/**
	 * Gnosis answered, and what it holds can never be copied (a unit from
	 * before registry V13 stored no payload). Asking again gets the same
	 * answer: a calm end, nothing to tap — not a "could not check" that
	 * retries for ever.
	 */
	| 'not_copyable';

/**
 * `registry_backup::BackupRow` — the Keys block's row for a state, as the core
 * says it: which corpus keys, in which tone, and what a tap does. One rule on
 * all four apps, where each shell used to map the state itself.
 */
export interface EthereumBackupRow {
	title_key: string;
	subtitle_key: string;
	/** Never a caution: a copy is optional and costs a fee. */
	tone: 'neutral' | 'positive';
	/** `copy` opens the sheet with `call`; `retry` runs the walk again. */
	action: 'none' | 'copy' | 'retry';
	/**
	 * The paragraph under the Keys block for this state, by corpus key —
	 * `registry_backup::EXPLAIN_KEY` for every state a copy can still be made
	 * or checked in. ABSENT for a record that can never be copied
	 * (`not_copyable`): the paragraph describes making the copy, and telling
	 * a wallet that cannot be copied how to copy it was PR 3 note 6. Absent ⇒
	 * no paragraph is drawn.
	 */
	explain_key?: string | null;
}

/** `registry_backup::TITLE_KEY`. */
const TITLE_KEY = 'settingsModals.backup.title';
/** `registry_backup::EXPLAIN_KEY`. */
const EXPLAIN_KEY = 'settingsModals.backup.explain';

/**
 * The row while the walk is still running — before the core has a state to
 * make a row of. The core's own second line for it
 * (`registry_backup::CHECKING_KEY`), neutral, nothing to tap.
 */
export const CHECKING_ROW: EthereumBackupRow = {
	title_key: TITLE_KEY,
	subtitle_key: 'componentsUi.funding.checking',
	tone: 'neutral',
	action: 'none',
	// While the walk is still running the explanation is drawn, as it always
	// was: nothing is known yet that would take it away.
	explain_key: EXPLAIN_KEY
};

/**
 * `BackupState::CouldNotCheck.row()`, for the one case the core never gets to
 * answer: the walk itself threw or never finished, so there is no step to
 * read a row from. The same words, tone and tap as the core's own
 * could-not-check (`registry-backup.test.ts` holds the two equal).
 */
const COULD_NOT_ROW: EthereumBackupRow = {
	title_key: TITLE_KEY,
	subtitle_key: 'settingsModals.backup.couldNotCheck',
	tone: 'neutral',
	action: 'retry',
	explain_key: EXPLAIN_KEY
};

/** `registry_backup::BackupCall` — the one transaction that performs the backup. */
export interface EthereumBackupCall {
	chain_id: number;
	to: string;
	/** Decimal wei; always `"0"`. */
	value: string;
	/** The Gnosis registration's verbatim calldata. */
	data: string;
}

export interface EthereumBackupCheck {
	state: EthereumBackupState;
	/** Present exactly when `state` is `not_backed_up`. */
	call: EthereumBackupCall | null;
	/** The group's unit id on Gnosis, once known. */
	unitId: number | null;
	/**
	 * What the Keys block draws for `state` — the core's `BackupState::row()`.
	 * `null` when nothing is drawn: no registry on Ethereum, or no record.
	 */
	row: EthereumBackupRow | null;
}

interface BackupDone {
	type: 'done';
	state: EthereumBackupState;
	call: EthereumBackupCall | null;
	unit_id: number | null;
	/** Omitted by the core when the state draws no row. */
	row?: EthereumBackupRow;
}

const COULD_NOT: EthereumBackupCheck = {
	state: 'could_not_check',
	call: null,
	unitId: null,
	row: COULD_NOT_ROW
};

/**
 * Where this wallet's founding record stands on Ethereum.
 *
 * `foundingPublicKeyHex` is the account's FIRST founding key (`keys[0]`, or the
 * legacy single `public_key_hex`). `targetChainId` is Ethereum unless a
 * rehearsal names Base; the core refuses any other chain. Never throws.
 */
export async function checkEthereumBackup(
	address: string,
	foundingPublicKeyHex: string,
	options: { targetChainId?: number } = {}
): Promise<EthereumBackupCheck> {
	try {
		await loadCore();
		const done = await runWalk<BackupDone>((answers) =>
			registryBackupStep(address, foundingPublicKeyHex, answers, options.targetChainId)
		);
		if (done === null) return COULD_NOT;
		const check = {
			state: done.state,
			call: done.call,
			unitId: done.unit_id,
			row: done.row ?? null
		};
		// A rehearsal against another chain is not this account's Ethereum answer.
		if (options.targetChainId === undefined)
			rememberBackupRow(address, foundingPublicKeyHex, check);
		return check;
	} catch {
		return COULD_NOT;
	}
}

// ---------------------------------------------------------------------------
// The last row drawn, per account (perf-8)
// ---------------------------------------------------------------------------

/**
 * In `localStorage`, not the IndexedDB store: the block's first frame is drawn
 * from it, and an IndexedDB read is often still out when a warm core has
 * already put the account on screen. A few hundred bytes per account, filed
 * in the core's storage catalog like every other key (erase and "clear all
 * caches" find it there).
 */
const LAST_ROWS_KEY = 'vela.backupCheck';
/** Read once, on first use; the map is `address#foundingKey` → the core's last row. */
let lastRows: Map<string, EthereumBackupRow | null> | undefined;

function rowKey(address: string, foundingPublicKeyHex: string): string {
	return `${address.trim().toLowerCase()}#${foundingPublicKeyHex.trim().toLowerCase()}`;
}

/** A stored row is only believed in the shape the core writes. */
function isRow(raw: unknown): raw is EthereumBackupRow {
	if (raw === null || typeof raw !== 'object') return false;
	const r = raw as Record<string, unknown>;
	return (
		typeof r.title_key === 'string' &&
		typeof r.subtitle_key === 'string' &&
		(r.tone === 'neutral' || r.tone === 'positive') &&
		(r.action === 'none' || r.action === 'copy' || r.action === 'retry') &&
		(r.explain_key === undefined || r.explain_key === null || typeof r.explain_key === 'string')
	);
}

function remembered(): Map<string, EthereumBackupRow | null> {
	if (lastRows !== undefined) return lastRows;
	lastRows = new Map();
	try {
		const raw = localStorage.getItem(LAST_ROWS_KEY);
		const parsed: unknown = raw ? JSON.parse(raw) : {};
		if (parsed !== null && typeof parsed === 'object') {
			for (const [key, row] of Object.entries(parsed as Record<string, unknown>)) {
				if (row === null || isRow(row)) lastRows.set(key, row);
			}
		}
	} catch {
		// No store, or a corrupt one: nothing is remembered.
	}
	return lastRows;
}

/**
 * The row the core drew the last time this account's record was asked about:
 * a row, `null` for "nothing drawn", or `undefined` when nothing is
 * remembered — then the block says "Checking…".
 */
export function lastBackupRow(
	address: string,
	foundingPublicKeyHex: string
): EthereumBackupRow | null | undefined {
	return remembered().get(rowKey(address, foundingPublicKeyHex));
}

/**
 * Keep the row the core drew for a verdict. "Could not check" is not one: it
 * is a fact about a moment, and the next visit says "Checking…" — and keeps
 * whatever the last verdict was. Not while an erase is sweeping the device
 * (`storageSealed`): a key written behind it would fail its last check.
 */
function rememberBackupRow(
	address: string,
	foundingPublicKeyHex: string,
	check: EthereumBackupCheck
): void {
	if (check.state === 'could_not_check' || storageSealed()) return;
	const rows = remembered();
	rows.set(rowKey(address, foundingPublicKeyHex), check.row);
	try {
		localStorage.setItem(LAST_ROWS_KEY, JSON.stringify(Object.fromEntries(rows)));
	} catch {
		// Best effort: the next visit says "Checking…" again.
	}
}
