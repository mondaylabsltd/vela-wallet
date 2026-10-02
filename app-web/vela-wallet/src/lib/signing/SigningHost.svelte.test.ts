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
import { afterEach, describe, expect, it, vi } from 'vitest';
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
vi.mock('$lib/settings/core/currency.svelte', () => ({ currency: { view: {} } }));
vi.mock('$lib/wallet/identicon', () => ({ identiconSvgForClient: () => '<svg></svg>' }));
vi.mock('$lib/services/networks', () => ({ explorerBaseURL: () => null }));
vi.mock('$lib/signing/fee-calls', () => ({ feeCallsOf: () => null }));
vi.mock('$lib/core/kernels', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/kernels')>()),
	feeRequoteDelayMs: () => 3_000,
	typicalInclusionSeconds: () => 5
}));
vi.mock('$lib/signing/fee-requote', () => ({
	FeeRequoteTimer: class {
		observe() {}
		stop() {}
	},
	heldFeeFailure: () => null,
	withLostContext: (view: unknown) => view
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
		get tier() {
			return 'standard';
		}
		get view() {
			return { open: false, rows: [] };
		}
		get feeInForce() {
			return { busy: false, failed: false, options: [] };
		}
		get feeQuote() {
			return { contextLost: false };
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
			blocks: [{ kind: 'intent', text: 'Send 0.001 xDAI', tone: 'neutral' }],
			tech: { title: 'Details', params: [], identities: [], copyLabel: 'Copy', explorerLabel: 'X' },
			techOpen: false,
			fee: { kind: 'hidden' },
			signer: { label: 'Signing account', name: 'One', identiconSvg: '<svg></svg>' },
			confirm: { hint: 'Slide to confirm', action: 'Send', enabled: true },
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
	signed: 'Signed!',
	maybeSent: 'May have been sent',
	closeBackground: 'Close · keep running',
	notSentHint: 'Nothing was sent',
	submitting: 'Submitting to network...',
	refused: 'The network refused it'
};
const FEE = { view: null, requote: () => {}, requestQuote: async () => {}, lastRequest: null };
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
		// The slide was made; the relay's reply was lost: the handoff is out
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
