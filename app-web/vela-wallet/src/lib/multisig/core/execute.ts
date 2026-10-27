/**
 * Executing a multisig transaction: the executor — an owner that is a Vela
 * account on this device — sends ONE user operation of its own, through the
 * same path a dApp's batch takes (`sendBatchCalls`: the relay, the in-band
 * fee paid by the executor, one passkey prompt). Its calls are the core's
 * (`executionCalls`): deploy the multisig and every approving owner that has
 * no code on the chain yet — both permissionless, from their geneses — then
 * `execTransaction`. The executor counts as an approval of its own
 * (`msg.sender`), so it needs no second signature.
 */
import { fromHex, verifySafeWebAuthn } from '$lib/core/kernels';
import { findAccountByAddress } from '$lib/services/accounts';
import { keySetOf, sendBatchCalls, type SignFn, type SubmitResult } from '$lib/services/safe-transaction';
import { signChallenge } from '$lib/signing/sign-challenge';
import { hasCode, type ChainMembers } from './chain';
import { executionCalls, sameAddress, type Genesis, type PasskeyApproval, type SafeTx } from './kernel';
import type { LocalOwner } from './owners';

export interface ExecutionInput {
	chainId: number;
	multisig: string;
	genesis: Genesis;
	members: ChainMembers;
	tx: SafeTx;
	/** Verified approvals; the executor's own, if any, is ignored (it signs as `msg.sender`). */
	approvals: PasskeyApproval[];
	executor: LocalOwner;
}

/** The executor's calls, with whatever must be deployed first. */
export async function planCalls(input: ExecutionInput) {
	const others = input.approvals.filter((a) => !sameAddress(a.owner, input.executor.address));
	const deployOwners: Genesis[] = [];
	for (const a of others) {
		if (!(await hasCode(input.chainId, a.owner))) deployOwners.push(a.ownerGenesis);
	}
	return executionCalls({
		multisig: input.multisig,
		deployMultisig: input.members.deployed ? null : input.genesis,
		deployOwners,
		tx: input.tx,
		approvals: others,
		executor: input.executor.address
	});
}

/** The executor's passkey over its user operation — the dApp path's signer. */
function executorSigner(owner: LocalOwner, chainId: number, tx: SafeTx): SignFn {
	const stored = findAccountByAddress(owner.address);
	const keys = stored ? keySetOf(stored).keys : [];
	return async (challenge) => {
		const assertion = await signChallenge(challenge, {
			account: owner.address,
			keys,
			credentials: keys.map((k) => ({ id: k.credentialId })),
			request: { method: 'vela_multisigExecute', params: { tx }, origin: '', chainId }
		});
		const compat = verifySafeWebAuthn(assertion);
		if (!compat.ok) throw new Error(compat.reason);
		return {
			signature: fromHex(assertion.signatureHex),
			authenticatorData: fromHex(assertion.authenticatorDataHex),
			clientDataJSON: fromHex(assertion.clientDataJSONHex),
			credentialId: assertion.credentialId
		};
	};
}

/** Send the execution. Resolves once the relay has the operation. */
export async function submitExecution(input: ExecutionInput): Promise<SubmitResult> {
	const stored = findAccountByAddress(input.executor.address);
	if (!stored) throw new Error('the executor is not an account on this device');
	const walletSigner = stored.keys && stored.keys.length > 1 ? keySetOf(stored) : stored.publicKeyHex;
	const calls = await planCalls(input);
	return sendBatchCalls(
		input.executor.address,
		calls,
		input.chainId,
		walletSigner,
		executorSigner(input.executor, input.chainId, input.tx)
	);
}
