/**
 * The signing sheet in a browser (spec 079).
 *
 * US1 — it closes only through its ✕: the owner lost a dApp's request to a
 * stray touch ("除非用户明确关掉，不应该很容易误操作，比如下滑就关掉了"), and the
 * page then had to ask again. US2 — its fee row carries the send form's
 * refresh control and stale note, and says why a quote failed.
 */
import { tick } from 'svelte';
import { afterAll, describe, expect, it, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import SigningSheet from './SigningSheet.svelte';
import type { FeeModel, SigningModel } from './model';

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

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
	await pause(300); // past the entry animation
	const root = document.body;
	const sheet = root.querySelector<HTMLElement>('[role="dialog"]')!;
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
 * This sheet reserves no place for a verdict (spec 082 RG6): what lands late
 * is the relay's "will fail" line and the "No asset changes" card. The
 * renderer draws balance rows for the boards, so the tall card here — four
 * rows and the unverified-token note — is the one the apps draw.
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
