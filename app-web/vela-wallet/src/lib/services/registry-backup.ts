/**
 * Backing the founding record up to Ethereum — WEB transport (spec 062 §5a).
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
	/** Somebody did not answer. Never drawn as either verdict. */
	| 'could_not_check';

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
}

interface BackupDone {
	type: 'done';
	state: EthereumBackupState;
	call: EthereumBackupCall | null;
	unit_id: number | null;
}

const COULD_NOT: EthereumBackupCheck = { state: 'could_not_check', call: null, unitId: null };

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
		return { state: done.state, call: done.call, unitId: done.unit_id };
	} catch {
		return COULD_NOT;
	}
}
