/* eslint-disable @typescript-eslint/no-explicit-any -- stand-ins for the resident machines */
/**
 * The signing host in a browser (spec 082 T214; G37 part, G65).
 *
 * G37 (EX-W1): the relay's reply was lost, so the landing rose before the page
 * was answered; the chain check found the op and 已确认 closed itself after its
 * beat — and the sheet underneath came back as 提交至网络… with a spinner for
 * 49 s, a confirmed row showing under a "submitting" sheet, until the page's
 * answer finally went out. After a landing for a request was raised and
 * closed, that request's sheet waits hidden for its answer.
 *
 * G65 (EX4): A signed with B's Connect already queued; the full-panel
 * 已签名！ tick covered B's card for ≥ 1.4 s. When another request is owed,
 * the tick is skipped; with none, it shows as before (L-PANEL).
 *
 * The resident machines are stood in for (their own suites drive the core);
 * what is real here is `SigningHost`, the sheet, the receipt and the panel's
 * surface.
 */
import { flushSync, tick } from 'svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';

vi.mock('$lib/signing/core/sign-resident.svelte', () => ({
	signRequest: {
		get view() {
			return fake.sign.view;
		},
		get answered() {
			return fake.sign.answered;
		},
		get progress() {
			return fake.sign.progress;
		},
		dispatch: (event: unknown) => fake.dispatched.push(event)
	}
}));
vi.mock('$lib/signing/core/sheet.svelte', () => ({
	signingSheet: {
		present: async () => {},
		dismiss: () => {},
		clear: null,
		guard: { rewritten_params_json: null, unlimited_consented: false },
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
import { loadCore } from '$lib/core/client';

beforeAll(() => loadCore());

vi.mock('$lib/settings/core/currency.svelte', () => ({
	currency: {
		view: {},
		boot: async () => {
			fake.currencyBoots += 1;
		}
	}
}));
vi.mock('$lib/wallet/identicon', () => ({ identiconSvgForClient: () => '<svg></svg>' }));
vi.mock('$lib/services/networks', () => ({ explorerBaseURL: () => null }));
vi.mock('$lib/signing/fee-calls', () => ({ feeCallsOf: () => fake.calls }));
// The node's reply to the sheet's one simulation. `checkRequest` and the
// core's reading of the reply (`simOutcome`) are real.
vi.mock('$lib/services/sim/sim-engine-rpc', () => ({
	rpcSimulateReply: (from: string, calls: unknown[], chainId: number) => {
		fake.simulated.push({ from, calls, chainId });
		return fake.nodeReply();
	}
}));
vi.mock('$lib/core/kernels', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/kernels')>()),
	typicalInclusionSeconds: () => 5
}));
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
		balanceChanges(calls: unknown[], changes: unknown[]) {
			fake.told.push({ calls, changes });
		}
		get tier() {
			return 'standard';
		}
		get view() {
			return { open: false, rows: [] };
		}
		get feeInForce() {
			return { busy: false, failed: null, failure: null, options: [] };
		}
		get feeQuote() {
			return {};
		}
		feeOptions() {
			return [];
		}
	}
}));
vi.mock('$lib/wallet/core/tracker-resident', () => ({
	subscribeTxTracker: (fn: () => void) => {
		fake.trackerListeners.add(fn);
		return () => fake.trackerListeners.delete(fn);
	},
	txTrackerView: () => ({ entries: fake.trackerEntries })
}));
vi.mock('$lib/signing/live', () => ({
	// What the approve carries is `signing/live.test.ts`'s (spec 093).
	approveOptsOf: () => ({}),
	signingCloseEvent: (status: { closable: boolean } | undefined) =>
		!status ? 'reject_tapped' : status.closable ? 'dismiss_tapped' : null,
	buildSigningModel: (raw: any) => {
		if (raw.sign.surface === 'hidden' || !raw.sign.request) return null;
		return {
			id: 'cs1',
			dapp: { name: 'a.example', host: '', letter: 'A', tint: 'var(--color-fg-muted)' },
			network: { name: 'Gnosis', dot: 'var(--color-fg-muted)' },
			blocks: [
				{ kind: 'intent', text: 'Send 0.001 xDAI', tone: 'neutral' },
				// The builder's own rule is `signing/live.test.ts`'s; here, only
				// that the host hands it this request's verdict.
				...(raw.sim?.no_change_key
					? [
							{
								kind: 'balances',
								title: 'Balance changes',
								rows: [],
								note: `said:${raw.sim.no_change_key}`,
								verdict: true
							}
						]
					: [])
			],
			tech: { title: 'Details', params: [], identities: [], copyLabel: 'Copy', explorerLabel: 'X' },
			techOpen: false,
			fee: { kind: 'hidden' },
			signer: { label: 'Signing account', name: 'One', identiconSvg: '<svg></svg>' },
			confirm: { action: 'Send', enabled: true },
			closeLabel: 'Close',
			panelTitle: 'Signature request',
			...(raw.sign.is_submitting
				? {
						status: {
							stage: 'submitting',
							title: 'Submitting to the network…',
							captions: [],
							closable: true
						}
					}
				: {}),
			...(raw.sign.error
				? {
						status: {
							stage: 'failed',
							title: 'Failed',
							captions: [],
							closable: true,
							actions: {
								close: 'Done',
								...(raw.sign.failure_retryable ? { retry: 'Try Again' } : {})
							}
						}
					}
				: {})
		};
	}
}));
// The landing's words follow the tracker's entry; the core's ending rule has
// its own suite (`dapp-receipt.test.ts`).
vi.mock('$lib/signing/dapp-receipt', async (importOriginal) => {
	const real = await importOriginal<typeof import('$lib/signing/dapp-receipt')>();
	return {
		...real,
		landingFor: (
			entry: { status?: string; tx_hash?: string } | undefined,
			opHash: string,
			maybeSent: boolean
		) =>
			entry?.status === 'confirmed'
				? { kind: 'confirmed', opHash, txHash: entry.tx_hash ?? '' }
				: entry?.status === 'rejected'
					? { kind: 'refused', opHash }
					: maybeSent
						? { kind: 'maybe_sent', opHash }
						: { kind: 'submitted', opHash }
	};
});

import SigningHost from './SigningHost.svelte';
import { panelSurface } from '$lib/dapp/panel-surface.svelte';
import { INITIAL_SIGN_VIEW } from '$lib/signing/core/sign-resident.svelte';

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** The resident machines' state, as reactive as theirs. */
class Fake {
	sign = $state({
		view: { ...INITIAL_SIGN_VIEW } as any,
		answered: null as { id: string; ok: boolean } | null,
		progress: { stage: 'idle' } as any
	});
	dispatched: unknown[] = [];
	/** What `feeCallsOf` answers: `null`, a request with no fee to ask about. */
	calls: unknown[] | null = null;
	/** Every `eth_simulateV1` read the host made. */
	simulated: { from: string; calls: unknown[]; chainId: number }[] = [];
	/** What the node answers it: by default, a node that does not offer it. */
	nodeReply: () => Promise<string> = async () =>
		JSON.stringify({ error: { code: -32601, message: 'method not found' } });
	/** What the fee machine was told the calls move. */
	told: { calls: unknown[]; changes: unknown[] }[] = [];
	quoted = 0;
	disposed = 0;
	/** How many times this host asked the display-currency store to boot. */
	currencyBoots = 0;
	trackerEntries: any[] = [];
	trackerListeners = new Set<() => void>();
	trackerChanged(): void {
		for (const fn of [...this.trackerListeners]) fn();
	}
}
let fake = new Fake();

const RECEIPT = {
	confirming: 'Confirming…',
	confirmingHint: 'Usually a few seconds',
	submitted: 'Submitted',
	confirmed: 'Confirmed',
	failed: 'Failed',
	failedHint: 'Try again',
	opHashLabel: 'Operation',
	txHashLabel: 'Transaction',
	explorer: 'Explorer',
	done: 'Done',
	stillConfirming: 'Still confirming',
	unknownOutcome: 'Unknown',
	relayFunding: 'The relay is topping up its gas',
	relaySending: 'relay sending',
	signed: 'Signed!',
	maybeSent: 'May have been sent',
	closeBackground: 'Close · keep running',
	notSentHint: 'Nothing was sent',
	submitting: 'Submitting to network...',
	refused: 'The network refused it'
};
const FEE = {
	view: null,
	requote: () => {},
	requestQuote: async () => {
		fake.quoted += 1;
	},
	dispose: () => {
		fake.disposed += 1;
	},
	lastRequest: null
};
const OP = `0x${'ab'.repeat(32)}`;

function request(id: string, kind: string, origin = 'https://a.example') {
	return {
		id,
		method: kind === 'transaction' ? 'eth_sendTransaction' : 'personal_sign',
		kind,
		params_json: '[]',
		origin,
		dapp: null,
		chain_id: 100,
		signer_address: null
	};
}

function mount() {
	const screen = render(SigningHost, {
		props: { messages: {} as any, fee: FEE as any, receipt: RECEIPT }
	});
	return {
		screen,
		sheet: () => document.body.querySelector('[role="dialog"]'),
		landing: () => document.body.querySelector('.landing-over'),
		text: () => document.body.querySelector('.landing-over')?.textContent ?? ''
	};
}

function fakeChrome() {
	const toSurface: ((m: unknown) => void)[] = [];
	return {
		chrome: {
			runtime: {
				id: 'ext',
				connect: () => ({
					postMessage: () => {},
					disconnect: () => {},
					onMessage: { addListener: (fn: (m: unknown) => void) => toSurface.push(fn) },
					onDisconnect: { addListener: () => {} }
				}),
				sendMessage: async () => ({ delivered: true })
			},
			windows: { getCurrent: async () => ({ id: 3 }) },
			storage: { onChanged: { addListener: () => {}, removeListener: () => {} } }
		},
		push: (m: unknown) => {
			for (const fn of [...toSurface]) fn(m);
		}
	};
}

afterEach(() => {
	panelSurface.stop();
	delete (globalThis as { chrome?: unknown }).chrome;
	fake = new Fake();
});

/**
 * The sheet prices what it shows in the display currency, and no money figure
 * is drawn until that currency is the person's (the core's rule,
 * `CurrencyView.committed`). The wallet and Settings routes boot the store
 * themselves; the extension's request window mounts only this host — so the
 * host boots it, or that window's figures would wait for ever.
 */
describe('the host boots the display currency it prices in', () => {
	it('asks the store to boot when it mounts, before any request', async () => {
		const view = mount();
		flushSync();
		await tick();
		expect(fake.currencyBoots).toBe(1);
		await view.screen.unmount();
	});
});

/**
 * PR 2 note 1: the core asks a failed fee again by itself, through the timer
 * the fee session answers. A sheet that has gone must not go on asking — in
 * the wallet, in Settings' backup, and in the extension's request window
 * alike, which all mount this host — so the request's fee session ends with
 * the request, and the next request starts a new one.
 */
describe('the sheet’s fee session ends with its request (PR 2 note 1)', () => {
	it('a request that goes takes its fee session, and the core’s re-asks, with it', async () => {
		fake.calls = [{ to: '0x' + '11'.repeat(20), value: '0', data: '0x' }];
		const view = mount();
		fake.sign.view = {
			...INITIAL_SIGN_VIEW,
			surface: 'sheet' as const,
			request: request('tx:1', 'transaction')
		};
		flushSync();
		await tick();
		expect(fake.quoted).toBe(1);
		expect(fake.disposed).toBe(0);
		// Answered, closed, or rejected: the sheet goes.
		fake.sign.view = { ...INITIAL_SIGN_VIEW };
		flushSync();
		await tick();
		expect(fake.disposed).toBe(1);
		// The next request asks on a session of its own.
		fake.sign.view = {
			...INITIAL_SIGN_VIEW,
			surface: 'sheet' as const,
			request: request('tx:2', 'transaction')
		};
		flushSync();
		await tick();
		expect(fake.quoted).toBe(2);
		await view.screen.unmount();
	});

	it('a message has no fee: nothing is asked, nothing ended', async () => {
		const view = mount();
		fake.sign.view = {
			...INITIAL_SIGN_VIEW,
			surface: 'sheet' as const,
			request: request('m:1', 'personal_sign')
		};
		flushSync();
		await tick();
		fake.sign.view = { ...INITIAL_SIGN_VIEW };
		flushSync();
		await tick();
		expect(fake.quoted).toBe(0);
		expect(fake.disposed).toBe(0);
		await view.screen.unmount();
	});
});

/**
 * PR 3 device round, item 3: the read the host already runs for the fee
 * (`eth_simulateV1`, issue 411) is the sheet's too. What it means is the
 * core's (`simOutcome`, real here): a check under which nothing of the
 * person's moves hands the sheet the core's "No asset changes" key — for the
 * request it was measured for, and no other.
 */
describe('the sheet’s one simulation, read by the core (device round, item 3)', () => {
	const CALLS = [{ to: '0x' + '22'.repeat(20), value: '0', data: '0x095ea7b3' }];
	const ME = '0x1111111111111111111111111111111111111111';
	const NOTHING_MOVES = JSON.stringify({ result: [{ calls: [{ status: '0x1', logs: [] }] }] });
	const REVERTS = JSON.stringify({ result: [{ calls: [{ status: '0x0', logs: [] }] }] });
	const raise = (id: string, kind = 'transaction') => {
		fake.sign.view = {
			...INITIAL_SIGN_VIEW,
			surface: 'sheet' as const,
			request: request(id, kind)
		};
	};
	const settle = async () => {
		flushSync();
		await tick();
		await pause(30);
		flushSync();
	};
	const card = () => document.body.querySelector('[role="dialog"] section.balances');
	const confirmTop = () =>
		document.body.querySelector('[data-testid="signing-confirm"]')!.getBoundingClientRect().top;
	/** The reads about THIS suite's calls. */
	const asked = () =>
		fake.simulated.filter((read) => JSON.stringify(read.calls).includes(CALLS[0].to));

	// An earlier test's host may still be on its way to the node (the engine
	// is loaded on first use): let it land on a state nobody reads.
	beforeEach(async () => {
		await pause(60);
		fake = new Fake();
	});

	it('nothing moves: one read tells the fee and lands the card — and the confirm is where it was', async () => {
		fake.calls = CALLS;
		let answer: (reply: string) => void = () => {};
		fake.nodeReply = () => new Promise((resolve) => (answer = resolve));
		const view = mount();
		raise('tx:1');
		await settle();
		await pause(350); // past the entry animation
		// Asked once, about this request's calls, from the account that signs.
		expect(asked()).toEqual([
			{ from: ME, calls: [{ to: CALLS[0].to, value: '0', data: CALLS[0].data }], chainId: 100 }
		]);
		// Nothing is said while the node has not answered.
		expect(card()).toBeNull();
		const was = confirmTop();

		answer(NOTHING_MOVES);
		await settle();
		expect(card()?.textContent).toContain('said:componentsUi.signing.simResultNoChange');
		expect(card()?.hasAttribute('data-verdict')).toBe(true);
		expect(card()?.querySelectorAll('.row')).toHaveLength(0);
		// The same read told the fee machine: a check, nothing moved.
		expect(fake.told).toEqual([{ calls: CALLS, changes: [] }]);
		expect(asked()).toHaveLength(1);
		expect(Math.abs(confirmTop() - was)).toBeLessThan(0.1);
		await view.screen.unmount();
	});

	it('a revert, or a node that cannot check: no card, and the fee is told nothing', async () => {
		fake.calls = CALLS;
		for (const reply of [REVERTS, undefined]) {
			if (reply) fake.nodeReply = async () => reply;
			const view = mount();
			raise('tx:1');
			await settle();
			expect(view.sheet()).not.toBeNull();
			expect(asked()).toHaveLength(1);
			expect(card()).toBeNull();
			expect(fake.told).toEqual([]);
			await view.screen.unmount();
			fake = new Fake();
			fake.calls = CALLS;
		}
	});

	it('it belongs to its request: the next request starts with none, and a late answer lands on nobody', async () => {
		fake.calls = CALLS;
		const answers: ((reply: string) => void)[] = [];
		fake.nodeReply = () => new Promise((resolve) => answers.push(resolve));
		const view = mount();
		raise('tx:1');
		await settle();
		answers[0](NOTHING_MOVES);
		await settle();
		expect(card()).not.toBeNull();

		// Another request takes the sheet: no card until ITS simulation says so.
		raise('tx:2');
		await settle();
		expect(asked()).toHaveLength(2);
		expect(card()).toBeNull();
		answers[1](REVERTS);
		await settle();
		expect(card()).toBeNull();

		// A third, while the second's sheet went: its predecessor's late "nothing
		// moves" is not this one's.
		fake.sign.view = { ...INITIAL_SIGN_VIEW };
		await settle();
		raise('tx:3');
		await settle();
		expect(asked()).toHaveLength(3);
		raise('tx:4');
		await settle();
		answers[2](NOTHING_MOVES);
		await settle();
		expect(card()).toBeNull();
		// Nor was the fee machine told about calls nobody is looking at.
		expect(fake.told).toHaveLength(1);
		await view.screen.unmount();
	});

	it('a message is not simulated, and says nothing', async () => {
		const view = mount();
		raise('m:1', 'personal_sign');
		await settle();
		expect(view.sheet()).not.toBeNull();
		expect(fake.simulated).toEqual([]);
		expect(fake.told).toEqual([]);
		expect(card()).toBeNull();
		await view.screen.unmount();
	});
});

describe('the write-ahead hand-off (spec 082 RJ1)', () => {
	it('raises no landing while its POST is out; the relay taking it does', async () => {
		const view = mount();
		// Signed and hashed, the record written, the POST not answered yet: the
		// hand-off is out ("may have been sent") and the sheet says 提交至网络….
		const writeAhead = {
			...INITIAL_SIGN_VIEW,
			surface: 'sheet' as const,
			request: request('tx:1', 'transaction'),
			is_submitting: true,
			tracker_handoff: {
				user_op_hash: OP,
				record_ids: ['dapp-1-tx'],
				chain_id: 100,
				maybe_sent: true,
				submit_block: null,
				admitted: false
			}
		};
		fake.sign.view = writeAhead;
		flushSync();
		await tick();
		expect(view.landing()).toBeNull();

		// The relay took it: the admitted hand-off lands.
		fake.sign.view = {
			...writeAhead,
			pending_op_hash: OP,
			tracker_handoff: { ...writeAhead.tracker_handoff, maybe_sent: false, admitted: true }
		};
		flushSync();
		await tick();
		expect(view.landing()).not.toBeNull();
		expect(view.text()).not.toContain('May have been sent');
		await view.screen.unmount();
	});
});

describe('after the landing closes, the sheet does not fall back to submitting (G37)', () => {
	it('a confirmed landing closes itself and its still-unanswered request stays hidden', async () => {
		const view = mount();
		// The confirm was tapped; the relay's reply was lost: the handoff is out
		// before the page's answer.
		fake.sign.view = {
			...INITIAL_SIGN_VIEW,
			surface: 'sheet',
			request: request('tx:1', 'transaction'),
			is_submitting: true,
			// The submit ended "may have been sent": the core names the op.
			pending_op_hash: OP,
			pending_op_maybe_sent: true,
			tracker_handoff: {
				user_op_hash: OP,
				record_ids: [],
				chain_id: 100,
				maybe_sent: true,
				submit_block: null,
				admitted: false
			}
		};
		flushSync();
		await tick();
		expect(view.text()).toContain('May have been sent');

		// The chain check found it: 已确认, then its own beat.
		fake.trackerEntries = [
			{ user_op_hash: OP, status: 'confirmed', tx_hash: `0x${'cd'.repeat(32)}` }
		];
		fake.trackerChanged();
		flushSync();
		await tick();
		expect(view.text()).toContain('Confirmed');
		await pause(2_900);
		flushSync();
		await tick();
		expect(view.landing()).toBeNull();

		// The page has not been answered yet; the sheet must not come back.
		expect(fake.sign.view.is_submitting).toBe(true);
		expect(view.sheet()).toBeNull();
		await pause(300);
		expect(view.sheet()).toBeNull();

		// The answer goes out, and the next request's sheet shows as usual.
		fake.sign.view = { ...INITIAL_SIGN_VIEW };
		flushSync();
		fake.sign.view = {
			...INITIAL_SIGN_VIEW,
			surface: 'sheet',
			request: request('tx:2', 'transaction')
		};
		flushSync();
		await tick();
		await vi.waitFor(() => expect(view.sheet()).not.toBeNull());
		await view.screen.unmount();
	});

	it('Done on the landing also leaves the request hidden until it is answered', async () => {
		const view = mount();
		fake.sign.view = {
			...INITIAL_SIGN_VIEW,
			surface: 'sheet',
			request: request('tx:1', 'transaction'),
			is_submitting: true,
			// The submit ended "may have been sent": the core names the op.
			pending_op_hash: OP,
			pending_op_maybe_sent: true,
			tracker_handoff: {
				user_op_hash: OP,
				record_ids: [],
				chain_id: 100,
				maybe_sent: true,
				submit_block: null,
				admitted: false
			}
		};
		flushSync();
		await tick();
		const done = [
			...document.body.querySelectorAll<HTMLButtonElement>('.landing-over button')
		].find((b) => b.textContent?.trim() === RECEIPT.closeBackground);
		expect(done).toBeDefined();
		done!.click();
		flushSync();
		await tick();
		expect(view.landing()).toBeNull();
		expect(view.sheet()).toBeNull();
		await view.screen.unmount();
	});
});

describe('the signed tick and a queued request (G65)', () => {
	async function signed(view: ReturnType<typeof mount>, over?: () => void) {
		fake.sign.view = {
			...INITIAL_SIGN_VIEW,
			surface: 'sheet',
			request: request('s:1', 'personal_sign')
		};
		flushSync();
		await tick();
		await vi.waitFor(() => expect(view.sheet()).not.toBeNull());
		// The core answers and clears the sheet.
		fake.sign.answered = { id: 's:1', ok: true };
		fake.sign.view = { ...INITIAL_SIGN_VIEW };
		flushSync();
		over?.();
		flushSync();
		await tick();
	}

	it('with nothing queued, the tick shows (L-PANEL unchanged)', async () => {
		const view = mount();
		await signed(view);
		expect(view.text()).toContain('Signed!');
		await view.screen.unmount();
	});

	it('with another request owed in the panel, the tick is skipped', async () => {
		const env = fakeChrome();
		(globalThis as { chrome?: unknown }).chrome = env.chrome as any;
		await panelSurface.start({ kind: 'panel', windowId: 3 });
		const owed = (rid: string, id: string, origin: string) => ({
			type: 'owed',
			request: { rid, id, method: 'personal_sign', params: [], origin, tabId: 7, at: 1 }
		});
		// The panel owes A (the one being signed)…
		env.push(owed('7:s:1', 's:1', 'https://a.example'));
		const view = mount();
		// …and once A is answered, the worker hands B over.
		await signed(view, () => env.push(owed('9:b:1', 'b:1', 'https://b.example')));
		expect(view.landing()).toBeNull();
		await view.screen.unmount();
	});

	it('the request just signed is not "another" one: its own tick still shows', async () => {
		const env = fakeChrome();
		(globalThis as { chrome?: unknown }).chrome = env.chrome as any;
		await panelSurface.start({ kind: 'panel', windowId: 3 });
		env.push({
			type: 'owed',
			request: {
				rid: '7:s:1',
				id: 's:1',
				method: 'personal_sign',
				params: [],
				origin: 'https://a.example',
				tabId: 7,
				at: 1
			}
		});
		const view = mount();
		await signed(view);
		expect(view.text()).toContain('Signed!');
		await view.screen.unmount();
	});
});

/**
 * Spec 096 F8: a failure that sent nothing stays on the sheet, unanswered —
 * Try again is the core's `retry_tapped`, Done the plain close that answers
 * the page.
 */
describe('a failure before anything was sent', () => {
	const failed = () => ({
		...INITIAL_SIGN_VIEW,
		surface: 'sheet' as const,
		request: request('tx:9', 'transaction'),
		error: { kind: 'submit_failed', detail: 'Could not estimate gas' },
		failure_retryable: true,
		swipe_action: 'dismiss'
	});
	const button = (label: string) =>
		[
			...document.body.querySelectorAll<HTMLButtonElement>(
				'[data-testid="signing-status-actions"] button'
			)
		].find((b) => b.textContent?.trim() === label);

	it('Try again tells the core, and nothing is answered', async () => {
		fake = new Fake();
		const view = mount();
		fake.sign.view = failed();
		flushSync();
		await tick();
		await pause(300);
		button('Try Again')!.click();
		expect(fake.dispatched).toEqual([{ type: 'retry_tapped' }]);
		await view.screen.unmount();
	});

	it('Done is the plain close the core answers the failure on', async () => {
		fake = new Fake();
		const view = mount();
		fake.sign.view = failed();
		flushSync();
		await tick();
		await pause(300);
		button('Done')!.click();
		await vi.waitFor(() => expect(fake.dispatched).toEqual([{ type: 'dismiss_tapped' }]), {
			timeout: 1500
		});
		await view.screen.unmount();
	});
});

/**
 * Spec 097 N4: the relay took the op, the landing said "Submitted", and the
 * relay then refused it. The core holds the page's answer while the refusal
 * shows (096 F8's rule); the landing says it in its own words and stays —
 * and its Done is the close that answers the page. Before 097 the core
 * answered with the verdict and the extension window closed over the words.
 */
describe('a refusal after "Submitted"', () => {
	const submitted = () => ({
		...INITIAL_SIGN_VIEW,
		surface: 'sheet' as const,
		request: request('tx:n4', 'transaction', 'https://app.aave.com'),
		is_submitting: true,
		phase: 'submitting' as const,
		swipe_action: 'dismiss' as const,
		pending_op_hash: OP,
		tracker_handoff: {
			user_op_hash: OP,
			record_ids: ['dapp-1-tx'],
			chain_id: 56,
			maybe_sent: false,
			submit_block: null,
			admitted: true
		}
	});

	it('stays on the landing in its own words, and Done answers through the core', async () => {
		fake = new Fake();
		const view = mount();
		fake.sign.view = submitted();
		flushSync();
		await tick();
		// The landing rises for the op the relay took.
		await vi.waitFor(() => expect(view.landing()).not.toBeNull());
		fake.trackerEntries = [{ user_op_hash: OP, status: 'pending', tx_hash: null }];
		fake.trackerChanged();
		flushSync();
		await tick();
		expect(view.text()).toContain('Submitted');

		// The tracker's verdict, and the core holding it on the sheet.
		fake.trackerEntries = [{ user_op_hash: OP, status: 'rejected', tx_hash: null }];
		fake.sign.view = {
			...submitted(),
			is_submitting: false,
			phase: 'idle',
			pending_op_hash: null,
			error: {
				kind: 'submit_failed',
				detail: 'the network refused this transaction; nothing was sent'
			},
			failure_refused: true
		};
		fake.trackerChanged();
		flushSync();
		await tick();
		expect(view.text()).toContain('Failed');
		expect(view.text()).toContain(RECEIPT.refused);
		expect(view.text()).not.toContain('Try Again');
		// Longer than any landing's own beat: it waits for the person.
		await pause(3_000);
		expect(view.landing()).not.toBeNull();
		expect(fake.dispatched).toEqual([]);

		const done = [
			...document.body.querySelectorAll<HTMLButtonElement>('.landing-over button')
		].find((b) => b.textContent?.trim() === RECEIPT.done);
		expect(done).toBeDefined();
		done!.click();
		flushSync();
		await tick();
		expect(fake.dispatched).toEqual([{ type: 'dismiss_tapped' }]);
		expect(view.landing()).toBeNull();
		await view.screen.unmount();
	});
});
