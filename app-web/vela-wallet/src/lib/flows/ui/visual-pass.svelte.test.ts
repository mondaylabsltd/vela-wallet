/**
 * The visual pass's send-screen fixes, in a browser:
 *
 * - vis-1: the typed recipient wraps and is read whole, in the text's face
 *   and colour; Enter adds no line, a pasted break is dropped.
 * - vis-8: a title and its network pill never sit on each other.
 * - vis-9 / vis-10: a fact's value wraps rather than losing its tail.
 * - vis-12: a small control's target is 44px, its drawing unchanged.
 * - vis-14: Enter in the amount continues; Escape on a flow screen is ‹.
 */
import { createRawSnippet, tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';
import '$lib/tokens/tokens.css';
import '../../../app.css';
import AmountInput from './AmountInput.svelte';
import FactRow from './FactRow.svelte';
import FlowScreen from './FlowScreen.svelte';
import RecipientField from './RecipientField.svelte';

const ADDRESS = '0x031d7D57c99CAF891e1C250554691Fd12D84772b';
const empty = createRawSnippet(() => ({ render: () => '<div></div>' }));

function host(width: number): HTMLElement {
	const el = document.createElement('div');
	el.style.cssText = `width: ${width}px;`;
	document.body.appendChild(el);
	return el;
}

describe('RecipientField (vis-1)', () => {
	const draw = (oninput: (v: string) => void, width = 320) =>
		render(RecipientField, {
			target: host(width),
			props: {
				label: 'Recipient',
				lines: [ADDRESS.slice(0, 21), ADDRESS.slice(21)],
				identiconSvg: '',
				pickLabel: 'Choose a contact',
				oninput
			}
		});

	it('wraps the whole address in the mono face and the text colour', async () => {
		const screen = draw(() => {});
		await tick();
		const field = screen.container.querySelector('textarea') as HTMLTextAreaElement;
		expect(field.value).toBe(ADDRESS);
		const style = getComputedStyle(field);
		const probe = document.createElement('span');
		probe.style.cssText = 'color: var(--color-fg-base); font-family: var(--font-mono);';
		document.body.appendChild(probe);
		expect(style.color).toBe(getComputedStyle(probe).color);
		expect(style.fontFamily).toBe(getComputedStyle(probe).fontFamily);
		probe.remove();
		// Every character is on screen: nothing scrolls sideways out of view.
		expect(field.scrollWidth).toBeLessThanOrEqual(field.clientWidth);
		expect(field.scrollHeight).toBeLessThanOrEqual(field.clientHeight + 1);
	});

	it('adds no line on Enter, and drops a pasted break', async () => {
		const oninput = vi.fn();
		const screen = draw(oninput);
		await tick();
		const field = screen.container.querySelector('textarea') as HTMLTextAreaElement;
		field.focus();
		field.value = '';
		await userEvent.keyboard('{Enter}');
		expect(field.value).toBe('');
		field.value = `${ADDRESS}\n`;
		field.dispatchEvent(new InputEvent('input', { bubbles: true }));
		expect(oninput).toHaveBeenLastCalledWith(ADDRESS);
	});
});

describe('FactRow (vis-9, vis-10)', () => {
	it('wraps a long value instead of cutting its end', async () => {
		const screen = render(FactRow, {
			target: host(280),
			props: {
				fact: { label: 'Geschätzte Gebühr', value: '0.0168 xDAI · ≈ $0.02 per transfer today' }
			}
		});
		await tick();
		const value = screen.container.querySelector('.value') as HTMLElement;
		expect(getComputedStyle(value).textOverflow).not.toBe('ellipsis');
		expect(value.scrollWidth).toBeLessThanOrEqual(value.clientWidth);
		const label = screen.container.querySelector('.label') as HTMLElement;
		// The label is never narrower than its longest word.
		expect(label.scrollWidth).toBeLessThanOrEqual(label.clientWidth);
	});
});

describe('FlowScreen', () => {
	const header = {
		title: 'Transaktionsverlauf',
		backLabel: 'Back',
		pill: { dots: ['red'], label: 'All networks' }
	};

	it('puts the pill under a title that leaves it no room (vis-8)', async () => {
		const el = host(300);
		el.style.height = '600px';
		el.style.setProperty('--text-scale', '1.35');
		const screen = render(FlowScreen, { target: el, props: { header, children: empty } });
		await tick();
		const title = screen.container.querySelector('h1') as HTMLElement;
		const pill = screen.container.querySelector('.pill') as HTMLElement;
		const a = title.getBoundingClientRect();
		const b = pill.getBoundingClientRect();
		const overlap = a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
		expect(overlap).toBe(false);
	});

	it('steps back on Escape (vis-14)', async () => {
		const onback = vi.fn();
		const el = host(390);
		el.style.height = '600px';
		render(FlowScreen, { target: el, props: { header, onback, children: empty } });
		await tick();
		await userEvent.keyboard('{Escape}');
		expect(onback).toHaveBeenCalledOnce();
	});

	it('gives the back chevron a 44px target, drawn as before (vis-12)', async () => {
		const el = host(390);
		el.style.height = '600px';
		const onback = vi.fn();
		const screen = render(FlowScreen, { target: el, props: { header, onback, children: empty } });
		await tick();
		const back = screen.container.querySelector('.back') as HTMLButtonElement;
		const box = back.getBoundingClientRect();
		expect(box.width).toBeLessThan(44);
		// Just outside the drawn box, still inside the 44px target.
		const hit = document.elementFromPoint(box.right + 2, box.top + box.height / 2);
		expect(hit === back || back.contains(hit)).toBe(true);
	});
});

describe('AmountInput (vis-14)', () => {
	it('continues on Enter', async () => {
		const onsubmit = vi.fn();
		const screen = render(AmountInput, {
			props: { value: '1', fiat: '≈ $1.00', denomLabel: 'xDAI', oninput: () => {}, onsubmit }
		});
		await tick();
		const input = screen.container.querySelector('input.entry') as HTMLInputElement;
		input.focus();
		await userEvent.keyboard('{Enter}');
		expect(onsubmit).toHaveBeenCalledOnce();
	});
});
