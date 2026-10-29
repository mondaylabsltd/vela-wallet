/**
 * r4C-2 / r4C-9: an outside owner acts on a multisig transaction from the
 * connect page alone — no Vela on the other end, no sheet left open.
 *
 * The link names the transaction (`ms=<chainId>.<safe>.<safeTxHash>`). The
 * page reads it from the mailbox itself, and believes none of it: the core
 * re-hashes the SafeTx and the one the link names must be the one it gets
 * (else: refused, said); the core reads what it does; every approval in the
 * mailbox is re-verified, and counts only if its owner is an owner on the
 * chain NOW (`readProposal`, the wallet's own rule, plus approvals the chain
 * holds, `approveHash`). Who may act is the chain's word, never the
 * mailbox's.
 *
 * Then, from the owner's own wallet:
 * - approve: `eth_signTypedData_v4` of the SafeTx (or, for a wallet that
 *   cannot sign typed data, Safe's `eth_sign` form, or `approveHash` on
 *   chain); the answer is verified by the core before it is handed to the
 *   mailbox, which merges it with the others;
 * - execute, once the approvals meet the threshold: `execTransaction` sent
 *   by the wallet, which pays the fee (estimated first, and said);
 * - reject: the same-number cancellation, proposed with the owner's approval.
 *
 * Proposing a new transaction from here is not offered: it needs the send
 * flow (token, amount, recipient, the next free number), which is Vela's.
 */
import { poolRpcCall } from '$lib/services/rpc-pool';
import { hasCode, readMembers, type ChainMembers } from '../core/chain';
import {
	cancelTx,
	ecdsaForm,
	executionCalls,
	genesisAddress,
	isPasskeyApproval,
	namedGenesis,
	safeTxHash,
	safeTxHashes,
	safeTxTypedData,
	sameAddress,
	verifyEcdsaApproval,
	type Approval,
	type Call,
	type EcdsaApproval,
	type Genesis,
	type SafeTx,
	type SafeTxHashes
} from '../core/kernel';
import type { ProposalLink } from '../core/link';
import { readProposal, type ProposalItem } from '../core/proposals.svelte';
import {
	fetchGenesis,
	fetchProposals,
	ServiceBusyError,
	ServiceUnavailableError,
	submitApproval,
	type Proposal
} from '../core/service';
import type { Eip1193Provider } from './eip6963';
import { walletProblem, type WalletProblem } from './errors';
import { approveHashCall, onChainApprovals } from './onchain';
import { ethSignForm } from './typed';

export interface OutsideView {
	chainId: number;
	safe: string;
	tx: SafeTx;
	/** The transaction as this page reads it: approvals verified, counted by the chain's owners. */
	item: ProposalItem;
	members: ChainMembers;
	hashes: SafeTxHashes;
	/** The multisig's genesis, derived to its address by the core; `null` when the registry has none. */
	genesis: Genesis | null;
}

export type OutsideLoad =
	| { kind: 'loading' }
	| { kind: 'ready'; view: OutsideView }
	/** The mailbox does not hold it (not proposed yet, or Vela's side still has it). */
	| { kind: 'missing' }
	/** What the mailbox holds under that hash is not that transaction: refused. */
	| { kind: 'mismatch' }
	/** The mailbox or the chain did not answer. */
	| { kind: 'unavailable'; what: 'mailbox' | 'chain'; detail: string };

export type OutsideAction = 'approve' | 'approveOnChain' | 'execute' | 'reject';

export type OutsideStep =
	| { kind: 'idle' }
	/** Moving the wallet to the transaction's network. */
	| { kind: 'switching'; since: number }
	/** The wallet is asking the owner (`index` of `total` transactions, for an execution). */
	| { kind: 'wallet'; what: OutsideAction; since: number; index: number; total: number }
	/** The wallet cannot sign typed data: the `eth_sign` form, or approving on chain, is offered. */
	| { kind: 'unsupported' }
	/** Handing the approval to the mailbox (`retryAt`: sent again then, it was busy or away). */
	| { kind: 'sending'; retryAt: number | null }
	/** A transaction sent, waiting for the chain. */
	| { kind: 'mining'; what: OutsideAction; since: number; txHash: string }
	| { kind: 'approved'; form: 'typedData' | 'ethSign' | 'onChain' }
	| { kind: 'executed'; txHash: string }
	| { kind: 'rejected' }
	| {
			kind: 'failed';
			what: OutsideAction;
			problem: WalletProblem | null;
			/** Why, when it is not the wallet's: the mailbox's refusal, a check that failed. */
			reason: string;
			/** A signed approval the mailbox has not taken: Send again re-sends it, nothing re-signed. */
			unsent: boolean;
	  };

/** What an execution costs the owner's wallet, before it is sent. */
export interface ExecutionFee {
	/** Wei, all of the transactions it can price now. */
	wei: bigint;
	/** Transactions it takes (a multisig not deployed yet is deployed first). */
	count: number;
	/** Some could not be priced before the ones ahead of them land. */
	partial: boolean;
}

/** The mailbox: how long a signed approval is sent again on its own. */
export const POST_PATIENCE_MS = 120_000;
const RECEIPT_TIMEOUT_MS = 180_000;

class Cancelled extends Error {
	constructor() {
		super('Cancelled');
		this.name = 'AbortError';
	}
}

function race<T>(promise: Promise<T>, signal: AbortSignal): Promise<T> {
	return new Promise<T>((resolve, reject) => {
		if (signal.aborted) return reject(new Cancelled());
		const onAbort = () => reject(new Cancelled());
		signal.addEventListener('abort', onAbort, { once: true });
		promise.then(
			(v) => {
				signal.removeEventListener('abort', onAbort);
				resolve(v);
			},
			(e) => {
				signal.removeEventListener('abort', onAbort);
				reject(e);
			}
		);
	});
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** The proposal the mailbox holds under the link's hash, re-hashed by the core. */
export function matchProposal(
	link: ProposalLink,
	list: Proposal[]
): { found: Proposal } | 'missing' | 'mismatch' {
	const found = list.find((p) => p.safeTxHash.toLowerCase() === link.safeTxHash.toLowerCase());
	if (!found) return 'missing';
	const hash = safeTxHash(link.chainId, link.multisig, found.safeTx);
	return hash.toLowerCase() === link.safeTxHash.toLowerCase() ? { found } : 'mismatch';
}

export class OutsideTransaction {
	load = $state<OutsideLoad>({ kind: 'loading' });
	step = $state<OutsideStep>({ kind: 'idle' });
	readonly link: ProposalLink;
	#controller = new AbortController();
	/** A signed approval not taken by the mailbox yet (Send again). */
	#unsent: { safeTx: SafeTx; approval: EcdsaApproval; then: OutsideStep } | null = null;

	constructor(link: ProposalLink) {
		this.link = link;
	}

	/** Read it again: the mailbox's copy, the chain's members, what the chain approved. */
	async refresh(): Promise<void> {
		const { chainId, multisig: safe } = this.link;
		let answer;
		try {
			answer = await fetchProposals(chainId, safe);
		} catch (error) {
			this.load = { kind: 'unavailable', what: 'mailbox', detail: detailOf(error) };
			return;
		}
		const matched = matchProposal(this.link, answer?.proposals ?? []);
		if (matched === 'missing' || matched === 'mismatch') {
			this.load = { kind: matched };
			return;
		}
		const genesis = await this.#genesis();
		let members: ChainMembers;
		try {
			if (!genesis && !(await hasCode(chainId, safe))) {
				// Undeployed, and no genesis to say who its members are: nothing to judge by.
				this.load = { kind: 'unavailable', what: 'chain', detail: 'no genesis' };
				return;
			}
			members = await readMembers(chainId, safe, genesis as Genesis);
		} catch (error) {
			this.load = { kind: 'unavailable', what: 'chain', detail: detailOf(error) };
			return;
		}
		const p = matched.found;
		const onChain = members.deployed
			? await onChainApprovals(
					chainId,
					safe,
					p.safeTxHash,
					members.owners.filter((o) => !p.approvals.some((a) => sameAddress(a.owner, o)))
				)
			: [];
		const item = readProposal({ address: safe }, members, p, onChain);
		if (!item) {
			this.load = { kind: 'mismatch' };
			return;
		}
		this.load = {
			kind: 'ready',
			view: {
				chainId,
				safe,
				tx: p.safeTx,
				item,
				members,
				hashes: safeTxHashes(chainId, safe, p.safeTx),
				genesis
			}
		};
	}

	/** The registry's genesis for the multisig, only if the core derives its address from it. */
	async #genesis(): Promise<Genesis | null> {
		try {
			const answer = await fetchGenesis(this.link.multisig);
			if (!answer.found || !answer.genesis) return null;
			return sameAddress(genesisAddress(answer.genesis), this.link.multisig)
				? answer.genesis
				: null;
		} catch {
			return null;
		}
	}

	get view(): OutsideView | null {
		return this.load.kind === 'ready' ? this.load.view : null;
	}

	/** Whether `account` is an owner on the chain now. */
	isOwner(account: string | null): boolean {
		return !!account && !!this.view?.members.owners.some((o) => sameAddress(o, account));
	}

	/** Whether `account` has an approval that counts. */
	hasApproved(account: string | null): boolean {
		return (
			!!account &&
			!!this.view?.item.approvals.some((a) => a.counts && sameAddress(a.owner, account))
		);
	}

	/** Open, readable by the core, at the next number or later. */
	get open(): boolean {
		const v = this.view;
		return !!v && v.item.status !== 'done' && v.item.review.action !== null;
	}

	canApprove(account: string | null): boolean {
		return this.open && this.isOwner(account) && !this.hasApproved(account);
	}

	/**
	 * Whether `account`'s wallet can execute it now: its number is next, and
	 * the approvals meet the threshold — with the owner's own, as `msg.sender`,
	 * when it is one that has not approved. A rename never deploys.
	 */
	canExecute(account: string | null): boolean {
		const v = this.view;
		if (!v || !account || !this.open || v.item.status === 'queued') return false;
		if (v.item.renameUndeployed) return false;
		if (!v.members.deployed && !v.genesis) return false;
		const own = this.isOwner(account) && !this.hasApproved(account) ? 1 : 0;
		return v.item.approved + own >= v.item.threshold;
	}

	/** Who may reject: an owner, on a transaction that is not a cancellation itself. */
	canReject(account: string | null): boolean {
		return this.open && this.isOwner(account) && this.view?.item.review.action?.kind !== 'cancel';
	}

	/** Stop waiting on the wallet (its own prompt stays until dismissed there). */
	cancel(): void {
		this.#controller.abort();
		this.#controller = new AbortController();
		this.step = { kind: 'idle' };
	}

	// -- approve ------------------------------------------------------------------

	/** Approve with the wallet: typed data, or Safe's `eth_sign` form (`ethSign`). */
	async approve(
		provider: Eip1193Provider,
		account: string,
		form: 'typedData' | 'ethSign' = 'typedData'
	): Promise<void> {
		const v = this.view;
		if (!v) return;
		const signal = this.#controller.signal;
		try {
			const approval = await this.#sign(provider, account, v.chainId, v.safe, v.tx, form, signal);
			const signed = ecdsaForm(approval.signature);
			try {
				await this.#post(v.tx, approval, { kind: 'approved', form: signed });
			} catch (error) {
				// r4C-16: a mailbox that takes only typed data: said as that, with approving on chain.
				if (signed === 'ethSign' && error instanceof MailboxRefused && refusedForm(error.refusal)) {
					throw new HashFormRefused();
				}
				throw error;
			}
			await this.refresh();
		} catch (error) {
			this.#failed('approve', error);
		}
	}

	/** Approve on chain: `approveHash(safeTxHash)` sent by the wallet (it pays the fee). */
	async approveOnChain(provider: Eip1193Provider, account: string): Promise<void> {
		const v = this.view;
		if (!v || !v.members.deployed) return;
		const signal = this.#controller.signal;
		try {
			const call = { to: v.safe, value: '0x0', data: approveHashCall(v.item.safeTxHash) };
			await this.#send(provider, account, v.chainId, [call], 'approveOnChain', signal);
			this.step = { kind: 'approved', form: 'onChain' };
			await this.refresh();
		} catch (error) {
			this.#failed('approveOnChain', error);
		}
	}

	/** Send again an approval the mailbox has not taken (nothing is signed again). */
	async sendAgain(): Promise<void> {
		const unsent = this.#unsent;
		if (!unsent) return;
		try {
			await this.#post(unsent.safeTx, unsent.approval, unsent.then);
			await this.refresh();
		} catch (error) {
			this.#failed(unsent.then.kind === 'rejected' ? 'reject' : 'approve', error);
		}
	}

	// -- reject -------------------------------------------------------------------

	/** Propose the cancellation at its number, approved at once by `account`. */
	async reject(provider: Eip1193Provider, account: string): Promise<void> {
		const v = this.view;
		if (!v) return;
		const signal = this.#controller.signal;
		try {
			const cancel = cancelTx(v.safe, v.item.nonce.toString());
			const approval = await this.#sign(
				provider,
				account,
				v.chainId,
				v.safe,
				cancel,
				'typedData',
				signal
			);
			await this.#post(cancel, approval, { kind: 'rejected' });
		} catch (error) {
			this.#failed('reject', error);
		}
	}

	// -- execute ------------------------------------------------------------------

	/**
	 * The transactions `account`'s wallet sends to execute it: whatever must
	 * be deployed first (the multisig, a passkey member's account its check
	 * needs), then `execTransaction` — the owner's own approval as
	 * `msg.sender` when it is one.
	 */
	async executionCalls(account: string): Promise<Call[]> {
		const v = this.view;
		if (!v) throw new Error('not loaded');
		const owner = this.isOwner(account);
		const approvals: Approval[] = v.item.approvals
			.filter((a) => a.counts && !(owner && sameAddress(a.owner, account)))
			.map((a) => a.approval);
		const deployOwners: Genesis[] = [];
		for (const a of approvals) {
			if (isPasskeyApproval(a) && !(await hasCode(v.chainId, a.owner))) {
				deployOwners.push(a.ownerGenesis);
			}
		}
		return executionCalls({
			multisig: v.safe,
			deployMultisig: v.members.deployed ? null : v.genesis,
			deployOwners,
			tx: v.tx,
			approvals,
			executor: owner ? account : null
		});
	}

	/** What executing costs the wallet, estimated on the chain now; `null`: it cannot say. */
	async fee(account: string): Promise<ExecutionFee | null> {
		const v = this.view;
		if (!v) return null;
		try {
			const calls = await this.executionCalls(account);
			const price = await poolRpcCall('eth_gasPrice', [], v.chainId);
			if (typeof price.result !== 'string') return null;
			let gas = 0n;
			let partial = false;
			for (const call of calls) {
				const estimate = await poolRpcCall(
					'eth_estimateGas',
					[{ from: account, to: call.to, data: call.data, value: call.value }],
					v.chainId
				);
				if (typeof estimate.result === 'string') gas += BigInt(estimate.result);
				else partial = true;
			}
			if (gas === 0n) return null;
			return { wei: gas * BigInt(price.result), count: calls.length, partial };
		} catch {
			return null;
		}
	}

	async execute(provider: Eip1193Provider, account: string): Promise<void> {
		const v = this.view;
		if (!v) return;
		const signal = this.#controller.signal;
		try {
			const calls = await this.executionCalls(account);
			const hash = await this.#send(provider, account, v.chainId, calls, 'execute', signal);
			this.step = { kind: 'executed', txHash: hash };
			await this.refresh();
		} catch (error) {
			this.#failed('execute', error);
		}
	}

	// -- the wallet ---------------------------------------------------------------

	async #onChain(provider: Eip1193Provider, chainId: number, signal: AbortSignal): Promise<void> {
		const now = Number(BigInt(String(await provider.request({ method: 'eth_chainId' }))));
		if (now === chainId) return;
		this.step = { kind: 'switching', since: Date.now() };
		await race(
			provider.request({
				method: 'wallet_switchEthereumChain',
				params: [{ chainId: `0x${chainId.toString(16)}` }]
			}),
			signal
		);
		const after = Number(BigInt(String(await provider.request({ method: 'eth_chainId' }))));
		if (after !== chainId) throw Object.assign(new Error('other chain'), { code: 4901 });
	}

	/** A verified approval of `tx` by `account`, from its wallet. */
	async #sign(
		provider: Eip1193Provider,
		account: string,
		chainId: number,
		safe: string,
		tx: SafeTx,
		form: 'typedData' | 'ethSign',
		signal: AbortSignal
	): Promise<EcdsaApproval> {
		await this.#onChain(provider, chainId, signal);
		const hash = safeTxHash(chainId, safe, tx);
		this.step = { kind: 'wallet', what: 'approve', since: Date.now(), index: 1, total: 1 };
		let signature: unknown;
		try {
			signature =
				form === 'typedData'
					? await race(
							provider.request({
								method: 'eth_signTypedData_v4',
								params: [account, JSON.stringify(safeTxTypedData(chainId, safe, tx))]
							}),
							signal
						)
					: ethSignForm(
							String(
								await race(
									provider.request({ method: 'personal_sign', params: [hash, account] }),
									signal
								)
							)
						);
		} catch (error) {
			if (form === 'typedData' && walletProblem(error).kind === 'unsupported') {
				throw new TypedDataUnsupported();
			}
			throw error;
		}
		if (typeof signature !== 'string') throw new BadSignature();
		// Checked here as Vela and the Safe will check it: never handed on otherwise.
		try {
			const checked = verifyEcdsaApproval(hash, { owner: account, signature });
			if (!sameAddress(checked.owner, account)) throw new BadSignature();
			return checked;
		} catch {
			throw new BadSignature();
		}
	}

	/** Send `calls` from `account`, one after another, each waited for on chain. Returns the last hash. */
	async #send(
		provider: Eip1193Provider,
		account: string,
		chainId: number,
		calls: Call[],
		what: OutsideAction,
		signal: AbortSignal
	): Promise<string> {
		await this.#onChain(provider, chainId, signal);
		let last = '';
		for (const [i, call] of calls.entries()) {
			this.step = { kind: 'wallet', what, since: Date.now(), index: i + 1, total: calls.length };
			const hash = await race(
				provider.request({
					method: 'eth_sendTransaction',
					params: [{ from: account, to: call.to, data: call.data, value: call.value }]
				}),
				signal
			);
			if (typeof hash !== 'string') throw new Error('The wallet answered no transaction');
			this.step = { kind: 'mining', what, since: Date.now(), txHash: hash };
			const ok = await waitForReceipt(chainId, hash, signal);
			if (!ok) throw new Reverted(hash);
			last = hash;
		}
		return last;
	}

	// -- the mailbox ----------------------------------------------------------------

	/** Hand a verified approval to the mailbox: sent again while it is busy or away, then said. */
	async #post(safeTx: SafeTx, approval: EcdsaApproval, then: OutsideStep): Promise<void> {
		const v = this.view;
		if (!v) return;
		this.#unsent = { safeTx, approval, then };
		const started = Date.now();
		for (let tries = 0; ; tries += 1) {
			this.step = { kind: 'sending', retryAt: null };
			try {
				const answer = await submitApproval({
					chainId: v.chainId,
					safe: v.safe,
					safeTx,
					approval,
					...(v.genesis ? { genesis: namedGenesis(v.genesis) } : {})
				});
				if (!answer.ok) throw new MailboxRefused(answer.refusal);
				this.#unsent = null;
				this.step = then;
				return;
			} catch (error) {
				if (!(error instanceof ServiceUnavailableError)) throw error;
				const wait = Math.max(
					1_000,
					error instanceof ServiceBusyError && error.waitMs !== null
						? error.waitMs
						: Math.min(30_000, 2_000 * 2 ** tries)
				);
				if (Date.now() + wait > started + POST_PATIENCE_MS) throw new NotSent();
				this.step = { kind: 'sending', retryAt: Date.now() + wait };
				await sleep(wait);
			}
		}
	}

	#failed(what: OutsideAction, error: unknown): void {
		if (error instanceof Cancelled) {
			this.step = { kind: 'idle' };
			return;
		}
		if (error instanceof TypedDataUnsupported) {
			this.step = { kind: 'unsupported' };
			return;
		}
		const ours =
			error instanceof BadSignature ||
			error instanceof HashFormRefused ||
			error instanceof MailboxRefused ||
			error instanceof NotSent ||
			error instanceof Reverted;
		this.step = {
			kind: 'failed',
			what,
			problem: ours ? null : walletProblem(error),
			reason: ours ? (error as Error).message : '',
			unsent: error instanceof NotSent
		};
	}
}

/** The wallet cannot sign typed data: offered the other ways (r4C-16). */
export class TypedDataUnsupported extends Error {
	constructor() {
		super('typed data unsupported');
	}
}
/** The wallet's signature is not this owner's for this transaction. */
export class BadSignature extends Error {
	constructor() {
		super('badSignature');
	}
}
/** The mailbox refused the approval (its answer, short). */
export class MailboxRefused extends Error {
	constructor(readonly refusal: unknown) {
		super(`refused: ${shortRefusal(refusal)}`);
	}
}
/** The mailbox does not take the `eth_sign` form: other members cannot see it there. */
export class HashFormRefused extends Error {
	constructor() {
		super('noEthSign');
	}
}
/** Not taken by the mailbox within {@link POST_PATIENCE_MS}: kept, Send again. */
export class NotSent extends Error {
	constructor() {
		super('notSent');
	}
}
/** The transaction landed and failed. */
export class Reverted extends Error {
	constructor(readonly txHash: string) {
		super(`reverted: ${txHash}`);
	}
}

/**
 * The mailbox's refusal in one line: its own words where it has them
 * (`detail`, `reason`, `message`), else what kind it is — the service's
 * `Refusal` is `{ kind, detail? , error? }`, an `error` itself tagged.
 */
export function shortRefusal(refusal: unknown): string {
	const words = (value: unknown): string | null => {
		if (typeof value === 'string') return value;
		if (!value || typeof value !== 'object') return null;
		const r = value as Record<string, unknown>;
		for (const key of ['detail', 'reason', 'message']) {
			if (typeof r[key] === 'string') return r[key] as string;
		}
		const inner = words(r.error);
		if (inner) return inner;
		return typeof r.kind === 'string' ? r.kind : null;
	};
	return (words(refusal) ?? JSON.stringify(refusal) ?? String(refusal)).slice(0, 120);
}

/**
 * Whether the mailbox refused a plain account's approval for its form — the
 * `eth_sign` form (`v` 31/32), which a mailbox that takes only EIP-712
 * signatures calls "not an EIP-712 signature".
 */
export function refusedForm(refusal: unknown): boolean {
	return /eip-?712/i.test(JSON.stringify(refusal ?? ''));
}

function detailOf(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/** `true` once `hash` landed and succeeded, `false` if it reverted; throws on a timeout or a cancel. */
async function waitForReceipt(
	chainId: number,
	hash: string,
	signal: AbortSignal
): Promise<boolean> {
	const until = Date.now() + RECEIPT_TIMEOUT_MS;
	while (Date.now() < until) {
		if (signal.aborted) throw new Cancelled();
		const answer = await poolRpcCall('eth_getTransactionReceipt', [hash], chainId).catch(
			() => null
		);
		const receipt = answer?.result as { status?: string } | null | undefined;
		if (receipt && typeof receipt.status === 'string') return BigInt(receipt.status) === 1n;
		await sleep(2_000);
	}
	throw new Error(`not on chain after ${RECEIPT_TIMEOUT_MS / 1000} s: ${hash}`);
}
