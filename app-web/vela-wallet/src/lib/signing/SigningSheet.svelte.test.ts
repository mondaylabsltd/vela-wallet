/**
 * The signing sheet in a browser (spec 079).
 *
 * US1 — it closes only through its ✕: the owner lost a dApp's request to a
 * stray touch ("除非用户明确关掉，不应该很容易误操作，比如下滑就关掉了"), and the
 * page then had to ask again. US2 — its fee row carries the send form's
 * refresh control and stale note, and says why a quote failed.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
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
}) {
	const onclose = vi.fn();
	const screen = render(SigningSheet, {
		props: {
			model: props.model ?? model(),
			dismissible: props.dismissible ?? true,
			onclose,
			onfeerefresh: props.onfeerefresh,
			onretry: props.onretry
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

	it('the form gives way to the status: no fee, no slide — never a greyed one', async () => {
		const SLIDE = '[data-testid="signing-confirm"]';
		// The control, before the approval: the slide is there.
		const form = await drawn({});
		expect(form.sheet.querySelector(SLIDE)).not.toBeNull();
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
		expect(view.sheet.querySelector(SLIDE)).toBeNull();
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

	it('keeps the stale note’s line standing, so the slide never moves when it speaks', async () => {
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
	const BACKUP = 'Back up public keys';
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
