/**
 * The hand-off card (spec 102, D4), in a real browser: the four things it
 * says, Open shut for a refused page, and the way back once it opened.
 */
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { HandoffModel } from '../model';
import HandoffCard from './HandoffCard.svelte';

const READY: HandoffModel = {
	title: 'Review and sign on your trusted page',
	key: 'Confirm with Savings',
	page: { name: 'Official', host: 'sign.getvela.app' },
	integrity: {
		text: "Version 0ba8ee8c · matches Vela's published build list · checked 14:32",
		tone: 'ok'
	},
	open: { label: 'Continue to signing page', enabled: true }
};

function drawn(handoff: HandoffModel) {
	let opened = 0;
	const screen = render(HandoffCard, { props: { handoff, onopen: () => (opened += 1) } });
	const button = () => screen.container.querySelector<HTMLButtonElement>('button');
	return { root: screen.container, button, opened: () => opened };
}

describe('the hand-off card', () => {
	it('says where, with which key, and what was checked — and opens', () => {
		const view = drawn(READY);
		const text = view.root.textContent ?? '';
		expect(text).toContain('Review and sign on your trusted page');
		expect(text).toContain('Confirm with Savings');
		expect(text).toContain('sign.getvela.app');
		expect(view.root.querySelector('.integrity .version')?.textContent).toBe('0ba8ee8c');
		view.button()?.click();
		expect(view.opened()).toBe(1);
	});

	it('a page that failed its check never gets Open', () => {
		const view = drawn({
			...READY,
			integrity: {
				text: "Version 7d41e0b9 isn't on Vela's published build list. Not opened.",
				tone: 'error'
			},
			open: { ...READY.open, enabled: false }
		});
		expect(view.button()?.disabled).toBe(true);
		view.button()?.click();
		expect(view.opened()).toBe(0);
		// No shield over a refusal: the mark says what the line says.
		expect(view.root.querySelector<HTMLElement>('.mark')?.dataset.tone).toBe('error');
	});

	it('once opened: where to look, and the way back to the page', () => {
		const view = drawn({
			...READY,
			waiting: {
				title: 'Waiting for the signing page…',
				hint: 'Check the request on the page that opened, and sign it there.',
				reopen: 'Open the page again'
			}
		});
		expect(view.root.textContent).toContain('Waiting for the signing page…');
		expect(view.button()?.textContent?.trim()).toBe('Open the page again');
		view.button()?.click();
		expect(view.opened()).toBe(1);
	});
});
