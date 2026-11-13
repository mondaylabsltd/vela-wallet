/**
 * set-4: the add-contact form took a mixed-case address whose case is not
 * its EIP-55 checksum — a typo or an altered copy — and the book saved it
 * lowercased, as if it were fine. The form now asks the core's one address
 * rule (`addressEntry`) and says which way an address is not one.
 */
import { tick } from 'svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import type { ContactFormCopy } from '../forms';
import ContactForm from './ContactForm.svelte';

const COPY: ContactFormCopy = {
	title: 'Add contact',
	nameLabel: 'Name',
	namePlaceholder: 'Name',
	addressLabel: 'Address',
	addressPlaceholder: '0x…',
	save: 'Save',
	cancel: 'Cancel',
	invalidAddress: 'Enter a valid address',
	invalidChecksum: 'Checksum does not match'
};

const GOOD = '0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed';
const TYPO = '0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAeD';

// The book renders the form only once its core is up.
beforeAll(() => loadCore());

async function typeInto(input: HTMLInputElement, value: string) {
	input.value = value;
	input.dispatchEvent(new Event('input', { bubbles: true }));
	await tick();
}

describe('ContactForm', () => {
	it('refuses a failing checksum and says so; saves the right spelling', async () => {
		const onsave = vi.fn();
		const screen = render(ContactForm, { props: { copy: COPY, onsave } });
		const [name, address] = Array.from(screen.container.querySelectorAll('input'));
		await typeInto(name, 'Bob');
		await typeInto(address, TYPO);
		expect(screen.container.textContent).toContain(COPY.invalidChecksum);
		const save = screen.container.querySelector('button') as HTMLButtonElement;
		expect(save.disabled).toBe(true);

		await typeInto(address, '0x123');
		expect(screen.container.textContent).toContain(COPY.invalidAddress);

		// One case carries no checksum, and passes; so does the right spelling.
		await typeInto(address, GOOD.toLowerCase());
		expect(screen.container.textContent).not.toContain(COPY.invalidAddress);
		await typeInto(address, ` ${GOOD} `);
		expect(save.disabled).toBe(false);
		save.click();
		expect(onsave).toHaveBeenCalledWith({ name: 'Bob', address: GOOD });
	});

	// Round 3: the note field, where the caller gives it words (live).
	it('edits the note — kept, changed, or emptied to clear it', async () => {
		const onsave = vi.fn();
		const screen = render(ContactForm, {
			props: {
				copy: { ...COPY, noteLabel: 'Note', notePlaceholder: 'Optional' },
				initial: { name: 'Bob', address: GOOD, note: 'old' },
				onsave
			}
		});
		const note = screen.container.querySelector('textarea') as HTMLTextAreaElement;
		expect(note.value).toBe('old');
		note.value = ' Pays in USDC\nmonthly ‮';
		note.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		(screen.container.querySelector('button') as HTMLButtonElement).click();
		expect(onsave).toHaveBeenLastCalledWith({
			name: 'Bob',
			address: GOOD,
			note: 'Pays in USDC\nmonthly'
		});
		note.value = '';
		note.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		(screen.container.querySelector('button') as HTMLButtonElement).click();
		expect(onsave).toHaveBeenLastCalledWith({ name: 'Bob', address: GOOD, note: '' });
	});

	it('has no note field where the caller gave it none (the gallery)', () => {
		const screen = render(ContactForm, { props: { copy: COPY, onsave: () => {} } });
		expect(screen.container.querySelector('textarea')).toBeNull();
	});
});
