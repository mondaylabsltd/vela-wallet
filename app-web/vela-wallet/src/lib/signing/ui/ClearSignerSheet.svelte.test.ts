/**
 * The Clear Signer's sheet, in a browser (spec 071, contract §5): while the
 * page is open it says so, with the hint, "Open the page again" and Cancel;
 * after, one sentence and a way to close it — never a spinner left behind.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { clearSignerModel } from '../live';
import type { SigningMessages } from '../messages';
import ClearSignerSheet from './ClearSignerSheet.svelte';

/** The corpus is resolved at build time; the words this sheet reads, in English. */
const m = {
	close: 'Close',
	clearSignerWaiting: 'Waiting for the Clear Signer…',
	clearSignerWaitingHint: 'Check the request on the page that opened, and sign it there.',
	clearSignerReopen: 'Open the page again',
	clearSignerCancel: 'Cancel',
	clearSignerClosed: 'The Clear Signer was closed without signing.',
	clearSignerRefused: 'The Clear Signer would not sign this request. Its page says why.',
	clearSignerMismatch:
		'The Clear Signer’s answer does not match this request, so nothing was sent.',
	clearSignerTimeout: 'The Clear Signer did not answer in time.'
} as SigningMessages;

async function drawn(model: NonNullable<ReturnType<typeof clearSignerModel>>) {
	const taps: string[] = [];
	const screen = render(ClearSignerSheet, {
		props: {
			model,
			onreopen: () => taps.push('reopen'),
			ondismiss: () => taps.push('dismiss')
		}
	});
	await tick();
	const buttons = [...screen.container.querySelectorAll('button')];
	return {
		taps,
		text: screen.container.textContent ?? '',
		labels: buttons.map((button) => button.textContent?.trim()),
		press: (label: string) =>
			buttons.find((button) => button.textContent?.trim() === label)?.click()
	};
}

describe('the Clear Signer’s sheet', () => {
	it('waiting: the hint, open it again, cancel', async () => {
		const view = await drawn(clearSignerModel({ waiting: true, notice: null }, m)!);
		expect(view.text).toContain(m.clearSignerWaiting);
		expect(view.text).toContain(m.clearSignerWaitingHint);
		expect(view.labels).toEqual([m.clearSignerReopen, m.clearSignerCancel]);
		view.press(m.clearSignerReopen);
		view.press(m.clearSignerCancel);
		expect(view.taps).toEqual(['reopen', 'dismiss']);
	});

	it('Escape is this sheet’s, and never reaches the signing sheet under it', async () => {
		const view = await drawn(clearSignerModel({ waiting: true, notice: null }, m)!);
		let beneath = 0;
		const underneath = (event: KeyboardEvent) => {
			if (event.key === 'Escape') beneath += 1;
		};
		window.addEventListener('keydown', underneath);
		document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		window.removeEventListener('keydown', underneath);
		expect(view.taps).toEqual(['dismiss']);
		expect(beneath).toBe(0);
	});

	it('an ending: its sentence, and close — nothing to reopen', async () => {
		const view = await drawn(clearSignerModel({ waiting: false, notice: 'refused' }, m)!);
		expect(view.text).toContain(m.clearSignerRefused);
		expect(view.text).not.toContain(m.clearSignerWaitingHint);
		expect(view.labels).toEqual([m.close]);
	});
});
