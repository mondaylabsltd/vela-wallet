/**
 * What a field is for sits behind an ⓘ beside its label and opens in place
 * (a disclosure, not a hover: phones have none). A state — the badge, a
 * refusal — is never behind it.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import UrlField from './UrlField.svelte';

describe('a field with what it is for', () => {
	it('keeps it behind the ⓘ until it is opened, and says so to assistive tech', async () => {
		const screen = render(UrlField, {
			props: {
				field: {
					id: 'genesis_service',
					label: 'Multisig service',
					value: '',
					info: 'It sees your multisigs.',
					hint: 'Not saved: use https://',
					badge: { tone: 'error', label: 'HTTPS required', dot: true }
				}
			}
		});
		await tick();
		const root = screen.container;
		const button = root.querySelector<HTMLButtonElement>('button[aria-controls]');
		expect(button).toBeTruthy();
		const info = root.querySelector<HTMLElement>(`#${button!.getAttribute('aria-controls')}`);
		expect(info?.textContent).toBe('It sees your multisigs.');
		expect(info?.hidden).toBe(true);
		expect(button!.getAttribute('aria-expanded')).toBe('false');
		// States stay out in the open.
		expect(root.textContent).toContain('Not saved: use https://');
		expect(root.textContent).toContain('HTTPS required');

		button!.click();
		await tick();
		expect(info?.hidden).toBe(false);
		expect(button!.getAttribute('aria-expanded')).toBe('true');
		button!.click();
		await tick();
		expect(info?.hidden).toBe(true);
	});

	it('draws no ⓘ for a field with nothing behind it', async () => {
		const screen = render(UrlField, {
			props: { field: { id: 'rpc', label: 'RPC', value: '', hint: 'A state' } }
		});
		await tick();
		expect(screen.container.querySelector('button[aria-controls]')).toBeNull();
		expect(screen.container.textContent).toContain('A state');
	});
});
