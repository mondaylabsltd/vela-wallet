/**
 * A multisig's setup saved to Ethereum — what "save your public key to
 * Ethereum" is for a Vela account. The genesis service records a multisig on
 * its home chain; this puts the same record (with its name) in the registry
 * on Ethereum, so the multisig can be found and deployed from there too.
 *
 * Anyone may send the registry's record call (it re-derives the address from
 * what it is given): an account on this device sends it, through the signing
 * sheet a Vela account's own backup uses.
 */
import { requestFirstPartyTransaction, type BackupSettled } from '$lib/backup/ethereum-backup';
import { signRequest } from '$lib/signing/core/sign-resident.svelte';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { hasCode } from './chain';
import { GENESIS_REGISTRY, recordCall } from './kernel';
import type { MultisigAccount } from './store.svelte';

export const ETHEREUM = 1;

export type BackupState = 'checking' | 'backedUp' | 'notBackedUp' | 'unavailable' | 'couldNotCheck';

/** `isRecorded(address)` */
const IS_RECORDED = '0xba340d1c';

/** Where the multisig's record stands on Ethereum. */
export async function checkBackup(account: Pick<MultisigAccount, 'address'>): Promise<BackupState> {
	try {
		if (!(await hasCode(ETHEREUM, GENESIS_REGISTRY))) return 'unavailable';
		const data = IS_RECORDED + account.address.slice(2).toLowerCase().padStart(64, '0');
		const answer = await poolRpcCall(
			'eth_call',
			[{ to: GENESIS_REGISTRY, data }, 'latest'],
			ETHEREUM
		);
		if (answer.error || typeof answer.result !== 'string') return 'couldNotCheck';
		return BigInt(answer.result) === 1n ? 'backedUp' : 'notBackedUp';
	} catch {
		return 'couldNotCheck';
	}
}

/** Open the signing sheet for the record call on Ethereum, sent from `from` (a Vela account here). */
export async function startBackup(
	account: Pick<MultisigAccount, 'genesis'>,
	from: string
): Promise<BackupSettled> {
	const call = recordCall(account.genesis);
	await signRequest.boot();
	return requestFirstPartyTransaction(from, {
		chain_id: ETHEREUM,
		to: call.to,
		value: '0',
		data: call.data
	});
}
