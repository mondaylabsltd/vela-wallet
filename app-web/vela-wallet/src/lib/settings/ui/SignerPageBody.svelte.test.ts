/**
 * The Clear Signer's page, in a browser (spec 071, contract §6): Save hands
 * over what was TYPED, the core's refusal is drawn under the field with the
 * typed text still there to fix, and the page's own copy is only replaced
 * when the page in force actually changes — not every time the settings
 * model is rebuilt for something unrelated.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { SignerPageModel } from '../model';
import SignerPageBody from './SignerPageBody.svelte';

const OFFICIAL = 'https://sign.getvela.app/';

const PAGE: SignerPageModel = {
	title: 'Clear Signer page',
	subtitle: 'The page the Clear Signer opens.',
	field: { id: 'signer-url', label: 'Clear Signer page', value: OFFICIAL },
	save: 'Save'
};

async function drawn(page: SignerPageModel = PAGE) {
	const saved: string[] = [];
	let resets = 0;
	const screen = render(SignerPageBody, {
		props: { page, onsave: (text: string) => saved.push(text), onreset: () => (resets += 1) }
	});
	await tick();
	const input = () => screen.container.querySelector('input') as HTMLInputElement;
	return {
		screen,
		saved,
		resets: () => resets,
		input,
		async type(text: string) {
			input().value = text;
			input().dispatchEvent(new Event('input', { bubbles: true }));
			await tick();
		},
		button: (name: string) =>
			[...screen.container.querySelectorAll('button')].find(
				(button) => button.textContent?.trim() === name
			) as HTMLButtonElement | undefined,
		text: () => screen.container.textContent ?? ''
	};
}

describe('the Clear Signer page', () => {
	it('shows the page in force, and Save hands over what was typed', async () => {
		const view = await drawn();
		expect(view.input().value).toBe(OFFICIAL);
		await view.type('http://127.0.0.1:8137');
		view.button('Save')?.click();
		expect(view.saved).toEqual(['http://127.0.0.1:8137']);
	});

	it('Enter in the field is Save', async () => {
		const view = await drawn();
		await view.type('sign.example.org');
		view.input().form?.requestSubmit();
		expect(view.saved).toEqual(['sign.example.org']);
	});

	it('a refusal is drawn under the field, over the text still there to fix', async () => {
		const view = await drawn();
		await view.type('http://sign.example.org');
		// The core refused: nothing in force changed, the model is rebuilt with its reason.
		await view.screen.rerender({
			page: { ...PAGE, field: { ...PAGE.field }, error: 'Use an https page.' }
		});
		await tick();
		expect(view.text()).toContain('Use an https page.');
		expect(view.input().value).toBe('http://sign.example.org');
	});

	it('a new page in force replaces the typed text; offers the reset; says the rpId line', async () => {
		const view = await drawn();
		expect(view.button('Use the official page')).toBeUndefined();
		await view.type('sign.example.org');
		await view.screen.rerender({
			page: {
				...PAGE,
				field: { ...PAGE.field, value: 'https://sign.example.org/' },
				reset: 'Use the official page',
				foreign: 'Your passkeys belong to getvela.app.'
			}
		});
		await tick();
		expect(view.input().value).toBe('https://sign.example.org/');
		expect(view.text()).toContain('Your passkeys belong to getvela.app.');
		view.button('Use the official page')?.click();
		expect(view.resets()).toBe(1);
	});
});
