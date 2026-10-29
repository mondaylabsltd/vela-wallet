/**
 * Executing a multisig transaction: the executor — an owner that is a Vela
 * account on this device — sends ONE user operation of its own, through the
 * same path a dApp's batch takes (`sendBatchCalls`: the relay, the in-band
 * fee paid by the executor, one passkey prompt). Its calls are the core's
 * (`executionCalls`): deploy the multisig and every approving owner that has
 * no code on the chain yet — both permissionless, from their geneses — then
 * `execTransaction`. The executor counts as an approval of its own
 * (`msg.sender`), so it needs no second signature.
 *
 * Before any passkey the execution is looked at as it would run
 * ({@link previewExecution}): what it spends against what the multisig holds,
 * its call made from the multisig's own address, and the relay's simulation
 * of the executor's operation — whose price is the fee shown, and the fee
 * signed.
 */
import { fromHex, verifySafeWebAuthn } from '$lib/core/kernels';
import { findAccountByAddress } from '$lib/services/accounts';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { pollUserOpStatus } from '$lib/services/tx-reconciler';
import {
	keySetOf,
	sendBatchCalls,
	type BeforeSign,
	type QuotedInBandFee,
	type SignFn,
	type SubmitResult
} from '$lib/services/safe-transaction';
import { signChallenge } from '$lib/signing/sign-challenge';
import { hasCode, type ChainMembers } from './chain';
import { tokenInfo } from './display';
import { quoteExecutionFee, type ExecutionQuote } from './execution-fee';
import {
	executionCalls,
	isEcdsaApproval,
	sameAddress,
	type Action,
	type Approval,
	type Call,
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
	/**
	 * What the transaction does, as the core read it (`reviewTx`): what it
	 * spends is checked against the multisig's balances before anything is
	 * signed. Omitted: the simulation alone speaks.
	 */
	action?: Action | null;
}

/** The executor's calls, and what they set up on the chain on the way. */
export interface PlannedExecution {
	calls: Call[];
	/** The multisig itself is deployed by this execution (its first on the chain). */
	deploysMultisig: boolean;
	/**
	 * Approving members whose Vela account this execution deploys: a passkey
	 * approval is a contract signature (EIP-1271) that the multisig checks by
	 * calling the member's account, so the account must exist there first.
	 */
	activates: string[];
}

/**
 * The executor's calls, with whatever must be deployed first.
 *
 * Only the approvals the threshold needs are carried — the multisig reads
 * the first `threshold` signatures and no more — and among them first those
 * that need nothing deployed (a plain account's, or a Vela account that
 * exists there already): a member's account is never deployed, at the
 * executor's cost, for an approval the multisig would not read.
 */
export async function planExecution(input: ExecutionInput): Promise<PlannedExecution> {
	const owner = input.members.owners.some((o) => sameAddress(o, input.executor.address));
	const others = owner
		? input.approvals.filter((a) => !sameAddress(a.owner, input.executor.address))
		: input.approvals;
	// A passkey owner's check needs its wallet's code; a plain account signs as it is.
	const ready = await Promise.all(
		others.map(async (a) => isEcdsaApproval(a) || (await hasCode(input.chainId, a.owner)))
	);
	const needed = Math.max(0, input.members.threshold - (owner ? 1 : 0));
	const chosen = others
		.map((approval, i) => ({ approval, ready: ready[i] }))
		.sort((a, b) => Number(b.ready) - Number(a.ready))
		.slice(0, needed);
	const deployOwners: Genesis[] = [];
	const activates: string[] = [];
	for (const { approval, ready } of chosen) {
		if (!ready && !isEcdsaApproval(approval)) {
			deployOwners.push(approval.ownerGenesis);
			activates.push(approval.owner);
		}
	}
	// Whether this execution deploys the multisig: asked here, and only here.
	const deployMultisig = input.members.deployed ? null : input.genesis;
	const calls = executionCalls({
		multisig: input.multisig,
		deployMultisig,
		deployOwners,
		tx: input.tx,
		approvals: chosen.map((c) => c.approval),
		executor: owner ? input.executor.address : null
	});
	return { calls, deploysMultisig: deployMultisig !== null, activates };
}

/** The executor's calls, with whatever must be deployed first. */
export async function planCalls(input: ExecutionInput): Promise<Call[]> {
	return (await planExecution(input)).calls;
}

// ---------------------------------------------------------------------------
// Before the passkey: would it run, and what does it cost?
// ---------------------------------------------------------------------------

/** Why an execution would fail if it were sent now. */
export type ExecutionBlock =
	/** The multisig holds less of a coin than the transaction spends. */
	| { kind: 'short'; symbol: string; decimals: number; have: bigint; need: bigint }
	/** It reverts when it runs; the reason the chain or relay gave, if any. */
	| { kind: 'reverts'; reason: string | null };

export type ExecutionPreview =
	| { kind: 'fails'; why: ExecutionBlock }
	/** The fee could not be priced now (the relay or the chain did not answer). */
	| { kind: 'unpriced' }
	| {
			kind: 'priced';
			/** The calls priced — exactly the ones signed. */
			calls: Call[];
			/** The fee shown, and signed verbatim, in the chain's coin. */
			fee: QuotedInBandFee;
			/** The executor's coin on the chain; `null` when it could not be read. */
			balance: bigint | null;
			/** The executor holds the fee (or its balance could not be read). */
			canPay: boolean;
			setup: {
				multisig: boolean;
				/** Members whose accounts it activates (deploys) for their approvals. */
				members: string[];
				/** The executor's own account is deployed by this, its first operation there. */
				executor: boolean;
				/** About how much of the fee is that one-time setup; `null` when unmeasured. */
				amount: bigint | null;
			};
	  };

/** What the preview reads from the chain and the relay — injectable for tests. */
export interface PreviewReads {
	quote: (chainId: number, executor: string, calls: Call[]) => Promise<ExecutionQuote>;
	/** A holder's balance of `token` (`null`: the chain's coin), or `null` when unreadable. */
	balanceOf: (chainId: number, token: string | null, holder: string) => Promise<bigint | null>;
	/** The multisig's own call, as an `eth_call` from its address: `null` when it runs. */
	callRefusal: (chainId: number, multisig: string, tx: SafeTx) => Promise<ExecutionBlock | null>;
	/** One call's gas on its own, or `null` when unmeasured. */
	gasOf: (chainId: number, from: string, call: Call) => Promise<bigint | null>;
	token: (chainId: number, token: string | null) => Promise<{ symbol: string; decimals: number }>;
}

/** What a transaction spends from the multisig, per coin (`token: null` — the chain's). */
export function spendingOf(action: Action | null): { token: string | null; amount: bigint }[] {
	const out: { token: string | null; amount: bigint }[] = [];
	const add = (token: string | null, amount: bigint) => {
		if (amount <= 0n) return;
		const hit = out.find((o) =>
			o.token === null || token === null ? o.token === token : sameAddress(o.token, token)
		);
		if (hit) hit.amount += amount;
		else out.push({ token, amount });
	};
	const value = (v: string) => {
		try {
			return BigInt(v || '0');
		} catch {
			return 0n;
		}
	};
	if (action?.kind === 'transfer') add(action.token, value(action.amount));
	else if (action?.kind === 'call') add(null, value(action.value));
	else if (action?.kind === 'batch') {
		for (const call of action.calls) {
			if (call.action.kind === 'transfer') add(call.action.token, value(call.action.amount));
			else add(null, value(call.value));
		}
	}
	return out;
}

const ERROR_STRING = '0x08c379a0';

/** `Error(string)` revert data → its text, else `null`. */
export function revertText(data: unknown): string | null {
	if (typeof data !== 'string' || !data.toLowerCase().startsWith(ERROR_STRING)) return null;
	try {
		const body = data.slice(10);
		const length = Number(BigInt(`0x${body.slice(64, 128)}`));
		const hex = body.slice(128, 128 + length * 2);
		const bytes = new Uint8Array((hex.match(/../g) ?? []).map((b) => parseInt(b, 16)));
		const text = new TextDecoder().decode(bytes).trim();
		return text || null;
	} catch {
		return null;
	}
}

/**
 * The relay's reason, kept only when it says something about THIS
 * transaction: its generic wrapper ("the target call in executeUserOp
 * reverted") says only that it reverts, which is said without it.
 */
export function plainReason(reason: string | null): string | null {
	if (!reason) return null;
	if (/target call|executeUserOp|execution reverted$|^reverted$/i.test(reason.trim())) return null;
	return reason.trim();
}

async function readBalance(
	chainId: number,
	token: string | null,
	holder: string
): Promise<bigint | null> {
	const answer = await (
		token === null
			? poolRpcCall('eth_getBalance', [holder, 'latest'], chainId)
			: poolRpcCall(
					'eth_call',
					[
						{ to: token, data: `0x70a08231${holder.slice(2).toLowerCase().padStart(64, '0')}` },
						'latest'
					],
					chainId
				)
	).catch(() => null);
	const raw = answer?.result;
	if (typeof raw !== 'string' || !/^0x[0-9a-fA-F]+$/.test(raw)) return null;
	return BigInt(raw.length > 66 ? raw.slice(0, 66) : raw);
}

/**
 * The transaction's own call, from the multisig's address: what it does when
 * the multisig makes it. A plain call only (a batch is a DELEGATECALL into
 * MultiSend, which an `eth_call` cannot make from it); an RPC that did not
 * answer says nothing — only a revert is a verdict.
 */
async function readCallRefusal(
	chainId: number,
	multisig: string,
	tx: SafeTx
): Promise<ExecutionBlock | null> {
	if (Number(tx.operation) !== 0) return null;
	const answer = await poolRpcCall(
		'eth_call',
		[{ from: multisig, to: tx.to, value: tx.value, data: tx.data || '0x' }, 'latest'],
		chainId
	).catch(() => null);
	const error = answer?.error as { code?: number; message?: string; data?: unknown } | undefined;
	if (!error) return null;
	const message = String(error.message ?? '');
	if (error.code !== 3 && !/revert/i.test(message)) return null;
	const reason =
		revertText(error.data) ?? plainReason(message.replace(/^execution reverted:?\s*/i, '') || null);
	return { kind: 'reverts', reason };
}

async function readGas(chainId: number, from: string, call: Call): Promise<bigint | null> {
	const answer = await poolRpcCall(
		'eth_estimateGas',
		[{ from, to: call.to, value: call.value, data: call.data }],
		chainId
	).catch(() => null);
	const raw = answer?.result;
	return typeof raw === 'string' && /^0x[0-9a-fA-F]+$/.test(raw) ? BigInt(raw) : null;
}

const LIVE_READS: PreviewReads = {
	quote: quoteExecutionFee,
	balanceOf: readBalance,
	callRefusal: readCallRefusal,
	gasOf: readGas,
	token: tokenInfo
};

/**
 * Whether the execution would run now, and what it costs the executor —
 * before any passkey. In order, each read only if the last said nothing:
 *
 * 1. what it spends, against what the multisig holds ("the multisig holds
 *    0.28 USDC, and this needs 0.5"), the words a person can act on;
 * 2. its own call made from the multisig's address (a contract refusing it);
 * 3. the relay's simulation of the executor's whole operation, which is
 *    also the fee's price: the fee `fee_policy` quotes for those exact calls.
 *
 * Of the fee, the share that is one-time setup — deploying the multisig, or a
 * member's account its approval is checked through — is measured on its own
 * (`eth_estimateGas` of each deployment) and said as "about".
 */
export async function previewExecution(
	input: ExecutionInput,
	reads: PreviewReads = LIVE_READS
): Promise<ExecutionPreview> {
	for (const need of spendingOf(input.action ?? null)) {
		const have = await reads.balanceOf(input.chainId, need.token, input.multisig);
		if (have === null || have >= need.amount) continue;
		const info = await reads
			.token(input.chainId, need.token)
			.catch(() => ({ symbol: '', decimals: 18 }));
		return {
			kind: 'fails',
			why: { kind: 'short', symbol: info.symbol, decimals: info.decimals, have, need: need.amount }
		};
	}
	const refusal = await reads.callRefusal(input.chainId, input.multisig, input.tx);
	if (refusal) return { kind: 'fails', why: refusal };

	let plan: PlannedExecution;
	try {
		plan = await planExecution(input);
	} catch {
		return { kind: 'unpriced' };
	}
	const executor = input.executor.address;
	const [quote, balance] = await Promise.all([
		reads.quote(input.chainId, executor, plan.calls),
		reads.balanceOf(input.chainId, null, executor)
	]);
	if (quote.kind === 'reverts') {
		return { kind: 'fails', why: { kind: 'reverts', reason: plainReason(quote.reason) } };
	}
	if (quote.kind === 'failed') return { kind: 'unpriced' };

	const deployments = plan.calls.slice(0, plan.calls.length - 1);
	let amount: bigint | null = null;
	// The executor's own first deployment rides in its initCode, unmeasured
	// here: then no share is said rather than one that leaves it out.
	if (deployments.length > 0 && quote.totalGas > 0n && quote.executorDeployed) {
		const gas = await Promise.all(deployments.map((c) => reads.gasOf(input.chainId, executor, c)));
		if (gas.every((g): g is bigint => g !== null)) {
			// Measured each on its own, each carries a transaction's own 21,000;
			// inside the one operation they do not.
			const setupGas = gas.reduce((sum, g) => sum + (g > 21_000n ? g - 21_000n : 0n), 0n);
			const share = (quote.fee.amount * setupGas) / quote.totalGas;
			amount = share > quote.fee.amount ? quote.fee.amount : share;
		}
	}
	return {
		kind: 'priced',
		calls: plan.calls,
		fee: quote.fee,
		balance,
		canPay: balance === null || balance >= quote.fee.amount,
		setup: {
			multisig: plan.deploysMultisig,
			members: plan.activates,
			executor: !quote.executorDeployed,
			amount
		}
	};
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

/** It would fail if sent now (see {@link ExecutionBlock}): nothing was signed. */
export class ExecutionWouldFailError extends Error {
	constructor(readonly why: ExecutionBlock) {
		super('the execution would fail');
		this.name = 'ExecutionWouldFailError';
	}
}

/** The fee could not be priced now: nothing was signed, and a retry may price it. */
export class ExecutionUnpricedError extends Error {
	constructor() {
		super('the network fee could not be worked out');
		this.name = 'ExecutionUnpricedError';
	}
}

/** Whether `error` stopped the execution before its passkey: nothing was signed or sent. */
export function stoppedBeforeSigning(error: unknown): boolean {
	return (
		error instanceof ExecutorCannotPayError ||
		error instanceof ExecutionWouldFailError ||
		error instanceof ExecutionUnpricedError
	);
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
 *
 * `follow`: an execution the relay took (a hash came back) is followed past
 * "held" — its refusal at any later stage, before any block, is said at once
 * rather than after the receipt wait runs out.
 */
export async function relayRefused(
	userOpHash: string,
	chainId: number,
	signal: AbortSignal,
	status: typeof pollUserOpStatus = pollUserOpStatus,
	pauseMs = 2_000,
	follow = false
): Promise<boolean> {
	const start = Date.now();
	while (!signal.aborted) {
		const answer = await status(userOpHash, chainId);
		if (signal.aborted) return false;
		if (answer?.status === 'rejected' && !answer.txHash) return true;
		if (follow) {
			if (answer?.status === 'included' || answer?.status === 'failed') return false;
		} else {
			if (answer && answer.status !== 'not_found') return false;
			if (answer?.status === 'not_found' && Date.now() - start >= NOT_TAKEN_AFTER_MS) return true;
		}
		await new Promise((r) => setTimeout(r, pauseMs));
	}
	return false;
}

/**
 * Send the execution. Resolves once the relay has the operation.
 *
 * `preview` is the one the person was shown: its calls and its fee are
 * signed exactly. Without one it is looked at now. Either way nothing is
 * signed for an execution that would fail, that has no price, or that the
 * executor cannot pay — each said before the passkey.
 */
export async function submitExecution(
	input: ExecutionInput,
	preview?: ExecutionPreview
): Promise<SubmitResult> {
	const stored = findAccountByAddress(input.executor.address);
	if (!stored) throw new Error('the executor is not an account on this device');
	const walletSigner =
		stored.keys && stored.keys.length > 1 ? keySetOf(stored) : stored.publicKeyHex;
	const ready = preview ?? (await previewExecution(input));
	if (ready.kind === 'fails') throw new ExecutionWouldFailError(ready.why);
	if (ready.kind === 'unpriced') throw new ExecutionUnpricedError();
	if (!ready.canPay && ready.balance !== null) {
		throw new ExecutorCannotPayError(ready.fee.amount, ready.balance);
	}
	const signer = executorSigner(input.executor, input.chainId, input.tx);
	signer.beforeSign = canPay(input.executor.address, input.chainId);
	return sendBatchCalls(
		input.executor.address,
		ready.calls,
		input.chainId,
		walletSigner,
		signer,
		undefined,
		null,
		ready.fee
	);
}
