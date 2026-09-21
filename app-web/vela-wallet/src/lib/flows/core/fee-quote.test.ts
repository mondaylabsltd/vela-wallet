// Issue 212 — the WIRING between the fee quote and the 15 s fee-signal cache.
//
// `invalidateFeeSignals` itself is pinned in
// services/safe-transaction-fee-signals.test.ts; what is pinned here is that
// `FeeQuote` calls it at the two moments that matter, because a unit test over
// the cache alone stays green with either line deleted:
//  1. the refresh affordance drops the cache BEFORE the dispatch that reads it,
//     so "look again" really measures again;
//  2. a quote that settles as failed drops it too, so a retry never reuses the
//     inputs that failed it — and a quote that settles well leaves it held.
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FeeView } from '$lib/core/generated/FeeView';

const seams = vi.hoisted(() => ({
	invalidate: vi.fn(),
	deployed: vi.fn(async (): Promise<boolean> => true),
	dispatch: vi.fn(),
	start: vi.fn(),
	onView: null as ((view: unknown) => void) | null
}));

vi.mock('$lib/services/safe-transaction', () => ({
	invalidateFeeSignals: seams.invalidate,
	accountIsDeployed: seams.deployed
}));
vi.mock('$lib/core/client', () => ({ loadCore: vi.fn(async () => {}) }));
vi.mock('./send-estimates', () => ({ resolveFee: () => null }));
vi.mock('./fee-session', () => ({
	createFeeSession: (options: { onView: (view: unknown) => void }) => {
		seams.onView = options.onView;
		return { start: seams.start, dispatch: seams.dispatch, dispose: vi.fn() };
	}
}));

import { FeeQuote, IDLE_FEE_VIEW } from './fee-quote.svelte';

const BSC = 56;
const REQUEST = {
	chainId: BSC,
	account: '0x' + '88'.repeat(20),
	calls: [],
	feeToken: null,
	publicKeyHex: undefined
};
const view = (patch: Partial<FeeView>): FeeView => ({ ...IDLE_FEE_VIEW, ...patch });
const FAILED = { failed: { kind: 'gas_quote_too_high' } } as unknown as Partial<FeeView>;
const SETTLED = { fee: { total_wei: '332400000000000' } } as unknown as Partial<FeeView>;

/** A quote that has reached the core and is waiting on its run. */
async function asked(): Promise<{ quote: FeeQuote; outcome: Promise<unknown> }> {
	const quote = new FeeQuote();
	const outcome = quote.requestQuote(REQUEST);
	await vi.waitFor(() => expect(seams.start).toHaveBeenCalledTimes(1));
	return { quote, outcome };
}

beforeEach(() => {
	seams.invalidate.mockClear();
	seams.dispatch.mockClear();
	seams.start.mockReset();
	// As the real core does: the dispatch itself publishes the run as busy, so
	// the request's promise waits on the run instead of settling on a blank view.
	seams.start.mockImplementation(() => seams.onView?.({ ...IDLE_FEE_VIEW, busy: true }));
	seams.deployed.mockReset();
	seams.deployed.mockResolvedValue(true);
	seams.onView = null;
});

describe('FeeQuote.requote — the refresh measures again (issue 212)', () => {
	it('drops the chain’s fee signals BEFORE dispatching the requote that reads them', async () => {
		const { quote } = await asked();
		expect(seams.invalidate).not.toHaveBeenCalled();
		quote.requote();
		expect(seams.invalidate).toHaveBeenCalledExactlyOnceWith(BSC);
		expect(seams.dispatch).toHaveBeenCalledExactlyOnceWith({ type: 'requote' });
		expect(seams.invalidate.mock.invocationCallOrder[0]).toBeLessThan(
			seams.dispatch.mock.invocationCallOrder[0]
		);
	});

	it('when the last ask never reached the core, it still drops them before asking again', async () => {
		seams.deployed.mockRejectedValueOnce(new Error('eth_getCode indeterminate'));
		const quote = new FeeQuote();
		expect(await quote.requestQuote(REQUEST)).toEqual({ kind: 'context_unavailable' });
		quote.requote();
		expect(seams.invalidate).toHaveBeenCalledExactlyOnceWith(BSC);
		await vi.waitFor(() => expect(seams.start).toHaveBeenCalledTimes(1));
		expect(seams.invalidate.mock.invocationCallOrder[0]).toBeLessThan(
			seams.start.mock.invocationCallOrder[0]
		);
	});
});

describe('FeeQuote — a failed quote never leaves its inputs held (issue 212)', () => {
	it('a quote that settles as failed drops the fee signals, so the retry re-measures', async () => {
		const { outcome } = await asked();
		expect(seams.invalidate).not.toHaveBeenCalled();
		seams.onView?.(view(FAILED));
		expect(seams.invalidate).toHaveBeenCalledExactlyOnceWith(BSC);
		expect(await outcome).toMatchObject({ kind: 'failed' });
	});

	it('a quote that settles well keeps them — that is the calm the cache is for', async () => {
		const { outcome } = await asked();
		seams.onView?.(view(SETTLED));
		expect(await outcome).toMatchObject({ kind: 'ok' });
		expect(seams.invalidate).not.toHaveBeenCalled();
	});

	it('a run still in flight is left alone even while the last failure is on the view', async () => {
		await asked();
		seams.onView?.(view({ ...FAILED, busy: true }));
		expect(seams.invalidate).not.toHaveBeenCalled();
	});
});

// Spec 068 — the tier is a quote PARAMETER, and the session never invents one.
//
// The class used to carry `const TIER = 'fast'` and put it on every
// `quote_requested`, which is why no shell could offer a speed at all. What
// has to hold now is the pair of rules that keep the screen and the chain in
// step: a request that names a tier is priced at THAT tier, and a request that
// names none is priced exactly as it was before this feature existed.
describe('FeeQuote — the tier is the caller’s (spec 068)', () => {
	const tierOf = (mock: typeof seams.start | typeof seams.dispatch) =>
		(mock.mock.calls[0]?.[0] as { tier?: string } | undefined)?.tier;

	it('prices at the tier the request names', async () => {
		const quote = new FeeQuote();
		void quote.requestQuote({ ...REQUEST, tier: 'slow' });
		await vi.waitFor(() => expect(seams.start).toHaveBeenCalledTimes(1));
		expect(tierOf(seams.start)).toBe('slow');
	});

	it('prices at `fast` when the request names none — the pre-068 behaviour, unchanged', async () => {
		const quote = new FeeQuote();
		void quote.requestQuote(REQUEST);
		await vi.waitFor(() => expect(seams.start).toHaveBeenCalledTimes(1));
		expect(tierOf(seams.start)).toBe('fast');
	});

	it('carries the new tier on a re-ask, so a speed change re-prices the same operation', async () => {
		const quote = new FeeQuote();
		void quote.requestQuote({ ...REQUEST, tier: 'fast' });
		await vi.waitFor(() => expect(seams.start).toHaveBeenCalledTimes(1));
		void quote.requestQuote({ ...REQUEST, tier: 'standard' });
		await vi.waitFor(() => expect(seams.dispatch).toHaveBeenCalledTimes(1));
		expect(tierOf(seams.dispatch)).toBe('standard');
	});
});

// The other rows of the speed picker are priced by their own sessions, which
// skip re-pricing while the OPERATION is unchanged — otherwise a keystroke
// would cost two extra relay round trips. That made them freeze: the relay was
// re-deployed with per-tier prices, the row in force re-priced, and the other
// two went on showing the figure they were first told. Three rows, one of them
// live, and no way to tell which. `generation` is the signal that says "the
// answer moved even though the question did not".
describe('FeeQuote.generation — the other tiers re-price when this one does', () => {
	it('moves for a new ask and for a refresh, and only ever forward', async () => {
		const quote = new FeeQuote();
		const first = quote.generation;

		void quote.requestQuote(REQUEST);
		await vi.waitFor(() => expect(seams.start).toHaveBeenCalledTimes(1));
		const asked = quote.generation;
		expect(asked).toBeGreaterThan(first);

		// The same operation, asked again: the calls are identical, so nothing
		// in the request can tell the previews to look again — this can.
		void quote.requestQuote(REQUEST);
		await vi.waitFor(() => expect(seams.dispatch).toHaveBeenCalledTimes(1));
		const reasked = quote.generation;
		expect(reasked).toBeGreaterThan(asked);

		// And the refresh, which is the person saying "look again" — it must not
		// refresh one row out of three.
		quote.requote();
		expect(quote.generation).toBeGreaterThan(reasked);
	});

	it('remembers the operation it was asked to price, so the picker can re-price it elsewhere', async () => {
		const quote = new FeeQuote();
		expect(quote.lastRequest).toBeNull();
		void quote.requestQuote({ ...REQUEST, tier: 'slow' });
		await vi.waitFor(() => expect(seams.start).toHaveBeenCalledTimes(1));
		// The SAME calls, account and fee coin — a preview at another tier must
		// price the same transaction, not a shell-built approximation of it.
		expect(quote.lastRequest).toEqual({ ...REQUEST, tier: 'slow' });
	});
});
