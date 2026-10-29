/**
 * The one review of a multisig transaction (docs/multisig-vault-design.md
 * §3.6), the same on every member's device and for every action: what a Vela
 * account's signing sheet tells its signer — what moves, who it goes to,
 * the AI's opinion, the bytes signed — and the state-diff guard a Vela
 * account does not need (Bybit, 2025).
 *
 * Every rule is the core's (`multisig_review`): it names the reads
 * (`reviewReads`), this file makes them with the wallet's own RPC pool and
 * hands back what each one answered (`multisigReview`). Nothing is decided
 * here; nothing is kept between reviews — each one reads the chain now.
 */
import { contactsBook } from '$lib/contacts/core/contacts-book.svelte';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { resolveRecipientRisk } from '$lib/services/recipient-risk';
import { rpcSimulateReply } from '$lib/services/sim/sim-engine-rpc';
import { judgeCheck } from '$lib/services/sim/sim-trust';
import { session } from '$lib/session/core/session.svelte';
import type { SimEnded } from '$lib/signing/verdict';
import type { ChainMembers } from './chain';
import {
	multisigReview,
	reviewReads,
	type Genesis,
	type KnownAddress,
	type MultisigReviewOut,
	type RecipientFacts,
	type ReviewInput,
	type ReviewReadsInput,
	type SafeTx
} from './kernel';
import { multisig } from './store.svelte';

/** A JSON-RPC answer as the core reads one: the envelope, or `{unreachable: true}`. */
async function envelope(chainId: number, method: string, params: unknown[]): Promise<unknown> {
	try {
		const answer = await poolRpcCall(method, params, chainId);
		return answer.error ? { error: answer.error } : { result: answer.result ?? null };
	} catch {
		return { unreachable: true };
	}
}

/** What the review is of: the transaction, and the Safe on its chain now. */
export interface ReviewSubject {
	safe: string;
	chainId: number;
	tx: SafeTx;
	members: Pick<ChainMembers, 'deployed' | 'owners' | 'threshold'>;
	genesis: Genesis | null;
	/** The site that asked, for a dApp's calls. */
	origin?: string | null;
}

/**
 * The addresses the person knows — what a lookalike is measured against:
 * the contacts (saved and the ones sent to before), this device's accounts
 * and multisigs, the members.
 */
export function knownAddresses(members: string[]): KnownAddress[] {
	const out: KnownAddress[] = [];
	for (const c of contactsBook.view?.contacts ?? []) {
		out.push({ address: c.address, label: c.name ?? c.resolved_name ?? c.address });
	}
	for (const row of session.view.accounts ?? []) {
		out.push({ address: row.account.address, label: row.account.name });
	}
	for (const a of multisig.accounts) {
		out.push({ address: a.address, label: a.genesis.name ?? a.label ?? a.address });
	}
	for (const m of members) out.push({ address: m, label: m });
	return out;
}

/** The core's reads, the chain's answers: everything `multisigReview` takes but `blind`. */
export async function gatherReview(subject: ReviewSubject): Promise<ReviewInput> {
	const { chainId, safe, tx, members } = subject;
	const base: ReviewReadsInput = {
		chainId,
		safe,
		tx,
		owners: members.owners,
		threshold: members.threshold,
		deployed: members.deployed,
		genesis: subject.genesis,
		codes: {}
	};
	let reads = reviewReads(base);
	if (reads.needCode.length > 0) {
		const codes = await Promise.all(
			reads.needCode.map(async (a) => {
				const answer = (await envelope(chainId, 'eth_getCode', [a, 'latest'])) as {
					result?: unknown;
				};
				return [a, typeof answer.result === 'string' ? answer.result : null] as const;
			})
		);
		base.codes = Object.fromEntries(codes);
		reads = reviewReads(base);
	}
	const simulate =
		reads.simCalls.length > 0 ? rpcSimulateReply(safe, reads.simCalls, chainId) : null;
	// Both, side by side: the trace (every write) where the network offers it
	// and its execution ran; the probe otherwise — the core picks (a trace
	// that reverted reads nothing, never "nothing changes").
	const stateRead = async (): Promise<{ trace: unknown; probe: unknown }> => {
		const [trace, probe] = await Promise.all([
			reads.trace ? envelope(chainId, reads.trace.method, reads.trace.params) : null,
			reads.probe ? envelope(chainId, reads.probe.method, reads.probe.params) : null
		]);
		return { trace, probe };
	};
	const facts = Promise.all(
		reads.recipients.map(async (address): Promise<RecipientFacts> => {
			const risk = await resolveRecipientRisk(chainId, address);
			return { address, isContract: risk.isContract, firstTime: risk.firstInteraction };
		})
	);
	const [state, simReply, recipientFacts] = await Promise.all([
		stateRead(),
		simulate ?? Promise.resolve(null),
		facts
	]);
	return {
		...base,
		...state,
		simReply,
		recipientFacts,
		known: knownAddresses(members.owners),
		origin: subject.origin ?? null
	};
}

/** The review of what was gathered, with the clear reading's `blind` (a call Vela cannot read). */
export function reviewOf(input: ReviewInput, blind: boolean): MultisigReviewOut {
	return multisigReview({ ...input, blind });
}

/**
 * The simulation as the signing sheet draws it (`SimEnded`): the core's
 * verdict, its moves judged by the token rules for the Safe.
 */
export async function simEndedOf(
	review: MultisigReviewOut,
	chainId: number,
	safe: string
): Promise<SimEnded> {
	const verdict = review.simulation;
	if (verdict.kind !== 'deltas') return { verdict, judged: null };
	const judged = await judgeCheck(verdict.deltas, chainId, safe).catch(() => null);
	return { verdict, judged };
}

/** How often a sheet reads its review again while it is open. */
export const REVIEW_FRESH_MS = 30_000;

/** One review on a sheet: gathered once it starts, read again on `refresh()`. */
export class ProposalReview {
	input = $state.raw<ReviewInput | null>(null);
	sim = $state.raw<SimEnded | null>(null);
	failed = $state(false);
	#disposed = false;
	#run = 0;

	constructor(readonly subject: ReviewSubject) {}

	async refresh(): Promise<void> {
		const run = ++this.#run;
		this.failed = false;
		try {
			const input = await gatherReview(this.subject);
			if (this.#disposed || run !== this.#run) return;
			this.input = input;
			const out = reviewOf(input, false);
			const sim = await simEndedOf(out, this.subject.chainId, this.subject.safe);
			if (this.#disposed || run !== this.#run) return;
			this.sim = sim;
		} catch (error) {
			console.warn('[multisig review]', error);
			if (!this.#disposed && run === this.#run) this.failed = true;
		}
	}

	/** The core's review now, with the clear reading's `blind`. */
	read(blind: boolean): MultisigReviewOut | null {
		return this.input ? reviewOf(this.input, blind) : null;
	}

	dispose(): void {
		this.#disposed = true;
	}
}
