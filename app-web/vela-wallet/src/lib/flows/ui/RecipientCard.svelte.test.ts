/**
 * Issue 471: every split row has its own two doors — the book and the
 * scanner — as the single recipient field has. Scanning into a split used to
 * be "Scan to fill the address", a row inside the contact picker.
 *
 * A `.svelte.test.ts`: whether two icons, a field and an amount fit one row
 * of a 390 px phone is the browser's to answer.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { RecipientCardModel } from '../model';
import RecipientCard from './RecipientCard.svelte';

const ALICE = '0x' + 'a1'.repeat(20);

function row(over: Partial<RecipientCardModel> = {}): RecipientCardModel {
	return {
		id: 'r1',
		ordinal: 'Recipient 1',
		name: '',
		address: '',
		identiconSvg: '',
		amount: '',
		amountValue: '',
		addressLabel: 'To',
		addressPlaceholder: '0x... address',
		pickLabel: 'Choose a contact',
		scanLabel: 'Scan a QR code',
		removeLabel: 'Remove',
		...over
	};
}

/** The card as a phone draws it: 390 wide less the screen's two gutters. */
async function drawn(
	recipient: RecipientCardModel,
	handlers: { live?: boolean; doors?: boolean } = {}
) {
	const onpick = vi.fn();
	const onscan = vi.fn();
	const live = handlers.live ?? true;
	const doors = live && (handlers.doors ?? true);
	const screen = render(RecipientCard, {
		props: {
			recipient,
			symbol: 'USDT',
			oninput: live ? () => {} : undefined,
			onpick: doors ? onpick : undefined,
			onscan: doors ? onscan : undefined
		}
	});
	screen.container.style.width = '342px';
	await tick();
	const root = screen.container;
	const button = (label: string) =>
		root.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`);
	return { root, button, onpick, onscan };
}

describe('RecipientCard — the row’s own doors (issue 471)', () => {
	it('an empty row draws the book and the scanner, side by side, in that order', async () => {
		const { root, button } = await drawn(row());
		const pick = button('Choose a contact');
		const scan = button('Scan a QR code');
		expect(pick).not.toBeNull();
		expect(scan).not.toBeNull();
		const a = pick!.getBoundingClientRect();
		const b = scan!.getBoundingClientRect();
		// One line, the scanner after the book, neither over the other.
		expect(Math.abs(a.top - b.top)).toBeLessThan(1);
		expect(b.left).toBeGreaterThanOrEqual(a.right - 0.5);
		// Both inside the card, and both a size a finger can find.
		const card = (root.querySelector('.card') as HTMLElement).getBoundingClientRect();
		expect(b.right).toBeLessThanOrEqual(card.right + 0.5);
		expect(a.width).toBeGreaterThanOrEqual(28);
		expect(b.width).toBeGreaterThanOrEqual(28);
	});

	it('each door answers for itself', async () => {
		const { button, onpick, onscan } = await drawn(row());
		button('Scan a QR code')!.click();
		expect(onscan).toHaveBeenCalledTimes(1);
		expect(onpick).not.toHaveBeenCalled();
		button('Choose a contact')!.click();
		expect(onpick).toHaveBeenCalledTimes(1);
		expect(onscan).toHaveBeenCalledTimes(1);
	});

	// Inside the field, the two icons left it 77 px of the 101 px that
	// "0x... address" needs at 390 px — the prompt read "0x... addr…".
	it('the two doors leave the address field room to say what it wants, at 390 px', async () => {
		const { root } = await drawn(row());
		const input = root.querySelector('input.address') as HTMLInputElement;
		const probe = document.createElement('span');
		probe.textContent = input.placeholder;
		probe.style.cssText = `position:absolute;visibility:hidden;white-space:nowrap;font:${getComputedStyle(input).font}`;
		document.body.appendChild(probe);
		const wanted = probe.getBoundingClientRect().width;
		probe.remove();
		expect(input.getBoundingClientRect().width).toBeGreaterThanOrEqual(wanted);
	});

	it('the doors sit on the label line, over the end of the field, and add no height', async () => {
		const withDoors = await drawn(row());
		const height = (withDoors.root.querySelector('.card') as HTMLElement).getBoundingClientRect()
			.height;
		const label = (
			withDoors.root.querySelector('.ordinal') as HTMLElement
		).getBoundingClientRect();
		const scan = withDoors.button('Scan a QR code')!.getBoundingClientRect();
		const well = (
			withDoors.root.querySelector('.address-well') as HTMLElement
		).getBoundingClientRect();
		// Centred on the label's line…
		expect(Math.abs((scan.top + scan.bottom) / 2 - (label.top + label.bottom) / 2)).toBeLessThan(1);
		// …ending where the field ends…
		expect(Math.abs(scan.right - well.right)).toBeLessThan(1);
		// …and the card is exactly as tall as one with no doors at all.
		const bare = await drawn(row({ pickLabel: undefined, scanLabel: undefined }), { doors: false });
		expect((bare.root.querySelector('.card') as HTMLElement).getBoundingClientRect().height).toBe(
			height
		);
	});

	it('a filled row at rest shows its person; the doors come back with the hand, in place', async () => {
		const { root, button } = await drawn(
			row({ name: 'Alice', addressShort: '0xa1a1…a1a1', address: ALICE, amountValue: '5' })
		);
		const doors = root.querySelector('.doors') as HTMLElement;
		const scan = button('Scan a QR code')!;
		const before = scan.getBoundingClientRect();
		const card = (root.querySelector('.card') as HTMLElement).getBoundingClientRect();
		expect(getComputedStyle(doors).opacity).toBe('0');
		expect(getComputedStyle(doors).pointerEvents).toBe('none');
		(root.querySelector('input.address') as HTMLInputElement).focus();
		await tick();
		expect(getComputedStyle(doors).opacity).toBe('1');
		// Nothing moved to make room: the door is where it was, the card as tall.
		const after = scan.getBoundingClientRect();
		expect(after.left).toBe(before.left);
		expect(after.top).toBe(before.top);
		expect((root.querySelector('.card') as HTMLElement).getBoundingClientRect().height).toBe(
			card.height
		);
	});

	it('the drawn card (the gallery’s) has no doors: nothing there can be filled', async () => {
		const { root } = await drawn(row({ name: 'Alice', address: ALICE, amount: '5' }), {
			live: false
		});
		expect(root.querySelectorAll('button[aria-label="Scan a QR code"]')).toHaveLength(0);
		expect(root.querySelectorAll('button[aria-label="Choose a contact"]')).toHaveLength(0);
	});

	it('a row whose model names no scanner draws none', async () => {
		const { button } = await drawn(row({ scanLabel: undefined }));
		expect(button('Scan a QR code')).toBeNull();
		expect(button('Choose a contact')).not.toBeNull();
	});
});
