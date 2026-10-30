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
import { names } from './names.svelte';
import { bounded } from './bounded';

/**
 * How long one of the review's reads is waited for (round 5, as the
 * preview's): past it, it did not answer — said as such by the core (a state
 * not read, a move not simulated), never a review held open for good. The
 * signing gate reads through here too, so a hung node never holds a passkey.
 */
export const REVIEW_READ_MS = 8_000;

/** A JSON-RPC answer as the core reads one: the envelope, or `{unreachable: true}`. */
async function envelope(chainId: number, method: string, params: unknown[]): Promise<unknown> {
	const read = async (): Promise<unknown> => {
		try {
			const answer = await poolRpcCall(method, params, chainId);
			return answer.error ? { error: answer.error } : { result: answer.result ?? null };
		} catch {
			return { unreachable: true };
		}
	};
	return bounded(read(), { unreachable: true }, REVIEW_READ_MS);
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
 * The addresses the person knows — what a lookalike is measured against —
 * and whose each is (r4a-18): the contacts (saved and the ones sent to
 * before), this device's own accounts and multisigs, the members by the name
 * this device has for them. The core never calls one's own account or a
 * member a stranger ("first time", "coins lost in a contract").
 */
export function knownAddresses(members: string[]): KnownAddress[] {
	const out: KnownAddress[] = [];
	for (const c of contactsBook.view?.contacts ?? []) {
		out.push({ address: c.address, label: c.name ?? c.resolved_name ?? c.address });
	}
	for (const row of session.view.accounts ?? []) {
		out.push({ address: row.account.address, label: row.account.name, kind: 'own' });
	}
	for (const a of multisig.accounts) {
		out.push({ address: a.address, label: a.genesis.name ?? a.label ?? a.address, kind: 'own' });
	}
	for (const m of members) out.push({ address: m, label: names.label(m), kind: 'member' });
	return out;
}

/**
 * r5reg-14: the reviews gathered lately, by exactly what they were of — the
 * transaction, the Safe's members and nonce, the site. Approving took 4–6 s
 * (r4: ~2 s) because the signing gate gathered the whole review again — the
 * trace, the simulation, each recipient's history — seconds after the sheet
 * had gathered the very same one. The gate takes the sheet's while it is
 * fresh (`recentReview`); a moved nonce or other members is another key,
 * and who else can act for the Safe is still read right before the passkey
 * (`checkNow`).
 */
// eslint-disable-next-line svelte/prefer-svelte-reactivity -- a cache of reads, never drawn
const recent = new Map<string, { at: number; input: ReviewInput }>();
const RECENT_CAP = 16;

function subjectKey(subject: ReviewSubject): string {
	const { members } = subject;
	const nonce = (members as { nonce?: bigint }).nonce;
	return JSON.stringify([
		subject.chainId,
		subject.safe.toLowerCase(),
		subject.tx,
		members.deployed,
		members.threshold,
		members.owners.map((o) => o.toLowerCase()),
		nonce === undefined ? null : nonce.toString(),
		subject.origin ?? null
	]);
}

/** The review of exactly `subject` gathered within `maxAgeMs`, or `null`. */
export function recentReview(
	subject: ReviewSubject,
	maxAgeMs: number = REVIEW_FRESH_MS
): ReviewInput | null {
	const hit = recent.get(subjectKey(subject));
	return hit && Date.now() - hit.at <= maxAgeMs ? hit.input : null;
}

/** Tests: nothing gathered lately. */
export function forgetRecentReviews(): void {
	recent.clear();
}

/** The core's reads, the chain's answers: everything `multisigReview` takes but `blind`. */
export async function gatherReview(subject: ReviewSubject): Promise<ReviewInput> {
	const input = await gatherNow(subject);
	const key = subjectKey(subject);
	recent.delete(key);
	recent.set(key, { at: Date.now(), input });
	while (recent.size > RECENT_CAP) recent.delete(recent.keys().next().value as string);
	return input;
}

async function gatherNow(subject: ReviewSubject): Promise<ReviewInput> {
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
		reads.simCalls.length > 0
			? bounded(
					rpcSimulateReply(safe, reads.simCalls, chainId).catch(() => null),
					null,
					REVIEW_READ_MS
				)
			: null;
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
			const unknown: RecipientFacts = { address, isContract: null, firstTime: null };
			const risk = await bounded(
				resolveRecipientRisk(chainId, address).catch(() => null),
				null,
				REVIEW_READ_MS
			);
			return risk
				? { address, isContract: risk.isContract, firstTime: risk.firstInteraction }
				: unknown;
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
/**
 * r5reg-14: how soon a review that could not read the chain (its state or
 * its simulation unreachable — an RPC outage) is read again. Recovery took
 * 25–35 s: such a review waited out the 30 s round, "couldn't check" on
 * screen long after the network was back.
 */
export const REVIEW_RETRY_MS = 5_000;

/** One review on a sheet: gathered once it starts, read again on `refresh()`. */
export class ProposalReview {
	input = $state.raw<ReviewInput | null>(null);
	sim = $state.raw<SimEnded | null>(null);
	failed = $state(false);
	/** The last gathering could not read the chain (the state or the simulation): read again soon. */
	unread = $state(false);
	#disposed = false;
	#run = 0;
	#busy = false;

	constructor(readonly subject: ReviewSubject) {}

	async refresh(): Promise<void> {
		const run = ++this.#run;
		this.failed = false;
		this.#busy = true;
		try {
			const input = await gatherReview(this.subject);
			if (this.#disposed || run !== this.#run) return;
			this.input = input;
			const out = reviewOf(input, false);
			this.unread =
				out.state.verdict.kind === 'unavailable' || out.simulation.kind === 'unreachable';
			const sim = await simEndedOf(out, this.subject.chainId, this.subject.safe);
			if (this.#disposed || run !== this.#run) return;
			this.sim = sim;
		} catch (error) {
			console.warn('[multisig review]', error);
			if (!this.#disposed && run === this.#run) {
				this.failed = true;
				this.unread = true;
			}
		} finally {
			if (run === this.#run) this.#busy = false;
		}
	}

	/**
	 * Read it again while its sheet is open: every {@link REVIEW_FRESH_MS},
	 * and every {@link REVIEW_RETRY_MS} while the last read could not reach
	 * the chain (r5reg-14). The returned function stops.
	 */
	keepFresh(): () => void {
		const fresh = setInterval(() => void this.refresh(), REVIEW_FRESH_MS);
		const retry = setInterval(() => {
			if (this.unread && !this.#busy) void this.refresh();
		}, REVIEW_RETRY_MS);
		return () => {
			clearInterval(fresh);
			clearInterval(retry);
		};
	}

	/** The core's review now, with the clear reading's `blind`. */
	read(blind: boolean): MultisigReviewOut | null {
		return this.input ? reviewOf(this.input, blind) : null;
	}

	dispose(): void {
		this.#disposed = true;
	}
}
