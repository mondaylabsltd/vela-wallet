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
		confirm: { hint: 'Slide to confirm', action: 'Send', enabled: true },
		closeLabel: 'Close',
		panelTitle: 'Signature request',
		...over
	};
}

async function drawn(props: {
	model?: SigningModel;
	dismissible?: boolean;
	onfeerefresh?: () => void;
}) {
	const onclose = vi.fn();
	const screen = render(SigningSheet, {
		props: {
			model: props.model ?? model(),
			dismissible: props.dismissible ?? true,
			onclose,
			onfeerefresh: props.onfeerefresh
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
