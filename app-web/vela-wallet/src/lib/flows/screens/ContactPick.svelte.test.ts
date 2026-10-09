/**
 * Issue 467: the recipient picker answers with the ADDRESS of the row that
 * was tapped, never its place. The book re-sorts by favourite, recency and
 * name while names resolve; a place in the list read after the re-sort is
 * somebody else — and that somebody gets the money.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { ContactPickModel } from '../model';
import ContactPick from './ContactPick.svelte';

const ALICE = '0x' + 'a1'.repeat(20);
const BOB = '0x' + 'b2'.repeat(20);
const CAROL = '0x' + 'c3'.repeat(20);

function person(name: string, address: string): ContactPickModel['contacts'][number] {
	return {
		name,
		addressDisplay: `${address.slice(0, 6)}…${address.slice(-4)}`,
		addressFull: address,
		identiconSvg: `<svg data-seed="${address}"></svg>`
	};
}

function book(contacts: ContactPickModel['contacts']): ContactPickModel {
	return {
		title: 'Choose a contact',
		closeLabel: 'Close',
		searchPlaceholder: 'Search',
		scanRow: 'Scan to fill',
		groupsTitle: 'Groups',
		groups: [],
		contactsTitle: 'Contacts',
		contacts
	};
}

function row(container: Element, name: string): HTMLElement {
	const found = [...container.querySelectorAll<HTMLElement>('li button.main')].find((b) =>
		b.textContent?.includes(name)
	);
	if (found === undefined) throw new Error(`no row for ${name}`);
	return found;
}

describe('ContactPick picks by address (issue 467)', () => {
	it('a tap answers with the address the row was drawn for', () => {
		const onselect = vi.fn();
		const screen = render(ContactPick, {
			props: { model: book([person('Alice', ALICE), person('Bob', BOB)]), onselect }
		});
		row(screen.container, 'Bob').click();
		expect(onselect).toHaveBeenCalledWith(BOB);
	});

	it('the book re-sorting under the finger still picks the person tapped', async () => {
		const onselect = vi.fn();
		const screen = render(ContactPick, {
			props: {
				model: book([person('Alice', ALICE), person('Bob', BOB), person('Carol', CAROL)]),
				onselect
			}
		});
		const bob = row(screen.container, 'Bob');
		// A name resolved and the book re-sorted: Carol is now where Bob was.
		await screen.rerender({
			model: book([person('Carol', CAROL), person('Alice', ALICE), person('Bob', BOB)]),
			onselect
		});
		await tick();
		// The row element is still Bob's (keyed by address), and it says so.
		expect(bob.isConnected).toBe(true);
		expect(bob.textContent).toContain('Bob');
		bob.click();
		expect(onselect).toHaveBeenCalledWith(BOB);
	});

	it('a filtered list picks the person shown, not the one at that place in the book', async () => {
		const onselect = vi.fn();
		const screen = render(ContactPick, {
			props: { model: book([person('Alice', ALICE), person('Bob', BOB)]), onselect }
		});
		const search = screen.container.querySelector<HTMLInputElement>('input')!;
		search.value = 'bob';
		search.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		expect(screen.container.querySelectorAll('li')).toHaveLength(1);
		row(screen.container, 'Bob').click();
		expect(onselect).toHaveBeenCalledWith(BOB);
	});
});
