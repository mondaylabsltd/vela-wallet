/**
 * The simulation's verdict, as the signing sheet draws it.
 *
 * The verdict is the one part of a signing sheet the site being signed for
 * cannot write: every other line starts as something the dApp claimed — its
 * calldata, its intent, the token it names — and this starts as the result of
 * running the call. The phones and the desktop have drawn it since spec 082;
 * the web ran the same simulation for its fee and drew two sentences of it.
 * This is the rest: the same verdict from the same core data, by the same
 * rules, in the same order.
 *
 * **Nothing here is decided.** Three records of the core's say everything:
 *
 * - `SimVerdict` (`simOutcome`, `sim_outcome::verdict`) — what the node's
 *   reply was: a check (`deltas`), a call that fails (`reverts`, with the
 *   sanitised reason and the notice's key and tone), or no check at all
 *   (`not_offered`, `unreachable`);
 * - `TrustSimView` (the `token_trust` machine) — a check's moves, judged:
 *   the native coin and a token the wallet vouches for carry their figure;
 *   a token nobody vouches for carries only which way it moves
 *   (`TrustSimDirection`) and no figure at all, because the simulation's
 *   number for it is whatever the site chose to emit; and `no_change_key`
 *   when nothing of the person's moves;
 * - `formatSignedTokenAmount` (`l10n::format_signed_token_amount`) — a
 *   figure as written: the token ladder in the person's number preset,
 *   U+2212 for a minus, a dust amount exactly (never "−0"), and nothing for
 *   a zero.
 *
 * What this file does is put them in the sheet's vocabulary, the way
 * `SigningLive.swift`, `SigningLive.kt` and the desktop's `signing/live.rs`
 * do:
 *
 * | the core says                         | the sheet draws                               |
 * |---------------------------------------|-----------------------------------------------|
 * | a check, something moves              | the balance card: one row per move            |
 * | …an unverified token among them       | its row is a direction; one caution under all |
 * | a check, nothing moves                | the card saying "No asset changes"            |
 * | a call fails                          | the danger line, with the reason if one read  |
 * | no check / nobody judged / no row     | the caution "Vela couldn't check…"            |
 * | still out                             | the card saying "Checking…"                   |
 */
import { formatSignedTokenAmount, simOutcome } from '$lib/core/kernels';
import type { SimVerdict } from '$lib/core/generated/SimVerdict';
import type { TrustSimJudgment } from '$lib/core/generated/TrustSimJudgment';
import type { TrustSimView } from '$lib/core/generated/TrustSimView';
import { fill } from '$lib/wallet/messages';
import { SIM_WILL_FAIL_KEY, SIM_WILL_FAIL_REASON_KEY, type SigningMessages } from './messages';
import { reverted } from './sim-replies';
import type { BalanceRow, BalancesBlock, VerdictBlock, WarningBlock } from './model';

/**
 * A request's simulation once it has ENDED — everything the host holds of
 * it. Until then there is none, and the verdict's place says "Checking…".
 */
export interface SimEnded {
	/**
	 * The core's reading of the node's reply (`simOutcome`). `null` when
	 * there was no reply to read: nothing could be asked, or the answer did
	 * not read as a record.
	 */
	verdict: SimVerdict | null;
	/**
	 * The trust machine's judged view of a check's moves. `null` for every
	 * verdict that is not a check, and when nobody judged this one.
	 */
	judged: TrustSimView | null;
}

/** The words a verdict is said in — all of them the corpus's. */
export type VerdictWords = Pick<
	SigningMessages,
	| 'balancesTitle'
	| 'simChecking'
	| 'simSaid'
	| 'balanceUnverifiedToken'
	| 'warnUnverifiedAmount'
	| 'warnSimUnavailable'
	| 'warnWillFail'
	| 'warnWillFailReason'
>;

export interface VerdictContext {
	m: VerdictWords;
	/** The chain's own coin — the shell's vocabulary, as on every client. */
	nativeSymbol: string;
	/** The person's number preset, by its wire name (`comma_dot`, …). */
	preset: string;
}

/** What a landed verdict says. */
export type VerdictSaid =
	/** The core's notice: a call that fails (danger), or no check (caution). */
	| { kind: 'notice'; block: WarningBlock }
	/** Checked, and nothing of the person's moves: `line` is the core's sentence. */
	| { kind: 'no_change'; line: string; block: BalancesBlock }
	/** Checked, and something moves: the rows. */
	| { kind: 'moves'; block: BalancesBlock };

/** The minus every signed figure in this app is written with (spec 082 RJ15). */
const MINUS = '−';

/** Native coins use 18 decimals on every chain Vela reaches. */
const NATIVE_DECIMALS = 18;

/**
 * One judgment as a row, or none.
 *
 * A figure is the core's (`formatSignedTokenAmount`): a zero is written as
 * nothing and is no row. Its tone is its sign's — what arrives is a gain,
 * what leaves is plain ink (the phones' rule; red is kept for danger).
 *
 * An unverified token is its label, the caution's ink and a sign: "+" coming
 * in, "−" going out, no sign at all where the core could not read a
 * direction — and no row for a move of nothing. Never a figure: the judgment
 * carries none, so there is none here to print.
 */
function balanceRow(judgment: TrustSimJudgment, ctx: VerdictContext): BalanceRow | null {
	const figure = (symbol: string, delta: string, decimals: number): BalanceRow | null => {
		const text = formatSignedTokenAmount(delta, decimals, ctx.preset);
		if (text === null) return null;
		return { symbol, delta: text, tone: text.startsWith(MINUS) ? 'neutral' : 'success' };
	};
	switch (judgment.type) {
		case 'native':
			return figure(ctx.nativeSymbol, judgment.delta, NATIVE_DECIMALS);
		case 'erc20_trusted':
			return figure(judgment.symbol, judgment.delta, judgment.decimals);
		case 'erc20_unverified': {
			const symbol = ctx.m.balanceUnverifiedToken;
			switch (judgment.direction) {
				case 'in':
					return { symbol, delta: '+', tone: 'caution' };
				case 'out':
					return { symbol, delta: MINUS, tone: 'caution' };
				case 'unreadable':
					return { symbol, delta: '', tone: 'caution' };
				case 'still':
					return null;
			}
		}
	}
}

/** A check's judgments as the card's rows, in the core's order. */
export function balanceRows(judgments: TrustSimJudgment[], ctx: VerdictContext): BalanceRow[] {
	return judgments.flatMap((judgment) => {
		const row = balanceRow(judgment, ctx);
		return row === null ? [] : [row];
	});
}

/**
 * The caution that nothing could be checked (`sim_outcome::KEY_UNAVAILABLE`):
 * "Vela couldn't check what this transaction does. Review it before you
 * sign." — the banner the phones and the desktop draw for it.
 */
export function couldNotCheck(m: VerdictWords): WarningBlock {
	return { kind: 'warning', tone: 'caution', text: m.warnSimUnavailable };
}

/**
 * The core's notice as one banner: its tone from the core's risk, its words
 * from the core's key. A revert is the danger "expected to fail", with the
 * reason the core cleaned when one read; a node that could not check is the
 * caution. A key this build has no words for is said as "couldn't check" in
 * the core's tone — never a dotted path, and never nothing: the place is not
 * left reading as "nothing to report".
 */
function noticeBlock(verdict: SimVerdict, m: VerdictWords): WarningBlock {
	const tone = verdict.notice_risk === 'danger' ? 'danger' : 'caution';
	const key = verdict.notice_key;
	if (key === SIM_WILL_FAIL_REASON_KEY && verdict.revert_reason) {
		return {
			kind: 'warning',
			tone,
			text: fill(m.warnWillFailReason, { reason: verdict.revert_reason })
		};
	}
	if (key === SIM_WILL_FAIL_KEY || key === SIM_WILL_FAIL_REASON_KEY) {
		return { kind: 'warning', tone, text: m.warnWillFail };
	}
	// The could-not-check key, and anything this build cannot name.
	return { kind: 'warning', tone, text: m.warnSimUnavailable };
}

/** The balance card with one quiet line and no row. */
function quietCard(line: string, m: VerdictWords): BalancesBlock {
	return { kind: 'balances', title: m.balancesTitle, rows: [], note: line };
}

/**
 * The verdict of a simulation that has ended.
 *
 * - No reply to read, or a reply that is no check: the core's notice — a
 *   revert's danger, a could-not-check caution.
 * - A check nobody judged (the trust machine gave no ready view): a check
 *   this sheet cannot report. That is not "nothing moves": the caution.
 * - A check under which nothing of the person's moves, when the JUDGED view
 *   says so (`no_change_key`) and in its sentence: the quiet card.
 * - Otherwise the rows — and, with a token nobody vouches for among them,
 *   one caution under the card, once: the same sentence each time, and
 *   repeating it per row is how people stop reading it.
 * - A check with moves and no row to draw for them (figures the core would
 *   not write): the caution again, never an empty card.
 */
export function landedVerdict(sim: SimEnded, ctx: VerdictContext): VerdictSaid {
	const { m } = ctx;
	const unchecked: VerdictSaid = { kind: 'notice', block: couldNotCheck(m) };
	const verdict = sim.verdict;
	if (verdict === null) return unchecked;
	if (verdict.kind !== 'deltas') {
		return verdict.notice_key === null
			? unchecked
			: { kind: 'notice', block: noticeBlock(verdict, m) };
	}
	const judged = sim.judged;
	if (judged === null || !judged.ready) return unchecked;
	if (judged.no_change_key !== null) {
		const line = m.simSaid[judged.no_change_key];
		return line ? { kind: 'no_change', line, block: quietCard(line, m) } : unchecked;
	}
	const rows = balanceRows(judged.judgments, ctx);
	if (rows.length === 0) return unchecked;
	const unverified = judged.judgments.some(
		(judgment) => judgment.type === 'erc20_unverified' && judgment.direction !== 'still'
	);
	return {
		kind: 'moves',
		block: {
			kind: 'balances',
			title: m.balancesTitle,
			rows,
			...(unverified ? { note: m.warnUnverifiedAmount, noteTone: 'caution' as const } : {})
		}
	};
}

/**
 * What the place holds while the simulation is out: the balance card's own
 * outline and title with "Checking…" where its rows will be — so the card
 * that lands fills in the card that was there.
 */
export function checkingCard(m: VerdictWords): BalancesBlock {
	return quietCard(m.simChecking, m);
}

/** The balance rows a sheet keeps room for: a swap's — what leaves and what comes back. */
export const USUAL_MOVES = 2;

/**
 * A reason longer than the core prints, for {@link longestRevertReason}: the
 * core cuts it where it cuts every reason.
 */
const A_LONG_REASON =
	'ERC20: transfer amount exceeds the balance of the sending account, and more of it';

let longestReason: string | null | undefined;

/**
 * The longest revert reason a sheet prints, read back from the CORE: a
 * revert whose `Error(string)` runs past the core's cap, read like any other
 * (`simOutcome`), so it comes back cut where the core cuts it — the cap is
 * never restated here. `null` if the core gave none.
 */
export function longestRevertReason(): string | null {
	if (longestReason === undefined) {
		longestReason = simOutcome('', reverted(A_LONG_REASON))?.revert_reason ?? null;
	}
	return longestReason;
}

/**
 * The verdicts a sheet usually ends on, as rooms: the place is at least as
 * tall as the tallest of them from the request's first frame, so whichever
 * lands moves nothing — not the form above it, not the confirm under it
 * (Android's `verdictRooms`, the same four):
 *
 * - the core's "could not check" line (a chain whose nodes have no
 *   simulator says it on every request);
 * - its "expected to fail" line at the longest reason it prints;
 * - "No asset changes";
 * - a balance card of {@link USUAL_MOVES} rows — what a send (one) and a
 *   swap (two) show.
 *
 * Each is the block the real verdict is, so a room is as tall as the card it
 * stands for. What can still be taller is only the unusual — a third row,
 * the warning under an unverified token — and that is drawn whole, the place
 * as tall as it is.
 */
export function verdictRooms(m: VerdictWords): VerdictBlock[] {
	const reason = longestRevertReason();
	const noChange = Object.values(m.simSaid)[0];
	return [
		couldNotCheck(m),
		{
			kind: 'warning',
			tone: 'danger',
			text: reason === null ? m.warnWillFail : fill(m.warnWillFailReason, { reason })
		},
		quietCard(noChange || m.simChecking, m),
		{
			kind: 'balances',
			title: m.balancesTitle,
			rows: Array.from({ length: USUAL_MOVES }, () => ({
				symbol: '0',
				delta: '0',
				tone: 'neutral' as const
			}))
		}
	];
}
