/**
 * Issue #328 (reported on Android; the web had the same shape): every flow
 * sheet the person closes — the token, the transaction, the code, add by
 * address, the contact picker, the fee coin, the import — tells the route,
 * through every door (✕, the scrim, Escape; the drag shares BottomSheet's one
 * close path). The token, transaction and code sheets only hid themselves:
 * their step stayed on the route's stack, so the same row tapped again pushed
 * the same step, which `FlowNav.push` ignores, and nothing opened.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { buildFlowState, MOBILE_FLOW_STATES } from './fixtures';
import FlowsMobile from './FlowsMobile.svelte';
import type { WalletFlowMessages } from './messages';
import type { FlowStateId } from './model';
import { FlowNav } from './nav.svelte';

/**
 * Every string is its own key (the corpus is the wasm engine's, not a browser
 * test's), with an `{{n}}` so numbered rows (收款人 1, 2, 3) stay distinct.
 */
const words = new Proxy({}, { get: (_, key) => `${String(key)} {{n}}` }) as WalletFlowMessages;
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const build = (state: FlowStateId) => buildFlowState(state, words, identicon);
const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const SHEET_STATES = MOBILE_FLOW_STATES.filter((state) => build(state).sheet !== undefined);

async function drawn(state: FlowStateId) {
	const onsheetclose = vi.fn();
	const host = document.createElement('div');
	host.style.cssText = 'position: relative; width: 390px; height: 844px; overflow: hidden;';
	document.body.appendChild(host);
	const screen = render(FlowsMobile, { target: host, props: { model: build(state), onsheetclose } });
	await tick();
	await pause(300);
	const dialog = () => host.querySelector<HTMLElement>('[role="dialog"]');
	return { screen, host, dialog, onsheetclose };
}

describe('a closed flow sheet tells the route', () => {
	it('covers every sheet kind the phone raises', () => {
		const kinds = new Set(SHEET_STATES.map((state) => build(state).sheet?.kind));
		expect([...kinds].sort()).toEqual([
			'add-token',
			'batch-import',
			'contact-pick',
			'fee-token',
			'receive-qr',
			'token-detail',
			'tx-detail'
		]);
	});

	it.each(SHEET_STATES)('%s: the scrim closes it and the route hears it once', async (state) => {
		const view = await drawn(state);
		expect(view.dialog(), 'the sheet is up').not.toBeNull();
		view.host.querySelector<HTMLElement>('.scrim')!.click();
		await vi.waitFor(() => expect(view.onsheetclose).toHaveBeenCalledTimes(1));
		await tick();
		expect(view.dialog(), 'the sheet is gone').toBeNull();
		view.screen.unmount();
	});

	it.each(SHEET_STATES)('%s: Escape closes it and the route hears it', async (state) => {
		const view = await drawn(state);
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		await vi.waitFor(() => expect(view.onsheetclose).toHaveBeenCalledTimes(1));
		view.screen.unmount();
	});

	it.each(SHEET_STATES.filter((state) => build(state).sheet?.model.closeLabel !== undefined))(
		'%s: the ✕, where the sheet draws one, closes it and the route hears it',
		async (state) => {
			const view = await drawn(state);
			const close = view.host.querySelector<HTMLElement>('button.close');
			if (close === null) {
				// A sheet whose content IS its heading (the token, the
				// transaction, the code) has no title row: scrim, drag, Escape.
				expect(['receive-qr', 'tx-detail', 'token-detail']).toContain(build(state).sheet?.kind);
			} else {
				close.click();
				await vi.waitFor(() => expect(view.onsheetclose).toHaveBeenCalledTimes(1));
			}
			view.screen.unmount();
		}
	);
});

describe('the reported steps, ten times over', () => {
	it('Assets → a token → close → another token opens its sheet', async () => {
		// The route's half, as the wallet page wires it: the close pops the step.
		const nav = new FlowNav();
		nav.enter('assets');
		const view = await drawn('t1');
		await view.screen.rerender({ onsheetclose: () => nav.sheetClosed(nav.mobileTop!) });
		for (let i = 0; i < 10; i++) {
			nav.push('token-detail');
			await view.screen.rerender({ model: build(nav.mobileTop!) });
			await pause(300);
			expect(view.dialog(), `tap #${i} opens a sheet`).not.toBeNull();
			view.host.querySelector<HTMLElement>('.scrim')!.click();
			await vi.waitFor(() => expect(nav.mobileTop, `close #${i}`).toBe('t1'));
			await view.screen.rerender({ model: build(nav.mobileTop!) });
			expect(view.dialog(), `close #${i} leaves the list`).toBeNull();
		}
		view.screen.unmount();
	});
});
