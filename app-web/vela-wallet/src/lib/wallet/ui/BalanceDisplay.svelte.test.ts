/**
 * Issue 462: the hero's "↻ Updated 2m". A press asks for a read; while it is
 * out the glyph turns, the words say "Updating…", the control takes no second
 * press — and nothing on the hero moves, the control's own box included.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { BalanceModel } from '../model';
import BalanceDisplay from './BalanceDisplay.svelte';

/** `null`: no read has settled yet. */
function hero(spinning: boolean, updated: string | null = 'Updated 2m'): BalanceModel {
	return {
		label: 'Total balance',
		currency: 'USD',
		state: 'normal',
		integer: '$1,383',
		decimals: '28',
		status: { kind: 'warning', text: "Some tokens couldn't be priced." },
		refresh: { updated: updated ?? undefined, updating: 'Updating…', spinning },
		a11yHide: 'Hide balance',
		a11yShow: 'Show balance'
	};
}

function control(container: Element): HTMLButtonElement {
	const button = container.querySelector<HTMLButtonElement>('[data-testid="balance-refresh"]');
	if (button === null) throw new Error('no refresh control');
	return button;
}

/** The words a person can see — the hidden label is laid out, not shown. */
function shownWords(button: HTMLElement): string {
	return [...button.querySelectorAll<HTMLElement>('.words > span')]
		.filter((span) => getComputedStyle(span).visibility === 'visible')
		.map((span) => span.textContent)
		.join('');
}

describe('BalanceDisplay — the refresh control (issue 462)', () => {
	it('says when the figure was read, and a press asks for a read', async () => {
		const onrefresh = vi.fn();
		const screen = render(BalanceDisplay, { props: { balance: hero(false), onrefresh } });
		const button = control(screen.container);
		expect(shownWords(button)).toBe('Updated 2m');
		expect(button.getAttribute('aria-busy')).toBe('false');
		expect(button.hasAttribute('aria-disabled')).toBe(false);
		button.click();
		expect(onrefresh).toHaveBeenCalledTimes(1);
	});

	it('turns and says "Updating…" while the read is out, and takes no second press', async () => {
		const onrefresh = vi.fn();
		const screen = render(BalanceDisplay, { props: { balance: hero(true), onrefresh } });
		const button = control(screen.container);
		expect(shownWords(button)).toBe('Updating…');
		expect(button.getAttribute('aria-busy')).toBe('true');
		expect(button.getAttribute('aria-disabled')).toBe('true');
		const glyph = button.querySelector<HTMLElement>('.glyph')!;
		// Svelte scopes the keyframes' name.
		expect(getComputedStyle(glyph).animationName).toMatch(/spin$/);
		button.click();
		expect(onrefresh).not.toHaveBeenCalled();
		// Busy is never dimmed: the same ink as at rest.
		await screen.rerender({ balance: hero(false), onrefresh });
		const rest = getComputedStyle(control(screen.container)).color;
		await screen.rerender({ balance: hero(true), onrefresh });
		expect(getComputedStyle(control(screen.container)).color).toBe(rest);
		expect(getComputedStyle(control(screen.container)).opacity).toBe('1');
	});

	it('moves nothing when it starts and stops turning', async () => {
		const host = document.createElement('div');
		host.style.width = '390px';
		document.body.appendChild(host);
		const screen = render(BalanceDisplay, { target: host, props: { balance: hero(false) } });
		const box = () => control(screen.container).getBoundingClientRect();
		const status = () => screen.container.querySelector('.status')!.getBoundingClientRect();
		const before = { control: box(), status: status(), hero: host.getBoundingClientRect() };
		for (const spinning of [true, false, true]) {
			await screen.rerender({ balance: hero(spinning) });
			await tick();
			const now = box();
			expect(now.top).toBe(before.control.top);
			expect(now.left).toBe(before.control.left);
			expect(now.width).toBe(before.control.width);
			expect(now.height).toBe(before.control.height);
			expect(status().top).toBe(before.status.top);
			expect(host.getBoundingClientRect().height).toBe(before.hero.height);
		}
		screen.unmount();
		host.remove();
	});

	it('before any read has settled it is the glyph alone, still a door', async () => {
		const onrefresh = vi.fn();
		const screen = render(BalanceDisplay, {
			props: { balance: hero(false, null), onrefresh }
		});
		const button = control(screen.container);
		expect(shownWords(button)).toBe('');
		button.click();
		expect(onrefresh).toHaveBeenCalledTimes(1);
	});

	it('a board with no control draws none', () => {
		const screen = render(BalanceDisplay, {
			props: { balance: { ...hero(false), refresh: undefined } }
		});
		expect(screen.container.querySelector('[data-testid="balance-refresh"]')).toBeNull();
	});
});
