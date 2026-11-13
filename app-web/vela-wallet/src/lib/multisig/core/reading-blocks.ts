/**
 * r8-1, r8-2, r8-5: the core's reading of a multisig transaction's calls
 * (`MultisigReview.reading`) in the signing sheet's own block kinds — every
 * call, every leg of every batch, nested ones too — and the lines the core's
 * concerns say about them. The core decided everything (what each call is,
 * its token's decimals, what is unlimited, what is refused); this file only
 * words it, as every platform does. Nothing here decodes a byte.
 */
import { fillTemplate } from '$lib/i18n/fill';
import type { Block, KeyValueRow, Tone } from '$lib/signing/model';
import type { ReadingMessages } from '../messages';
import type {
	MultisigReviewOut,
	ReadingAmount,
	ReadingCall,
	ReadingIntent,
	ReadingToken,
	ReviewConcern
} from './kernel';
import { eip55 } from '../external/eip55';
import { formatUnits } from './units';

export interface ReadingContext {
	r: ReadingMessages;
	/** `{{n}}`: a batch's call ("Call 2"). */
	callNumber: string;
	/** The network's coin, for a coin the core names by `token: null`. */
	nativeSymbol: string;
}

function short(address: string): string {
	const a = eip55(address);
	return `${a.slice(0, 6)}…${a.slice(-4)}`;
}

/** A token as said: its symbol, else its contract (never a made-up name). */
export function tokenName(token: ReadingToken, ctx: ReadingContext): string {
	if (token.token === null) return ctx.nativeSymbol;
	return token.symbol ?? short(token.token);
}

/**
 * An amount as the core read it: unlimited is "Unlimited"; unknown decimals
 * are the raw integer with "decimals unknown" — never a friendly amount in
 * 18 decimals (r8-2).
 */
export function amountText(
	amount: ReadingAmount,
	token: ReadingToken,
	ctx: ReadingContext
): string {
	if (amount.unlimited) return ctx.r.unlimited;
	const name = tokenName(token, ctx);
	if (amount.decimals === null) {
		return `${fillTemplate(ctx.r.decimalsUnknown, { raw: amount.raw })} ${name}`.trim();
	}
	return `${formatUnits(amount.raw, amount.decimals)} ${name}`.trim();
}

/** What a call is, in one or two words ("Send", "Approve", "Batch of 2 calls"). */
export function intentTitle(intent: ReadingIntent, ctx: ReadingContext): string {
	const r = ctx.r;
	switch (intent.kind) {
		case 'send':
			return r.send;
		case 'sendFrom':
			return r.sendFrom;
		case 'approve':
			return {
				approve: r.approve,
				increase: r.increase,
				decrease: r.decrease,
				permit: r.permit,
				daiPermit: r.daiPermit,
				permit2: r.permit2
			}[intent.grant];
		case 'approveAll':
			return intent.granted ? r.approveAll : r.revokeAll;
		case 'batch':
			return intent.calls.length === 1
				? r.batchOne
				: fillTemplate(r.batchMany, { total: intent.calls.length });
		case 'unreadable':
			return r.unreadable;
		case 'members':
			return r.members;
		case 'rename':
			return fillTemplate(r.rename, { name: intent.name });
		case 'call':
			return r.call;
	}
}

/** A call's tone: danger for what grants it all or cannot be read. */
function intentTone(intent: ReadingIntent): Tone {
	switch (intent.kind) {
		case 'approve':
			return intent.amount.unlimited && intent.grant !== 'decrease' ? 'danger' : 'neutral';
		case 'approveAll':
			return intent.granted ? 'danger' : 'neutral';
		case 'unreadable':
		case 'batch':
			return 'danger';
		case 'call':
			return 'caution';
		default:
			return 'neutral';
	}
}

/** The rows of one call, each fact the core read. */
function callRows(call: ReadingCall, ctx: ReadingContext): KeyValueRow[] {
	const r = ctx.r;
	const i = call.intent;
	const rows: KeyValueRow[] = [];
	switch (i.kind) {
		case 'send':
			rows.push({ label: r.amount, value: amountText(i.amount, i.token, ctx) });
			rows.push({ label: r.to, value: eip55(i.to), mono: true });
			break;
		case 'sendFrom':
			rows.push({ label: r.amount, value: amountText(i.amount, i.token, ctx) });
			rows.push({ label: r.from, value: eip55(i.from), mono: true });
			rows.push({ label: r.to, value: eip55(i.to), mono: true });
			break;
		case 'approve': {
			const unlimited = i.amount.unlimited && i.grant !== 'decrease';
			rows.push({ label: r.token, value: tokenName(i.token, ctx) });
			rows.push({
				label: r.allowance,
				value:
					i.grant === 'daiPermit' && unlimited ? r.allOfIt : amountText(i.amount, i.token, ctx),
				valueTone: unlimited ? 'danger' : undefined
			});
			rows.push({ label: r.spender, value: eip55(i.spender), mono: true });
			break;
		}
		case 'approveAll':
			rows.push({ label: r.collection, value: eip55(i.collection), mono: true });
			rows.push({
				label: r.operator,
				value: eip55(i.operator),
				mono: true,
				valueTone: i.granted ? 'danger' : undefined
			});
			break;
		case 'batch':
			rows.push({ label: r.runner, value: eip55(i.runner), mono: true });
			break;
		case 'call':
			if (i.selector) rows.push({ label: r.function, value: i.selector, mono: true });
			break;
		default:
			break;
	}
	if (i.kind !== 'send' && i.kind !== 'approveAll' && call.value.raw !== '0') {
		rows.push({
			label: r.alsoSends,
			value: amountText(call.value, { token: null, symbol: null, decimals: 18, known: true }, ctx)
		});
	}
	return rows;
}

/** r10-10: the core's leg number; the lone call has none (never "Call  · …"). */
function cardTitle(call: ReadingCall, ctx: ReadingContext): string {
	const leg = call.leg === undefined ? call.path.join('.') || null : call.leg;
	const title = intentTitle(call.intent, ctx);
	return leg === null ? title : `${fillTemplate(ctx.callNumber, { n: leg })} · ${title}`;
}

/** Each call as a card, a batch's legs after it, every level (r8-5). */
function cards(calls: ReadingCall[], ctx: ReadingContext): Block[] {
	return calls.flatMap((call): Block[] => {
		const card: Block = {
			kind: 'card',
			title: cardTitle(call, ctx),
			rows: callRows(call, ctx),
			tone: intentTone(call.intent)
		};
		if (call.intent.kind !== 'batch') return [card];
		return [
			card,
			{ kind: 'source', text: ctx.r.runnerNote, tone: 'caution' },
			...cards(call.intent.calls, ctx)
		];
	});
}

/**
 * The reading's blocks: the lone call as the signing sheet draws one (its
 * verb, its amount or allowance, who it reaches), or every call of a batch
 * as a card. Nothing for what the multisig reads as its own (a member
 * change, a rename, a cancellation): those have their own drawing.
 */
export function readingBlocks(review: MultisigReviewOut | null, ctx: ReadingContext): Block[] {
	const calls = review?.reading?.calls ?? [];
	if (calls.length === 0) return [];
	const [only] = calls;
	if (calls.length === 1 && only.path.length === 0 && only.intent.kind !== 'batch') {
		const i = only.intent;
		const r = ctx.r;
		const out: Block[] = [{ kind: 'intent', text: intentTitle(i, ctx), tone: intentTone(i) }];
		if (i.kind === 'send') {
			out.push({
				kind: 'rows',
				rows: [{ label: r.amount, value: amountText(i.amount, i.token, ctx) }]
			});
			out.push({ kind: 'party', label: r.to, name: eip55(i.to), address: eip55(i.to) });
		} else if (i.kind === 'approve' || i.kind === 'sendFrom') {
			const rows = callRows(only, ctx).filter((row) => !row.mono);
			out.push({ kind: 'rows', rows });
			if (i.kind === 'approve') {
				out.push({
					kind: 'party',
					label: r.spender,
					name: eip55(i.spender),
					address: eip55(i.spender)
				});
			} else {
				out.push({ kind: 'party', label: r.from, name: eip55(i.from), address: eip55(i.from) });
				out.push({ kind: 'party', label: r.to, name: eip55(i.to), address: eip55(i.to) });
			}
		} else {
			const rows = callRows(only, ctx);
			if (rows.length > 0) out.push({ kind: 'rows', rows });
		}
		return out;
	}
	// A transaction whose one call is itself a batch: its legs, as a batch's.
	return cards(calls, ctx);
}

/** The reading's title for the sheet's header, or `null` when it has none. */
export function readingTitle(review: MultisigReviewOut | null, ctx: ReadingContext): string | null {
	const calls = review?.reading?.calls ?? [];
	if (calls.length === 0) return null;
	if (calls.length === 1 && calls[0].path.length === 0) {
		const i = calls[0].intent;
		if (i.kind === 'send') return `${ctx.r.send} ${amountText(i.amount, i.token, ctx)}`;
		if (i.kind === 'approve') {
			return `${intentTitle(i, ctx)} · ${tokenName(i.token, ctx)}`;
		}
		return intentTitle(i, ctx);
	}
	return calls.length === 1
		? ctx.r.batchOne
		: fillTemplate(ctx.r.batchMany, { total: calls.length });
}

/** What one of the reading's concerns says, and how loud. */
function concernLine(
	c: ReviewConcern,
	review: MultisigReviewOut,
	ctx: ReadingContext
): { text: string; tone: 'caution' | 'danger' } | null {
	const r = ctx.r;
	const named = (address: string) => {
		const t = review.reading.calls.length ? findToken(review.reading.calls, address) : null;
		return t?.symbol ?? short(address);
	};
	switch (c.kind) {
		case 'unlimitedApproval':
			return { text: fillTemplate(r.unlimitedLine, { spender: eip55(c.spender) }), tone: 'danger' };
		case 'nestedBatch':
			return { text: r.nestedLine, tone: 'danger' };
		case 'unreadableBatch':
			return { text: r.unreadableLine, tone: 'danger' };
		case 'decimalsUnknown':
			return { text: fillTemplate(r.decimalsLine, { token: named(c.token) }), tone: 'caution' };
		case 'toOwnToken':
			return { text: fillTemplate(r.ownTokenLine, { token: named(c.token) }), tone: 'danger' };
		case 'blind':
			return { text: r.blindLine, tone: 'caution' };
		default:
			return null;
	}
}

function findToken(calls: ReadingCall[], address: string): ReadingToken | null {
	for (const call of calls) {
		const i = call.intent;
		if ((i.kind === 'send' || i.kind === 'sendFrom' || i.kind === 'approve') && i.token.token) {
			if (i.token.token.toLowerCase() === address.toLowerCase()) return i.token;
		}
		if (i.kind === 'batch') {
			const found = findToken(i.calls, address);
			if (found) return found;
		}
	}
	return null;
}

/**
 * The lines the core's concerns about the calls say (r8-1, r8-2, r8-5),
 * each once (the core deduped them, r8-14): an unlimited grant, a nested
 * batch, one that cannot be read, decimals unknown, tokens sent to their own
 * contract, a call nobody reads. The review's other concerns (the state
 * guard, recipients, simulation) have their own blocks.
 */
export function concernBlocks(review: MultisigReviewOut | null, ctx: ReadingContext): Block[] {
	if (!review) return [];
	const out: Block[] = [];
	for (const c of review.concerns) {
		const line = concernLine(c, review, ctx);
		if (line && !out.some((b) => b.kind === 'warning' && b.text === line.text)) {
			out.push({ kind: 'warning', tone: line.tone, text: line.text });
		}
	}
	return out;
}
