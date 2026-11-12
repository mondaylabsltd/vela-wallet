/**
 * The one review's blocks (§3.6) in the signing sheet's own kinds — what
 * moves in the verdict's place, who it reaches as parties, the state-diff
 * guard as a card or a line — and its technical details: the SafeTx fields,
 * the EIP-712 typed data as `RawData` draws it, the three hashes. Every
 * verdict is the core's (`multisigReview`); this file only puts its words on
 * it. Nothing here runs the core.
 */
import { fillTemplate } from '$lib/i18n/fill';
import type { SigningMessages } from '$lib/signing/messages';
import type { Block, KeyValueRow, PartyBadge, TechModel } from '$lib/signing/model';
import { rawWords } from '$lib/signing/raw-json';
import { checkingCard, landedVerdict, verdictRooms, type SimEnded } from '$lib/signing/verdict';
import type { MultisigMessages } from '../messages';
import type { CriticalWrite, MultisigReviewOut, ReviewRecipient } from './kernel';

export interface ReviewContext {
	m: MultisigMessages;
	signing: SigningMessages;
	/** The network's name, as the sheet says it. */
	network: string;
	nativeSymbol: string;
	/** The person's number preset (`resolvedFormatKeys().number`). */
	preset: string;
	/**
	 * R22: where this network's RPC settings open ("Only use my node"), for
	 * the hint under a simulation nobody independent confirmed.
	 */
	ownNodeHref?: string;
	/** The name this device knows for an address, if any. */
	nameOf: (address: string) => string | null;
	/**
	 * The name this multisig keeps for a member — the one typed when they
	 * were added (r5reg-16) — if any. Asked first for a member being added.
	 */
	memberName?: (address: string) => string | null;
}

/** One undeclared change, in words. */
export function writeLine(w: CriticalWrite, m: MultisigMessages['review']): string[] {
	switch (w.kind) {
		case 'singleton':
			return [fillTemplate(m.writeSingleton, { before: w.before, after: w.after })];
		case 'module':
			return [fillTemplate(w.enabled ? m.writeModuleOn : m.writeModuleOff, { address: w.address })];
		case 'guard':
			return [fillTemplate(m.writeGuard, { after: w.after })];
		case 'moduleGuard':
			return [fillTemplate(m.writeModuleGuard, { after: w.after })];
		case 'handler':
			return [fillTemplate(m.writeHandler, { after: w.after })];
		case 'owners':
			return [
				...w.added.map((address) => fillTemplate(m.writeAdded, { address })),
				...w.removed.map((address) => fillTemplate(m.writeRemoved, { address }))
			];
		case 'ownerEntry':
			return [fillTemplate(m.writeOwnerEntry, { address: w.address })];
		case 'threshold':
			return [fillTemplate(m.writeThreshold, { before: w.before, after: w.after })];
		case 'ownerCount':
			return [fillTemplate(m.writeOwnerCount, { after: w.after })];
		case 'nonce':
			return [m.writeNonce];
		case 'storage':
			return [fillTemplate(m.writeStorage, { slot: w.slot })];
	}
}

/** The state-diff guard, as the sheet says it. */
export function stateBlocks(review: MultisigReviewOut, ctx: ReviewContext): Block[] {
	const m = ctx.m.review;
	const v = review.state.verdict;
	const network = ctx.network;
	const source = review.state.source === 'trace' ? m.sourceTrace : m.sourceProbe;
	switch (v.kind) {
		case 'undeclared': {
			const lines = [...new Set(v.writes.flatMap((w) => writeLine(w, m)))];
			return [
				{ kind: 'intent', text: m.stateTitle, tone: 'danger' },
				{ kind: 'warning', tone: 'danger', text: fillTemplate(m.stateBody, { network }) },
				{
					kind: 'card',
					tone: 'danger',
					// Each write as a line of its own, read left to right (no value column).
					rows: lines.map((label) => ({ label, value: '' }))
				},
				{ kind: 'source', text: source, tone: 'neutral' }
			];
		}
		case 'unchanged':
		case 'declared':
			return [
				{
					kind: 'positive',
					text: fillTemplate(v.kind === 'declared' ? m.stateDeclared : m.stateUnchanged, {
						network
					})
				},
				{ kind: 'source', text: source, tone: 'neutral' }
			];
		case 'unavailable': {
			// §39 (r22-2): one node could check, where the pool holds no other
			// provider — said as that, with the way to add one's own node (the
			// signing sheet's own destination: this network's RPC settings).
			if (v.reason === 'oneNode' && m.stateOneNode) {
				return [
					{
						kind: 'warning',
						tone: 'caution',
						text: fillTemplate(m.stateOneNode, { network }),
						ownNode: {
							text: '',
							action: m.addOwnNode,
							...(ctx.ownNodeHref ? { href: ctx.ownNodeHref } : {})
						}
					}
				];
			}
			// R22 (review 1-2): the core names why — one node's word nobody
			// independent confirmed, or two nodes that disagree; anything else is
			// "couldn't check" as before.
			const text =
				v.reason === 'uncorroborated' && m.stateUnconfirmed
					? m.stateUnconfirmed
					: v.reason === 'disagreed' && m.stateDisagreed
						? m.stateDisagreed
						: m.stateUnread;
			return [{ kind: 'warning', tone: 'caution', text: fillTemplate(text, { network }) }];
		}
		case 'reverts':
			// The verdict's place says it fails; nothing would change.
			return [];
		case 'withheld':
			// r9-9: refused for another reason, said once (`refused`): no
			// reassurance beside it.
			return [];
	}
}

function roleLabel(r: ReviewRecipient, m: MultisigMessages['review']): string {
	const role =
		r.role === 'recipient'
			? m.roleRecipient
			: r.role === 'contract'
				? m.roleContract
				: m.roleMember;
	return r.call === null ? role : `${fillTemplate(m.callNumber, { n: String(r.call) })} · ${role}`;
}

/**
 * What is known of a recipient, as its row's badge ("First time sending
 * here"). The core's cues (r4a-18, r5reg-8, r5reg-16) are never of one's own
 * account, a member, or a member being added (who is sent nothing); for
 * those the badge says whose it is ("Your account" / "Member").
 */
function recipientBadge(r: ReviewRecipient, ctx: ReviewContext): PartyBadge | undefined {
	const m = ctx.m.review;
	const tags = [
		r.firstTimeCue ? ctx.signing.tagFirstTime : null,
		r.contractCue ? ctx.signing.tagContract : null
	].filter((t): t is string => t !== null);
	if (tags.length > 0) return { text: tags.join(' · '), tone: r.lookalike ? 'danger' : 'caution' };
	const whose =
		r.known?.kind === 'own'
			? m.knownOwn
			: r.known?.kind === 'member' && r.role !== 'member'
				? m.knownMember
				: null;
	return whose ? { text: whose, tone: 'neutral' } : undefined;
}

/**
 * r5reg-15: the addresses `blocks` (the call's own review) already name as a
 * party — its "Recipient" — lowercased. The review says each of them once.
 */
export function partyAddresses(blocks: Block[]): string[] {
	return blocks.flatMap((b) => (b.kind === 'party' && b.address ? [b.address.toLowerCase()] : []));
}

/**
 * Each recipient's badge, by lowercased address: worn by the call's own row
 * for that address when the review's row is left out (r5reg-15).
 */
export function recipientBadges(
	review: MultisigReviewOut | null,
	ctx: ReviewContext
): Record<string, PartyBadge> {
	const out: Record<string, PartyBadge> = {};
	for (const r of review?.recipients ?? []) {
		const badge = recipientBadge(r, ctx);
		const key = r.address.toLowerCase();
		if (badge && !out[key]) out[key] = badge;
	}
	return out;
}

/**
 * The name the review knows for a recipient — the one typed for a member
 * being added (r5reg-16), this device's, or the core's match — if any.
 */
function recipientName(r: ReviewRecipient, ctx: ReviewContext): string | null {
	return (
		(r.role === 'member' ? ctx.memberName?.(r.address) : null) ??
		ctx.nameOf(r.address) ??
		r.known?.label ??
		null
	);
}

/**
 * Each recipient's name, by lowercased address, when the review knows one:
 * worn by the call's own row for that address (r5reg-15 left the review's
 * named row out), so one's own account or a member reads by its name there
 * too (r4a-18) — not as a bare address beside "Your account".
 */
export function recipientNames(
	review: MultisigReviewOut | null,
	ctx: ReviewContext
): Record<string, string> {
	const out: Record<string, string> = {};
	for (const r of review?.recipients ?? []) {
		const name = recipientName(r, ctx);
		const key = r.address.toLowerCase();
		if (name && name.toLowerCase() !== key && !out[key]) out[key] = name;
	}
	return out;
}

/**
 * Every address a leg sends to, calls or makes a member — with what is known
 * of it. One the call's own review names already (`drawn`, lowercased) is
 * not named twice (r5reg-15: "Recipient 0xd9c7…" then "To 0xd9c7…"); its
 * warnings still stand.
 */
export function recipientBlocks(
	review: MultisigReviewOut,
	ctx: ReviewContext,
	drawn: readonly string[] = []
): Block[] {
	const m = ctx.m.review;
	const out: Block[] = [];
	for (const r of review.recipients) {
		if (!drawn.includes(r.address.toLowerCase())) {
			out.push({
				kind: 'party',
				label: roleLabel(r, m),
				// A member being added keeps the name typed for them (r5reg-16).
				name: recipientName(r, ctx) ?? r.address,
				address: r.address,
				badge: recipientBadge(r, ctx)
			});
		}
		if (r.lookalike) {
			out.push({
				kind: 'warning',
				tone: 'danger',
				text: fillTemplate(m.lookalike, {
					address: r.address,
					label: r.lookalike.label,
					known: r.lookalike.address
				})
			});
		}
		if (r.contractCue) {
			out.push({
				kind: 'warning',
				tone: 'caution',
				text: fillTemplate(m.toContract, { address: r.address })
			});
		}
	}
	return out;
}

/** The core's `simulation.kind` when a refusal withholds what moves (`multisig_review::SIM_WITHHELD`). */
export const SIM_WITHHELD = 'withheld';

/** The review says nothing about what moves: refused, and not by the state guard. */
export function simWithheld(review: MultisigReviewOut): boolean {
	return review.simulation.kind === SIM_WITHHELD;
}

/**
 * What moves, in the verdict's place — the signing sheet's own card: checking
 * while the review is out, then the core's verdict judged by the token rules.
 */
export function movesBlocks(
	review: MultisigReviewOut | null,
	sim: SimEnded | null,
	ctx: ReviewContext
): Block[] {
	const m = ctx.signing;
	// Refused for a reason other than the state guard's (a delegatecall, a
	// batch too deep): the core withholds what moves — nothing in its place,
	// never "No asset changes" or "Checking…".
	if (review && simWithheld(review)) return [];
	if (!review || !sim) {
		return [{ kind: 'verdict', inner: [checkingCard(m)], rooms: verdictRooms(m), landed: false }];
	}
	// R22: an unconfirmed simulation and an own-node check are said as the
	// signing sheet says them (`landedVerdict`: the core's notice key, its
	// `source_key`), with where a trusted node is set.
	const said = landedVerdict(sim, {
		m,
		nativeSymbol: ctx.nativeSymbol,
		preset: ctx.preset,
		...(ctx.ownNodeHref ? { ownNodeHref: ctx.ownNodeHref } : {})
	});
	const out: Block[] = [
		{ kind: 'verdict', inner: [said.block], rooms: verdictRooms(m), landed: true }
	];
	if (review.simulationSource === 'probe' && sim.verdict?.kind === 'deltas') {
		out.push({ kind: 'source', text: ctx.m.review.movesProbe, tone: 'neutral' });
	}
	return out;
}

/**
 * The review's own blocks, after the call's reading: the guard, who it
 * reaches (but those the call's reading names, `drawn`), what moves.
 */
export function reviewBlocks(
	review: MultisigReviewOut | null,
	sim: SimEnded | null,
	ctx: ReviewContext,
	drawn: readonly string[] = []
): Block[] {
	if (!review) return movesBlocks(null, null, ctx);
	return [
		...stateBlocks(review, ctx),
		...recipientBlocks(review, ctx, drawn),
		...movesBlocks(review, sim, ctx)
	];
}

/** The hashes a hardware wallet shows, in its order. */
export function hashRows(review: MultisigReviewOut, m: MultisigMessages): KeyValueRow[] {
	return [
		{ label: m.external.domainHash, value: review.hashes.domainHash, mono: true },
		{ label: m.external.messageHash, value: review.hashes.messageHash, mono: true },
		{ label: m.external.safeTxHash, value: review.hashes.safeTxHash, mono: true }
	];
}

/**
 * Technical details as the signing sheet folds them: the SafeTx's ten fields
 * exactly as signed, then the EIP-712 typed data in `RawData` (highlighted
 * JSON, copied exactly).
 */
export function reviewTech(review: MultisigReviewOut, ctx: ReviewContext): TechModel {
	const m = ctx.m.review;
	return {
		title: m.techTitle,
		params: review.fields.map((f) => ({ label: f.name, value: f.value, mono: true })),
		identities: [],
		raw: {
			label: m.typedData,
			hex: JSON.stringify(review.typedData, null, 2),
			words: rawWords(ctx.signing)
		},
		copyLabel: ctx.m.external.copy,
		explorerLabel: ''
	};
}

/** The confirm's words under the review's rule: "… anyway" where the signing sheet says it. */
export function anywayLabel(
	label: 'approve' | 'approveExecute' | 'execute' | 'propose',
	plain: string,
	review: MultisigReviewOut | null,
	m: MultisigMessages
): string {
	if (!review?.sign.anyway) return plain;
	return {
		approve: m.review.approveAnyway,
		approveExecute: m.review.approveExecuteAnyway,
		execute: m.review.executeAnyway,
		propose: m.review.proposeAnyway
	}[label];
}
