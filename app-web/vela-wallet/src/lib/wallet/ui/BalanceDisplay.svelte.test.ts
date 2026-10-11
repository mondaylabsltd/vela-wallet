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
	 * PR 3 final note F19: the first read of an account says "Checking…" on
	 * that line (the core's `checking_key`), then "Live · listening for
	 * payments" or "Can't reach…" once a round has ended. Three sentences, one
	 * line: the control above it, the figure and the hero's own height are
	 * where they were through all three, at every width and text size.
	 */
	it('"Checking…", "Live" and "Can’t reach" take turns on one line, and nothing moves between them', async () => {
		for (const [width, scale] of [
			['390px', '1'],
			['320px', '1'],
			['390px', '1.35']
		] as const) {
			const at = `${width} × ${scale}`;
			const host = document.createElement('div');
			host.style.width = width;
			host.style.setProperty('--text-scale', scale);
			document.body.appendChild(host);
			// A cached zero, its first read still out.
			const zero: BalanceModel = {
				...hero(false, null),
				integer: '$0',
				decimals: '00',
				status: undefined
			};
			const screen = render(BalanceDisplay, {
				target: host,
				props: { balance: { ...zero, checkingText: 'Checking…' } }
			});
			const said = () => screen.container.querySelector('.said') as HTMLElement;
			const measure = () => ({
				control: control(screen.container).getBoundingClientRect().top,
				figure: screen.container.querySelector('.amount')!.getBoundingClientRect().top,
				slotTop: said().getBoundingClientRect().top,
				slotHeight: said().getBoundingClientRect().height,
				hero: host.getBoundingClientRect().height
			});
			const before = measure();
			const checking = screen.container.querySelector('.checking') as HTMLElement;
			expect(checking.textContent?.trim(), at).toBe('Checking…');
			// Quiet: a line of words, not a door, and not a warning.
			expect(checking.tagName, at).toBe('P');
			expect(screen.container.querySelector('.status'), at).toBeNull();
			const checkingBox = checking.getBoundingClientRect();
			const dot = (line: HTMLElement) =>
				line.querySelector('.live-dot')!.getBoundingClientRect().left;
			const checkingDot = dot(checking);

			// The round settles, every chain answering: live, where it stood.
			await screen.rerender({
				balance: {
					...zero,
					state: 'zero-live',
					liveText: 'Live · listening for payments',
					refresh: { ...zero.refresh!, updated: 'Updated now' }
				}
			});
			await tick();
			expect(screen.container.querySelector('.checking'), at).toBeNull();
			const live = screen.container.querySelector('.live') as HTMLElement;
			expect(live.textContent, at).toContain('Live · listening for payments');
			expect(live.getBoundingClientRect().top, at).toBe(checkingBox.top);
			expect(live.getBoundingClientRect().height, at).toBe(checkingBox.height);
			expect(dot(live), at).toBe(checkingDot);
			expect(measure(), at).toEqual(before);

			// …or it settles with a chain missing: the warning, on that line.
			await screen.rerender({
				balance: {
					...zero,
					status: { kind: 'warning', text: "Can't reach Gnosis right now" },
					refresh: { ...zero.refresh!, updated: 'Updated now' }
				}
			});
			await tick();
			const status = screen.container.querySelector('.status') as HTMLElement;
			expect(status.getBoundingClientRect().top, at).toBe(checkingBox.top);
			expect(status.getBoundingClientRect().height, at).toBe(checkingBox.height);
			expect(measure(), at).toEqual(before);

			// "Checking…" wins the line while it is said: no chain has answered yet.
			await screen.rerender({
				balance: {
					...zero,
					checkingText: 'Checking…',
					status: { kind: 'warning', text: "Can't reach Gnosis right now" }
				}
			});
			await tick();
			expect(screen.container.querySelector('.status'), at).toBeNull();
			expect(screen.container.querySelector('.checking'), at).not.toBeNull();
			expect(measure(), at).toEqual(before);
			screen.unmount();
			host.remove();
		}
	});

	/*
	 * PR 3 final note F16: the status is ONE line. A sentence longer than the
	 * line wrapped onto a second one and grew the line the hero keeps — the
	 * page dropped a line when "No podemos cargar la lista de tokens de Tempo
	 * por ahora" landed at 320 px. It is set a little smaller first (never
	 * under 85 %), then ends in "…"; the whole sentence is the line's name.
	 */
	const frames = () =>
		new Promise<void>((resolve) =>
			requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
		);
	const LONGEST = [
		// `assets.tokenListUnreachable`, the longest two the corpus has.
		'No podemos cargar la lista de tokens de Tempo por ahora',
		"Al momento non riusciamo a caricare l'elenco dei token di Tempo",
		// Vela's own-fault sentence: the first status that was long enough to wrap.
		'Something went wrong inside Vela. If it keeps happening, reopen the app.'
	];

	it('a sentence longer than its line is one line with "…", and the hero is as tall as with none', async () => {
		for (const [width, scale] of [
			['320px', '1'],
			['288px', '1'],
			['390px', '1.35']
		] as const) {
			for (const text of LONGEST) {
				const at = `${text.slice(0, 24)}… @ ${width} × ${scale}`;
				const host = document.createElement('div');
				host.style.width = width;
				host.style.setProperty('--text-scale', scale);
				document.body.appendChild(host);
				const quiet: BalanceModel = { ...hero(false), status: undefined };
				const screen = render(BalanceDisplay, { target: host, props: { balance: quiet } });
				const said = () => screen.container.querySelector('.said') as HTMLElement;
				const measure = () => ({
					control: control(screen.container).getBoundingClientRect().top,
					figure: screen.container.querySelector('.amount')!.getBoundingClientRect().top,
					slot: said().getBoundingClientRect().height,
					hero: host.getBoundingClientRect().height
				});
				const before = measure();

				await screen.rerender({ balance: { ...hero(false), status: { kind: 'warning', text } } });
				await tick();
				await frames();
				expect(measure(), at).toEqual(before);

				const door = screen.container.querySelector('.status') as HTMLButtonElement;
				const sentence = door.querySelector('.sentence') as HTMLElement;
				// One line, inside the hero's column, ending in "…".
				expect(door.getBoundingClientRect().right, at).toBeLessThanOrEqual(
					host.getBoundingClientRect().right + 0.5
				);
				expect(getComputedStyle(sentence).whiteSpace, at).toBe('nowrap');
				expect(getComputedStyle(sentence).textOverflow, at).toBe('ellipsis');
				expect(sentence.scrollWidth, at).toBeGreaterThan(sentence.clientWidth);
				// Set smaller first — to the floor, no further.
				const base = parseFloat(getComputedStyle(door).fontSize);
				expect(parseFloat(getComputedStyle(sentence).fontSize) / base, at).toBeCloseTo(0.85, 2);
				// The whole sentence is still what the line is called, and what it holds.
				expect(door.getAttribute('aria-label'), at).toBe(text);
				expect(door.getAttribute('title'), at).toBe(text);
				expect(sentence.textContent, at).toBe(text);
				// Both glyphs are whole: the warning mark and the chevron.
				const glyphs = [...door.querySelectorAll(':scope > svg')].map((g) =>
					g.getBoundingClientRect()
				);
				expect(glyphs, at).toHaveLength(2);
				expect(glyphs[0].width, at).toBe(glyphs[1].width);
				expect(glyphs[1].right, at).toBeLessThanOrEqual(host.getBoundingClientRect().right + 0.5);
				screen.unmount();
				host.remove();
			}
		}
	});

	it('a sentence a little too long is set a little smaller and drawn whole; a short one is left alone', async () => {
		const host = document.createElement('div');
		host.style.width = '320px';
		document.body.appendChild(host);
		const text = "Can't reach 24 networks right now";
		const screen = render(BalanceDisplay, {
			target: host,
			props: { balance: { ...hero(false), status: { kind: 'warning', text } } }
		});
		await tick();
		await frames();
		const sentence = () => screen.container.querySelector('.sentence') as HTMLElement;
		const door = () => screen.container.querySelector('.status') as HTMLElement;
		const sizeRatio = () =>
			parseFloat(getComputedStyle(sentence()).fontSize) /
			parseFloat(getComputedStyle(door()).fontSize);
		// Room to spare: as set.
		expect(sizeRatio()).toBe(1);
		expect(sentence().style.getPropertyValue('--shrink')).toBe('');
		const heroHeight = host.getBoundingClientRect().height;
		const doorMiddle = () => {
			const box = door().getBoundingClientRect();
			return Math.round((box.top + box.height / 2) * 10) / 10;
		};
		const middle = doorMiddle();

		// The column narrows until the sentence is a little too long for it.
		const full = sentence().getBoundingClientRect().width;
		const beside = door().getBoundingClientRect().width - full;
		host.style.width = `${Math.floor(full * 0.93 + beside)}px`;
		await frames();
		await frames();
		expect(sizeRatio()).toBeGreaterThanOrEqual(0.85);
		expect(sizeRatio()).toBeLessThan(1);
		// Whole: nothing cut.
		expect(sentence().scrollWidth).toBeLessThanOrEqual(sentence().clientWidth + 1);
		// The line did not move or change height for it.
		expect(host.getBoundingClientRect().height).toBe(heroHeight);
		expect(doorMiddle()).toBe(middle);

		// …and the room coming back sets it at full size again.
		host.style.width = '320px';
		await frames();
		await frames();
		expect(sizeRatio()).toBe(1);
		screen.unmount();
		host.remove();
	});

	it('"Checking…" stands under the skeleton too, in the line the figure’s status will take', async () => {
		const host = document.createElement('div');
		host.style.width = '320px';
		document.body.appendChild(host);
		const waiting: BalanceModel = {
			...hero(false, null),
			state: 'loading',
			integer: undefined,
			decimals: undefined,
			status: undefined,
			checkingText: 'Checking…'
		};
		const screen = render(BalanceDisplay, { target: host, props: { balance: waiting } });
		const checkingTop = screen.container.querySelector('.checking')!.getBoundingClientRect().top;
		const heroHeight = host.getBoundingClientRect().height;
		const controlTop = control(screen.container).getBoundingClientRect().top;
		await screen.rerender({
			balance: {
				...hero(false, 'Updated now'),
				status: { kind: 'warning', text: "Can't reach Gnosis right now" }
			}
		});
		await tick();
		expect(screen.container.querySelector('.status')!.getBoundingClientRect().top).toBe(
			checkingTop
		);
		expect(host.getBoundingClientRect().height).toBe(heroHeight);
		expect(control(screen.container).getBoundingClientRect().top).toBe(controlTop);
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
			expect(figure.classList.contains('fitted'), at).toBe(true);
			expect(Number(figure.style.getPropertyValue('--fit')), at).toBeLessThan(1);
			expect(drawn.width, at).toBeGreaterThan(column.width * 0.9);
			// The page is not given a sideways scroll by the figure's layout box.
			expect(host.scrollWidth, at).toBeLessThanOrEqual(host.clientWidth);
			// The hero is as tall as it was with the skeleton, and the control is where it was.
			expect(measure(), at).toEqual(before);

			// A figure that fits is drawn as set: no transform at all.
			await screen.rerender(props({ ...long, integer: '₫1,250' }));
			await tick();
			await new Promise((r) => requestAnimationFrame(() => r(null)));
			const short = screen.container.querySelector('.figure') as HTMLElement;
			expect(short.classList.contains('fitted'), at).toBe(false);
			expect(getComputedStyle(short).transform, at).toBe('none');
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
