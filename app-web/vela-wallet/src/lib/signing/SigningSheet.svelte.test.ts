/**
 * The signing sheet in a browser (spec 079).
 *
 * US1 — it closes only through its ✕: the owner lost a dApp's request to a
 * stray touch ("除非用户明确关掉，不应该很容易误操作，比如下滑就关掉了"), and the
 * page then had to ask again. US2 — its fee row carries the send form's
 * refresh control and stale note, and says why a quote failed.
 */
import { tick } from 'svelte';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import SigningSheet from './SigningSheet.svelte';
import type { FeeModel, SigningModel } from './model';
import { verdictRooms, type VerdictWords } from './verdict';

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Past the sheet's entry animation — WAITED for, never timed. This was a
 * 300 ms pause over a 220 ms slide: on a busy machine the slide had not
 * finished, the sheet was measured mid-entrance, and a suite about things
 * not moving failed on a different test each run, by a pixel or by a hundred.
 * The sheet's own animation and its scrim's are the ones awaited (a turning
 * refresh control never finishes); then a frame, so what is measured is drawn.
 */
async function entered(sheet: HTMLElement): Promise<void> {
	const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
	await frame();
	const scrim = sheet.parentElement?.querySelector('.scrim');
	const playing = [...sheet.getAnimations(), ...(scrim?.getAnimations() ?? [])];
	await Promise.all(playing.map((animation) => animation.finished.catch(() => undefined)));
	await frame();
}

function model(over: Partial<SigningModel> = {}): SigningModel {
	return {
		id: 'cs1',
		dapp: { name: 'app.example', host: '', letter: 'A', tint: 'var(--color-fg-muted)' },
		network: { name: 'Gnosis', dot: 'var(--color-fg-muted)' },
		blocks: [{ kind: 'intent', text: 'Send 1 xDAI', tone: 'neutral' }],
		tech: {
			title: 'Details',
			params: [],
			identities: [],
			copyLabel: 'Copy',
			explorerLabel: 'Explorer'
		},
		techOpen: false,
		fee: { kind: 'hidden' },
		signer: { label: 'Signing account', name: 'My Wallet', identiconSvg: '<svg></svg>' },
		confirm: { action: 'Send', enabled: true },
		closeLabel: 'Close',
		panelTitle: 'Signature request',
		...over
	};
}

async function drawn(props: {
	model?: SigningModel;
	dismissible?: boolean;
	onfeerefresh?: () => void;
	onretry?: () => void;
	onconfirm?: () => void;
}) {
	const onclose = vi.fn();
	const screen = render(SigningSheet, {
		props: {
			model: props.model ?? model(),
			dismissible: props.dismissible ?? true,
			onclose,
			onfeerefresh: props.onfeerefresh,
			onretry: props.onretry,
			onconfirm: props.onconfirm
		}
	});
	await tick();
	const root = document.body;
	const sheet = root.querySelector<HTMLElement>('[role="dialog"]')!;
	await entered(sheet);
	return {
		screen,
		onclose,
		sheet,
		scrim: root.querySelector<HTMLElement>('.scrim')!,
		close: sheet.querySelector<HTMLButtonElement>('header.header .close')
	};
}

describe('the signing sheet closes only on its ✕', () => {
	it('the scrim and Escape leave it open, and nothing is answered', async () => {
		const view = await drawn({});
		view.scrim.click();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		await pause(600);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(view.sheet.isConnected).toBe(true);
		await view.screen.unmount();
	});

	it('its header ✕, named, closes it once — the host’s refusal', async () => {
		const view = await drawn({});
		expect(view.close).not.toBeNull();
		expect(view.close!.getAttribute('aria-label')).toBe('Close');
		view.close!.click();
		view.close!.click();
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await pause(300);
		expect(view.onclose).toHaveBeenCalledOnce();
		await view.screen.unmount();
	});

	it('while the signature is in flight the ✕ is drawn shut', async () => {
		const view = await drawn({ dismissible: false });
		expect(view.close!.disabled).toBe(true);
		view.close!.click();
		await pause(600);
		expect(view.onclose).not.toHaveBeenCalled();
		await view.screen.unmount();
	});

	it('a refused request offers one way out — its labelled Close, no second ✕', async () => {
		const view = await drawn({ model: model({ dismissOnly: 'Close' }) });
		expect(view.close).toBeNull();
		await view.screen.unmount();
	});

	it('the header says the site once when its name is its host', async () => {
		const view = await drawn({});
		expect(view.sheet.querySelector('.header .name')?.textContent).toBe('app.example');
		expect(view.sheet.querySelector('.header .host')).toBeNull();
		await view.screen.unmount();
	});
});

describe('after the approval the sheet is a status (spec 079, F11)', () => {
	const submitting = {
		stage: 'submitting' as const,
		title: 'Submitting to network...',
		captions: ['Send · -0.0001 xDAI', 'Closing this page keeps the transaction running'],
		closable: true
	};

	it('the form gives way to the status: no fee, no confirm — never a greyed one', async () => {
		const CONFIRM = '[data-testid="signing-confirm"]';
		// The control, before the approval: the confirm is there.
		const form = await drawn({});
		expect(form.sheet.querySelector(CONFIRM)).not.toBeNull();
		await form.screen.unmount();

		const view = await drawn({
			model: model({
				fee: {
					kind: 'onchain',
					label: 'Network fee',
					value: '0.0001 xDAI',
					refreshLabel: 'Refresh fee'
				},
				status: submitting
			})
		});
		const status = view.sheet.querySelector<HTMLElement>('[data-testid="signing-status"]');
		expect(status?.textContent).toContain('Submitting to network...');
		expect(status?.textContent).toContain('Closing this page keeps the transaction running');
		expect(view.sheet.querySelector(CONFIRM)).toBeNull();
		expect(view.sheet.querySelector('button.refresh')).toBeNull();
		expect(view.sheet.textContent).not.toContain('Network fee');
		await view.screen.unmount();
	});

	it('a failure that sent nothing: Close answers, Try again goes back (spec 096 F8)', async () => {
		const onretry = vi.fn();
		const failed = {
			stage: 'failed' as const,
			title: 'Failed',
			captions: [
				'Swap · \u22120.003 BNB',
				'The transaction couldn’t be submitted. Your funds are safe — please try again.'
			],
			closable: true,
			actions: { close: 'Done', retry: 'Try Again' }
		};
		const view = await drawn({ model: model({ status: failed }), onretry });
		const actions = view.sheet.querySelector<HTMLElement>('[data-testid="signing-status-actions"]');
		const buttons = [...(actions?.querySelectorAll('button') ?? [])];
		expect(buttons.map((b) => b.textContent?.trim())).toEqual(['Done', 'Try Again']);
		buttons[1].click();
		expect(onretry).toHaveBeenCalledOnce();
		expect(view.onclose).not.toHaveBeenCalled();
		buttons[0].click();
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();

		// A refusal: one way out, no Try again.
		const refused = await drawn({
			model: model({ status: { ...failed, actions: { close: 'Done' } } }),
			onretry
		});
		const only = refused.sheet.querySelectorAll('[data-testid="signing-status-actions"] button');
		expect([...only].map((b) => b.textContent?.trim())).toEqual(['Done']);
		await refused.screen.unmount();
	});

	it('not sent yet (PR 2 polish): the waiting disc, never the failure’s — with Close and Try again', async () => {
		const onretry = vi.fn();
		const view = await drawn({
			model: model({
				status: {
					stage: 'not_sent',
					title: 'Not sent yet',
					captions: [
						'Send · −1 USDC',
						'Your previous transaction on this network is still being processed. Try again once it’s done.'
					],
					closable: true,
					actions: { close: 'Done', retry: 'Try Again' }
				}
			}),
			onretry
		});
		const status = view.sheet.querySelector<HTMLElement>('[data-testid="signing-status"]')!;
		expect(status.textContent).toContain('Not sent yet');
		expect(status.textContent).not.toContain('Failed');
		const disc = status.querySelector<HTMLElement>('.disc')!;
		expect(disc.classList.contains('not_sent')).toBe(true);
		expect(disc.classList.contains('failed')).toBe(false);
		const probe = document.createElement('span');
		probe.style.color = 'var(--color-error-base)';
		document.body.appendChild(probe);
		expect(getComputedStyle(disc).color).not.toBe(getComputedStyle(probe).color);
		probe.remove();
		const buttons = [
			...view.sheet.querySelectorAll<HTMLButtonElement>(
				'[data-testid="signing-status-actions"] button'
			)
		];
		expect(buttons.map((b) => b.textContent?.trim())).toEqual(['Done', 'Try Again']);
		buttons[1].click();
		expect(onretry).toHaveBeenCalledOnce();
		await view.screen.unmount();
	});

	it('closable: the ✕ is live and closes (the host makes it a plain close)', async () => {
		const view = await drawn({ model: model({ status: submitting }) });
		expect(view.close!.disabled).toBe(false);
		view.close!.click();
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('waiting for the passkey: the ✕ is shut (the host passes dismissible=false)', async () => {
		const view = await drawn({
			model: model({
				status: {
					stage: 'submitting',
					title: 'Waiting for biometric...',
					captions: [],
					closable: false
				}
			}),
			dismissible: false
		});
		expect(view.sheet.textContent).toContain('Waiting for biometric...');
		expect(view.close!.disabled).toBe(true);
		await view.screen.unmount();
	});
});

/**
 * Issue 461: the sheet confirms with a tap — the shared primary button the
 * Send screen confirms with, full width, its label the action alone. It used
 * to be a slide whose label read "Slide to confirm · …".
 */
describe('the confirm is a button (issue 461)', () => {
	const confirmOf = (sheet: HTMLElement) =>
		sheet.querySelector<HTMLButtonElement>('button[data-testid="signing-confirm"]');

	it('says the action alone, fills the footer, and one tap confirms', async () => {
		const onconfirm = vi.fn();
		const view = await drawn({ onconfirm });
		const confirm = confirmOf(view.sheet)!;
		expect(confirm).not.toBeNull();
		expect(confirm.textContent?.trim()).toBe('Send');
		expect(confirm.classList.contains('primary')).toBe(true);
		expect(confirm.disabled).toBe(false);
		// The rows' column: the signing account's row, above it in the body.
		const column = view.sheet.querySelector('.footer') as HTMLElement;
		expect(
			Math.abs(confirm.getBoundingClientRect().width - column.getBoundingClientRect().width)
		).toBeLessThanOrEqual(1);
		confirm.click();
		expect(onconfirm).toHaveBeenCalledOnce();
		expect(view.onclose).not.toHaveBeenCalled();
		await view.screen.unmount();
	});

	it('is shut while the core’s gate is, and says why under it', async () => {
		const onconfirm = vi.fn();
		const view = await drawn({
			model: model({ confirm: { action: 'Send', enabled: false, note: 'Getting the fee…' } }),
			onconfirm
		});
		const confirm = confirmOf(view.sheet)!;
		expect(confirm.disabled).toBe(true);
		confirm.click();
		expect(onconfirm).not.toHaveBeenCalled();
		const note = view.sheet.querySelector('.confirm-note');
		expect(note?.textContent).toBe('Getting the fee…');
		expect(note!.getBoundingClientRect().top).toBeGreaterThanOrEqual(
			confirm.getBoundingClientRect().bottom
		);
		await view.screen.unmount();
	});

	// The side panel's column (360 − 2 × 24): the longest action in the corpus
	// wraps inside the button, which grows; nothing is cut or spills.
	it('wraps a long action inside itself in the side panel’s width', async () => {
		const view = await drawn({
			model: model({ confirm: { action: 'Резервное копирование открытых ключей', enabled: true } })
		});
		const confirm = confirmOf(view.sheet)!;
		(confirm.closest('[data-signing-foot]') as HTMLElement).style.width = '312px';
		await tick();
		expect(confirm.scrollWidth).toBeLessThanOrEqual(confirm.clientWidth);
		expect(confirm.getBoundingClientRect().width).toBeLessThanOrEqual(312.5);
		await view.screen.unmount();
	});
});

describe('the signing fee row (spec 079 US2)', () => {
	const shown: FeeModel = {
		kind: 'onchain',
		label: 'Network fee',
		value: '0.0001 xDAI',
		tappable: false,
		refreshLabel: 'Refresh fee',
		refreshing: false,
		chevron: false
	};

	it('carries the send form’s refresh control, and a tap asks again', async () => {
		const asked = vi.fn();
		const view = await drawn({ model: model({ fee: shown }), onfeerefresh: asked });
		const refresh = view.sheet.querySelector<HTMLButtonElement>('button.refresh')!;
		expect(refresh.getAttribute('aria-label')).toBe('Refresh fee');
		expect(refresh.disabled).toBe(false);
		refresh.click();
		expect(asked).toHaveBeenCalledOnce();
		await view.screen.unmount();
	});

	it('turns, and refuses a second tap, while a measurement is out', async () => {
		const asked = vi.fn();
		const view = await drawn({
			model: model({ fee: { ...shown, value: 'Estimating…', refreshing: true } }),
			onfeerefresh: asked
		});
		const refresh = view.sheet.querySelector<HTMLButtonElement>('button.refresh')!;
		expect(refresh.disabled).toBe(true);
		expect(refresh.querySelector('.turn')?.classList.contains('spinning')).toBe(true);
		await view.screen.unmount();
	});

	it('keeps the stale note’s line standing, so the confirm never moves when it speaks', async () => {
		const quiet = await drawn({ model: model({ fee: shown }), onfeerefresh: () => {} });
		const quietStale = quiet.sheet.querySelector<HTMLElement>('.stale')!;
		expect(getComputedStyle(quietStale).visibility).toBe('hidden');
		const quietHeight = quietStale.getBoundingClientRect().height;
		expect(quietHeight).toBeGreaterThan(0);
		await quiet.screen.unmount();

		const loud = await drawn({
			model: model({ fee: { ...shown, staleNote: 'Measured a while ago' } }),
			onfeerefresh: () => {}
		});
		const loudStale = loud.sheet.querySelector<HTMLElement>('.stale')!;
		expect(loudStale.textContent).toContain('Measured a while ago');
		expect(Math.abs(loudStale.getBoundingClientRect().height - quietHeight)).toBeLessThanOrEqual(1);
		await loud.screen.unmount();
	});

	it('a failed quote says why, and the row is a retry with no chevron (one coin)', async () => {
		const view = await drawn({
			model: model({
				fee: {
					...shown,
					value: 'Tap to retry',
					tappable: true,
					warning: 'Couldn’t reach Vela — check your connection. We’ll retry automatically.'
				}
			}),
			onfeerefresh: () => {}
		});
		const row = view.sheet.querySelector<HTMLButtonElement>('button.row')!;
		expect(row).not.toBeNull();
		expect(row.querySelector('svg')).toBeNull();
		expect(view.sheet.querySelector('.warning')?.textContent).toContain('Couldn’t reach Vela');
		await view.screen.unmount();
	});
});

// Issue 314, then the first-party header (2026-10-08): the wallet's own key
// backup has no requester. Its header used to repeat the wallet's mark, "Vela
// Wallet" and the network chip over a headline that said the same thing again.
// Now the header IS the headline, beside the ✕ — the sheet's only exit, so it
// stays in every mode; a site's sheet keeps its requester row and the eyebrow
// its drawn scenarios were built around.
describe('the wallet’s own request (first-party)', () => {
	const BACKUP = "Copy this wallet's record";
	const own = (over: Partial<SigningModel> = {}) =>
		model({
			dapp: { name: 'Vela Wallet', host: '', letter: 'V', tint: 'var(--color-fg-muted)' },
			headline: { text: BACKUP, tone: 'success' },
			blocks: [
				{
					kind: 'rows',
					rows: [
						{ label: 'Network', value: 'Ethereum' },
						{ label: 'Public keys', value: '1' }
					]
				}
			],
			...over
		});
	const size = (el: Element) => parseFloat(getComputedStyle(el).fontSize);
	const box = (el: Element) => el.getBoundingClientRect();

	it('is one row: the headline and the ✕ — no mark, no name, no network chip', async () => {
		const view = await drawn({ model: own() });
		const header = view.sheet.querySelector<HTMLElement>('header.header')!;
		const headline = header.querySelector<HTMLElement>('.headline')!;
		expect(headline.textContent).toBe(BACKUP);
		for (const gone of ['.site', '.who', '.name', '.network']) {
			expect(header.querySelector(gone), gone).toBeNull();
		}
		expect(header.textContent).not.toContain('Vela Wallet');
		expect(header.textContent).not.toContain('Ethereum');
		// Side by side, on one line: the ✕ at the end of the headline's row.
		expect(view.close).not.toBeNull();
		const [h, x] = [box(headline), box(view.close!)];
		expect(x.left).toBeGreaterThanOrEqual(h.right - 1);
		expect(Math.abs((h.top + h.bottom) / 2 - (x.top + x.bottom) / 2)).toBeLessThanOrEqual(2);
		// In the title's ink and type, larger than any site's eyebrow.
		const probe = document.createElement('span');
		probe.style.color = 'var(--color-fg-base)';
		document.body.appendChild(probe);
		expect(getComputedStyle(headline).color).toBe(getComputedStyle(probe).color);
		probe.remove();
		// The intent is not said a second time below it.
		expect(view.sheet.querySelector('.intent')).toBeNull();
		await view.screen.unmount();
	});

	it('keeps its ✕ when the sheet is a status', async () => {
		const view = await drawn({
			model: own({
				status: { stage: 'submitting', title: 'Submitting…', captions: [], closable: true }
			})
		});
		expect(view.sheet.querySelector('.headline')?.textContent).toBe(BACKUP);
		expect(view.close).not.toBeNull();
		view.close!.click();
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('wraps a long headline beside the ✕ instead of cutting it', async () => {
		const LONG = 'Резервное копирование открытых ключей в сеть Ethereum';
		const view = await drawn({ model: own({ headline: { text: LONG, tone: 'neutral' } }) });
		const headline = view.sheet.querySelector<HTMLElement>('.headline')!;
		expect(headline.textContent).toBe(LONG);
		expect(headline.scrollWidth).toBeLessThanOrEqual(headline.clientWidth);
		expect(box(view.close!).left).toBeGreaterThanOrEqual(box(headline).right - 1);
		await view.screen.unmount();
	});

	it('a site’s request keeps its requester row and the small eyebrow', async () => {
		const site = await drawn({
			model: model({
				blocks: [
					{ kind: 'intent', text: BACKUP, tone: 'success' },
					{ kind: 'rows', rows: [{ label: 'Public keys', value: '1' }] }
				]
			})
		});
		expect(site.sheet.querySelector('.headline')).toBeNull();
		expect(site.sheet.querySelector('.header .name')?.textContent).toBe('app.example');
		expect(site.sheet.querySelector('.header .network')).not.toBeNull();
		const eyebrow = site.sheet.querySelector<HTMLElement>('.intent')!;
		expect(eyebrow.textContent).toBe(BACKUP);
		const eyebrowSize = size(eyebrow);
		await site.screen.unmount();

		const ownView = await drawn({ model: own() });
		expect(eyebrowSize).toBeLessThan(size(ownView.sheet.querySelector('.headline')!));
		await ownView.screen.unmount();
	});
});

/**
 * Nothing above moves while the fee is measured.
 *
 * On the Android device the whole sheet moved about 33 px on every fee
 * refresh, speed change and 30 s re-quote: the sheet is bottom-anchored and
 * wraps its content, and two lines came and went with the measurement — the
 * confirm's note under the button ("Working out the network fee…") and the
 * shortfall line under the fee card. The web drew both the same way. Each
 * sequence below is what the builder hands the sheet as a quote lands and is
 * asked again; the header, the blocks and the fee card must sit where they sat.
 */
describe('a fee being measured moves nothing above it', () => {
	const MEASURING = 'Working out the network fee…';
	const NO_COIN = 'None of your coins can pay this fee';
	const landed: FeeModel = {
		kind: 'onchain',
		label: 'Network fee',
		value: '0.0021 ETH · ≈ $6.30',
		tappable: false,
		refreshLabel: 'Refresh fee',
		refreshing: false,
		chevron: false
	};
	/** The 30 s re-quote: the figure stays, the gate shuts and says why. */
	const requote: FeeModel = { ...landed, refreshing: true };
	/** A new speed: no figure of its own yet. */
	const newSpeed: FeeModel = { ...landed, value: 'Estimating…', refreshing: true };
	const open = { action: 'Confirm', enabled: true };
	const shut = { action: 'Confirm', enabled: false, note: MEASURING };

	/** Where the parts above the fee's own lines sit, in one frame. */
	const frame = (sheet: HTMLElement) => ({
		header: sheet.querySelector('header.header')!.getBoundingClientRect().top,
		blocks: sheet.querySelector('.blocks')!.getBoundingClientRect().top,
		fee: sheet.querySelector('.line')!.getBoundingClientRect().top,
		confirm: sheet.querySelector('[data-testid="signing-confirm"]')!.getBoundingClientRect().top
	});

	async function walk(steps: Partial<SigningModel>[]) {
		const view = await drawn({ model: model(steps[0]), onfeerefresh: () => {} });
		const frames = [];
		for (const step of steps) {
			await view.screen.rerender({ model: model(step) });
			await tick();
			await pause(50);
			frames.push(frame(view.sheet));
		}
		await view.screen.unmount();
		return frames;
	}

	it('a funded wallet: measured at open, landed, re-quoted, a new speed, landed', async () => {
		const frames = await walk([
			{ fee: newSpeed, confirm: shut },
			{ fee: landed, confirm: open },
			{ fee: requote, confirm: shut },
			{ fee: landed, confirm: open },
			{ fee: newSpeed, confirm: shut },
			{ fee: landed, confirm: open }
		]);
		for (const at of frames.slice(1)) expect(at).toEqual(frames[0]);
	});

	it('a wallet no coin of which can pay: the shortfall line keeps its height while measured', async () => {
		const short: FeeModel = { ...landed, warning: NO_COIN };
		const frames = await walk([
			{ fee: newSpeed, confirm: shut },
			{ fee: short, confirm: { action: 'Confirm', enabled: false } },
			{ fee: requote, confirm: shut },
			{ fee: short, confirm: { action: 'Confirm', enabled: false } },
			{ fee: newSpeed, confirm: shut },
			{ fee: short, confirm: { action: 'Confirm', enabled: false } }
		]);
		// From the first landing on (the first measurement had no shortfall
		// to hold yet), not a pixel.
		for (const at of frames.slice(2)) expect(at).toEqual(frames[1]);
	});

	it('a held line is invisible and silent, and goes when the fee lands with nothing to say', async () => {
		const short: FeeModel = { ...landed, warning: NO_COIN };
		const view = await drawn({ model: model({ fee: short, confirm: shut }) });
		await view.screen.rerender({ model: model({ fee: requote, confirm: open }) });
		await tick();
		const note = view.sheet.querySelector<HTMLElement>('.confirm-note')!;
		expect(note.textContent?.trim()).toBe(MEASURING);
		expect(getComputedStyle(note).visibility).toBe('hidden');
		expect(note.getAttribute('aria-hidden')).toBe('true');
		const held = view.sheet.querySelector<HTMLElement>('.warning')!;
		expect(held.textContent).toBe(NO_COIN);
		expect(getComputedStyle(held).visibility).toBe('hidden');
		expect(held.getAttribute('role')).toBeNull();
		// The coin can pay now: the shortfall is gone for good, not held.
		await view.screen.rerender({ model: model({ fee: landed, confirm: open }) });
		await tick();
		expect(view.sheet.querySelector('.warning')).toBeNull();
		await view.screen.unmount();
	});
});

/*
 * Device walk of 2026-10-11, item 2. On Android the sheet climbed twice as its
 * fee landed — 77 px a step, the ✕ moving under a finger. The web's sheet did
 * the same by smaller steps (`signing-still.e2e.ts` measured it on the real
 * machines: 16 px for the speed's note, 47 with "No token can pay this fee",
 * 27 for a failure's reason at 390 × 844; at 360 the fee row itself grew 20 px
 * when its money wrapped under the coin).
 *
 * The suite above starts its comparison at the first LANDING — "the first
 * measurement had no shortfall to hold yet". This one starts at the first
 * FRAME: the fee is being worked out and nothing has been said. The models
 * are what the live builder hands the sheet (`moneyLine`, `rooms`, the speed's
 * `freeRoom` — `live.test.ts` holds that it names them in every state), and
 * the same walk without them moves, by the measured steps.
 */
describe('the fee says its last word in room that was there from the first frame (item 2)', () => {
	const NO_COIN = 'No token can pay this fee';
	const SHORT = 'Not enough ETH for the network fee';
	const FREE = 'Fast costs no more on this network, so this send uses Fast.';
	const REASON = "Couldn't get a price from the relay. Retrying automatically.";
	const LONG_REASON = "The relay couldn't test-run this transaction yet. Retrying automatically.";
	const speed = (said: boolean, room: boolean) => ({
		label: 'Speed',
		value: 'Fast',
		open: false,
		onceNote: '',
		gasPriceLabel: 'Gas Bid',
		gasPriceLine: false,
		options: [],
		...(said ? { freeNote: FREE } : {}),
		...(room ? { freeRoom: FREE } : {})
	});
	/** The fee in each state, as the live builder draws it — or (`held: false`) as it used to. */
	const fees = (held: boolean) => {
		const room = held ? { moneyLine: true, rooms: [NO_COIN, SHORT] } : {};
		const base = { kind: 'onchain' as const, label: 'Network fee', refreshLabel: 'Refresh fee' };
		const priced = {
			...base,
			...room,
			value: '0.000825 ETH',
			valueFiat: '≈ $2.48',
			tappable: false,
			chevron: false,
			refreshing: false
		};
		const failed = (warning: string) => ({
			...base,
			...room,
			value: '—',
			tappable: true,
			chevron: false,
			refreshing: false,
			warning,
			speed: speed(false, held)
		});
		return {
			estimating: {
				...base,
				...room,
				value: 'Estimating...',
				tappable: false,
				chevron: false,
				refreshing: true,
				speed: speed(false, held)
			},
			priced: { ...priced, speed: speed(false, held) },
			'priced, with the speed’s note': { ...priced, speed: speed(true, held) },
			'no token can pay': { ...priced, warning: NO_COIN, speed: speed(true, held) },
			refused: failed(REASON),
			'a long reason': failed(LONG_REASON)
		} satisfies Record<string, FeeModel>;
	};
	const shut = { action: 'Confirm', enabled: false, note: 'Working out the network fee…' };
	const open = { action: 'Confirm', enabled: true };
	const top = (sheet: HTMLElement, selector: string) =>
		Math.round((sheet.querySelector(selector)?.getBoundingClientRect().top ?? NaN) * 100) / 100;
	const places = (sheet: HTMLElement) => ({
		sheet: Math.round(sheet.getBoundingClientRect().top * 100) / 100,
		header: top(sheet, '[data-signing-top]'),
		close: top(sheet, 'header.header .close'),
		blocks: top(sheet, '.blocks'),
		fee: top(sheet, '.footer > .line'),
		speed: top(sheet, '.footer > .speed'),
		signer: top(sheet, '.footer > .signer'),
		confirm: top(sheet, '[data-testid="signing-confirm"]')
	});

	afterAll(() => page.viewport(414, 896));

	/** The sheet with its fee being worked out; then each ending, each from that first frame. */
	async function walk(width: number, height: number, held: boolean) {
		await page.viewport(width, height);
		const states = fees(held);
		const moved: Record<string, Record<string, number>> = {};
		for (const [ending, fee] of Object.entries(states).slice(1)) {
			const view = await drawn({
				model: model({ fee: states.estimating, confirm: shut }),
				onfeerefresh: () => {}
			});
			const first = places(view.sheet);
			await view.screen.rerender({
				model: model({ fee, confirm: 'warning' in fee ? shut : open })
			});
			await tick();
			await pause(80);
			const last = places(view.sheet);
			moved[ending] = Object.fromEntries(
				Object.entries(first).map(([part, at]) => [
					part,
					Math.round(Math.abs(last[part as keyof typeof last] - at) * 100) / 100
				])
			);
			await view.screen.unmount();
		}
		return moved;
	}

	const STILL = {
		sheet: 0,
		header: 0,
		close: 0,
		blocks: 0,
		fee: 0,
		speed: 0,
		signer: 0,
		confirm: 0
	};

	it.each([
		['the phone sheet', 390, 844],
		['a short phone', 360, 640],
		['the side panel’s width', 320, 600],
		['the centred card', 1440, 900]
	] as const)('%s: nothing moves, whatever the fee comes to say', async (_name, width, height) => {
		const moved = await walk(width, height, true);
		for (const [ending, parts] of Object.entries(moved)) {
			expect(parts, `${ending} @ ${width} × ${height}`).toEqual(STILL);
		}
	});

	it('without the rooms the same walk moves the sheet’s top and its ✕ — what the phones showed', async () => {
		const moved = await walk(390, 844, false);
		// The speed's note alone: a line.
		expect(moved['priced, with the speed’s note'].close).toBeGreaterThan(10);
		// The shortfall and the note: two.
		expect(moved['no token can pay'].close).toBeGreaterThan(
			moved['priced, with the speed’s note'].close
		);
		expect(moved['no token can pay'].sheet).toBe(moved['no token can pay'].close);
		// A failure's reason: a line, less the 4 px a <button> row was shorter by.
		expect(moved.refused.close).toBeGreaterThan(10);
		// The confirm never did: it is pinned in the foot.
		for (const parts of Object.values(moved)) expect(parts.confirm).toBe(0);
	});

	it('a room is unseen and unsaid; what is said stands in its place, and is the only alert', async () => {
		await page.viewport(390, 844);
		const states = fees(true);
		const view = await drawn({ model: model({ fee: states.estimating, confirm: shut }) });
		const rooms = () => [...view.sheet.querySelectorAll<HTMLElement>('.said .room')];
		expect(rooms().map((room) => room.textContent)).toEqual([NO_COIN, SHORT]);
		for (const room of rooms()) {
			expect(getComputedStyle(room).visibility).toBe('hidden');
			expect(room.getAttribute('aria-hidden')).toBe('true');
			expect(room.getAttribute('role')).toBeNull();
		}
		expect(view.sheet.querySelector('[role="alert"]')).toBeNull();
		const noteRoom = view.sheet.querySelector<HTMLElement>('.speed .free.room')!;
		expect(noteRoom.textContent).toBe(FREE);
		expect(getComputedStyle(noteRoom).visibility).toBe('hidden');
		expect(noteRoom.getAttribute('aria-hidden')).toBe('true');
		const place = view.sheet.querySelector('.said')!.getBoundingClientRect();

		await view.screen.rerender({
			model: model({ fee: states['no token can pay'], confirm: shut })
		});
		await tick();
		const alerts = [...view.sheet.querySelectorAll<HTMLElement>('[role="alert"]')];
		expect(alerts.map((alert) => alert.textContent)).toEqual([NO_COIN]);
		// In the room that was kept: the same box, and the line inside it.
		const said = alerts[0].getBoundingClientRect();
		const now = view.sheet.querySelector('.said')!.getBoundingClientRect();
		expect([now.top, now.height]).toEqual([place.top, place.height]);
		expect(said.top).toBeGreaterThanOrEqual(now.top);
		expect(said.bottom).toBeLessThanOrEqual(now.bottom + 0.5);
		// The speed's note is said where its room was.
		const note = view.sheet.querySelector<HTMLElement>('.speed .free:not(.room)')!;
		expect(note.textContent).toBe(FREE);
		expect(view.sheet.querySelector('.speed .free.room')).toBeNull();
		await view.screen.unmount();
	});

	it('a failure’s long reason runs on into the blank line under it, whole, and touches nothing', async () => {
		await page.viewport(390, 844);
		const states = fees(true);
		const view = await drawn({ model: model({ fee: states['a long reason'], confirm: shut }) });
		const reason = view.sheet.querySelector<HTMLElement>('[role="alert"]')!;
		expect(reason.textContent).toBe(LONG_REASON);
		const box = reason.getBoundingClientRect();
		const place = view.sheet.querySelector('.said')!.getBoundingClientRect();
		// Longer than the room kept for a shortfall…
		expect(box.height).toBeGreaterThan(place.height + 4);
		// …so it runs on under it, over the standing line — which says nothing…
		const standing = view.sheet.querySelector<HTMLElement>('.footer .stale')!;
		expect(getComputedStyle(standing).visibility).toBe('hidden');
		// …and ends before the speed control.
		const speedTop = view.sheet.querySelector('.footer > .speed')!.getBoundingClientRect().top;
		expect(box.bottom).toBeLessThanOrEqual(speedTop + 0.5);
		// Nothing of it is cut.
		expect(reason.scrollHeight).toBeLessThanOrEqual(Math.ceil(box.height) + 1);
		await view.screen.unmount();
	});

	it('the row is one height whether it is a fact or a control: a failed fee is a <button>', async () => {
		await page.viewport(390, 844);
		const states = fees(true);
		const view = await drawn({ model: model({ fee: states.priced, confirm: open }) });
		const row = () => view.sheet.querySelector<HTMLElement>('.footer > .line .row')!;
		expect(row().tagName).toBe('DIV');
		const stated = row().getBoundingClientRect().height;
		await view.screen.rerender({ model: model({ fee: states.refused, confirm: shut }) });
		await tick();
		expect(row().tagName).toBe('BUTTON');
		expect(row().getBoundingClientRect().height).toBe(stated);
		await view.screen.unmount();
	});
});

/*
 * PR 3 final note F2, then the device round (item 1, security).
 *
 * The simulation's verdict is the one part of a signing sheet a site cannot
 * write, so no part of it may be hidden: every row and every line is drawn
 * whole, at its own height — nothing clipped, nothing scrolled inside a
 * block, no fold. And the confirm is never moved by it: it stands in the
 * sheet's foot, outside the scroll, with the line the core says under a shut
 * one. A sheet that no longer fits scrolls its BODY, under the header and
 * over the foot, and what landed is scrolled into sight.
 *
 * These are the blocks that land with NO place kept for them: the relay's
 * "will fail" line under the intent, and — on the wallet's own request — the
 * simulation's card after its rows. (A site's transaction keeps the
 * verdict's place from its first frame: the suite at the end of this file.)
 */
describe('a verdict is shown whole, and the confirm does not move (device round, item 1)', () => {
	const INTENT = { kind: 'intent', text: 'Send 1 xDAI', tone: 'neutral' } as const;
	const ROWS = {
		kind: 'rows',
		rows: Array.from({ length: 4 }, (_, i) => ({
			label: `Field ${i + 1}`,
			value: `value ${i + 1}`
		}))
	} as const;
	const WILL_FAIL = {
		kind: 'warning',
		tone: 'danger',
		text: 'This transaction is expected to fail — you’d still pay gas.',
		verdict: true
	} as const;
	/** The longest the relay's line gets: its sentence around a 64-character reason. */
	const LONGEST_FAIL =
		'This transaction is expected to fail: ERC20: transfer amount exceeds the allowance granted to it. You’d still pay gas.';
	const UNVERIFIED =
		'Amounts for unverified tokens are shown as the token reports them, and may not be what you receive.';
	type Blocks = SigningModel['blocks'];
	type Card = Extract<Blocks[number], { kind: 'balances' }>;
	const MOVES: Card['rows'] = [
		{ symbol: 'USDC', delta: '−1,250.00', tone: 'neutral' },
		{ symbol: 'WETH', delta: '+0.4312', tone: 'success' },
		{ symbol: 'SCAM-LP', delta: '+1,000,000', tone: 'caution' },
		{ symbol: 'DAI', delta: '−18.5', tone: 'neutral' }
	];
	const card = (rows: number, note?: string): Card => ({
		kind: 'balances',
		title: 'Balance changes',
		rows: MOVES.slice(0, rows),
		...(note ? { note } : {}),
		verdict: true
	});
	const NO_CHANGE = card(0, 'No asset changes');
	const TALL = card(4, UNVERIFIED);
	const fee: FeeModel = {
		kind: 'onchain',
		label: 'Network fee',
		value: '0.0021 ETH',
		valueFiat: '≈ $6.30',
		tappable: false,
		refreshLabel: 'Refresh fee',
		refreshing: false,
		chevron: false
	};
	/** The request's own blocks, then whatever has landed: a line under the intent, a card after them. */
	const sheetWith = (landed: { line?: string; card?: Blocks[number] } = {}) =>
		model({
			fee,
			blocks: [
				INTENT,
				...(landed.line ? [{ ...WILL_FAIL, text: landed.line }] : []),
				ROWS,
				...(landed.card ? [landed.card] : [])
			] as Blocks
		});

	const top = (el: Element) => Math.round(el.getBoundingClientRect().top * 10) / 10;
	const confirm = (sheet: HTMLElement) =>
		sheet.querySelector('[data-testid="signing-confirm"]') as HTMLElement;
	const scrollerOf = (sheet: HTMLElement) => sheet.querySelector('.content') as HTMLElement;
	const headerOf = (sheet: HTMLElement) => sheet.querySelector('[data-signing-top]') as HTMLElement;
	const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

	// The harness's own window, back as the other suites expect it.
	afterAll(() => page.viewport(414, 896));

	async function at(width: number, height: number, first: SigningModel = sheetWith()) {
		await page.viewport(width, height);
		return drawn({ model: first });
	}
	async function land(view: Awaited<ReturnType<typeof drawn>>, next: SigningModel) {
		await view.screen.rerender({ model: next });
		await tick();
		await frame();
		await frame();
	}

	/** Whole, on screen, and the thing a tap there reaches. */
	function expectConfirmWhole(sheet: HTMLElement): void {
		const box = confirm(sheet).getBoundingClientRect();
		expect(box.top).toBeGreaterThanOrEqual(0);
		expect(box.bottom).toBeLessThanOrEqual(window.innerHeight);
		expect(box.bottom).toBeLessThanOrEqual(sheet.getBoundingClientRect().bottom);
		// Outside the scroll: nothing the body holds can cover it or carry it off.
		expect(scrollerOf(sheet).contains(confirm(sheet))).toBe(false);
		const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
		expect(confirm(sheet).contains(hit)).toBe(true);
	}

	/**
	 * The card is as tall as what it holds, and nothing between a row and the
	 * sheet's one scroll cuts or scrolls it.
	 */
	function expectCardWhole(sheet: HTMLElement, rows: number): HTMLElement {
		const section = sheet.querySelector('section.balances') as HTMLElement;
		const parts = [...section.children] as HTMLElement[];
		// A title, each row, and the note.
		expect(section.querySelectorAll('.row')).toHaveLength(rows);
		expect(parts).toHaveLength(rows + 2);
		const style = getComputedStyle(section);
		const chrome =
			parseFloat(style.paddingTop) +
			parseFloat(style.paddingBottom) +
			parseFloat(style.borderTopWidth) +
			parseFloat(style.borderBottomWidth);
		const content = parts.reduce((sum, part) => sum + part.getBoundingClientRect().height, 0);
		expect(Math.abs(section.getBoundingClientRect().height - (content + chrome))).toBeLessThan(1);
		expect(section.scrollHeight).toBeLessThanOrEqual(section.clientHeight);
		expect(section.scrollWidth).toBeLessThanOrEqual(section.clientWidth);
		for (const part of parts) {
			expect(part.getBoundingClientRect().height).toBeGreaterThan(0);
			expect(part.scrollHeight).toBeLessThanOrEqual(part.clientHeight + 1);
			expect(part.scrollWidth).toBeLessThanOrEqual(part.clientWidth + 1);
		}
		// No scroll, clip or height of its own on the way up to the body.
		const scroller = scrollerOf(sheet);
		for (let el: HTMLElement | null = section; el && el !== scroller; el = el.parentElement) {
			const own = getComputedStyle(el);
			expect(own.overflowY, el.className).toBe('visible');
			expect(own.maxHeight, el.className).toBe('none');
		}
		expect(scroller.contains(section)).toBe(true);
		return section;
	}

	/** In the body's window: under the header, over the foot. */
	function inSight(sheet: HTMLElement, el: Element): boolean {
		const box = el.getBoundingClientRect();
		return (
			box.top >= headerOf(sheet).getBoundingClientRect().bottom - 0.5 &&
			box.bottom <= scrollerOf(sheet).getBoundingClientRect().bottom + 0.5
		);
	}

	const SCREENS = [
		['the phone sheet', 390, 844],
		['the extension panel', 360, 640],
		['the centred card', 1400, 900]
	] as const;

	it.each(SCREENS)(
		'%s: four rows and the unverified note are whole, and the confirm is where it was with one row and with none',
		async (_name, width, height) => {
			const view = await at(width, height);
			const none = top(confirm(view.sheet));
			expectConfirmWhole(view.sheet);

			await land(view, sheetWith({ card: card(1) }));
			expect(top(confirm(view.sheet))).toBe(none);

			await land(view, sheetWith({ card: TALL }));
			const section = expectCardWhole(view.sheet, 4);
			expect(section.textContent).toContain(UNVERIFIED);
			expect(top(confirm(view.sheet))).toBe(none);
			expectConfirmWhole(view.sheet);
			// It landed in sight, whole: nobody has to look for it.
			expect(inSight(view.sheet, section)).toBe(true);
			await view.screen.unmount();
		}
	);

	it.each([
		['the phone sheet', 390, 520],
		['the extension panel', 360, 480],
		['the centred card', 1400, 560]
	] as const)(
		'%s on a screen too short for it: the body scrolls, every row can be read, the confirm has not moved',
		async (_name, width, height) => {
			const view = await at(width, height);
			const scroller = scrollerOf(view.sheet);
			const none = top(confirm(view.sheet));
			expectConfirmWhole(view.sheet);

			await land(view, sheetWith({ card: TALL }));
			const section = expectCardWhole(view.sheet, 4);
			// The sheet is as tall as it may be: its body scrolls, the card does not.
			expect(scroller.scrollHeight).toBeGreaterThan(scroller.clientHeight);
			expect(top(confirm(view.sheet))).toBe(none);
			expectConfirmWhole(view.sheet);
			// What landed was scrolled into sight: all of it when the body's
			// window can hold it, else from its top, under the header.
			const header = headerOf(view.sheet).getBoundingClientRect();
			const room = scroller.getBoundingClientRect().bottom - header.bottom;
			if (section.getBoundingClientRect().height <= room) {
				expect(inSight(view.sheet, section)).toBe(true);
			} else {
				expect(Math.abs(section.getBoundingClientRect().top - header.bottom)).toBeLessThan(1);
			}
			expect(scroller.scrollTop).toBeGreaterThan(0);
			// Who is asking, and the ✕, are still at the top of the sheet.
			expect(header.top).toBeGreaterThanOrEqual(scroller.getBoundingClientRect().top - 0.5);

			// Each row and the note can be brought fully into view, and reading
			// them moves no confirm.
			for (const part of [...section.children]) {
				const box = part.getBoundingClientRect();
				scroller.scrollTop += box.top - headerOf(view.sheet).getBoundingClientRect().bottom;
				if (!inSight(view.sheet, part)) {
					scroller.scrollTop +=
						part.getBoundingClientRect().bottom - scroller.getBoundingClientRect().bottom;
				}
				expect(inSight(view.sheet, part), part.textContent ?? '').toBe(true);
				expect(part.getBoundingClientRect().height).toBeLessThanOrEqual(room);
				expect(top(confirm(view.sheet))).toBe(none);
			}
			expectConfirmWhole(view.sheet);

			// The card goes (a new request's sheet has none): the confirm is still there.
			await land(view, sheetWith());
			expect(top(confirm(view.sheet))).toBe(none);
			await view.screen.unmount();
		}
	);

	it.each([...SCREENS, ['a short phone', 320, 568], ['a short card', 1400, 600]] as const)(
		'%s: the common verdicts move no confirm',
		async (_name, width, height) => {
			const view = await at(width, height);
			const none = top(confirm(view.sheet));
			const landings: [string, SigningModel][] = [
				['No asset changes', sheetWith({ card: NO_CHANGE })],
				['one row', sheetWith({ card: card(1) })],
				['two rows', sheetWith({ card: card(2) })],
				['will fail', sheetWith({ line: WILL_FAIL.text })],
				['the longest will-fail line', sheetWith({ line: LONGEST_FAIL })],
				['will fail, and nothing moves', sheetWith({ line: WILL_FAIL.text, card: NO_CHANGE })],
				['none again', sheetWith()]
			];
			for (const [name, next] of landings) {
				await land(view, next);
				expect(top(confirm(view.sheet)), name).toBe(none);
				expectConfirmWhole(view.sheet);
				// Whatever landed last is in sight.
				const landed = [...view.sheet.querySelectorAll('[data-verdict]')];
				if (name === 'will fail, and nothing moves') expect(landed).toHaveLength(2);
			}
			await view.screen.unmount();
		}
	);

	it('the phone sheet grows upward for what lands, and gives the room back', async () => {
		const view = await at(390, 844);
		const was = top(confirm(view.sheet));
		const sheetTop = top(view.sheet);
		await land(view, sheetWith({ line: WILL_FAIL.text }));
		const line = view.sheet.querySelector('[data-verdict]') as HTMLElement;
		expect(line.textContent).toContain('expected to fail');
		expect(top(confirm(view.sheet))).toBe(was);
		// The sheet made the room above itself.
		expect(top(view.sheet)).toBeLessThan(sheetTop - 20);
		// …and gives it back when a new estimate takes the line away.
		await land(view, sheetWith());
		expect(top(confirm(view.sheet))).toBe(was);
		expect(top(view.sheet)).toBe(sheetTop);
		await view.screen.unmount();
	});

	it('the centred card grows upward only — not from its middle — and comes back to its middle', async () => {
		const view = await at(1400, 900);
		const was = top(confirm(view.sheet));
		const cardTop = top(view.sheet);
		const cardHeight = view.sheet.getBoundingClientRect().height;
		await land(view, sheetWith({ line: WILL_FAIL.text }));
		const grown = view.sheet.getBoundingClientRect().height - cardHeight;
		expect(grown).toBeGreaterThan(20);
		expect(top(confirm(view.sheet))).toBe(was);
		// All of the growth went up (from its middle, half of it would have).
		expect(Math.abs(cardTop - top(view.sheet) - grown)).toBeLessThanOrEqual(1);

		// A longer reason: the line is taller, the confirm still does not move.
		await land(view, sheetWith({ line: LONGEST_FAIL }));
		expect(top(confirm(view.sheet))).toBe(was);

		// The line goes: the card is where it started.
		await land(view, sheetWith());
		expect(top(confirm(view.sheet))).toBe(was);
		expect(Math.abs(top(view.sheet) - cardTop)).toBeLessThanOrEqual(0.5);
		await view.screen.unmount();
	});

	it('a card taller than the window allows keeps its margin: the body scrolls, the foot is held', async () => {
		// Where the card's top stands when it is as tall as it may be.
		const tallest = await at(1400, 700, sheetWith({ line: LONGEST_FAIL, card: TALL }));
		const highest = tallest.sheet.getBoundingClientRect().top;
		expect(highest).toBeGreaterThan(0);
		await tallest.screen.unmount();

		const view = await at(1400, 700);
		const was = top(confirm(view.sheet));
		const scroller = scrollerOf(view.sheet);
		expect(scroller.scrollHeight).toBeLessThanOrEqual(scroller.clientHeight);
		await land(view, sheetWith({ line: LONGEST_FAIL, card: TALL }));
		expect(top(confirm(view.sheet))).toBe(was);
		expectConfirmWhole(view.sheet);
		// Its top stops where the tallest card's does, and no higher…
		expect(Math.abs(view.sheet.getBoundingClientRect().top - highest)).toBeLessThan(1);
		// …and the rest is the body's to scroll.
		expect(scroller.scrollHeight).toBeGreaterThan(scroller.clientHeight);
		expectCardWhole(view.sheet, 4);
		await view.screen.unmount();
	});

	it('a verdict that lands under the fold is scrolled into sight; one already in sight moves nothing', async () => {
		// The body already scrolls, and the person is reading its top.
		const view = await at(390, 520);
		const scroller = scrollerOf(view.sheet);
		expect(scroller.scrollHeight).toBeGreaterThan(scroller.clientHeight);
		expect(scroller.scrollTop).toBe(0);

		// Under the intent, in sight: nothing is scrolled for it.
		await land(view, sheetWith({ line: WILL_FAIL.text }));
		const line = view.sheet.querySelector('[data-verdict]') as HTMLElement;
		expect(inSight(view.sheet, line)).toBe(true);
		expect(scroller.scrollTop).toBe(0);

		// After the request's own blocks, under the fold: brought into sight.
		await land(view, sheetWith({ line: WILL_FAIL.text, card: NO_CHANGE }));
		const landed = view.sheet.querySelector('section.balances') as HTMLElement;
		expect(landed.textContent).toContain('No asset changes');
		expect(scroller.scrollTop).toBeGreaterThan(0);
		expect(inSight(view.sheet, landed)).toBe(true);
		await view.screen.unmount();
	});

	it('two that land together are both brought into sight; when the window cannot hold both, the danger line is', async () => {
		const both = sheetWith({ line: WILL_FAIL.text, card: NO_CHANGE });
		const outcomes: string[] = [];
		// From a window that holds everything down to one that holds neither whole.
		for (let height = 760; height >= 400; height -= 40) {
			const view = await at(390, height);
			const scroller = scrollerOf(view.sheet);
			const was = top(confirm(view.sheet));
			await land(view, both);
			const [line, landedCard] = [...view.sheet.querySelectorAll('[data-verdict]')];
			expect(landedCard.textContent).toContain('No asset changes');
			expect(top(confirm(view.sheet))).toBe(was);
			const window =
				scroller.getBoundingClientRect().bottom -
				headerOf(view.sheet).getBoundingClientRect().bottom;
			const span = landedCard.getBoundingClientRect().bottom - line.getBoundingClientRect().top;
			const scrolled = scroller.scrollTop > 0;
			if (span <= window) {
				// They fit together: both are whole, in sight.
				expect(inSight(view.sheet, line), `line at ${height}`).toBe(true);
				expect(inSight(view.sheet, landedCard), `card at ${height}`).toBe(true);
				outcomes.push(scrolled ? 'both, scrolled' : 'both');
			} else {
				// They do not: the body ends on the danger line.
				expect(inSight(view.sheet, line), `line at ${height}`).toBe(true);
				outcomes.push('line');
			}
			await view.screen.unmount();
		}
		// Each case was met — not one of them passed for want of a window.
		expect(outcomes).toContain('both');
		expect(outcomes).toContain('both, scrolled');
		expect(outcomes).toContain('line');
	});

	it('a long figure wraps inside the card instead of running out of it', async () => {
		const view = await at(320, 700);
		await land(
			view,
			sheetWith({
				card: {
					kind: 'balances',
					title: 'Balance changes',
					rows: [
						{
							symbol: 'AVERYLONGTOKENSYMBOLTHATNEVERENDS',
							delta: '+115,792,089,237,316,195,423,570,985,008,687,907,853,269.984665',
							tone: 'caution'
						}
					],
					note: UNVERIFIED,
					verdict: true
				}
			})
		);
		const section = expectCardWhole(view.sheet, 1);
		const scroller = scrollerOf(view.sheet);
		expect(scroller.scrollWidth).toBeLessThanOrEqual(scroller.clientWidth);
		for (const cell of section.querySelectorAll('.symbol, .delta')) {
			const box = cell.getBoundingClientRect();
			expect(box.left).toBeGreaterThanOrEqual(section.getBoundingClientRect().left);
			expect(box.right).toBeLessThanOrEqual(section.getBoundingClientRect().right);
		}
		await view.screen.unmount();
	});

	it('a refused request’s way out stands in the foot too; a status and a hand-off have none', async () => {
		const refused = await at(390, 844, model({ dismissOnly: 'Close' }));
		const out = refused.sheet.querySelector('[data-signing-foot] .dismiss button') as HTMLElement;
		expect(out.textContent?.trim()).toBe('Close');
		expect(scrollerOf(refused.sheet).contains(out)).toBe(false);
		await refused.screen.unmount();

		const status = await at(
			390,
			844,
			model({ status: { stage: 'submitting', title: 'Submitting…', captions: [], closable: true } })
		);
		expect(status.sheet.querySelector('[data-signing-foot]')).toBeNull();
		await status.screen.unmount();
	});
});

/*
 * The simulation verdict's PLACE (the web signing verdict).
 *
 * A site's transaction keeps a place for the verdict from its first frame —
 * the balance card saying "Checking…" — as tall as the verdicts a sheet
 * usually ends on. So a verdict that lands a moment after the sheet opened
 * moves nothing: not the sheet's own top, its header and the ✕ that refuses
 * (the phone sheet is anchored at the bottom: a card that ARRIVED pushed all
 * of that up by its height), not the fee row, the signing account and the
 * confirm under it.
 *
 * A verdict taller than the place — four balance rows and the warning under
 * an unverified token — is drawn whole: the place grows, nothing in it is
 * clipped or scrolled, and the confirm is still pinned and whole.
 *
 * The rooms are the real builder's (`verdictRooms`, the core's longest revert
 * reason among them); the words are the corpus's English.
 */
describe('the verdict’s place: there from the first frame, and nothing moves for what lands in it', () => {
	const WORDS: VerdictWords = {
		balancesTitle: 'Balance changes',
		simChecking: 'Checking…',
		simSaid: { 'componentsUi.signing.simResultNoChange': 'No asset changes' },
		balanceUnverifiedToken: 'Unverified token',
		warnUnverifiedAmount:
			'Token amount couldn’t be verified on-chain — double-check it before approving.',
		warnSimUnavailable:
			'Vela couldn’t check what this transaction does. Review it before you sign.',
		warnWillFail: 'This transaction is expected to fail — you’d still pay gas.',
		warnWillFailReason: 'Expected to fail: {{reason}} — you’d still pay gas.'
	};
	type Blocks = SigningModel['blocks'];
	type Said = Extract<Blocks[number], { kind: 'balances' | 'warning' }>;
	type Card = Extract<Said, { kind: 'balances' }>;
	const card = (rows: Card['rows'], note?: string): Card => ({
		kind: 'balances',
		title: WORDS.balancesTitle,
		rows,
		...(note ? { note, noteTone: 'caution' as const } : {})
	});
	const CHECKING: Said = { ...card([]), note: WORDS.simChecking };
	const NO_CHANGE: Said = { ...card([]), note: 'No asset changes' };
	const SEND = card([{ symbol: 'USDC', delta: '−10', tone: 'neutral' }]);
	const SWAP = card([
		{ symbol: 'USDC', delta: '−1,250.00', tone: 'neutral' },
		{ symbol: 'ETH', delta: '+0.4312', tone: 'success' }
	]);
	/** Four rows, one of them a token nobody vouches for, and the warning under them. */
	const TALL = card(
		[
			{ symbol: 'USDC', delta: '−1,250.00', tone: 'neutral' },
			{ symbol: 'ETH', delta: '+0.4312', tone: 'success' },
			{ symbol: 'Unverified token', delta: '+', tone: 'caution' },
			{ symbol: 'DAI', delta: '−18.5', tone: 'neutral' }
		],
		WORDS.warnUnverifiedAmount
	);
	const CAUTION: Said = { kind: 'warning', tone: 'caution', text: WORDS.warnSimUnavailable };
	const FAILS: Said = { kind: 'warning', tone: 'danger', text: WORDS.warnWillFail };
	const fee: FeeModel = {
		kind: 'onchain',
		label: 'Network fee',
		value: '0.0021 ETH',
		valueFiat: '≈ $6.30',
		moneyLine: true,
		tappable: false,
		refreshLabel: 'Refresh fee',
		refreshing: false,
		chevron: false
	};
	let rooms: Said[] = [];
	let LONGEST: Said;
	beforeAll(async () => {
		await loadCore();
		rooms = verdictRooms(WORDS);
		LONGEST = rooms[1];
	});
	/**
	 * A send's own blocks, then the verdict's place holding `inner`. `long`: a
	 * request with more to read above the place (a decoded call's fields).
	 */
	const sheetWith = (inner: Said, landed: boolean, long = false) =>
		model({
			fee,
			blocks: [
				{ kind: 'intent', text: 'Send', tone: 'neutral' },
				{
					kind: 'amount',
					line: { sign: '−', value: '1,250', symbol: 'USDC', tone: 'neutral' }
				},
				{ kind: 'party', label: 'Recipient', name: '0x7687…D141' },
				...(long
					? [
							{
								kind: 'rows',
								rows: Array.from({ length: 5 }, (_, i) => ({
									label: `Field ${i + 1}`,
									value: `value ${i + 1}`
								}))
							}
						]
					: []),
				{ kind: 'verdict', inner: [inner], rooms, landed }
			] as Blocks
		});
	const out = () => sheetWith(CHECKING, false);

	const round = (value: number) => Math.round(value * 10) / 10;
	const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
	const q = (sheet: HTMLElement, selector: string) => sheet.querySelector(selector) as HTMLElement;
	const placeOf = (sheet: HTMLElement) => q(sheet, '[data-testid="signing-verdict"]');
	const saidOf = (sheet: HTMLElement) => q(placeOf(sheet), '.said');
	const confirmOf = (sheet: HTMLElement) => q(sheet, '[data-testid="signing-confirm"]');
	const scrollerOf = (sheet: HTMLElement) => q(sheet, '.content');
	const headerOf = (sheet: HTMLElement) => q(sheet, '[data-signing-top]');
	/** Where everything a person could be reading, or reaching for, stands. */
	const where = (sheet: HTMLElement) => {
		const top = (selector: string) => round(q(sheet, selector).getBoundingClientRect().top);
		return {
			sheet: round(sheet.getBoundingClientRect().top),
			height: round(sheet.getBoundingClientRect().height),
			close: top('header.header .close'),
			intent: top('.blocks > :first-child'),
			place: round(placeOf(sheet).getBoundingClientRect().top),
			placeHeight: round(placeOf(sheet).getBoundingClientRect().height),
			details: top('section.tech'),
			fee: top('.line'),
			signer: top('.signer'),
			confirm: round(confirmOf(sheet).getBoundingClientRect().top),
			scrolled: round(scrollerOf(sheet).scrollTop)
		};
	};
	/**
	 * Where it all RESTS: the same figures over a run of painted frames, the
	 * confirm whole inside the window. No clock: on a busy machine the sheet's
	 * entrance takes as long as it takes.
	 */
	async function resting(sheet: HTMLElement) {
		let last = '';
		let still = 0;
		await vi.waitFor(
			async () => {
				await frame();
				await frame();
				const now = JSON.stringify(where(sheet));
				const inside = confirmOf(sheet).getBoundingClientRect().bottom <= window.innerHeight + 0.5;
				still = inside && now === last ? still + 1 : 0;
				last = now;
				expect(still).toBeGreaterThanOrEqual(4);
			},
			{ timeout: 15_000, interval: 10 }
		);
		return where(sheet);
	}
	async function open(width: number, height: number, first: SigningModel = out()) {
		await page.viewport(width, height);
		const view = await drawn({ model: first });
		return { ...view, rest: await resting(view.sheet) };
	}
	async function land(view: Awaited<ReturnType<typeof drawn>>, next: SigningModel) {
		await view.screen.rerender({ model: next });
		await tick();
		return resting(view.sheet);
	}
	/** In the body's window: under the header, over the foot. */
	function inSight(sheet: HTMLElement, el: Element): boolean {
		const box = el.getBoundingClientRect();
		return (
			box.top >= headerOf(sheet).getBoundingClientRect().bottom - 0.5 &&
			box.bottom <= scrollerOf(sheet).getBoundingClientRect().bottom + 0.5
		);
	}
	/** Whole, on screen, outside the scroll, and the thing a tap there reaches. */
	function expectConfirmPinned(sheet: HTMLElement): void {
		const confirm = confirmOf(sheet);
		const box = confirm.getBoundingClientRect();
		expect(box.top).toBeGreaterThanOrEqual(0);
		expect(box.bottom).toBeLessThanOrEqual(window.innerHeight);
		expect(box.bottom).toBeLessThanOrEqual(sheet.getBoundingClientRect().bottom);
		expect(scrollerOf(sheet).contains(confirm)).toBe(false);
		const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
		expect(confirm.contains(hit)).toBe(true);
	}
	/** The ✕ is at the top of the sheet, on screen, and reachable. */
	function expectCloseReachable(sheet: HTMLElement): void {
		const close = q(sheet, 'header.header .close');
		const box = close.getBoundingClientRect();
		expect(box.top).toBeGreaterThanOrEqual(sheet.getBoundingClientRect().top - 0.5);
		expect(box.bottom).toBeLessThanOrEqual(scrollerOf(sheet).getBoundingClientRect().bottom);
		const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
		expect(close.contains(hit)).toBe(true);
	}

	// The harness's own window, back as the other suites expect it.
	afterAll(() => page.viewport(414, 896));

	const SCREENS = [
		['a small phone', 320, 700],
		['the phone sheet', 390, 844],
		['the centred card', 1440, 900]
	] as const;

	it.each(SCREENS)(
		'%s (%i×%i): every usual verdict lands and NOTHING moves — the ✕, the request, the fee row, the account, the confirm',
		async (_name, width, height) => {
			const view = await open(width, height);
			const was = view.rest;
			expect(saidOf(view.sheet).textContent).toContain('Checking…');
			// Each lands where "Checking…" stood; then the next request's "Checking…".
			const landings: [string, SigningModel][] = [
				['No asset changes', sheetWith(NO_CHANGE, true)],
				['one row', sheetWith(SEND, true)],
				['two rows', sheetWith(SWAP, true)],
				['could not check', sheetWith(CAUTION, true)],
				['expected to fail', sheetWith(FAILS, true)],
				['expected to fail, at the longest reason', sheetWith(LONGEST, true)],
				['checking again', out()]
			];
			for (const [name, next] of landings) {
				expect(await land(view, next), `${name} @ ${width} × ${height}`).toEqual(was);
				expectConfirmPinned(view.sheet);
				expectCloseReachable(view.sheet);
			}
			await view.screen.unmount();
		}
	);

	it.each(SCREENS)(
		'%s (%i×%i): the place is as tall as its tallest room, and every usual verdict fits inside it',
		async (_name, width, height) => {
			const view = await open(width, height);
			const place = placeOf(view.sheet);
			const roomEls = [...place.querySelectorAll<HTMLElement>(':scope > .room')];
			expect(roomEls).toHaveLength(rooms.length);
			const tallest = Math.max(...roomEls.map((room) => room.getBoundingClientRect().height));
			expect(tallest).toBeGreaterThan(40);
			expect(Math.abs(place.getBoundingClientRect().height - tallest)).toBeLessThan(0.5);
			for (const said of [NO_CHANGE, SEND, SWAP, CAUTION, FAILS, LONGEST]) {
				await land(view, sheetWith(said, true));
				const box = saidOf(view.sheet).getBoundingClientRect();
				expect(box.height, JSON.stringify(said)).toBeLessThanOrEqual(tallest + 0.5);
				// From the top of the place, inside it.
				expect(Math.abs(box.top - place.getBoundingClientRect().top)).toBeLessThan(0.5);
				expect(box.bottom).toBeLessThanOrEqual(place.getBoundingClientRect().bottom + 0.5);
			}
			await view.screen.unmount();
		}
	);

	it('a room is laid out and never SAID: no text of it, no alert, nothing for a reader or a tap', async () => {
		const view = await open(390, 844);
		const place = placeOf(view.sheet);
		// What the sheet says while the simulation is out is "Checking…", and
		// no sentence of a verdict that has not been given.
		const text = view.sheet.textContent ?? '';
		expect(text).toContain('Checking…');
		for (const unsaid of [
			'couldn’t check',
			'expected to fail',
			'Expected to fail',
			'No asset changes'
		]) {
			expect(text, unsaid).not.toContain(unsaid);
			expect(view.sheet.innerText, unsaid).not.toContain(unsaid);
		}
		for (const room of place.querySelectorAll<HTMLElement>(':scope > .room')) {
			expect(room.getAttribute('aria-hidden')).toBe('true');
			expect(getComputedStyle(room).visibility).toBe('hidden');
			expect(getComputedStyle(room).pointerEvents).toBe('none');
			expect(room.querySelector('[role="alert"]')).toBeNull();
			expect(room.querySelector('[data-verdict]')).toBeNull();
			expect((room.textContent ?? '').trim()).toBe('');
			// …and yet it has the height of the sentence it stands for.
			expect(room.getBoundingClientRect().height).toBeGreaterThan(40);
		}
		// Nothing has landed: the sheet has nothing to bring into sight.
		expect(place.dataset.stage).toBe('out');
		expect(view.sheet.querySelector('[data-verdict]')).toBeNull();

		// A danger that lands IS said, as an alert, once.
		await land(view, sheetWith(FAILS, true));
		expect(place.dataset.stage).toBe('landed');
		const alerts = view.sheet.querySelectorAll('[role="alert"]');
		expect(alerts).toHaveLength(1);
		expect(alerts[0].textContent).toContain('expected to fail');
		expect(saidOf(view.sheet).hasAttribute('data-verdict')).toBe(true);
		expect((view.sheet.textContent ?? '').split('expected to fail').length - 1).toBe(1);
		await view.screen.unmount();
	});

	it.each(SCREENS)(
		'%s (%i×%i): four rows and the unverified warning are entirely visible, and the confirm is pinned where it was',
		async (_name, width, height) => {
			const view = await open(width, height);
			const was = view.rest;
			const after = await land(view, sheetWith(TALL, true));
			const place = placeOf(view.sheet);
			const said = saidOf(view.sheet);
			const section = q(said, 'section.balances');

			// Every row and the warning are there, each at its own height…
			const rows = [...section.querySelectorAll<HTMLElement>('.row')];
			expect(rows.map((row) => row.textContent?.replace(/\s+/g, ' ').trim())).toEqual([
				'USDC −1,250.00',
				'ETH +0.4312',
				'Unverified token +',
				'DAI −18.5'
			]);
			const note = q(section, '.note');
			expect(note.textContent).toBe(WORDS.warnUnverifiedAmount);
			expect(note.dataset.tone).toBe('caution');
			// …the unverified row a direction in the caution's ink, and no figure.
			const unverified = q(rows[2], '.delta');
			expect(unverified.textContent).toBe('+');
			expect(getComputedStyle(unverified).color).toBe(getComputedStyle(note).color);
			expect(getComputedStyle(unverified).color).not.toBe(
				getComputedStyle(q(rows[0], '.delta')).color
			);

			// Nothing between a row and the sheet's one scroll cuts or scrolls it:
			// the place grew to hold the card, and has no height of its own.
			const parts = [q(section, 'h3'), ...rows, note];
			const scroller = scrollerOf(view.sheet);
			for (let el: HTMLElement | null = section; el && el !== scroller; el = el.parentElement) {
				const own = getComputedStyle(el);
				expect(own.overflowY, el.className).toBe('visible');
				expect(own.maxHeight, el.className).toBe('none');
				expect(el.scrollHeight, el.className).toBeLessThanOrEqual(el.clientHeight + 1);
			}
			expect(place.getBoundingClientRect().height).toBeGreaterThan(was.placeHeight + 20);
			expect(
				Math.abs(place.getBoundingClientRect().height - section.getBoundingClientRect().height)
			).toBeLessThan(0.5);

			// ENTIRELY VISIBLE: the whole card is in the body's window, under the
			// header and over the foot, and every part of it is what a look at
			// its middle finds — nothing over it.
			expect(inSight(view.sheet, section)).toBe(true);
			for (const part of parts) {
				expect(inSight(view.sheet, part), part.textContent ?? '').toBe(true);
				const box = part.getBoundingClientRect();
				const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
				expect(part.contains(hit) || hit === section, part.textContent ?? '').toBe(true);
			}

			// The confirm is PINNED: where it was, whole, outside the scroll.
			expect(after.confirm).toBe(was.confirm);
			expectConfirmPinned(view.sheet);
			expectCloseReachable(view.sheet);

			// The verdict goes (the next request's "Checking…"): the place is its
			// rooms' height again, and the confirm has still not moved.
			const back = await land(view, out());
			expect(back.placeHeight).toBe(was.placeHeight);
			expect(back.confirm).toBe(was.confirm);
			await view.screen.unmount();
		}
	);

	it('a window too short to hold the tall verdict: the BODY scrolls, every part can be read, and the confirm is pinned', async () => {
		const view = await open(390, 560);
		const was = view.rest;
		const scroller = scrollerOf(view.sheet);
		const after = await land(view, sheetWith(TALL, true));
		const section = q(saidOf(view.sheet), 'section.balances');
		// The sheet is as tall as it may be; its body scrolls, the card does not.
		expect(scroller.scrollHeight).toBeGreaterThan(scroller.clientHeight);
		expect(section.scrollHeight).toBeLessThanOrEqual(section.clientHeight + 1);
		expect(after.confirm).toBe(was.confirm);
		expectConfirmPinned(view.sheet);
		// What landed was scrolled into sight: all of it when the body's window
		// can hold it, else from its top, under the header.
		const header = headerOf(view.sheet).getBoundingClientRect();
		const room = scroller.getBoundingClientRect().bottom - header.bottom;
		if (section.getBoundingClientRect().height <= room) {
			expect(inSight(view.sheet, section)).toBe(true);
		} else {
			expect(Math.abs(section.getBoundingClientRect().top - header.bottom)).toBeLessThan(1);
		}
		// Each row and the warning can be brought fully into view, and reading
		// them moves no confirm; the ✕ stays at the top of the sheet.
		for (const part of [...section.children]) {
			scroller.scrollTop +=
				part.getBoundingClientRect().top - headerOf(view.sheet).getBoundingClientRect().bottom;
			if (!inSight(view.sheet, part)) {
				scroller.scrollTop +=
					part.getBoundingClientRect().bottom - scroller.getBoundingClientRect().bottom;
			}
			expect(inSight(view.sheet, part), part.textContent ?? '').toBe(true);
			expect(round(confirmOf(view.sheet).getBoundingClientRect().top)).toBe(was.confirm);
			expectCloseReachable(view.sheet);
		}
		await view.screen.unmount();
	});

	it('a verdict that lands under the fold is brought into sight; one that lands in sight scrolls nothing', async () => {
		// The body already scrolls, the person is reading its top, and the
		// place is further down than the window shows.
		const view = await open(390, 520, sheetWith(CHECKING, false, true));
		const scroller = scrollerOf(view.sheet);
		expect(scroller.scrollHeight).toBeGreaterThan(scroller.clientHeight);
		expect(view.rest.scrolled).toBe(0);
		expect(inSight(view.sheet, saidOf(view.sheet))).toBe(false);
		const after = await land(view, sheetWith(CAUTION, true, true));
		expect(after.scrolled).toBeGreaterThan(0);
		expect(inSight(view.sheet, saidOf(view.sheet))).toBe(true);
		expect(after.confirm).toBe(view.rest.confirm);
		await view.screen.unmount();

		// A window that shows the place: a landing scrolls nothing.
		const tall = await open(390, 844);
		expect(inSight(tall.sheet, saidOf(tall.sheet))).toBe(true);
		expect((await land(tall, sheetWith(CAUTION, true))).scrolled).toBe(tall.rest.scrolled);
		await tall.screen.unmount();
	});

	it('the other order — the verdict lands first, the request is read after it: what the reading pushes under the fold is brought back', async () => {
		// The simulation answered before the request was read: "Loading…" where
		// its blocks will be, and the verdict already in its place, in sight.
		const unread = (inner: Said) =>
			model({
				fee,
				reading: 'out',
				blocks: [
					{ kind: 'sentence', text: 'Loading…', tone: 'neutral' },
					{ kind: 'verdict', inner: [inner], rooms, landed: true }
				] as Blocks
			});
		const view = await open(390, 520, unread(CAUTION));
		const scroller = scrollerOf(view.sheet);
		expect(inSight(view.sheet, saidOf(view.sheet))).toBe(true);
		expect(view.rest.scrolled).toBe(0);
		// The request is read: more above the place than the window shows. The
		// same verdict, said the same — nothing "landed" — and it is in sight.
		const after = await land(view, sheetWith(CAUTION, true, true));
		expect(scroller.scrollHeight).toBeGreaterThan(scroller.clientHeight);
		expect(after.scrolled).toBeGreaterThan(0);
		expect(inSight(view.sheet, saidOf(view.sheet))).toBe(true);
		expect(after.confirm).toBe(view.rest.confirm);
		expectConfirmPinned(view.sheet);
		expectCloseReachable(view.sheet);
		await view.screen.unmount();

		// Read in a window that still shows the verdict: nothing is scrolled.
		const tall = await open(390, 844, unread(SWAP));
		expect((await land(tall, sheetWith(SWAP, true))).scrolled).toBe(0);
		expect(inSight(tall.sheet, saidOf(tall.sheet))).toBe(true);
		await tall.screen.unmount();
	});

	it('in a larger text size the place is taller, by its rooms, before anything lands', async () => {
		const view = await open(390, 844);
		const usual = view.rest.placeHeight;
		view.sheet.style.setProperty('--text-scale', '1.35');
		const larger = await resting(view.sheet);
		expect(larger.placeHeight).toBeGreaterThan(usual + 10);
		// And still nothing moves when the longest usual verdict lands in it.
		expect(await land(view, sheetWith(LONGEST, true))).toEqual(larger);
		expect(await land(view, sheetWith(SWAP, true))).toEqual(larger);
		await view.screen.unmount();
	});
});
