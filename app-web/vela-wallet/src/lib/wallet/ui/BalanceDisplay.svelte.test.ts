/**
 * Issue 462: the hero's "↻ Updated 2m". A press asks for a read; while it is
 * out the glyph turns, the words say "Updating…", the control takes no second
 * press — and nothing on the hero moves, the control's own box included.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { page } from 'vitest/browser';
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
		refresh: {
			updated: updated ?? undefined,
			updating: 'Updating…',
			a11yIdle: 'Refresh balance',
			spinning
		},
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
		// Its name is what it says.
		await expect.element(page.getByRole('button', { name: 'Updated 2m' })).toBeInTheDocument();
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
		await expect.element(page.getByRole('button', { name: 'Updating…' })).toBeInTheDocument();
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

	it('before any read has settled it is the glyph alone, still a door, named', async () => {
		const onrefresh = vi.fn();
		const screen = render(BalanceDisplay, {
			props: { balance: hero(false, null), onrefresh }
		});
		const button = control(screen.container);
		expect(shownWords(button)).toBe('');
		// The glyph is aria-hidden and the invisible label names nothing: the
		// control says what it does, not "Updating…" while nothing is.
		await expect
			.element(page.getByRole('button', { name: 'Refresh balance', exact: true }))
			.toBeInTheDocument();
		expect(button.getAttribute('aria-label')).toBe('Refresh balance');
		button.click();
		expect(onrefresh).toHaveBeenCalledTimes(1);
		// Its first read turns it: then it is "Updating…", like the words.
		await screen.rerender({ balance: hero(true, null), onrefresh });
		expect(control(screen.container).getAttribute('aria-label')).toBe('Updating…');
	});

	// The hero's skeleton — the first read still out, or the display currency
	// not the person's yet (the core's rule) — stands in the figure's own line
	// box. The bar alone was a third shorter than the figure, so the control,
	// and the whole home under the hero, dropped when the figure landed.
	it('the figure arriving moves nothing: the skeleton holds its line', async () => {
		const host = document.createElement('div');
		host.style.width = '390px';
		document.body.appendChild(host);
		const waiting: BalanceModel = {
			...hero(false),
			// CNY stored, its rate on its way: named, and no figure yet.
			currency: 'CNY',
			state: 'loading',
			integer: undefined,
			decimals: undefined,
			status: undefined
		};
		const landed: BalanceModel = {
			...hero(false),
			currency: 'CNY',
			integer: '¥8,761',
			decimals: '40',
			status: undefined
		};
		const screen = render(BalanceDisplay, { target: host, props: { balance: waiting } });
		const measure = () => ({
			control: control(screen.container).getBoundingClientRect().top,
			hero: host.getBoundingClientRect().height
		});
		expect(screen.container.querySelector('.amount')).toBeNull();
		const before = measure();
		await screen.rerender({ balance: landed });
		await tick();
		expect(screen.container.querySelector('.amount')?.textContent).toContain('¥8,761');
		expect(measure()).toEqual(before);
		// …and the tappable figure (live) is the same height as the plain one.
		await screen.rerender({ balance: landed, ontoggle: () => {} });
		await tick();
		expect(measure()).toEqual(before);
		// At a larger text size too: the slot scales with the face.
		host.style.setProperty('--text-scale', '1.35');
		await screen.rerender({ balance: waiting });
		await tick();
		const large = measure();
		await screen.rerender({ balance: landed });
		await tick();
		expect(measure()).toEqual(large);
		screen.unmount();
		host.remove();
	});

	/*
	 * PR 3 note 26b. "Can't reach Gnosis right now", "Some tokens couldn't be
	 * priced", the new wallet's "Live · listening for payments": each arrived
	 * as a line of its own under the figure and pushed the refresh control —
	 * and the whole page under the hero — down a line; its going pulled them
	 * back. The line is the hero's from the first frame now, under the
	 * control, so a status lands in room that was already there.
	 */
	it('a status line arriving, changing and going moves nothing: its line is kept from the first frame', async () => {
		for (const [width, scale] of [
			['390px', '1'],
			['320px', '1'],
			['390px', '1.35']
		] as const) {
			const host = document.createElement('div');
			host.style.width = width;
			host.style.setProperty('--text-scale', scale);
			document.body.appendChild(host);
			const quiet: BalanceModel = { ...hero(false), status: undefined };
			const screen = render(BalanceDisplay, { target: host, props: { balance: quiet } });
			const measure = () => ({
				control: control(screen.container).getBoundingClientRect().top,
				figure: screen.container.querySelector('.amount')!.getBoundingClientRect().top,
				hero: host.getBoundingClientRect().height
			});
			const at = `${width} × ${scale}`;
			const before = measure();
			expect(screen.container.querySelector('.status'), at).toBeNull();

			// The line arrives…
			const warned: BalanceModel = {
				...hero(false),
				status: { kind: 'warning', text: "Can't reach Gnosis right now" }
			};
			await screen.rerender({ balance: warned });
			await tick();
			const status = screen.container.querySelector('.status') as HTMLElement;
			expect(status.textContent, at).toContain("Can't reach Gnosis right now");
			expect(measure(), at).toEqual(before);
			// …under the control a person presses, which is why that never moves.
			expect(status.getBoundingClientRect().top, at).toBeGreaterThanOrEqual(
				control(screen.container).getBoundingClientRect().bottom
			);
			// …and inside the hero's own box: it took room that was already there.
			expect(status.getBoundingClientRect().bottom, at).toBeLessThanOrEqual(
				host.getBoundingClientRect().bottom + 0.5
			);

			// …changes…
			await screen.rerender({
				balance: { ...hero(false), status: { kind: 'refreshing', text: 'Still updating…' } }
			});
			await tick();
			expect(measure(), at).toEqual(before);

			// …and goes.
			await screen.rerender({ balance: quiet });
			await tick();
			expect(screen.container.querySelector('.status'), at).toBeNull();
			expect(measure(), at).toEqual(before);
			screen.unmount();
			host.remove();
		}
	});

	it('the new wallet’s "Live · listening for payments" stands on that same line', async () => {
		const host = document.createElement('div');
		host.style.width = '390px';
		document.body.appendChild(host);
		const waiting: BalanceModel = {
			...hero(false, null),
			state: 'loading',
			integer: undefined,
			decimals: undefined,
			status: undefined
		};
		const screen = render(BalanceDisplay, { target: host, props: { balance: waiting } });
		const measure = () => ({
			control: control(screen.container).getBoundingClientRect().top,
			hero: host.getBoundingClientRect().height
		});
		const before = measure();
		// The first read lands: nothing held, and the wallet says it is listening.
		await screen.rerender({
			balance: {
				...hero(false, 'Updated now'),
				state: 'zero-live',
				integer: '$0',
				decimals: '00',
				status: undefined,
				liveText: 'Live · listening for payments'
			}
		});
		await tick();
		const live = screen.container.querySelector('.live') as HTMLElement;
		expect(live.textContent).toContain('Live · listening for payments');
		expect(measure()).toEqual(before);
		// A status said in its place stands where it stood.
		const liveTop = live.getBoundingClientRect().top;
		await screen.rerender({
			balance: {
				...hero(false, 'Updated now'),
				status: { kind: 'warning', text: "Can't reach Gnosis right now" }
			}
		});
		await tick();
		expect(measure()).toEqual(before);
		const status = screen.container.querySelector('.status') as HTMLElement;
		expect(status.getBoundingClientRect().top).toBe(liveTop);
		screen.unmount();
		host.remove();
	});

	/*
	 * The figure is one line, always. It used to wrap wherever it ran out,
	 * which broke a number inside itself ("₫112,500,00 / 0.00" at 320 px) and
	 * made the hero two lines tall — so the page dropped a line (44.8 px) when
	 * a long figure landed where the one-line skeleton had stood. A figure that
	 * does not fit is drawn smaller in the same line box.
	 */
	it('a figure too long for its line is drawn to fit it — one line, nothing under it moves', async () => {
		for (const live of [false, true]) {
			const host = document.createElement('div');
			// Narrower than the figure in any face (the test page has no web font).
			host.style.width = '240px';
			host.style.overflow = 'hidden';
			document.body.appendChild(host);
			const waiting: BalanceModel = {
				...hero(false),
				currency: 'VND',
				state: 'loading',
				integer: undefined,
				decimals: undefined,
				status: undefined
			};
			const props = (balance: BalanceModel) =>
				live ? { balance, ontoggle: () => {} } : { balance };
			const screen = render(BalanceDisplay, { target: host, props: props(waiting) });
			const measure = () => ({
				control: control(screen.container).getBoundingClientRect().top,
				hero: host.getBoundingClientRect().height
			});
			const before = measure();
			const long: BalanceModel = {
				...hero(false),
				currency: 'VND',
				integer: '₫112,500,000',
				decimals: '00',
				status: undefined
			};
			await screen.rerender(props(long));
			await tick();
			await new Promise((r) => requestAnimationFrame(() => r(null)));
			const figure = screen.container.querySelector('.figure') as HTMLElement;
			const column = host.getBoundingClientRect();
			const drawn = figure.getBoundingClientRect();
			const at = live ? 'tappable' : 'plain';
			// Whole, on one line, inside the column — and smaller than it is set.
			expect(figure.textContent?.replace(/\s+/g, ''), at).toBe('₫112,500,000.00');
			expect(getComputedStyle(figure).whiteSpace, at).toBe('nowrap');
			expect(drawn.right, at).toBeLessThanOrEqual(column.right + 0.5);
			expect(drawn.left, at).toBeGreaterThanOrEqual(column.left - 0.5);
			expect(Number(figure.style.getPropertyValue('--fit')), at).toBeLessThan(1);
			expect(drawn.width, at).toBeGreaterThan(column.width * 0.9);
			// The page is not given a sideways scroll by the figure's layout box.
			expect(host.scrollWidth, at).toBeLessThanOrEqual(host.clientWidth);
			// The hero is as tall as it was with the skeleton, and the control is where it was.
			expect(measure(), at).toEqual(before);

			// A figure that fits is drawn as set: no scale at all.
			await screen.rerender(props({ ...long, integer: '₫1,250' }));
			await tick();
			await new Promise((r) => requestAnimationFrame(() => r(null)));
			expect(
				Number(
					(screen.container.querySelector('.figure') as HTMLElement).style.getPropertyValue('--fit')
				),
				at
			).toBe(1);
			expect(measure(), at).toEqual(before);
			screen.unmount();
			host.remove();
		}
	});

	it('names the currency once it is known, and nothing before', () => {
		const label = (balance: BalanceModel) =>
			render(BalanceDisplay, { props: { balance } })
				.container.querySelector('.label')
				?.textContent?.trim();
		expect(label(hero(false))).toBe('Total balance · USD');
		// Not committed, nothing stored on its way: no currency is named —
		// never the placeholder's "USD", which then changed its mind.
		expect(label({ ...hero(false), currency: undefined, state: 'loading' })).toBe('Total balance');
		expect(label({ ...hero(false), currency: 'CNY', state: 'loading' })).toBe(
			'Total balance · CNY'
		);
	});

	it('a board with no control draws none', () => {
		const screen = render(BalanceDisplay, {
			props: { balance: { ...hero(false), refresh: undefined } }
		});
		expect(screen.container.querySelector('[data-testid="balance-refresh"]')).toBeNull();
	});
});
