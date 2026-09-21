/**
 * Issue 686 A, in a browser: when the fastest speed costs no more, this send
 * takes it — and for everybody already on the fastest, nothing extra happens.
 *
 * The real `FeeQuote`, the real `TierPreview` and the real `free-speed` steps,
 * wired with the wallet route's own two effects (the previews and the swap),
 * over fake core sessions. Every request that reaches "the relay" is a session
 * `start` or `dispatch`, so counting those is counting round trips — which is
 * what rule 1 is about: the factory default (`fast`) must not cost one.
 *
 * A browser test because this is effect wiring: a node test of `speedIsFree`
 * (see `speed-choice.test.ts`) stays green over a route that never runs it.
 */
import { flushSync, untrack } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { FeeView } from '$lib/core/generated/FeeView';
import { FASTEST_TIER } from '$lib/flows/speed-choice';

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
		sessions: [] as unknown[],
		make(options: { onView: (view: unknown) => void }) {
			const session = {
				onView: options.onView,
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
	invalidateFeeSignals: vi.fn(),
	accountIsDeployed: vi.fn(async () => true)
}));
vi.mock('$lib/core/client', () => ({ loadCore: vi.fn(async () => {}) }));
vi.mock('$lib/flows/core/fee-session', () => ({ createFeeSession: seams.make }));
vi.mock('$lib/flows/core/send-estimates', () => ({
	resolveFee: (fee: { total_wei: string } | null) =>
		fee === null ? null : { totalWei: fee.total_wei }
}));

import { FeeQuote } from './fee-quote.svelte';
import { TierPreview } from './tier-preview.svelte';
import { freeSpeedPartner, freeSpeedSwap, pickInForce, previewTiers } from './free-speed';

/**
 * `live-send`'s `OFFERED_TIERS`, restated: that module pulls in the core
 * client this file mocks away. The order is the picker's, fastest first.
 */
type OfferedTier = Exclude<FeeTier, 'rapid'>;
const OFFERED_TIERS: readonly OfferedTier[] = ['fast', 'standard', 'slow'];

interface FakeSession {
	onView: (view: FeeView) => void;
	start: ReturnType<typeof vi.fn>;
	dispatch: ReturnType<typeof vi.fn>;
	dispose: ReturnType<typeof vi.fn>;
}
const sessions = () => seams.sessions as FakeSession[];
const tierOf = (session: FakeSession) =>
	(session.start.mock.calls[0]?.[0] as { tier?: string } | undefined)?.tier;
/** The LATEST session started for a tier — a promotion re-prices the partner. */
const sessionFor = (tier: string) => sessions().findLast((session) => tierOf(session) === tier);
/** Round trips: every question any session put to the core. */
const asked = () =>
	sessions().reduce((n, s) => n + s.start.mock.calls.length + s.dispatch.mock.calls.length, 0);

const REQUEST = {
	chainId: 10,
	account: '0x' + '77'.repeat(20),
	calls: [],
	feeToken: null,
	publicKeyHex: undefined
};
/** Optimism, floor-clamped: every tier the same $0.01, each its own gas-price range. */
const RANGE: Record<string, [string, string]> = {
	fast: ['3244', '9000'],
	standard: ['2377', '6000'],
	slow: ['1937', '4500']
};
const settled = (tier: FeeTier, totalWei: string, ranged = true): FeeView =>
	({
		busy: false,
		failed: null,
		stale: false,
		fee_token: null,
		options: [],
		confirm_fee_ready: false,
		fee: {
			chain_id: 10,
			total_wei: totalWei,
			max_fee_per_gas: RANGE[tier][1],
			effective_gas_price: ranged ? RANGE[tier][0] : null,
			max_gas_price: ranged ? RANGE[tier][1] : null,
			tier,
			fee_asset: { type: 'native' },
			fee_recipient: '0xfee'
		}
	}) as unknown as FeeView;

beforeEach(() => {
	seams.sessions.length = 0;
});

/**
 * The route's speed wiring: its tier-in-force derivation, the preview effect
 * and the free-speed effect, each built from the same `free-speed` steps the
 * route calls. The composition itself is RESTATED here, not imported — the
 * route's effects live in `+page.svelte` — so what this file proves is the
 * steps and their order; the e2e (`fee-speed.e2e.ts`) is what runs the
 * route's own copy. `preferred` stands for `fee_tier_pref`; nothing here, and
 * nothing in the route, writes it (the e2e reads it back from IndexedDB).
 */
function wire(preferred: OfferedTier, picked: OfferedTier | null = null) {
	const feeQuote = new FeeQuote();
	const tierPreview = new TierPreview();
	const state = $state({ sendTier: picked, freeFast: false, open: false, onForm: true });
	const inForce = $derived<OfferedTier>(
		state.sendTier ?? (state.freeFast && preferred !== FASTEST_TIER ? FASTEST_TIER : preferred)
	);
	const partner = $derived(freeSpeedPartner(state.sendTier, preferred, inForce, state.onForm));

	const stop = $effect.root(() => {
		$effect(() => {
			const base = feeQuote.lastRequest;
			const tiers = previewTiers(state.open, OFFERED_TIERS, inForce, partner);
			if (tiers.length === 0 || base === null) {
				tierPreview.hide();
				return;
			}
			tierPreview.show(base, tiers, feeQuote.generation);
		});
		$effect(() => {
			const p = partner;
			const next = freeSpeedSwap({
				feeQuote,
				tierPreview,
				preferred,
				inForce,
				partner: p
			});
			if (next === null || p === null) return;
			untrack(() => {
				if (tierPreview.promote(feeQuote, p)) state.freeFast = next;
			});
		});
	});
	return {
		feeQuote,
		tierPreview,
		state,
		stop,
		inForce: () => inForce,
		/** The route's `pickSpeed` for a tap on the option already ticked. */
		tapInForce: () => {
			state.sendTier = pickInForce(state.sendTier, partner, inForce);
		},
		/** What `feeTier` names on the wire: the fee in force's own tier. */
		wireTier: () => feeQuote.view.fee?.tier ?? null
	};
}

/** Let the effects, and the core's answers behind them, run. */
async function settle() {
	for (let i = 0; i < 4; i += 1) {
		flushSync();
		await Promise.resolve();
	}
	flushSync();
}

describe('taking a free speed (issue 686 A)', () => {
	it('costs nothing extra for somebody already on the fastest speed', async () => {
		const w = wire('fast');
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'fast' });
		await vi.waitFor(() => expect(sessions()).toHaveLength(1));
		sessionFor('fast')!.onView(settled('fast', '10000'));
		await settle();
		// One session, one question: not a single extra quote was requested.
		expect(sessions()).toHaveLength(1);
		expect(asked()).toBe(1);
		expect(w.wireTier()).toBe('fast');
		w.stop();
	});

	it('sends the fastest speed when it costs what the slower default costs', async () => {
		const w = wire('slow');
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'slow' });
		// The default's own quote, and one beside it for the fastest speed.
		await vi.waitFor(() => expect(sessions()).toHaveLength(2));
		expect(tierOf(sessions()[1])).toBe('fast');
		sessionFor('slow')!.onView(settled('slow', '10000'));
		sessionFor('fast')!.onView(settled('fast', '10000'));
		const fastSession = sessionFor('fast')!;
		await settle();

		expect(w.state.freeFast).toBe(true);
		expect(w.inForce()).toBe('fast');
		// The fee in force IS the fastest speed's settled quote — promoted, not
		// re-asked: its session was never asked a second question.
		expect(fastSession.dispatch).not.toHaveBeenCalled();
		expect(w.feeQuote.view.fee?.tier).toBe('fast');
		expect(w.feeQuote.view.fee?.max_fee_per_gas).toBe(RANGE.fast[1]);
		expect(w.feeQuote.estimate).toEqual({ totalWei: '10000' });
		// …and it is the tier the submit names.
		expect(w.wireTier()).toBe('fast');
		w.stop();
	});

	it('keeps the default when the fastest speed is dearer', async () => {
		const w = wire('slow');
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'slow' });
		await vi.waitFor(() => expect(sessions()).toHaveLength(2));
		sessionFor('slow')!.onView(settled('slow', '10000'));
		sessionFor('fast')!.onView(settled('fast', '10001'));
		await settle();
		expect(w.state.freeFast).toBe(false);
		expect(w.inForce()).toBe('slow');
		expect(w.wireTier()).toBe('slow');
		w.stop();
	});

	it('honours a speed picked on this send, even when a faster one is free', async () => {
		const w = wire('standard', 'slow');
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'slow' });
		await vi.waitFor(() => expect(sessions()).toHaveLength(1));
		sessionFor('slow')!.onView(settled('slow', '10000'));
		await settle();
		// Nothing priced beside it — a pick is a decision, not a question.
		expect(sessions()).toHaveLength(1);
		expect(w.inForce()).toBe('slow');
		expect(w.wireTier()).toBe('slow');
		w.stop();
	});

	it('does not upgrade where nothing tells the speeds apart (B before A)', async () => {
		const w = wire('slow');
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'slow' });
		await vi.waitFor(() => expect(sessions()).toHaveLength(2));
		// Tempo: the same fee, and no gas price on either.
		sessionFor('slow')!.onView(settled('slow', '10000', false));
		sessionFor('fast')!.onView(settled('fast', '10000', false));
		await settle();
		expect(w.state.freeFast).toBe(false);
		expect(w.wireTier()).toBe('slow');
		w.stop();
	});

	it('honours a tap on the slower default made while Fast was still measuring', async () => {
		const w = wire('slow');
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'slow' });
		await vi.waitFor(() => expect(sessions()).toHaveLength(2));
		const fastSession = sessionFor('fast')!;
		sessionFor('slow')!.onView(settled('slow', '10000'));
		await settle();
		// Slow is ticked, Fast still reads "…" — and the person taps Slow.
		w.tapInForce();
		await settle();
		expect(w.state.sendTier).toBe('slow');
		// Fast then settles at exactly the same fee. A decision made on this
		// send is not overruled by a speed that turned out free afterwards.
		fastSession.onView(settled('fast', '10000'));
		await settle();
		expect(w.state.freeFast).toBe(false);
		expect(w.inForce()).toBe('slow');
		expect(w.wireTier()).toBe('slow');
		// …and nothing is priced for a question that no longer exists.
		expect(fastSession.dispose).toHaveBeenCalled();
		w.stop();
	});

	it('leaves a tap on the ticked Fast of a Fast default as no pick at all', async () => {
		const w = wire('fast');
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'fast' });
		await vi.waitFor(() => expect(sessions()).toHaveLength(1));
		sessionFor('fast')!.onView(settled('fast', '10000'));
		await settle();
		w.tapInForce();
		// No upgrade question, so nothing to decide: the confirm keeps saying
		// nothing about a speed nobody moved (spec 068).
		expect(w.state.sendTier).toBe(null);
		w.stop();
	});

	it('asks nothing once the send has left its form', async () => {
		const w = wire('slow');
		w.state.onForm = false;
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'slow' });
		await vi.waitFor(() => expect(sessions()).toHaveLength(1));
		sessionFor('slow')!.onView(settled('slow', '10000'));
		await settle();
		// The confirm names what was in force at Continue: no partner is priced
		// behind it, so nothing can swap the tier under the person reading it.
		expect(sessions()).toHaveLength(1);
		expect(w.inForce()).toBe('slow');
		w.stop();
	});

	it('keeps an upgrade taken on the form through the confirm', async () => {
		const w = wire('slow');
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'slow' });
		await vi.waitFor(() => expect(sessions()).toHaveLength(2));
		sessionFor('slow')!.onView(settled('slow', '10000'));
		sessionFor('fast')!.onView(settled('fast', '10000'));
		await settle();
		expect(w.inForce()).toBe('fast');
		// Continue. Whatever the partner (now Slow) would say next is not asked.
		w.state.onForm = false;
		await settle();
		expect(w.tierPreview.rows).toEqual([]);
		expect(w.inForce()).toBe('fast');
		expect(w.wireTier()).toBe('fast');
		w.stop();
	});

	it('holds while the fastest speed is still being measured', async () => {
		const w = wire('slow');
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'slow' });
		await vi.waitFor(() => expect(sessions()).toHaveLength(2));
		sessionFor('slow')!.onView(settled('slow', '10000'));
		await settle();
		expect(w.state.freeFast).toBe(false);
		expect(w.wireTier()).toBe('slow');
		w.stop();
	});
});
