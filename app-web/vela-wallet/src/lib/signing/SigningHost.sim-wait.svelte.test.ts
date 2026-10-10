/* eslint-disable @typescript-eslint/no-explicit-any -- stand-ins for the machines beside the sign core */
/**
 * PR 3 — the confirm waits for the simulation's verdict, in a browser.
 *
 * The hole: the confirm gate looked at the request, the reading, the approval
 * guard and the fee, and not at the simulation — a person could confirm
 * before the balance-changes verdict was on screen, and that verdict is the
 * one part of the sheet the site being signed for cannot write. The core now
 * owns a wait (`SignView.sim_checking`, four seconds at most, on a timer the
 * shell only runs); the web's part is three wires, and they are what is real
 * here:
 *
 * - `SigningHost` tells the core the simulation is out in the step that
 *   sends it, and that its verdict is on the sheet in the step that puts it
 *   there — however the read ends;
 * - the sign executor runs the core's `sim_verdict_timer` and nothing else;
 * - the sheet draws the gate's line under the shut confirm, and — once the
 *   deadline has passed — the core's caution in the verdict's place.
 *
 * The `sign_request` core is the REAL one, through the web's own session and
 * executor, and so are the gate (`signConfirmState`), the builder
 * (`buildSigningModel`) and the drawn sheet. The simulation is stubbed (the
 * node's reply is the test's to give) and the executor's clock is stopped
 * (its timer fires when the test fires it). The reading, the guard and the
 * fee are stood in for as READY, so the simulation is all that can hold the
 * confirm. The words are a stand-in too: that each key resolves to its
 * sentence in every language is `messages.test.ts`'s and `live.test.ts`'s.
 */
import { flushSync, tick } from 'svelte';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { SignEvent } from '$lib/core/generated/SignEvent';
import type { SignView } from '$lib/core/generated/SignView';

const ME = '0x1111111111111111111111111111111111111111';

vi.mock('$lib/signing/core/sign-resident.svelte', () => ({
	signRequest: {
		get view() {
			return core.view;
		},
		get answered() {
			return null;
		},
		get progress() {
			return { requestId: null, signed: false, ceremonyUp: false };
		},
		dispatch: (event: SignEvent) => core.dispatch(event)
	}
}));
// The reading and the guard, READY: neither holds the confirm.
vi.mock('$lib/signing/core/sheet.svelte', () => ({
	signingSheet: {
		present: async () => {},
		dismiss: () => {},
		get clear() {
			return READY_CLEAR;
		},
		get guard() {
			return READY_GUARD;
		},
		dispatchGuard: () => {}
	}
}));
vi.mock('$lib/session/core/session.svelte', () => ({
	session: {
		view: {
			loading: false,
			has_wallet: true,
			address: '0x1111111111111111111111111111111111111111',
			active_index: 0,
			accounts: [
				{
					account: {
						name: 'One',
						address: '0x1111111111111111111111111111111111111111',
						keys: [],
						public_key_hex: ''
					}
				}
			]
		}
	}
}));
vi.mock('$lib/settings/core/currency.svelte', () => ({
	currency: {
		view: { code: 'USD', rate: 1, committed: true, pending: null },
		boot: async () => {}
	}
}));
vi.mock('$lib/wallet/identicon', () => ({ identiconSvgForClient: () => '<svg></svg>' }));
// No picture is fetched from a test: the network's mark is its drawn one
// (and the page, served over http below, has no icon to ask for).
vi.mock('$lib/flows/marks', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/flows/marks')>()),
	chainLogoURL: () => undefined
}));
vi.mock('$lib/wallet/core/tracker-resident', () => ({
	subscribeTxTracker: () => () => {},
	txTrackerView: () => ({ entries: [] })
}));
// The stubbed simulation: the node's reply to the sheet's one `eth_simulateV1`.
// `checkRequest` and the core's reading of the reply (`simOutcome`) are real.
vi.mock('$lib/services/sim/sim-engine-rpc', () => ({
	rpcSimulateReply: (from: string, calls: unknown[], chainId: number) =>
		node.ask({ from, calls, chainId }),
	// Send's own simulation (`tx-simulation`), which no sheet draws: not run here.
	rpcSimulate: async () => null
}));
// The fee, READY at the speed in force: it does not hold the confirm either.
vi.mock('$lib/flows/core/speed-control.svelte', () => ({
	SpeedControl: class {
		attach() {}
		boot() {
			return Promise.resolve();
		}
		dispose() {}
		reset() {}
		refresh() {}
		toggle() {}
		pick() {}
		balanceChanges() {}
		get tier() {
			return 'fast';
		}
		get view() {
			return READY_SPEED;
		}
		get feeInForce() {
			return READY_FEE;
		}
		feeOptions() {
			return [];
		}
	}
}));

import { loadCore } from '$lib/core/client';
import { createSignRequestSession, type SignRequestSession } from '$lib/signing/core/sign-session';
import type { SignShellPorts } from '$lib/signing/core/sign-types';
import SigningHost from './SigningHost.svelte';

const READY_CLEAR = {
	resolving: false,
	resolved: true,
	message: null,
	surface: 'clear_sign',
	confirm: { type: 'confirm' },
	blind_typed: null,
	danger_haptic: false,
	plain_send: null,
	batch: null,
	record_intent: null,
	native_value: null,
	record_reading: null,
	result: {
		intent: 'Approve USDC',
		intent_term: null,
		contract_name: 'USD Coin',
		owner: null,
		fields: [],
		risk: 'normal',
		contract_address: '0x' + '22'.repeat(20),
		verified: true,
		provenance: 'built_in',
		sign_type: 'transaction',
		partial: false,
		best_effort: false,
		to_own_token: false,
		terms_off_chain: false
	}
};
const READY_GUARD = {
	surface: 'none',
	detected: null,
	meta: { symbol: '…', decimals: 18, verified: false, loading: false },
	editor: null,
	confirm_allowed: true,
	rewritten_params_json: null,
	unlimited_consented: false,
	unlimited_warning: false,
	increase_total: null,
	decimals_unverified: false,
	expired: false,
	batch: null
};
const READY_FEE = {
	busy: false,
	failed: null,
	fee: {
		chain_id: 100,
		total_wei: '2100000000000000',
		max_fee_per_gas: '1',
		network_fee_per_gas: '1',
		relayer_fee_per_gas: '0',
		bundler_gas_price: '1',
		in_band_gas_basis: '1',
		effective_gas_price: null,
		max_gas_price: null,
		total_gas: '1',
		deployed: true,
		tier: 'fast',
		quoted: true,
		fee_asset: { type: 'native' },
		fee_recipient: null
	},
	stale: false,
	fee_token: null,
	options: [],
	confirm_fee_ready: true,
	no_coin_pays: false,
	nothing_to_pay_from: false,
	provisional: false,
	failure: null
};
const READY_SPEED = {
	tier: 'fast',
	preferred: 'fast',
	previews: [],
	open: false,
	picked: false,
	free: false,
	free_note: false,
	single: false,
	gas_price_line: false,
	options: []
};

/** The two sentences this suite is about, by the keys the core names. */
const CHECKING_KEY = 'componentsUi.signing.confirmBlock.simChecking';
const CHECKING = 'Checking what this transaction does…';
const COULD_NOT_CHECK =
	'Vela couldn’t check what this transaction does. Review it before you sign.';
const NO_CHANGE = 'No asset changes';
/**
 * The sheet's words, stood in for: the lines under test are looked up by the
 * core's key exactly as the real map is; every other word reads as its name.
 */
const WORDS = new Proxy(
	{
		confirmBlock: {
			[CHECKING_KEY]: CHECKING,
			'componentsUi.signing.confirmBlock.reading': 'Reading the request…',
			'componentsUi.signing.confirmBlock.feeMeasuring': 'Working out the network fee…',
			'componentsUi.signing.confirmBlock.accountSwitching': 'Switching account…'
		},
		simSaid: { 'componentsUi.signing.simResultNoChange': NO_CHANGE },
		warnSimUnavailable: COULD_NOT_CHECK,
		balancesTitle: 'Balance changes',
		terms: {},
		feeReasons: {},
		signerReasons: {},
		speed: {
			label: 'Speed',
			names: { fast: 'Fast', standard: 'Standard', slow: 'Slow' },
			once: '',
			free: '',
			single: '',
			gasPriceLabel: ''
		}
	} as Record<string, unknown>,
	{
		get: (known, name) =>
			typeof name === 'string' && !(name in known) ? name : known[name as string]
	}
) as any;

const NOTHING_MOVES = JSON.stringify({ result: [{ calls: [{ status: '0x1', logs: [] }] }] });
const REVERTS = JSON.stringify({ result: [{ calls: [{ status: '0x0', logs: [] }] }] });
const NOT_OFFERED = JSON.stringify({ error: { code: -32601, message: 'method not found' } });

/** The node: every simulation the host sent, each answered when the test says. */
class Node {
	asked: { from: string; calls: unknown[]; chainId: number; answer: (reply: string) => void }[] =
		[];
	/** A node that answers by itself, at once — or `null`: the test answers. */
	auto: string | null = null;
	ask(read: { from: string; calls: unknown[]; chainId: number }): Promise<string> {
		if (this.auto !== null) {
			this.asked.push({ ...read, answer: () => {} });
			return Promise.resolve(this.auto);
		}
		return new Promise((answer) => this.asked.push({ ...read, answer }));
	}
}
let node = new Node();

const SIM = (event: SignEvent) => event.type === 'sim_started' || event.type === 'sim_settled';

/**
 * The resident `sign_request` machine, stood in for by the real core behind
 * the web's own session and executor — with the executor's clock stopped.
 * It hands events on as the resident does: one promise turn later, in order.
 */
class RealSign {
	view = $state<SignView>({ surface: 'hidden', request: null } as SignView);
	/** Every event the host sent the core. */
	sent: SignEvent[] = [];
	/** Every timer the core started, stopped: `fire` is its `ms` passing. */
	timers: { ms: number; fire: () => void }[] = [];
	faults: unknown[] = [];
	#loop: SignRequestSession | null = null;

	boot(): void {
		const ports: SignShellPorts = {
			transportFor: () => ({ sendResponse: () => {} }),
			opSubmitted: () => {},
			opSigned: () => {},
			ceremony: () => {},
			askerLive: async () => true,
			approvedAtMs: () => null,
			requestOrigin: () => 'http://127.0.0.1:8137',
			assetSim: () => null,
			switchActiveAccount: async () => {},
			recordsWritten: () => {}
		};
		this.#loop = createSignRequestSession({
			ports,
			onView: (view) => (this.view = view),
			onError: (error) => this.faults.push(error),
			timer: (ms) => new Promise<void>((fire) => this.timers.push({ ms, fire }))
		});
		this.#loop.start({ type: 'networks_changed', chain_ids: [1, 100] });
		this.#loop.dispatch({
			type: 'accounts_changed',
			accounts: [{ address: ME, credential_id: 'cred-1' }],
			active_index: 0
		});
	}

	dispatch(event: SignEvent): void {
		this.sent.push(event);
		// An approval's pipeline — the funding check, the passkey, the relay —
		// is not this suite's, and must not be run from a test: a press is
		// recorded here and goes no further.
		if (event.type === 'approve_tapped') return;
		void Promise.resolve().then(() => this.#loop?.dispatch(event));
	}

	/** A request arrives from a page (or, `own`, from the wallet itself). */
	arrive(id: string, kind: 'transaction' | 'message', own = false): void {
		this.#loop?.dispatch({
			type: 'request_arrived',
			id,
			method: kind === 'transaction' ? 'eth_sendTransaction' : 'personal_sign',
			params_json:
				kind === 'transaction'
					? JSON.stringify([
							{ from: ME, to: '0x' + '22'.repeat(20), value: '0x0', data: '0x095ea7b3' }
						])
					: JSON.stringify(['0x68656c6c6f', ME]),
			origin: 'http://127.0.0.1:8137',
			transport_id: 't1',
			dedicated_transport: true,
			per_request_chain: 100,
			dapp: null,
			granted_address: ME,
			requested_address: null,
			request_ts_ms: null,
			now_ms: Date.now(),
			first_party: own
		});
	}

	/** The simulation events the host has sent, in order. */
	get sim(): SignEvent[] {
		return this.sent.filter(SIM);
	}

	dispose(): void {
		this.#loop?.dispose();
		this.#loop = null;
	}
}
let core = new RealSign();

const FEE = {
	view: READY_FEE,
	requote: () => {},
	requestQuote: async () => {},
	selectAsset: () => {},
	dispose: () => {},
	lastRequest: null
};

const dialog = () => document.body.querySelector<HTMLElement>('[role="dialog"]');
const confirmEl = () =>
	document.body.querySelector<HTMLButtonElement>('[role="dialog"] [data-testid="signing-confirm"]');
/** The line under the confirm as a person sees it: its words, or `null` when none is shown. */
function heldLine(): string | null {
	const line = document.body.querySelector<HTMLElement>('[role="dialog"] .confirm-note');
	if (!line || getComputedStyle(line).visibility === 'hidden') return null;
	return line.textContent?.trim() ?? '';
}
const card = () => document.body.querySelector<HTMLElement>('[role="dialog"] section.balances');
const cardNote = () => card()?.querySelector<HTMLElement>('.note') ?? null;
const top = (el: Element) => Math.round(el.getBoundingClientRect().top * 10) / 10;
const confirmTop = () => top(confirmEl()!);

/**
 * Every frame the browser painted with a confirm on it: whether it could be
 * pressed, the line under it, and what the verdict's place said. A state
 * that only ever existed between two frames was never seen by anybody.
 */
function watchFrames() {
	const frames: { disabled: boolean; line: string | null; said: string | null }[] = [];
	let on = true;
	const sample = () => {
		if (!on) return;
		const button = confirmEl();
		if (button) {
			frames.push({
				disabled: button.disabled,
				line: heldLine(),
				said: cardNote()?.textContent?.trim() ?? null
			});
		}
		requestAnimationFrame(sample);
	};
	requestAnimationFrame(sample);
	return {
		frames,
		/** `more` further frames have been painted. */
		further: async (more = 6) => {
			const from = frames.length;
			await vi.waitFor(() => expect(frames.length).toBeGreaterThanOrEqual(from + more), {
				timeout: 10_000,
				interval: 20
			});
		},
		stop: () => (on = false)
	};
}

const WAIT = { timeout: 15_000, interval: 20 };
/** The host has sent the node `count` simulations. */
const asked = (count = 1) => vi.waitFor(() => expect(node.asked).toHaveLength(count), WAIT);
/** The sheet is up and its confirm is held, with the simulation's line under it. */
const held = () =>
	vi.waitFor(() => {
		expect(confirmEl()?.disabled).toBe(true);
		expect(heldLine()).toBe(CHECKING);
	}, WAIT);
/** The confirm can be pressed, and no line stands under it. */
const open = () =>
	vi.waitFor(() => {
		expect(confirmEl()?.disabled).toBe(false);
		expect(heldLine()).toBeNull();
	}, WAIT);

function mount() {
	return render(SigningHost, { props: { messages: WORDS, fee: FEE as any } });
}

beforeAll(() => loadCore());
beforeEach(() => {
	node = new Node();
	core = new RealSign();
	core.boot();
});
afterEach(() => {
	expect(core.faults).toEqual([]);
	core.dispose();
});
// The harness's own window, back as the other suites expect it.
afterAll(() => page.viewport(414, 896));

describe('a transaction’s confirm waits for the simulation’s verdict (PR 3)', () => {
	it('held with the core’s one line while the node has not answered; open, and the line gone, once the verdict is drawn', async () => {
		const frames = watchFrames();
		const screen = mount();
		core.arrive('tx:1', 'transaction');
		await asked();
		await held();
		// The core was told in the step that sent the simulation — once, for
		// this request — and started the wait's one timer: four seconds, its
		// own number, on the executor's (stopped) clock.
		expect(core.sim).toEqual([{ type: 'sim_started', id: 'tx:1' }]);
		expect(core.view.sim_checking).toBe(true);
		expect(core.timers.map((timer) => timer.ms)).toEqual([4_000]);
		expect(node.asked[0]).toMatchObject({ from: ME, chainId: 100 });
		// Nothing is in the verdict's place yet.
		expect(card()).toBeNull();
		// A press on the held confirm reaches nobody.
		confirmEl()!.click();
		await frames.further();
		expect(core.sent.some((event) => event.type === 'approve_tapped')).toBe(false);
		// From its first frame on, no frame showed a confirm that could be pressed.
		expect(frames.frames.length).toBeGreaterThan(0);
		expect(frames.frames.filter((frame) => !frame.disabled)).toEqual([]);

		// The node answers: nothing of theirs moves.
		node.asked[0].answer(NOTHING_MOVES);
		await vi.waitFor(() => expect(cardNote()?.textContent).toBe(NO_CHANGE), WAIT);
		await open();
		expect(core.sim).toEqual([
			{ type: 'sim_started', id: 'tx:1' },
			{ type: 'sim_settled', id: 'tx:1' }
		]);
		expect(core.view.sim_checking).toBe(false);
		expect(core.view.sim_waited_out_key).toBeNull();
		// The confirm opened WITH its verdict: no frame had one without the other.
		expect(frames.frames.filter((frame) => !frame.disabled && frame.said !== NO_CHANGE)).toEqual(
			[]
		);
		// The deadline, passing now, is nobody's: nothing is said, nothing shut.
		core.timers[0].fire();
		await frames.further();
		expect(confirmEl()!.disabled).toBe(false);
		expect(cardNote()?.textContent).toBe(NO_CHANGE);
		// And now a press is the approval.
		confirmEl()!.click();
		await vi.waitFor(
			() => expect(core.sent.some((event) => event.type === 'approve_tapped')).toBe(true),
			WAIT
		);
		frames.stop();
		await screen.unmount();
	});

	it('the simulation never answers: at the deadline the confirm opens, and the verdict’s place says as a caution that nothing was checked; a late answer replaces it', async () => {
		const frames = watchFrames();
		const screen = mount();
		core.arrive('tx:1', 'transaction');
		await asked();
		await held();
		expect(core.timers).toHaveLength(1);

		// Four seconds pass (the test's hand on the clock). No node has answered.
		core.timers[0].fire();
		await vi.waitFor(() => expect(cardNote()?.textContent).toBe(COULD_NOT_CHECK), WAIT);
		await open();
		expect(core.view.sim_waited_out_key).toBe('componentsUi.signing.simUnavailableWarning');
		// A caution: the card's note in the warning ink, on the sheet's own
		// card in the verdict's place — not a line folded into the details.
		const note = cardNote()!;
		expect(note.dataset.tone).toBe('caution');
		const probe = document.createElement('span');
		probe.style.color = 'var(--color-warning-base)';
		dialog()!.appendChild(probe);
		expect(getComputedStyle(note).color).toBe(getComputedStyle(probe).color);
		probe.style.color = 'var(--color-fg-subtle)';
		expect(getComputedStyle(note).color).not.toBe(getComputedStyle(probe).color);
		probe.remove();
		expect(card()!.hasAttribute('data-verdict')).toBe(true);
		const details = dialog()!.querySelector('section.tech')!;
		expect(details.querySelector('.toggle')?.getAttribute('aria-expanded')).toBe('false');
		expect(details.contains(card()!)).toBe(false);
		expect(details.textContent).not.toContain(COULD_NOT_CHECK);
		// The host said nothing: no verdict is on the sheet, and it knows it.
		expect(core.sim).toEqual([{ type: 'sim_started', id: 'tx:1' }]);
		// The confirm never opened onto an empty place.
		expect(
			frames.frames.filter((frame) => !frame.disabled && frame.said !== COULD_NOT_CHECK)
		).toEqual([]);

		// The node answers after all: its verdict takes the caution's place.
		node.asked[0].answer(NOTHING_MOVES);
		await vi.waitFor(() => expect(cardNote()?.textContent).toBe(NO_CHANGE), WAIT);
		expect(cardNote()!.dataset.tone).not.toBe('caution');
		expect(dialog()!.textContent).not.toContain(COULD_NOT_CHECK);
		await vi.waitFor(() => expect(core.view.sim_waited_out_key).toBeNull(), WAIT);
		expect(core.sim.at(-1)).toEqual({ type: 'sim_settled', id: 'tx:1' });
		// The confirm stayed open through it.
		await frames.further();
		expect(confirmEl()!.disabled).toBe(false);
		expect(heldLine()).toBeNull();
		frames.stop();
		await screen.unmount();
	});

	it('a late answer this sheet draws nothing for takes the caution away and leaves the confirm open', async () => {
		const screen = mount();
		core.arrive('tx:1', 'transaction');
		await asked();
		await held();
		core.timers[0].fire();
		await vi.waitFor(() => expect(cardNote()?.textContent).toBe(COULD_NOT_CHECK), WAIT);
		node.asked[0].answer(REVERTS);
		await vi.waitFor(() => expect(card()).toBeNull(), WAIT);
		await open();
		await screen.unmount();
	});

	it('the wallet’s own request is held, and shown the caution, the same way — on the sheet', async () => {
		const screen = mount();
		core.arrive('own:1', 'transaction', true);
		await asked();
		await held();
		expect(core.view.request?.first_party).toBe(true);
		core.timers[0].fire();
		await vi.waitFor(() => expect(cardNote()?.textContent).toBe(COULD_NOT_CHECK), WAIT);
		expect(cardNote()!.dataset.tone).toBe('caution');
		await open();
		await screen.unmount();
	});

	it('a simulation that throws still settles: the confirm is not left to the deadline', async () => {
		const screen = mount();
		// The engine itself fails — `simulateVerdict` reads that as "unreachable".
		node.ask = () => Promise.reject(new Error('every endpoint failed'));
		core.arrive('tx:1', 'transaction');
		await vi.waitFor(
			() => expect(core.sim.at(-1)).toEqual({ type: 'sim_settled', id: 'tx:1' }),
			WAIT
		);
		await open();
		expect(card()).toBeNull();
		// Nothing was left for the timer to do.
		expect(core.view.sim_checking).toBe(false);
		await screen.unmount();
	});

	it('the wait belongs to its request: an answer for one that has gone tells the core nothing', async () => {
		const screen = mount();
		core.arrive('tx:1', 'transaction');
		await asked();
		await held();
		// Refused with the ✕'s event; the next request takes the sheet.
		core.dispatch({ type: 'reject_tapped' });
		await vi.waitFor(() => expect(core.view.request).toBeNull(), WAIT);
		core.arrive('tx:2', 'transaction');
		await asked(2);
		await held();
		expect(core.sim).toEqual([
			{ type: 'sim_started', id: 'tx:1' },
			{ type: 'sim_started', id: 'tx:2' }
		]);
		// The first request's node answers now. Nobody is told, nothing is drawn.
		node.asked[0].answer(NOTHING_MOVES);
		await tick();
		flushSync();
		expect(core.sim.filter((event) => event.type === 'sim_settled')).toEqual([]);
		expect(card()).toBeNull();
		expect(confirmEl()!.disabled).toBe(true);
		// Its own answer opens it.
		node.asked[1].answer(NOTHING_MOVES);
		await open();
		expect(core.sim.at(-1)).toEqual({ type: 'sim_settled', id: 'tx:2' });
		await screen.unmount();
	});
});

describe('where there is no simulation, nothing is held (PR 3)', () => {
	it('a message is never held, is never simulated, and starts no timer', async () => {
		const frames = watchFrames();
		const screen = mount();
		core.arrive('m:1', 'message');
		await open();
		await frames.further(12);
		expect(node.asked).toEqual([]);
		expect(core.sim).toEqual([]);
		expect(core.timers).toEqual([]);
		expect(core.view.sim_checking).toBe(false);
		// No frame of it said "checking", and none put a card on the sheet.
		expect(frames.frames.length).toBeGreaterThan(0);
		expect(frames.frames.filter((frame) => frame.line === CHECKING || frame.said !== null)).toEqual(
			[]
		);
		frames.stop();
		await screen.unmount();
	});

	it('a node that answers "not offered" at once: after it, no frame is held, and its deadline says nothing', async () => {
		node.auto = NOT_OFFERED;
		const frames = watchFrames();
		const screen = mount();
		core.arrive('tx:1', 'transaction');
		await vi.waitFor(
			() =>
				expect(core.sim).toEqual([
					{ type: 'sim_started', id: 'tx:1' },
					{ type: 'sim_settled', id: 'tx:1' }
				]),
			WAIT
		);
		await vi.waitFor(() => expect(core.view.sim_checking).toBe(false), WAIT);
		await open();
		// From here on: every frame is an open confirm with no line under it.
		const from = frames.frames.length;
		await frames.further(12);
		// The wait's timer was started with the simulation; its `ms` pass now.
		expect(core.timers).toHaveLength(1);
		core.timers[0].fire();
		await frames.further(12);
		const after = frames.frames.slice(from);
		expect(after.length).toBeGreaterThanOrEqual(24);
		expect(after.filter((frame) => frame.disabled || frame.line !== null)).toEqual([]);
		// This sheet draws nothing for "not offered" (spec 082 RG6) — and the
		// deadline of a wait that is over is not "could not check" either.
		expect(after.filter((frame) => frame.said !== null)).toEqual([]);
		expect(core.view.sim_waited_out_key).toBeNull();
		frames.stop();
		await screen.unmount();
	});
});

/**
 * The hold line sits where the confirm's note already does, in the sheet's
 * foot outside its scroll, and the confirm does not move when the line
 * appears or goes — nor when the caution lands in the verdict's place, or a
 * late verdict takes it away again.
 */
describe('the confirm does not move while the line comes and goes (PR 3)', () => {
	const SCREENS = [
		['the phone sheet', 390, 844],
		['the centred card', 1400, 900]
	] as const;

	it.each(SCREENS)(
		'%s (%i×%i): the confirm’s top is the same held, open, cautioned and answered',
		async (name, width, height) => {
			await page.viewport(width, height);
			const frames = watchFrames();
			const screen = mount();
			core.arrive('tx:1', 'transaction');
			await asked();
			await held();
			/**
			 * Where the confirm rests: whole inside the window, and in the same
			 * place over a run of PAINTED frames. Two looks a moment apart can
			 * agree in the middle of the sheet's entrance when the machine is
			 * busy and no frame was drawn between them.
			 */
			const restingTop = async () => {
				let last = Number.NaN;
				let still = 0;
				await vi.waitFor(async () => {
					await frames.further(2);
					const now = confirmTop();
					const inside = confirmEl()!.getBoundingClientRect().bottom <= window.innerHeight;
					still = inside && now === last ? still + 1 : 0;
					last = now;
					expect(still).toBeGreaterThanOrEqual(4);
				}, WAIT);
				return last;
			};
			// Past the sheet's entrance, held with its line.
			const linePresent = await restingTop();
			expect(heldLine()).toBe(CHECKING);

			// The deadline: the line goes, the caution lands.
			core.timers[0].fire();
			await vi.waitFor(() => expect(cardNote()?.textContent).toBe(COULD_NOT_CHECK), WAIT);
			await open();
			const lineAbsent = await restingTop();

			// The late answer: "No asset changes" takes the caution's place.
			node.asked[0].answer(NOTHING_MOVES);
			await vi.waitFor(() => expect(cardNote()?.textContent).toBe(NO_CHANGE), WAIT);
			const answered = await restingTop();

			// Measured on this build: 739 on the phone sheet and 551 on the
			// centred card, in all three states.
			expect([name, lineAbsent]).toEqual([name, linePresent]);
			expect([name, answered]).toEqual([name, linePresent]);
			// Whole and on screen throughout.
			const box = confirmEl()!.getBoundingClientRect();
			expect(box.top).toBeGreaterThanOrEqual(0);
			expect(box.bottom).toBeLessThanOrEqual(window.innerHeight);
			frames.stop();
			await screen.unmount();
		}
	);
});
