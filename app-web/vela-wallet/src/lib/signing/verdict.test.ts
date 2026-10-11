/**
 * The simulation's verdict, as the web sheet draws it — held to the REAL core.
 *
 * Every verdict here starts as a node's `eth_simulateV1` reply and is read by
 * the core (`simOutcome`); every check's moves are judged by a real
 * `token_trust` machine (`judgedByFreshMachine`); every figure is the core's
 * (`formatSignedTokenAmount`). So what is pinned is this shell's half only:
 * that each of the core's records comes out as the block the phones and the
 * desktop draw for it, in the same order, with the same words.
 *
 * The rules, said once (`SigningLive.swift`, `SigningLive.kt`, the desktop's
 * `signing/live.rs`):
 *
 * - a figure the wallet vouches for is written by the core; what arrives is a
 *   gain, what leaves is plain ink; a zero is no row;
 * - a token nobody vouches for is a DIRECTION and never a figure, and one
 *   caution stands under the whole card;
 * - "No asset changes" is the judged view's to say;
 * - a call that fails is the danger line, with the reason the core cleaned;
 * - anything that is not a check this sheet can report is "could not check".
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
	formatSignedTokenAmount,
	judgedByFreshMachine,
	simOutcome,
	type TrustFacts
} from '$lib/core/kernels';
import type { TrustSimJudgment } from '$lib/core/generated/TrustSimJudgment';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { fill } from '$lib/wallet/messages';
import { SIM_SAID_KEYS } from './messages';
import {
	NOT_OFFERED,
	UNREACHABLE,
	answered,
	nativeLog,
	ran,
	reverted,
	transferLog
} from './sim-replies';
import {
	USUAL_MOVES,
	balanceRows,
	checkingCard,
	couldNotCheck,
	landedVerdict,
	longestRevertReason,
	verdictRooms,
	type SimEnded,
	type VerdictContext
} from './verdict';

const m = resolveSigningMessages('en');
const zh = resolveSigningMessages('zh');
const ME = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';
const SITE = '0x' + '51'.repeat(20);
/** Mainnet USDC: the core's own registry knows it (six decimals). */
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
/** A token that answers "USDC", 18 decimals — as any contract can. */
const LURE = '0x' + 'ba'.repeat(20);
/** A token whose `symbol()` does not answer. */
const MUTE = '0x' + 'cd'.repeat(20);
/** A token the account holds. */
const HELD = '0x' + 'e1'.repeat(20);
const FACTS: TrustFacts = {
	held: [HELD],
	meta: {
		[LURE]: { symbol: 'USDC', decimals: 18 },
		[HELD]: { symbol: 'WETH', decimals: 18 }
	}
};
const ctx = (over: Partial<VerdictContext> = {}): VerdictContext => ({
	m,
	nativeSymbol: 'ETH',
	preset: 'comma_dot',
	...over
});

/** A node's reply, read by the core and — for a check — judged by a real machine. */
function ended(reply: string, facts: TrustFacts = FACTS): SimEnded {
	const verdict = simOutcome(ME, reply);
	return {
		verdict,
		judged: verdict?.kind === 'deltas' ? judgedByFreshMachine(ME, 1, verdict.deltas, facts) : null
	};
}

const unit = (decimals: number) => 10n ** BigInt(decimals);
const CAUTION = { kind: 'warning', tone: 'caution', text: m.warnSimUnavailable };

describe('a check that moves something: the balance card', () => {
	it('"−10 USDC", "+0.0031 ETH": one row per move, the core’s figure, in the core’s order', () => {
		const said = landedVerdict(
			ended(
				ran(transferLog(USDC, ME, SITE, 10n * unit(6)), nativeLog(SITE, ME, 3_100_000_000_000_000n))
			),
			ctx()
		);
		expect(said).toEqual({
			kind: 'moves',
			block: {
				kind: 'balances',
				title: 'Balance changes',
				rows: [
					// What leaves is plain ink, with U+2212.
					{ symbol: 'USDC', delta: '−10', tone: 'neutral' },
					// What arrives is a gain.
					{ symbol: 'ETH', delta: '+0.0031', tone: 'success' }
				]
			}
		});
		// No caution under a card whose every row the wallet vouches for.
		expect(said.block).not.toHaveProperty('note');
	});

	it('every figure is the core’s own writing of the delta — in the person’s number preset', () => {
		const sim = ended(
			ran(
				transferLog(USDC, ME, SITE, 1_250n * unit(6)),
				transferLog(HELD, SITE, ME, 431_200_000_000_000_000n)
			)
		);
		const rows = (preset: string) => {
			const said = landedVerdict(sim, ctx({ preset }));
			return said.kind === 'moves' ? said.block.rows.map((row) => row.delta) : [];
		};
		expect(rows('comma_dot')).toEqual(['−1,250.00', '+0.4312']);
		expect(rows('dot_comma')).toEqual(['−1.250,00', '+0,4312']);
		for (const preset of ['comma_dot', 'dot_comma', 'space_comma', 'indian']) {
			expect(rows(preset)).toEqual([
				formatSignedTokenAmount('-1250000000', 6, preset),
				formatSignedTokenAmount('431200000000000000', 18, preset)
			]);
		}
	});

	it('the native coin is named by the chain it is on', () => {
		const sim = ended(ran(nativeLog(ME, SITE, unit(18))));
		const row = (nativeSymbol: string) => {
			const said = landedVerdict(sim, ctx({ nativeSymbol }));
			return said.kind === 'moves' ? said.block.rows : [];
		};
		expect(row('xDAI')).toEqual([{ symbol: 'xDAI', delta: '−1', tone: 'neutral' }]);
		expect(row('POL')).toEqual([{ symbol: 'POL', delta: '−1', tone: 'neutral' }]);
	});

	it('a dust amount is written exactly, never "−0"', () => {
		const said = landedVerdict(ended(ran(nativeLog(ME, SITE, 1000n))), ctx());
		expect(said.kind === 'moves' && said.block.rows).toEqual([
			{ symbol: 'ETH', delta: '−0.000000000000001', tone: 'neutral' }
		]);
	});

	it('a token the account holds arrives with its figure; in and out of one token nets to one row', () => {
		const said = landedVerdict(
			ended(
				ran(transferLog(HELD, SITE, ME, 3n * unit(18)), transferLog(HELD, ME, SITE, 1n * unit(18)))
			),
			ctx()
		);
		expect(said.kind === 'moves' && said.block.rows).toEqual([
			{ symbol: 'WETH', delta: '+2', tone: 'success' }
		]);
	});
});

describe('an unverified token is a direction and never a figure', () => {
	/** The figure the site chose to emit: 5,000 of an 18-decimal token. */
	const LURED = 5_000n * unit(18);

	it('"Unverified token +": the label, the sign, the caution’s ink — and one warning under the card', () => {
		const sim = ended(
			ran(nativeLog(ME, SITE, 1_500_000_000_000_000_000n), transferLog(LURE, SITE, ME, LURED))
		);
		// The real machine's judgment carries no figure for it at all.
		expect(sim.judged!.judgments[1]).toEqual({
			type: 'erc20_unverified',
			token: LURE,
			direction: 'in'
		});
		const said = landedVerdict(sim, ctx());
		expect(said).toEqual({
			kind: 'moves',
			block: {
				kind: 'balances',
				title: 'Balance changes',
				rows: [
					{ symbol: 'ETH', delta: '−1.5', tone: 'neutral' },
					{ symbol: 'Unverified token', delta: '+', tone: 'caution' }
				],
				note: "Token amount couldn't be verified on-chain — double-check it before approving.",
				noteTone: 'caution'
			}
		});
		// Not the number, not the name the contract gave itself.
		const drawn = JSON.stringify(said);
		expect(drawn).not.toContain('5000');
		expect(drawn).not.toContain('5,000');
		expect(drawn).not.toContain('USDC');
		expect(m.balanceUnverifiedToken).toBe('Unverified token');
		expect(m.warnUnverifiedAmount).toBe(said.block.kind === 'balances' ? said.block.note : '');
	});

	it('going out it wears the minus the figures wear; one nobody can name is unverified too', () => {
		const said = landedVerdict(ended(ran(transferLog(MUTE, ME, SITE, 7_000n * unit(18)))), ctx());
		expect(said.kind === 'moves' && said.block).toMatchObject({
			rows: [{ symbol: 'Unverified token', delta: '−', tone: 'caution' }],
			noteTone: 'caution'
		});
		expect(JSON.stringify(said)).not.toContain('7000');
		expect(JSON.stringify(said)).not.toContain('7,000');
	});

	it('the warning is said ONCE for the card, however many such rows', () => {
		const said = landedVerdict(
			ended(
				ran(
					transferLog(LURE, SITE, ME, LURED),
					transferLog(MUTE, ME, SITE, 1n),
					nativeLog(SITE, ME, unit(18))
				)
			),
			ctx()
		);
		expect(said.kind === 'moves' && said.block.rows).toEqual([
			{ symbol: 'Unverified token', delta: '+', tone: 'caution' },
			{ symbol: 'Unverified token', delta: '−', tone: 'caution' },
			{ symbol: 'ETH', delta: '+1', tone: 'success' }
		]);
		expect(JSON.stringify(said).split(m.warnUnverifiedAmount).length - 1).toBe(1);
	});

	it('every direction the core can name: in, out, unreadable (no sign, still a row), still (no row, no warning)', () => {
		const judged = (direction: 'in' | 'out' | 'still' | 'unreadable'): TrustSimJudgment => ({
			type: 'erc20_unverified',
			token: LURE,
			direction
		});
		const row = (delta: string) => ({ symbol: 'Unverified token', delta, tone: 'caution' });
		expect(balanceRows([judged('in')], ctx())).toEqual([row('+')]);
		expect(balanceRows([judged('out')], ctx())).toEqual([row('−')]);
		expect(balanceRows([judged('unreadable')], ctx())).toEqual([row('')]);
		expect(balanceRows([judged('still')], ctx())).toEqual([]);

		const view = (judgments: TrustSimJudgment[]) => ({
			verdict: simOutcome(ME, ran(nativeLog(ME, SITE, 1n)))!,
			judged: {
				address: ME.toLowerCase(),
				chain_id: 1,
				ready: true,
				judgments,
				no_change_key: null
			}
		});
		const native: TrustSimJudgment = { type: 'native', delta: '-5' };
		// A move of nothing beside a real one: its row is not drawn, and nothing warns.
		const quiet = landedVerdict(view([native, judged('still')]), ctx());
		expect(quiet.kind === 'moves' && quiet.block.rows).toHaveLength(1);
		expect(quiet.block).not.toHaveProperty('note');
		// A figure nobody could read keeps its row and its caution.
		const unread = landedVerdict(view([judged('unreadable')]), ctx());
		expect(unread.kind === 'moves' && unread.block).toMatchObject({
			rows: [row('')],
			note: m.warnUnverifiedAmount
		});
	});

	it('is said in the reader’s language', () => {
		const said = landedVerdict(
			ended(ran(transferLog(LURE, SITE, ME, LURED))),
			ctx({ m: zh, nativeSymbol: 'ETH' })
		);
		expect(said.kind === 'moves' && said.block).toMatchObject({
			title: '余额变化',
			rows: [{ symbol: '未验证代币', delta: '+', tone: 'caution' }]
		});
	});
});

describe('a check that moves nothing', () => {
	it('"No asset changes": the card with no row, when the judged view says so, in its sentence', () => {
		const sim = ended(ran());
		expect(sim.judged).toMatchObject({
			ready: true,
			judgments: [],
			no_change_key: SIM_SAID_KEYS[0]
		});
		expect(landedVerdict(sim, ctx())).toEqual({
			kind: 'no_change',
			line: 'No asset changes',
			block: { kind: 'balances', title: 'Balance changes', rows: [], note: 'No asset changes' }
		});
		expect(landedVerdict(sim, ctx({ m: zh }))).toMatchObject({ line: '无资产变动' });
	});

	it('a transfer to oneself nets to nothing, and the machine says nothing moves', () => {
		const sim = ended(ran(transferLog(USDC, ME, ME, 5n * unit(6))));
		expect(landedVerdict(sim, ctx()).kind).toBe('no_change');
	});

	it('other people’s transfers are not the person’s moves', () => {
		const other = '0x' + '77'.repeat(20);
		expect(landedVerdict(ended(ran(transferLog(USDC, SITE, other, 9n))), ctx()).kind).toBe(
			'no_change'
		);
	});
});

describe('a call that fails: the danger line', () => {
	it('"Expected to fail: <reason> — you’d still pay gas." with the reason the core cleaned', () => {
		const said = landedVerdict(ended(reverted('ERC20: transfer amount exceeds balance')), ctx());
		expect(said).toEqual({
			kind: 'notice',
			block: {
				kind: 'warning',
				tone: 'danger',
				text: 'Expected to fail: ERC20: transfer amount exceeds balance — you’d still pay gas.'
			}
		});
	});

	it('no readable reason: the plain sentence — a panic, a custom error, text that names a wallet sentence', () => {
		const plain = {
			kind: 'notice',
			block: {
				kind: 'warning',
				tone: 'danger',
				text: 'This transaction is expected to fail — you’d still pay gas.'
			}
		};
		expect(landedVerdict(ended(reverted()), ctx())).toEqual(plain);
		// A reason carrying the corpus's reference syntax is no reason (the core drops it whole).
		expect(landedVerdict(ended(reverted('$t(onboarding.common.close)')), ctx())).toEqual(plain);
		// A failure beside a success: the first failure speaks.
		const mixed = answered({ status: '0x1', logs: [] }, { status: '0x0', error: { code: 3 } });
		expect(landedVerdict(ended(mixed), ctx())).toEqual(plain);
	});

	it('a reason is cut where the core cuts it, and the sentence is filled once', () => {
		const long = 'x'.repeat(300);
		const said = landedVerdict(ended(reverted(long)), ctx());
		const reason = simOutcome(ME, reverted(long))!.revert_reason!;
		expect(reason.length).toBeLessThan(long.length);
		expect(said.block).toEqual({
			kind: 'warning',
			tone: 'danger',
			text: fill(m.warnWillFailReason, { reason })
		});
		// The longest a sheet prints is the core's cap, read back from the core.
		expect(longestRevertReason()).not.toBeNull();
		expect(longestRevertReason()!.length).toBe(reason.length);
		const source = readFileSync('../../rust/crates/vela-core/src/app/sim_outcome.rs', 'utf8');
		const cap = Number(/pub const REVERT_REASON_MAX_CHARS: usize = (\d+);/.exec(source)?.[1]);
		expect(longestRevertReason()!.length).toBe(cap);
	});
});

describe('no check this sheet can report: "Vela couldn’t check…"', () => {
	it('a node that does not offer it, an answer nobody can read, no node at all, no reply to read', () => {
		const unread: SimEnded[] = [
			ended(NOT_OFFERED),
			ended(UNREACHABLE),
			ended(answered({ status: '0x2', logs: [] })),
			ended('{}'),
			ended('not json'),
			{ verdict: null, judged: null }
		];
		for (const sim of unread) {
			expect(landedVerdict(sim, ctx()), JSON.stringify(sim.verdict)).toEqual({
				kind: 'notice',
				block: CAUTION
			});
		}
		expect(couldNotCheck(m)).toEqual(CAUTION);
		expect(CAUTION.text).toBe(
			'Vela couldn’t check what this transaction does. Review it before you sign.'
		);
	});

	it('a check nobody judged, or judged and not ready, is not "nothing moves"', () => {
		const check = ended(ran(transferLog(USDC, ME, SITE, 1n)));
		expect(landedVerdict({ ...check, judged: null }, ctx()).block).toEqual(CAUTION);
		expect(
			landedVerdict({ ...check, judged: { ...check.judged!, ready: false } }, ctx()).block
		).toEqual(CAUTION);
		const empty = ended(ran());
		expect(landedVerdict({ ...empty, judged: null }, ctx()).block).toEqual(CAUTION);
	});

	it('moves with no row to draw and no word from the machine: the caution, never an empty card', () => {
		const check = ended(ran(transferLog(USDC, ME, SITE, 1n)));
		const view = (judgments: TrustSimJudgment[], no_change_key: string | null = null) => ({
			...check,
			judged: { ...check.judged!, judgments, no_change_key }
		});
		// A figure the core will not write (decimals no uint8 holds).
		expect(
			landedVerdict(
				view([{ type: 'erc20_trusted', token: USDC, delta: '-5', symbol: 'X', decimals: 999 }]),
				ctx()
			).block
		).toEqual(CAUTION);
		// No judgment and no key.
		expect(landedVerdict(view([]), ctx()).block).toEqual(CAUTION);
		// A key this build has no words for.
		expect(landedVerdict(view([], 'componentsUi.signing.somethingNew'), ctx()).block).toEqual(
			CAUTION
		);
	});
});

describe('what the place holds before a verdict, and the room it keeps', () => {
	it('while the simulation is out: the card’s own outline and title, saying "Checking…"', () => {
		expect(checkingCard(m)).toEqual({
			kind: 'balances',
			title: 'Balance changes',
			rows: [],
			note: 'Checking…'
		});
		expect(checkingCard(zh)).toMatchObject({ title: '余额变化', note: '正在检查…' });
	});

	it('the rooms are the verdicts a sheet usually ends on, each the block the real one is', () => {
		const rooms = verdictRooms(m);
		expect(rooms).toEqual([
			CAUTION,
			{
				kind: 'warning',
				tone: 'danger',
				text: fill(m.warnWillFailReason, { reason: longestRevertReason()! })
			},
			{ kind: 'balances', title: 'Balance changes', rows: [], note: 'No asset changes' },
			{
				kind: 'balances',
				title: 'Balance changes',
				rows: Array.from({ length: USUAL_MOVES }, () => ({
					symbol: '0',
					delta: '0',
					tone: 'neutral'
				}))
			}
		]);
		expect(USUAL_MOVES).toBe(2);
		// Each usual verdict is one of them — so its landing changes no height.
		const usual = [
			landedVerdict(ended(NOT_OFFERED), ctx()).block,
			landedVerdict(ended(ran()), ctx()).block
		];
		for (const block of usual) expect(rooms).toContainEqual(block);
		// A send's one row and a swap's two fit the card of two rows.
		const swap = landedVerdict(
			ended(ran(transferLog(USDC, ME, SITE, 1n), nativeLog(SITE, ME, 1n))),
			ctx()
		);
		expect(swap.kind === 'moves' && swap.block.rows.length).toBe(USUAL_MOVES);
		expect(swap.block).not.toHaveProperty('note');
	});

	it('the rooms are in the reader’s language, at the sentences’ own length', () => {
		const rooms = verdictRooms(zh);
		expect(rooms[0]).toMatchObject({ text: zh.warnSimUnavailable });
		expect(rooms[2]).toMatchObject({ note: '无资产变动' });
	});
});
