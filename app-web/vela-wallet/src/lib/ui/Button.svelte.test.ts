/**
 * The shared button, in a browser: the accent rule (vis-6, owner 2026-10),
 * the link form running its handler (ob-1), and the label following the
 * person's text size (vis-7).
 */
import { createRawSnippet, tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import Button from './Button.svelte';

const label = (text: string) => createRawSnippet(() => ({ render: () => `<span>${text}</span>` }));

/** The colour a token resolves to here, as the browser writes it. */
function resolved(prop: 'color' | 'background-color', token: string): string {
	const probe = document.createElement('div');
	probe.style.setProperty(prop, `var(${token})`);
	document.body.appendChild(probe);
	const value = getComputedStyle(probe).getPropertyValue(prop);
	probe.remove();
	return value;
}

describe('Button', () => {
	it('draws `value` in the accent fill and `primary` in the neutral ink', async () => {
		const value = render(Button, {
			props: { variant: 'value', children: label('Confirm & Send') }
		});
		const primary = render(Button, { props: { variant: 'primary', children: label('Continue') } });
		await tick();
		const send = value.container.querySelector('button') as HTMLButtonElement;
		const next = primary.container.querySelector('button') as HTMLButtonElement;
		// r4D-11: the fill token, under which the white label is AA.
		expect(getComputedStyle(send).backgroundColor).toBe(
			resolved('background-color', '--color-accent-fill')
		);
		expect(getComputedStyle(next).backgroundColor).toBe(
			resolved('background-color', '--color-fg-base')
		);
		expect(getComputedStyle(next).color).toBe(resolved('color', '--color-fg-inverse'));
		expect(getComputedStyle(next).backgroundColor).not.toBe(
			resolved('background-color', '--color-accent-fill')
		);
	});

	it('draws `danger` in the error fill (r4D-11)', async () => {
		const screen = render(Button, { props: { variant: 'danger', children: label('Remove') } });
		await tick();
		const button = screen.container.querySelector('button') as HTMLButtonElement;
		expect(getComputedStyle(button).backgroundColor).toBe(
			resolved('background-color', '--color-error-fill')
		);
	});

	it('keeps its name while it spins (r4D-12)', async () => {
		const screen = render(Button, {
			props: { variant: 'primary', loading: true, children: label('Add') }
		});
		await tick();
		const button = screen.container.querySelector('button') as HTMLButtonElement;
		expect(button.getAttribute('aria-busy')).toBe('true');
		// The label stays in the accessibility tree: not hidden, only see-through.
		const text = button.querySelector('.label') as HTMLElement;
		expect(getComputedStyle(text).visibility).toBe('visible');
		expect(getComputedStyle(text).opacity).toBe('0');
		await expect.element(screen.getByRole('button', { name: 'Add' })).toBeInTheDocument();
	});

	it('as a link, still runs its handler before it goes (ob-1)', async () => {
		const onclick = vi.fn();
		const screen = render(Button, {
			props: { variant: 'primary', href: '#create', onclick, children: label('Create Wallet') }
		});
		await tick();
		const link = screen.container.querySelector('a') as HTMLAnchorElement;
		expect(link).not.toBeNull();
		link.click();
		expect(onclick).toHaveBeenCalledOnce();
	});

	it('sets its label in the person’s text size (vis-7)', async () => {
		const host = document.createElement('div');
		document.body.appendChild(host);
		const screen = render(Button, {
			target: host,
			props: { variant: 'primary', children: label('Continue') }
		});
		await tick();
		const button = screen.container.querySelector('button') as HTMLButtonElement;
		const base = parseFloat(getComputedStyle(button).fontSize);
		host.style.setProperty('--text-scale', '1.35');
		await tick();
		expect(parseFloat(getComputedStyle(button).fontSize)).toBeCloseTo(base * 1.35, 1);
		host.remove();
	});
});
