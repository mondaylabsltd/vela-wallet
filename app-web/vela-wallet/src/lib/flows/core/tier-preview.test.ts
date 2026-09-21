/**
 * Issue 681 — the price you tap is the price you get.
 *
 * The speed picker prices every tier with a real `fee_policy` session of its
 * own, so by the time somebody taps a row, a complete quote of THAT operation
 * at THAT tier already exists. Picking used to throw it away and ask the relay
 * again; the second answer can differ (gas moves, and the 15 s fee-signal
 * window of issue 212 can roll over while the picker sits open), so the figure
 * on screen stopped agreeing with the figure that had been tapped.
 *
 * What is pinned here is the promotion that replaced that round trip. The
 * load-bearing assertion is the NEGATIVE one — that nothing new is asked of the
 * core — because a promotion that quietly re-quotes would pass every test about
 * the number and still be the defect.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { FeeView } from '$lib/core/generated/FeeView';

interface FakeSession {
	onView: (view: FeeView) => void;
	publicKeyHex: () => string | undefined;
	start: ReturnType<typeof vi.fn>;
	dispatch: ReturnType<typeof vi.fn>;
	dispose: ReturnType<typeof vi.fn>;
}

const seams = vi.hoisted(() => {
	const IDLE = {
		busy: false,
		failed: null,
		fee: null,
		stale: false,
		fee_token: null,
		options: [],
		confirm_fee_ready: false
	};
	return {
		invalidate: vi.fn(),
		deployed: vi.fn(async (): Promise<boolean> => true),
		/** Every session the fake core handed out, oldest first. */
		sessions: [] as unknown[],
		make(options: { onView: (view: unknown) => void; publicKeyHex: () => string | undefined }) {
			const session = {
				onView: options.onView,
				publicKeyHex: options.publicKeyHex,
				// As the real core does: the dispatch itself publishes the run as
				// busy, so a request waits on the run instead of settling on a blank.
				start: vi.fn(() => session.onView({ ...IDLE, busy: true })),
				dispatch: vi.fn(),
				dispose: vi.fn()
			};
			seams.sessions.push(session);
			return session;
		}
	};
});

vi.mock('$lib/services/safe-transaction', () => ({
	invalidateFeeSignals: seams.invalidate,
	accountIsDeployed: seams.deployed
}));
vi.mock('$lib/core/client', () => ({ loadCore: vi.fn(async () => {}) }));
vi.mock('./send-estimates', () => ({
	// Enough of the real shape to tell two quotes apart: what the submit path
	// signs must be the quote that was tapped, not merely a quote.
	resolveFee: (fee: { total_wei: string } | null) =>
		fee === null ? null : { totalWei: fee.total_wei }
}));
vi.mock('./fee-session', () => ({ createFeeSession: seams.make }));

import { FeeQuote, IDLE_FEE_VIEW } from './fee-quote.svelte';
import { TierPreview } from './tier-preview.svelte';

const POLYGON = 137;
const REQUEST = {
	chainId: POLYGON,
	account: '0x' + '77'.repeat(20),
	calls: [],
	feeToken: null,
	publicKeyHex: '0xabc'
};

/** The owner's own receipt: three tiers of one POL send, in wei. */
const FEE = {
	fast: '1014643000000000000',
	standard: '754482000000000000',
	slow: '605996000000000000'
};
const settled = (tier: FeeTier): FeeView =>
	({
		...IDLE_FEE_VIEW,
		fee: { total_wei: FEE[tier as 'fast' | 'standard' | 'slow'], chain_id: POLYGON, tier }
	}) as unknown as FeeView;

const sessions = () => seams.sessions as FakeSession[];
/** Everything ever asked OF the core, across every session it handed out. */
const asksOfTheCore = () =>
	sessions().reduce((n, s) => n + s.start.mock.calls.length + s.dispatch.mock.calls.length, 0);

/**
 * Let anything a re-quote would have started actually get as far as the core.
 *
 * Counting asks on the line after a synchronous `promote()` proves NOTHING: a
 * promotion that re-quoted would suspend inside `requestQuote` at
 * `await accountIsDeployed` — and again at `await loadCore()` — long before it
 * reaches `start`/`dispatch`, so the counter is trivially unmoved at that
 * instant whichever implementation is in place. A macrotask boundary drains
 * every microtask chain those awaits are made of, so after this the count is
 * the real one.
 */
const untilTheCoreWouldHaveHeard = () => new Promise((resolve) => setTimeout(resolve, 0));

/**
 * The picker as the wallet route drives it: one settled session in force, and
 * one settled preview per tier that is not.
 */
async function picker(inForce: FeeTier = 'fast') {
	const feeQuote = new FeeQuote();
	void feeQuote.requestQuote({ ...REQUEST, tier: inForce });
	await vi.waitFor(() => expect(sessions()).toHaveLength(1));
	sessions()[0].onView(settled(inForce));

	const others = (['fast', 'standard', 'slow'] as FeeTier[]).filter((t) => t !== inForce);
	const preview = new TierPreview();
	preview.show(feeQuote.lastRequest!, others, feeQuote.generation);
	await vi.waitFor(() => expect(sessions()).toHaveLength(3));
	return { feeQuote, preview, others, main: sessions()[0] };
}

/** The session behind a preview row, so a test can speak to it directly. */
const sessionFor = (preview: TierPreview, tier: FeeTier) =>
	sessions()[1 + preview.rows.findIndex((row) => row.tier === tier)];

beforeEach(() => {
	seams.sessions.length = 0;
	seams.invalidate.mockClear();
	seams.deployed.mockReset();
	seams.deployed.mockResolvedValue(true);
});

describe('TierPreview.promote — the tapped quote becomes the quote in force', () => {
	it('adopts the settled row WITHOUT asking the core anything new', async () => {
		const { feeQuote, preview, others } = await picker();
		for (const tier of others) sessionFor(preview, tier).onView(settled(tier));
		const donor = preview.rows.find((row) => row.tier === 'slow')!.quote;

		const before = asksOfTheCore();
		expect(preview.promote(feeQuote, 'slow')).toBe(true);
		// THE point of the change. A promotion that re-quoted would agree with
		// every assertion below and still be the bug that was reported — and it
		// would ask the core one macrotask later, not on this line, which is why
		// the wait is here rather than being a tidier synchronous read.
		await untilTheCoreWouldHaveHeard();
		expect(asksOfTheCore()).toBe(before);

		// The figure on the row, the figure the row in force now shows and the
		// estimate the submit path signs are one quote.
		expect(feeQuote.view.fee).toEqual(settled('slow').fee);
		expect(feeQuote.estimate).toEqual({ totalWei: FEE.slow });
		expect(feeQuote.view.fee?.tier).toBe('slow');
		expect(feeQuote.lastRequest).toEqual({ ...REQUEST, tier: 'slow' });
		expect(feeQuote.pending).toBe(false);
		expect(feeQuote.view.busy).toBe(false);

		// The donor kept nothing, and the row left the picker's books.
		expect(preview.rows.map((row) => row.tier)).toEqual(['standard']);
		expect(donor.view).toEqual(IDLE_FEE_VIEW);
		expect(donor.lastRequest).toBeNull();
		expect(donor.pending).toBe(false);
	});

	it('re-points the session: its next view lands on the new owner, never the donor', async () => {
		const { feeQuote, preview, others } = await picker();
		for (const tier of others) sessionFor(preview, tier).onView(settled(tier));
		const slow = sessionFor(preview, 'slow');
		const donor = preview.rows.find((row) => row.tier === 'slow')!.quote;
		expect(preview.promote(feeQuote, 'slow')).toBe(true);

		// A session's callbacks are built once; if they still closed over the
		// instance that CREATED them, the fee in force would freeze here while
		// an object nobody renders took every later view.
		const later = { ...settled('slow'), stale: true } as FeeView;
		slow.onView(later);
		expect(feeQuote.view.stale).toBe(true);
		expect(donor.view).toEqual(IDLE_FEE_VIEW);
		// And the key the core reads for an undeployed Safe came across too.
		expect(slow.publicKeyHex()).toBe('0xabc');
	});

	it('leaves the donor inert: disposing it does not tear down the fee in force', async () => {
		const { feeQuote, preview, others } = await picker();
		for (const tier of others) sessionFor(preview, tier).onView(settled(tier));
		const slow = sessionFor(preview, 'slow');
		const donor = preview.rows.find((row) => row.tier === 'slow')!.quote;
		expect(preview.promote(feeQuote, 'slow')).toBe(true);

		donor.dispose();
		expect(slow.dispose).not.toHaveBeenCalled();
		expect(feeQuote.view.fee).toEqual(settled('slow').fee);
		// And folding the control away disposes the row it still owns, not the
		// one it handed over.
		preview.hide();
		expect(slow.dispose).not.toHaveBeenCalled();
		expect(sessionFor(preview, 'standard') ?? sessions()[1]).toBeDefined();
		expect(sessions()[1].dispose).toHaveBeenCalled();
	});

	it('closes the session it replaced, and moves `generation` so the other rows re-price', async () => {
		const { feeQuote, preview, others, main } = await picker();
		for (const tier of others) sessionFor(preview, tier).onView(settled(tier));
		const before = feeQuote.generation;

		expect(preview.promote(feeQuote, 'standard')).toBe(true);
		expect(main.dispose).toHaveBeenCalled();
		expect(feeQuote.generation).toBeGreaterThan(before);
	});
});

describe('TierPreview.promote — a row with nothing settled is refused', () => {
	it('refuses a row still measuring, and leaves the fee in force untouched', async () => {
		const { feeQuote, preview } = await picker();
		// `standard` and `slow` are both mid-run: `start` published `busy`.
		const before = asksOfTheCore();
		expect(preview.promote(feeQuote, 'slow')).toBe(false);
		expect(asksOfTheCore()).toBe(before);
		// Untouched, so the caller's re-quote effect still sees the OLD tier on
		// `lastRequest` and asks for a real measurement.
		expect(feeQuote.view.fee).toEqual(settled('fast').fee);
		expect(feeQuote.lastRequest?.tier).toBe('fast');
		expect(preview.rows.map((row) => row.tier)).toEqual(['standard', 'slow']);
	});

	it('refuses a row that failed — a refusal is not a price somebody can have tapped', async () => {
		const { feeQuote, preview } = await picker();
		sessionFor(preview, 'slow').onView({
			...IDLE_FEE_VIEW,
			failed: { kind: 'gas_quote_too_high' }
		} as unknown as FeeView);
		expect(preview.promote(feeQuote, 'slow')).toBe(false);
		expect(feeQuote.view.fee).toEqual(settled('fast').fee);
	});

	it('refuses a tier the picker never opened a row for', async () => {
		const { feeQuote, preview } = await picker();
		expect(preview.promote(feeQuote, 'rapid')).toBe(false);
		expect(feeQuote.view.fee).toEqual(settled('fast').fee);
	});
});

/**
 * The `send` core asks this very session what an operation costs — the warm
 * quote, the form's own, and Continue's pre-check, which leaves the form (and
 * this control) on screen for its whole round trip. A promotion supersedes
 * whatever is out, and what it tells that caller decides whether the person
 * sees a fee or an "estimate failed" they did nothing to earn: the route maps
 * `abandoned` to `estimate_failed`, and `precheck_fail` raises an alert on it.
 */
describe('TierPreview.promote — the question the core had out', () => {
	/** The calls of a DIFFERENT send — a second recipient, not a second speed. */
	const OTHER_CALLS = [{ to: '0x' + '11'.repeat(20), value: '1', data: '0x' }];

	it('answers a caller parked before the core with the tapped quote', async () => {
		const { feeQuote, preview, others } = await picker();
		for (const tier of others) sessionFor(preview, tier).onView(settled(tier));

		// The core's question, still waiting on `eth_getCode`.
		let deployedAnswers: (value: boolean) => void = () => {};
		seams.deployed.mockReturnValueOnce(
			new Promise<boolean>((resolve) => (deployedAnswers = resolve))
		);
		const coreAsked = feeQuote.requestQuote({ ...REQUEST, tier: 'fast' });
		expect(feeQuote.pending).toBe(true);

		expect(preview.promote(feeQuote, 'slow')).toBe(true);
		deployedAnswers(true);
		// The tapped quote, not a refusal. Telling the core "not estimated" here
		// would surface as `EstimateFailed` on a form somebody only tapped a
		// speed on — and the figure it is now holding IS an answer to what it
		// asked, priced at the speed the shell has just chosen.
		await expect(coreAsked).resolves.toEqual({ kind: 'ok', estimate: settled('slow').fee });
	});

	it('answers a caller already inside the core with the tapped quote', async () => {
		const { feeQuote, preview, others, main } = await picker();
		for (const tier of others) sessionFor(preview, tier).onView(settled(tier));

		// This one got all the way in: the run is out and the caller is parked
		// on its settlement rather than on a chain read.
		main.dispatch.mockImplementationOnce(() => main.onView({ ...IDLE_FEE_VIEW, busy: true }));
		const coreAsked = feeQuote.requestQuote({ ...REQUEST, tier: 'fast' });
		await vi.waitFor(() => expect(main.dispatch).toHaveBeenCalled());
		expect(feeQuote.pending).toBe(true);

		expect(preview.promote(feeQuote, 'slow')).toBe(true);
		await expect(coreAsked).resolves.toEqual({ kind: 'ok', estimate: settled('slow').fee });
	});

	it('refuses the pick when the measurement out is about a DIFFERENT operation', async () => {
		const { feeQuote, preview, others } = await picker();
		for (const tier of others) sessionFor(preview, tier).onView(settled(tier));

		let deployedAnswers: (value: boolean) => void = () => {};
		seams.deployed.mockReturnValueOnce(
			new Promise<boolean>((resolve) => (deployedAnswers = resolve))
		);
		const coreAsked = feeQuote.requestQuote({ ...REQUEST, calls: OTHER_CALLS, tier: 'fast' });

		// Nothing the picker is holding prices THAT send, so there is no answer
		// to give and the honest move is to leave the measurement alone.
		expect(preview.promote(feeQuote, 'slow')).toBe(false);
		expect(feeQuote.lastRequest?.calls).toEqual(OTHER_CALLS);
		deployedAnswers(true);
		await expect(coreAsked).resolves.toEqual({ kind: 'ok', estimate: settled('fast').fee });
	});
});

describe('TierPreview.promote — a pick while the fee in force is still measuring', () => {
	it('wins, and the measurement it superseded never reaches the adopted session', async () => {
		const { feeQuote, preview, others } = await picker();
		for (const tier of others) sessionFor(preview, tier).onView(settled(tier));

		// A measurement the shell has decided on but not yet dispatched — it is
		// waiting on `eth_getCode`. This is the window the person taps in.
		let deployedAnswers: (value: boolean) => void = () => {};
		seams.deployed.mockReturnValueOnce(
			new Promise<boolean>((resolve) => (deployedAnswers = resolve))
		);
		void feeQuote.requestQuote({ ...REQUEST, tier: 'fast' });
		expect(feeQuote.pending).toBe(true);

		const slow = sessionFor(preview, 'slow');
		expect(preview.promote(feeQuote, 'slow')).toBe(true);
		expect(feeQuote.pending).toBe(false);
		expect(feeQuote.view.fee).toEqual(settled('slow').fee);

		// The stale read comes back. It must not dispatch over the session it
		// now shares an owner with — that would re-price `fast` under `slow`'s
		// name, which is the defect inside out.
		const before = asksOfTheCore();
		deployedAnswers(true);
		await vi.waitFor(() => expect(feeQuote.view.fee).toEqual(settled('slow').fee));
		expect(asksOfTheCore()).toBe(before);
		expect(slow.dispatch).not.toHaveBeenCalled();
	});
});
