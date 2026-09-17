/**
 * Starting the Ethereum backup (spec 062 §5a) — the wallet asking ITSELF to
 * sign one transaction.
 *
 * The backup is a single call on Ethereum: `registry ← the Gnosis
 * registration's calldata`. Everything a person needs around that call already
 * exists and is the most carefully built surface in the product — the estimate
 * of the REAL operation, the fee row and fee coin, the funding guidance when
 * the account cannot pay, the passkey prompt, the pending record that survives
 * a closed tab, the receipt. A second, lesser copy of all that for one feature
 * would be a second implementation of the most dangerous screen there is.
 *
 * So this posts an `eth_sendTransaction` into the same seam a dApp transport
 * uses (`signRequest.registerTransport` + `request_arrived`), with the wallet's
 * own origin, pinned to Ethereum by `per_request_chain`. The core cannot tell —
 * and need not — that the requester is the wallet.
 *
 * Being the sender grants nothing: anyone may submit these bytes, and the
 * registry re-verifies every signature in them. The person's Safe is simply
 * the most convenient payer.
 */
import type { Account } from '$lib/core/generated/Account';
import {
	checkEthereumBackup,
	type EthereumBackupCall,
	type EthereumBackupCheck
} from '$lib/services/registry-backup';
import { signRequest } from '$lib/signing/core/sign-resident.svelte';

/** The account's FIRST founding key — the one the registry files its groups under. */
export function foundingKeyOf(account: Pick<Account, 'public_key_hex' | 'keys'>): string {
	return account.keys[0]?.public_key_hex ?? account.public_key_hex;
}

export type EthereumBackupOutcome =
	/** The signing sheet is open; `settled` follows the person's decision. */
	| { kind: 'requested'; check: EthereumBackupCheck; settled: Promise<BackupSettled> }
	/** Nothing to sign: already there, not available, not registered, or unknown. */
	| { kind: 'nothing_to_do'; check: EthereumBackupCheck };

export type BackupSettled =
	{ kind: 'submitted'; hash: string } | { kind: 'rejected'; code: number; message: string };

let counter = 0;

function requestSignature(address: string, call: EthereumBackupCall): Promise<BackupSettled> {
	return new Promise((resolve) => {
		const id = `vela-ethereum-backup-${Date.now()}-${++counter}`;
		const transportId = signRequest.registerTransport({
			sendResponse: (responseId, result, error) => {
				if (responseId !== id) return;
				signRequest.unregisterTransport(transportId);
				resolve(
					error
						? { kind: 'rejected', code: error.code, message: error.message }
						: { kind: 'submitted', hash: String(result) }
				);
			}
		});
		// Until the networks snapshot arrives every chain is unsupported
		// (fail-closed) — the resident says so, and a requester must send it.
		signRequest.syncNetworks();
		signRequest.dispatch({
			type: 'request_arrived',
			id,
			method: 'eth_sendTransaction',
			params_json: JSON.stringify([{ from: address, to: call.to, value: '0x0', data: call.data }]),
			origin: window.location.origin,
			transport_id: transportId,
			dedicated_transport: true,
			per_request_chain: call.chain_id,
			dapp: { name: 'Vela', url: window.location.origin },
			granted_address: null,
			requested_address: null,
			request_ts_ms: null,
			now_ms: Date.now()
		});
	});
}

/**
 * Check where the founding record stands and, if Ethereum does not hold it,
 * open the signing sheet for the one call that puts it there.
 *
 * A host for the sheet (`<SigningHost>`) must be mounted — the wallet route is.
 */
export async function startEthereumBackup(
	account: Pick<Account, 'address' | 'public_key_hex' | 'keys'>
): Promise<EthereumBackupOutcome> {
	const check = await checkEthereumBackup(account.address, foundingKeyOf(account));
	if (check.state !== 'not_backed_up' || check.call === null) {
		return { kind: 'nothing_to_do', check };
	}
	await signRequest.boot();
	return { kind: 'requested', check, settled: requestSignature(account.address, check.call) };
}
