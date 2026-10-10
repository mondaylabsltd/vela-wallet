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
 * Deliberately NOT cached. "Backed up" is permanent and could be, but the
 * question is asked when a person opens one screen, and a stale "not backed
 * up" next to a fee is a worse lie than five cheap reads.
 */
import { loadCore, registryBackupStep } from '$lib/core/client';
import { runWalk } from './core-walk';

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
}

/** `registry_backup::TITLE_KEY`. */
const TITLE_KEY = 'settingsModals.backup.title';

/**
 * The row while the walk is still running — before the core has a state to
 * make a row of. The core's own second line for it
 * (`registry_backup::CHECKING_KEY`), neutral, nothing to tap.
 */
export const CHECKING_ROW: EthereumBackupRow = {
	title_key: TITLE_KEY,
	subtitle_key: 'componentsUi.funding.checking',
	tone: 'neutral',
	action: 'none'
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
	action: 'retry'
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
		return { state: done.state, call: done.call, unitId: done.unit_id, row: done.row ?? null };
	} catch {
		return COULD_NOT;
	}
}
