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
import { poolRpcCall } from '$lib/services/rpc-pool';
import { pollUserOpStatus } from '$lib/services/tx-reconciler';
import {
	keySetOf,
	sendBatchCalls,
	type BeforeSign,
	type SignFn,
	type SubmitResult
} from '$lib/services/safe-transaction';
import { signChallenge } from '$lib/signing/sign-challenge';
import { hasCode, type ChainMembers } from './chain';
import {
	executionCalls,
	isPasskeyApproval,
	sameAddress,
	type Approval,
	type Genesis,
	type SafeTx
} from './kernel';
import type { LocalOwner } from './owners';

export interface ExecutionInput {
	chainId: number;
	multisig: string;
	genesis: Genesis;
	members: ChainMembers;
	tx: SafeTx;
	/** Verified approvals; the executor's own, if any, is ignored (it signs as `msg.sender`). */
	approvals: Approval[];
	/**
	 * A Vela account on this device. An owner counts as one approval (as
	 * `msg.sender`); any other account only sends it — `execTransaction` is
	 * anyone's to send once the approvals meet the threshold.
	 */
	executor: LocalOwner;
}

/** The executor's calls, with whatever must be deployed first. */
export async function planCalls(input: ExecutionInput) {
	const owner = input.members.owners.some((o) => sameAddress(o, input.executor.address));
	const others = owner
		? input.approvals.filter((a) => !sameAddress(a.owner, input.executor.address))
		: input.approvals;
	// A passkey owner's check needs its wallet's code; a plain account signs as it is.
	const deployOwners: Genesis[] = [];
	for (const a of others) {
		if (isPasskeyApproval(a) && !(await hasCode(input.chainId, a.owner))) {
			deployOwners.push(a.ownerGenesis);
		}
	}
	return executionCalls({
		multisig: input.multisig,
		deployMultisig: input.members.deployed ? null : input.genesis,
		deployOwners,
		tx: input.tx,
		approvals: others,
		executor: owner ? input.executor.address : null
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

/**
 * The executor cannot pay for the execution: its fee (deploying the multisig
 * and the owners it needs, then `execTransaction`) is more than the account
 * holds. Said before the passkey is asked — a relay may accept an operation
 * that cannot pay, and it would never land. `need` is the fee and what the
 * calls send; `have` the account's balance, both in the chain's coin.
 */
export class ExecutorCannotPayError extends Error {
	constructor(
		readonly need: bigint,
		readonly have: bigint
	) {
		super('the executor cannot pay the network fee');
		this.name = 'ExecutorCannotPayError';
	}
}

/**
 * Refuse, before the passkey, an execution the executor cannot pay: the fee
 * the operation will pay (deploying what it must, then `execTransaction`)
 * and what its calls send, against the account's coin. A fee in a token is
 * the relay's to judge; a balance that cannot be read is let through.
 */
function canPay(executor: string, chainId: number): BeforeSign {
	return async ({ token, amount, value }) => {
		if (token !== null) return;
		const answer = await poolRpcCall('eth_getBalance', [executor, 'latest'], chainId).catch(
			() => null
		);
		const raw = answer?.result;
		if (typeof raw !== 'string' || !/^0x[0-9a-fA-F]+$/.test(raw)) return;
		const have = BigInt(raw);
		if (have < amount + value) throw new ExecutorCannotPayError(amount + value, have);
	};
}

/**
 * How long a may-have-been-sent execution the relay keeps saying it never
 * heard of is given before it counts as not taken: its POST had an answer
 * that was not a hash (a 500, a refusal after a lost reply), and the relay's
 * record of an op it took is written before that answer goes out.
 */
export const NOT_TAKEN_AFTER_MS = 6_000;

/**
 * The relay's word on an execution that may have been sent (`maybeSent`):
 * resolves `true` once the relay says it refused it before any block, or
 * keeps saying it never heard of it past {@link NOT_TAKEN_AFTER_MS} — nothing
 * was sent, and the person may try again (the multisig's nonce lets at most
 * one execution land, so a second try can never pay twice). Resolves `false`
 * once the relay says it holds it, or when `signal` stops the watch.
 */
export async function relayRefused(
	userOpHash: string,
	chainId: number,
	signal: AbortSignal,
	status: typeof pollUserOpStatus = pollUserOpStatus,
	pauseMs = 2_000
): Promise<boolean> {
	const start = Date.now();
	while (!signal.aborted) {
		const answer = await status(userOpHash, chainId);
		if (signal.aborted) return false;
		if (answer?.status === 'rejected' && !answer.txHash) return true;
		if (answer && answer.status !== 'not_found') return false;
		if (answer?.status === 'not_found' && Date.now() - start >= NOT_TAKEN_AFTER_MS) return true;
		await new Promise((r) => setTimeout(r, pauseMs));
	}
	return false;
}

/** Send the execution. Resolves once the relay has the operation. */
export async function submitExecution(input: ExecutionInput): Promise<SubmitResult> {
	const stored = findAccountByAddress(input.executor.address);
	if (!stored) throw new Error('the executor is not an account on this device');
	const walletSigner =
		stored.keys && stored.keys.length > 1 ? keySetOf(stored) : stored.publicKeyHex;
	const calls = await planCalls(input);
	const signer = executorSigner(input.executor, input.chainId, input.tx);
	signer.beforeSign = canPay(input.executor.address, input.chainId);
	return sendBatchCalls(input.executor.address, calls, input.chainId, walletSigner, signer);
}
