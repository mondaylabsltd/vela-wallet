/**
 * A switch row says where it stands and reports a flip — and its keyboard
 * focus is the app's one neutral ring (owner, 2026-10-29: a reddish outline
 * stood around "Share anonymous usage statistics"; the ring was the accent,
 * and the row drew a second outline of its own outside it).
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';
import '$lib/tokens/tokens.css';
import '../../../app.css';
import SwitchRow from './SwitchRow.svelte';

const ROW = {
	id: 'settings-analytics',
	icon: 'eye-off' as const,
	title: 'Share anonymous usage statistics',
	subtitle: 'Which screens open and which steps finish — never addresses, amounts or names.',
	on: true
};

/** A token's colour as the browser computes it (`rgb(…)`). */
function tokenColour(name: string): string {
	const probe = document.createElement('span');
	probe.style.color = `var(${name})`;
	document.body.appendChild(probe);
	const colour = getComputedStyle(probe).color;
	probe.remove();
	return colour;
}

describe('SwitchRow', () => {
	it('shows where it stands and reports the flip', async () => {
		const flips: boolean[] = [];
		const screen = render(SwitchRow, { props: { row: ROW, onchange: (on) => flips.push(on) } });
		await tick();
		const toggle = screen.container.querySelector('[role="switch"]') as HTMLButtonElement;
		expect(toggle.getAttribute('aria-checked')).toBe('true');
		expect(toggle.textContent).toContain(ROW.title);
		expect(toggle.textContent).toContain(ROW.subtitle);
		expect(toggle.querySelector('.glyph svg')).not.toBe(null);
		toggle.click();
		expect(flips).toEqual([false]);
		await screen.unmount();
	});

	it('takes keyboard focus with the neutral ring, and no outline of its own', async () => {
		const screen = render(SwitchRow, { props: { row: ROW } });
		await tick();
		const toggle = screen.container.querySelector('[role="switch"]') as HTMLButtonElement;
		// Focused as the keyboard focuses it (a Tab into the frame lands on its body).
		toggle.focus({ focusVisible: true } as FocusOptions);
		await userEvent.keyboard('{Shift}');
		expect(document.activeElement).toBe(toggle);
		expect(toggle.matches(':focus-visible')).toBe(true);
		const style = getComputedStyle(toggle);
		// The ring is app.css's: drawn in fg.base, its outline transparent.
		expect(style.boxShadow).toContain(tokenColour('--color-fg-base'));
		expect(style.outlineColor).toBe('rgba(0, 0, 0, 0)');
		// And never the accent, anywhere on it.
		const accent = tokenColour('--color-accent-base');
		expect(style.boxShadow).not.toContain(accent);
		expect(style.outlineColor).not.toBe(accent);
		await screen.unmount();
	});
});
