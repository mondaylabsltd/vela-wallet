/**
 * Issue 681, in a browser: the number you tap is the number the fee row then shows.
 *
 * The owner tapped 较慢 at 0.605996 POL and the fee that came up was not the
 * figure they had tapped — "我选择的价格和展示的价格不一致了". A node test of
 * the promotion proves the session changes hands (see
 * `core/tier-preview.test.ts`); it cannot see the thing that was actually
 * reported, which is two rendered figures disagreeing. So this test wires the
 * real `FeeQuote` and `TierPreview` the way the wallet route wires them, draws
 * the real fee row and the real speed control, clicks a real option, and reads
 * the DOM.
 *
 * What it deliberately does NOT do is resolve the message corpus: that runs
 * through `engine.server`, which must never be imported from client code
 * (CLAUDE.md rule 2). The two model rules that matter here — a figure when
 * there is one, "…" while a measurement is out — are applied by the same tiny
 * formatter for BOTH surfaces, which is what makes "the same quote" testable
 * as "the same string". The model's own wording is pinned in
 * `live-send.test.ts`.
 */
import { flushSync, tick, untrack } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { FeeView } from '$lib/core/generated/FeeView';
import FeeRow from './FeeRow.svelte';
import FeeSpeedRow from './FeeSpeedRow.svelte';

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
	invalidateFeeSignals: seams.invalidate,
	accountIsDeployed: seams.deployed
}));
vi.mock('$lib/core/client', () => ({ loadCore: vi.fn(async () => {}) }));
vi.mock('$lib/flows/core/fee-session', () => ({ createFeeSession: seams.make }));
vi.mock('$lib/flows/core/send-estimates', () => ({
	resolveFee: (fee: { total_wei: string } | null) =>
		fee === null ? null : { totalWei: fee.total_wei }
}));

import { FeeQuote } from '$lib/flows/core/fee-quote.svelte';
import { TierPreview } from '$lib/flows/core/tier-preview.svelte';

interface FakeSession {
	onView: (view: FeeView) => void;
	start: ReturnType<typeof vi.fn>;
	dispatch: ReturnType<typeof vi.fn>;
	dispose: ReturnType<typeof vi.fn>;
}
const sessions = () => seams.sessions as FakeSession[];
/**
 * The session pricing a given tier, found by the event it was started with
 * rather than by creation order — three requests race through two awaits each,
 * and a test that assumed an order would be pinning the race, not the fix.
 */
const sessionFor = (tier: string) =>
	sessions().find(
		(session) => (session.start.mock.calls[0]?.[0] as { tier?: string } | undefined)?.tier === tier
	)!;

const POLYGON = 137;
const TIERS: FeeTier[] = ['fast', 'standard', 'slow'];
const NAME: Record<string, string> = { fast: '超快', standard: '标准', slow: '较慢' };
/** The owner's own receipt, to the digit. */
const POL: Record<string, string> = {
	fast: '1.014643 POL',
	standard: '0.754482 POL',
	slow: '0.605996 POL'
};
const WEI: Record<string, string> = {
	fast: '1014643000000000000',
	standard: '754482000000000000',
	slow: '605996000000000000'
};
const REQUEST = {
	chainId: POLYGON,
	account: '0x' + '77'.repeat(20),
	calls: [],
	feeToken: null,
	publicKeyHex: undefined
};
const settled = (tier: FeeTier): FeeView =>
	({
		busy: false,
		failed: null,
		stale: false,
		fee_token: null,
		options: [],
		confirm_fee_ready: false,
		fee: { total_wei: WEI[tier], chain_id: POLYGON, tier }
	}) as unknown as FeeView;

/**
 * `feeRow`/`feeSpeed`'s rule, in one line and used by both surfaces: the
 * figure when there is one, "…" while a measurement is out, "—" when the
 * answer came back empty.
 */
const money = (view: FeeView, busy: boolean) =>
	view.fee ? POL[(view.fee as { tier: string }).tier] : busy || view.busy ? '…' : '—';

beforeEach(() => {
	seams.sessions.length = 0;
	seams.invalidate.mockClear();
	seams.deployed.mockReset();
	seams.deployed.mockResolvedValue(true);
});

/**
 * The wallet route's speed wiring, with its own two effects: the previews
 * follow the operation and the generation, and the tier in force is re-priced
 * only when nothing is priced for it.
 */
function wire() {
	const feeQuote = new FeeQuote();
	const tierPreview = new TierPreview();
	let sendTier = $state<FeeTier>('fast');
	let speedOpen = $state(true);

	const feeInForce = $derived<FeeView>({
		...feeQuote.view,
		busy: feeQuote.view.busy || feeQuote.pending
	});
	const speedRows = $derived(
		TIERS.map((tier) => {
			if (tier === sendTier) return { tier, view: feeInForce, busy: feeQuote.pending };
			const row = tierPreview.rows.find((candidate) => candidate.tier === tier);
			return {
				tier,
				view: row?.quote.view ?? ({ ...settled('fast'), fee: null } as FeeView),
				busy: row?.quote.pending ?? true
			};
		})
	);

	const fee = $state({
		label: 'Network fee',
		mark: { ticker: 'POL', badgeColor: 'var(--color-fg-muted)' },
		value: '…',
		openLabel: 'Fee token',
		refreshLabel: 'Refresh fee',
		refreshing: false
	});
	const speed = $state({
		label: 'Speed',
		value: NAME.fast,
		open: true,
		onceNote: '仅这一笔，下次仍用默认',
		gasPriceLabel: 'Gas 价格',
		gasPriceLine: false,
		options: [] as { id: string; label: string; value: string; selected: boolean }[]
	});

	const stop = $effect.root(() => {
		$effect(() => {
			const base = feeQuote.lastRequest;
			if (!speedOpen || base === null) {
				tierPreview.hide();
				return;
			}
			tierPreview.show(
				base,
				TIERS.filter((tier) => tier !== sendTier),
				feeQuote.generation
			);
		});
		$effect(() => {
			const tier = sendTier;
			if (feeQuote.pending) return;
			untrack(() => {
				const base = feeQuote.lastRequest;
				if (base === null || base.tier === tier) return;
				void feeQuote.requestQuote({ ...base, tier });
			});
		});
		$effect(() => {
			fee.value = money(feeInForce, false);
			fee.refreshing = feeInForce.busy;
		});
		$effect(() => {
			speed.value = NAME[sendTier];
			speed.open = speedOpen;
			speed.options = speedRows.map((row) => ({
				id: row.tier,
				label: NAME[row.tier],
				value: money(row.view, row.busy),
				selected: row.tier === sendTier
			}));
		});
	});

	/** The route's `pickSpeed`, verbatim in shape. */
	const pick = (id: string) => {
		const tier = id as FeeTier;
		speedOpen = false;
		if (tier === sendTier) return;
		tierPreview.promote(feeQuote, tier);
		sendTier = tier;
	};

	return { feeQuote, tierPreview, fee, speed, pick, stop, tierNow: () => sendTier };
}

/** A picker with all three tiers settled and drawn. */
async function drawn() {
	const w = wire();
	void w.feeQuote.requestQuote({ ...REQUEST, tier: 'fast' });
	// One session in force plus one preview per other tier: the picker's own
	// effect opens those the moment the operation is known.
	await vi.waitFor(() => expect(sessions()).toHaveLength(3));
	for (const tier of TIERS) sessionFor(tier).onView(settled(tier));
	flushSync();

	const feeScreen = render(FeeRow, { props: { fee: w.fee, onrefresh: () => {} } });
	const speedScreen = render(FeeSpeedRow, { props: { speed: w.speed, onselect: w.pick } });
	await tick();
	return {
		...w,
		feeValue: () => feeScreen.container.querySelector('.value')?.textContent ?? '',
		refreshBusy: () =>
			feeScreen.container.querySelector('button.refresh')?.getAttribute('aria-busy') ?? '',
		feeContainer: feeScreen.container,
		option: (tier: string) =>
			[...speedScreen.container.querySelectorAll<HTMLButtonElement>('button.option')].find(
				(button) => button.querySelector('.name')?.textContent === NAME[tier]
			)!
	};
}

describe('picking a speed (issue 681)', () => {
	it('draws each tier’s own figure before anybody taps', async () => {
		const w = await drawn();
		expect(w.feeValue()).toBe(POL.fast);
		expect(w.option('slow').querySelector('.value')?.textContent).toBe(POL.slow);
		expect(w.option('standard').querySelector('.value')?.textContent).toBe(POL.standard);
		w.stop();
	});

	it('shows the figure that was tapped — and never a busy state on the way', async () => {
		const w = await drawn();
		const tapped = w.option('slow').querySelector('.value')?.textContent;
		expect(tapped).toBe(POL.slow);

		// Everything the fee row says from the tap onward, in order.
		const seen: string[] = [];
		const busy: string[] = [];
		const watch = new MutationObserver(() => {
			seen.push(w.feeValue());
			busy.push(w.refreshBusy());
		});
		watch.observe(w.feeContainer, {
			subtree: true,
			childList: true,
			characterData: true,
			attributes: true
		});

		w.option('slow').click();
		flushSync();
		await tick();
		await tick();
		watch.disconnect();

		// The defect, in one line: the price chosen and the price displayed.
		expect(w.feeValue()).toBe(tapped);
		// And nothing flickered in between — no "…", no spinning ⟳. A round
		// trip that bought nothing is still a round trip somebody watches.
		//
		// The log has to be non-empty first: the observer reads the live DOM per
		// batch rather than replaying records, so "contains no '…'" over an empty
		// log would be a pass nobody earned. (It also cannot see a transient that
		// opens AND closes inside one flush; what it does see is a busy state
		// that outlives the tap, which is what a round trip leaves behind.)
		expect(seen.length).toBeGreaterThan(0);
		expect(seen).not.toContain('…');
		expect(busy).not.toContain('true');
		// The quote the submit path would sign is that same one.
		expect(w.feeQuote.estimate).toEqual({ totalWei: WEI.slow });
		expect((w.feeQuote.view.fee as { tier: string } | null)?.tier).toBe('slow');
		w.stop();
	});

	it('asks the relay nothing at all for a pick', async () => {
		const w = await drawn();
		const before = sessions().reduce(
			(n, s) => n + s.start.mock.calls.length + s.dispatch.mock.calls.length,
			0
		);
		w.option('standard').click();
		flushSync();
		await tick();
		await tick();
		expect(w.feeValue()).toBe(POL.standard);
		expect(
			sessions().reduce((n, s) => n + s.start.mock.calls.length + s.dispatch.mock.calls.length, 0)
		).toBe(before);
		w.stop();
	});

	it('re-quotes, and says so, when the tapped row had nothing settled', async () => {
		// Only the tier in force has landed; the two previews are still measuring.
		const w = wire();
		void w.feeQuote.requestQuote({ ...REQUEST, tier: 'fast' });
		await vi.waitFor(() => expect(sessions()).toHaveLength(3));
		sessionFor('fast').onView(settled('fast'));
		flushSync();

		const feeScreen = render(FeeRow, { props: { fee: w.fee, onrefresh: () => {} } });
		const speedScreen = render(FeeSpeedRow, { props: { speed: w.speed, onselect: w.pick } });
		await tick();
		const slow = [
			...speedScreen.container.querySelectorAll<HTMLButtonElement>('button.option')
		].find((button) => button.querySelector('.name')?.textContent === NAME.slow)!;
		// Nothing to tap a price on — so there is no price this tap could
		// promise, and a real measurement is the honest answer.
		expect(slow.querySelector('.value')?.textContent).toBe('…');

		slow.click();
		flushSync();
		await tick();
		// It says so: the ⟳ turns from the moment the measurement is decided
		// on, not from the dispatch an `eth_getCode` later.
		await vi.waitFor(() =>
			expect(feeScreen.container.querySelector('button.refresh')?.getAttribute('aria-busy')).toBe(
				'true'
			)
		);
		w.stop();
	});
});
