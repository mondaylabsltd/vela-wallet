/**
 * The hand-off card (spec 102, D4), in a real browser: the four things it
 * says — the key as the sheet's label | value row — Open shut for a refused
 * page, the page named once, and the way back once it opened.
 */
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { HandoffModel } from '../model';
import HandoffCard from './HandoffCard.svelte';

const READY: HandoffModel = {
	title: 'Review and sign on a trusted signing page',
	key: { label: 'Confirm with', value: 'Phone or tablet' },
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
		expect(text).toContain('Review and sign on a trusted signing page');
		expect(text).toContain('sign.getvela.app');
		// The key is a row: its label at the start, its value at the end.
		const row = view.root.querySelector<HTMLElement>('.key');
		expect(row?.querySelector('.key-label')?.textContent).toBe('Confirm with');
		expect(row?.querySelector('.key-value')?.textContent).toBe('Phone or tablet');
		const [label, value] = [row?.querySelector('.key-label'), row?.querySelector('.key-value')];
		expect(label!.getBoundingClientRect().left).toBeLessThan(value!.getBoundingClientRect().left);
		// No fee on the card: the sheet's own row above it says it (D-18).
		expect(view.root.querySelector('.fee')).toBeNull();
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

	it('names a self-hosted page once, and holds the line’s room while it is checked', () => {
		const selfHosted = drawn({
			...READY,
			page: { name: 'Self-hosted · sign.example.com', host: 'sign.example.com' }
		});
		expect(selfHosted.root.querySelector('.host')).toBeNull();
		expect(drawn(READY).root.querySelector('.host')?.textContent).toBe('sign.getvela.app');

		// Where Open sits inside the card, at a phone's width — where the
		// verdict takes two lines and "checking" one.
		const top = (handoff: HandoffModel) => {
			const view = drawn(handoff);
			view.root.style.width = '360px';
			const card = view.root.querySelector<HTMLElement>('.handoff')!;
			return view.button()!.getBoundingClientRect().top - card.getBoundingClientRect().top;
		};
		const checked = top(READY);
		const checking = top({
			...READY,
			integrity: { text: 'Checking the page…', tone: 'checking' },
			open: { ...READY.open, enabled: false }
		});
		// "Checking the page…" → "Version … · checked 14:32": Open stays put.
		expect(checking).toBe(checked);
	});
});
