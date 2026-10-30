/**
 * THE bottom sheet's behaviour, in a browser (founder, 2026-09-27: one sheet,
 * like Android and iOS): the drag from the grabber and from the content, the
 * dismissible gate, scrim / ✕ / Escape through one close path, and focus in
 * and back out. The numbers themselves are sheet-gesture.test.ts's.
 */
import { createRawSnippet, tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import BottomSheet from './BottomSheet.svelte';
import { SLOP } from './sheet-gesture';

type Variant = 'wallet' | 'menu' | 'signing' | 'prompt';

const body = (html: string) => createRawSnippet(() => ({ render: () => html }));
const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function drawn(
	props: {
		variant?: Variant;
		dismissible?: boolean | 'explicit';
		html?: string;
		closeLabel?: string;
	} = {}
) {
	const onclose = vi.fn();
	const host = document.createElement('div');
	host.style.cssText = 'position: relative; width: 390px; height: 844px; overflow: hidden;';
	document.body.appendChild(host);
	const opener = document.createElement('button');
	opener.textContent = 'open';
	document.body.appendChild(opener);
	opener.focus();
	const screen = render(BottomSheet, {
		target: host,
		props: {
			title: 'Test sheet',
			closeLabel: props.closeLabel ?? 'Close',
			variant: props.variant ?? 'wallet',
			dismissible: props.dismissible ?? true,
			onclose,
			children: body(
				props.html ??
					'<div><p>Body</p><button type="button" class="a">A</button><button type="button" class="b">B</button></div>'
			)
		}
	});
	await tick();
	// Past the entry animation.
	await pause(300);
	const sheet = host.querySelector<HTMLElement>('[role="dialog"]')!;
	const grip = sheet.querySelector<HTMLElement>('.handle')!;
	const content = sheet.querySelector<HTMLElement>('.content')!;
	const scrim = host.querySelector<HTMLElement>('.scrim')!;
	const top = () => sheet.getBoundingClientRect().top;
	const rest = top();
	const height = sheet.getBoundingClientRect().height;
	return { screen, host, opener, sheet, grip, content, scrim, onclose, top, rest, height };
}

/** A finger on the grabber, dragged `dy` over `steps` moves `gap` ms apart. */
async function dragGrip(grip: HTMLElement, dy: number, steps = 10, gap = 16) {
	const box = grip.getBoundingClientRect();
	const x = box.left + box.width / 2;
	const y0 = box.top + 6;
	const fire = (type: string, y: number) =>
		grip.dispatchEvent(
			new PointerEvent(type, {
				pointerId: 7,
				pointerType: 'touch',
				isPrimary: true,
				clientX: x,
				clientY: y,
				bubbles: true,
				cancelable: true
			})
		);
	fire('pointerdown', y0);
	for (let i = 1; i <= steps; i++) {
		fire('pointermove', y0 + (dy * i) / steps);
		if (gap) await pause(gap);
	}
	fire('pointerup', y0 + dy);
	await tick();
}

/** A finger on the content, dragged `dy` (touch events, as a phone sends them). */
async function dragContent(target: HTMLElement, dy: number, steps = 10, gap = 16) {
	const box = target.getBoundingClientRect();
	const x = box.left + box.width / 2;
	const y0 = box.top + Math.min(20, box.height / 2);
	const point = (y: number) =>
		new Touch({ identifier: 3, target, clientX: x, clientY: y, pageX: x, pageY: y });
	const fire = (type: string, y: number | null) => {
		const touches = y === null ? [] : [point(y)];
		target.dispatchEvent(
			new TouchEvent(type, {
				touches,
				targetTouches: touches,
				changedTouches: [point(y ?? y0 + dy)],
				bubbles: true,
				cancelable: true
			})
		);
	};
	fire('touchstart', y0);
	for (let i = 1; i <= steps; i++) {
		fire('touchmove', y0 + (dy * i) / steps);
		if (gap) await pause(gap);
	}
	fire('touchend', null);
	await tick();
}

describe('BottomSheet — one sheet, every skin', () => {
	for (const variant of ['wallet', 'menu', 'signing', 'prompt'] as const) {
		it(`${variant}: draws a grabber and closes on a long drag from it`, async () => {
			const view = await drawn({ variant });
			expect(view.sheet.querySelector('.handle')).not.toBeNull();
			await dragGrip(view.grip, view.height * 0.5);
			await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
			await view.screen.unmount();
		});
	}
});

describe('BottomSheet — the title row stands on the content’s column (r7)', () => {
	// The page hand-off's prompt is the one raised skin that draws its title:
	// its title row once had no gutter, the title flush with the sheet's edge.
	afterEach(() => page.viewport(414, 896));
	for (const [layout, width] of [
		['phone', 430],
		['card', 1440]
	] as const) {
		for (const variant of ['wallet', 'signing', 'prompt'] as const) {
			it(`${variant} (${layout}): the title starts where the content does`, async () => {
				await page.viewport(width, 900);
				const view = await drawn({ variant, html: '<p class="first">Body</p>' });
				const title = view.sheet.querySelector<HTMLElement>('.head h2')!;
				const first = view.content.querySelector<HTMLElement>('.first')!;
				const sheetLeft = view.sheet.getBoundingClientRect().left;
				const titleLeft = title.getBoundingClientRect().left;
				expect(titleLeft - sheetLeft).toBeGreaterThan(0);
				expect(Math.abs(titleLeft - first.getBoundingClientRect().left)).toBeLessThan(1);
				// The ✕ stays inside the sheet, on the gutter.
				const close = view.sheet.querySelector<HTMLElement>('.close')!;
				expect(close.getBoundingClientRect().right).toBeLessThanOrEqual(
					view.sheet.getBoundingClientRect().right + 0.5
				);
				await view.screen.unmount();
			});
		}
	}
});

describe('BottomSheet — the drag', () => {
	it('follows the finger, and a short drag springs back without closing', async () => {
		const view = await drawn();
		// Short: well under the 28% line, slowly (no flick).
		await dragGrip(view.grip, Math.max(SLOP + 6, view.height * 0.15), 10, 24);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);
		await view.screen.unmount();
	});

	it('closes on a fast downward flick, even a short one', async () => {
		const view = await drawn();
		await dragGrip(view.grip, 70, 4, 0);
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('rubber-bands upward and comes back', async () => {
		const view = await drawn();
		const box = view.grip.getBoundingClientRect();
		const fire = (type: string, y: number) =>
			view.grip.dispatchEvent(
				new PointerEvent(type, {
					pointerId: 9,
					pointerType: 'touch',
					clientX: box.left + 10,
					clientY: y,
					bubbles: true
				})
			);
		fire('pointerdown', box.top + 6);
		for (let i = 1; i <= 10; i++) {
			fire('pointermove', box.top + 6 - 20 * i);
			await pause(10);
		}
		// Pulled up 200: it moved, but by less.
		const lifted = view.rest - view.top();
		expect(lifted).toBeGreaterThan(10);
		expect(lifted).toBeLessThan(200);
		fire('pointerup', box.top + 6 - 200);
		await pause(500);
		expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);
		expect(view.onclose).not.toHaveBeenCalled();
		await view.screen.unmount();
	});

	it('drags from the content when it is at its top', async () => {
		const view = await drawn({ html: '<p style="height: 40px">Short body</p>' });
		await dragContent(view.content.firstElementChild as HTMLElement, view.height * 0.5);
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('leaves a pull-down to the content while it is scrolled', async () => {
		const view = await drawn({
			html: '<div style="height: 3000px">Long body</div>'
		});
		view.content.scrollTop = 200;
		await dragContent(view.content.firstElementChild as HTMLElement, view.height * 0.5);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		await view.screen.unmount();
	});

	it('never takes a drag that starts in a text field', async () => {
		const view = await drawn({ html: '<input class="field" value="hello" />' });
		await dragContent(view.content.querySelector('.field') as HTMLElement, view.height * 0.5);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);
		await view.screen.unmount();
	});
});

describe('BottomSheet — the dismissible gate', () => {
	it('resists a long drag and comes back, with nothing closing it', async () => {
		const view = await drawn({ dismissible: false });
		const box = view.grip.getBoundingClientRect();
		const fire = (type: string, y: number) =>
			view.grip.dispatchEvent(
				new PointerEvent(type, {
					pointerId: 5,
					pointerType: 'touch',
					clientX: box.left + 10,
					clientY: y,
					bubbles: true
				})
			);
		fire('pointerdown', box.top + 6);
		for (let i = 1; i <= 10; i++) {
			fire('pointermove', box.top + 6 + view.height * 0.06 * i);
			await pause(16);
		}
		// Pulled 60% of its height: it gave, but far less than the finger moved.
		const moved = view.top() - view.rest;
		expect(moved).toBeGreaterThan(5);
		expect(moved).toBeLessThan(view.height * 0.6 * 0.7);
		fire('pointerup', box.top + 6 + view.height * 0.6);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);

		// A flick, the scrim, Escape, the ✕: nothing.
		await dragGrip(view.grip, 90, 3, 0);
		view.scrim.click();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		const close = view.sheet.querySelector<HTMLButtonElement>('.close')!;
		expect(close.disabled).toBe(true);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(view.sheet.isConnected).toBe(true);
		await view.screen.unmount();
	});

	it('the host can still close it (its own confirm)', async () => {
		const view = await drawn({ dismissible: false });
		(view.screen.component as unknown as { requestClose: () => void }).requestClose();
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});
});

describe('BottomSheet — explicit close only (spec 079)', () => {
	// The signing sheet and the consent card: a close there is the dApp's
	// 4001, and the owner lost a request to a stray touch. Only the ✕ (or the
	// host) closes them.
	for (const variant of ['wallet', 'signing'] as const) {
		it(`${variant}: a drag from the grabber takes nothing — no movement, no close`, async () => {
			const view = await drawn({ variant, dismissible: 'explicit' });
			await dragGrip(view.grip, view.height * 0.6, 10, 16);
			// Not even a rubber band: the drag is never taken.
			expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);
			await dragGrip(view.grip, 90, 3, 0); // …nor a flick
			await pause(500);
			expect(view.onclose).not.toHaveBeenCalled();
			expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);
			await view.screen.unmount();
		});
	}

	it('a pull on the content at its top takes nothing', async () => {
		const view = await drawn({
			dismissible: 'explicit',
			html: '<p style="height: 40px">Short body</p>'
		});
		await dragContent(view.content.firstElementChild as HTMLElement, view.height * 0.5);
		await pause(500);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(Math.abs(view.top() - view.rest)).toBeLessThan(1);
		await view.screen.unmount();
	});

	it('the scrim and Escape do nothing', async () => {
		const view = await drawn({ dismissible: 'explicit' });
		view.scrim.click();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		await pause(600);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(view.sheet.isConnected).toBe(true);
		await view.screen.unmount();
	});

	it('draws no grabber, and keeps its room', async () => {
		const view = await drawn({ dismissible: 'explicit' });
		const handle = view.sheet.querySelector<HTMLElement>('.handle')!;
		expect(getComputedStyle(handle).visibility).toBe('hidden');
		expect(handle.getBoundingClientRect().height).toBeGreaterThan(0);
		await view.screen.unmount();
	});

	it('the ✕ closes it, once, after the exit', async () => {
		const view = await drawn({ dismissible: 'explicit' });
		const close = view.sheet.querySelector<HTMLButtonElement>('.close')!;
		expect(close.disabled).toBe(false);
		close.click();
		close.click();
		expect(view.onclose).not.toHaveBeenCalled(); // the exit plays first
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await pause(300);
		expect(view.onclose).toHaveBeenCalledOnce();
		await view.screen.unmount();
	});

	it('a ✕ drawn by the content closes it through the same door (`close()`)', async () => {
		const view = await drawn({ variant: 'signing', dismissible: 'explicit' });
		(view.screen.component as unknown as { close: () => void }).close();
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('locked (a signature in flight), the content’s ✕ does nothing either', async () => {
		const view = await drawn({ variant: 'signing', dismissible: false });
		(view.screen.component as unknown as { close: () => void }).close();
		await pause(600);
		expect(view.onclose).not.toHaveBeenCalled();
		await view.screen.unmount();
	});
});

describe('BottomSheet — the other doors, one close path', () => {
	it('the scrim closes it (after the exit)', async () => {
		const view = await drawn();
		view.scrim.click();
		expect(view.onclose).not.toHaveBeenCalled(); // the exit plays first
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('the ✕ closes it', async () => {
		const view = await drawn();
		view.sheet.querySelector<HTMLButtonElement>('.close')!.click();
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('Escape closes it — but not while an IME is composing', async () => {
		const view = await drawn();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', isComposing: true }));
		await pause(400);
		expect(view.onclose).not.toHaveBeenCalled();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await view.screen.unmount();
	});

	it('closes once, however many doors are knocked on', async () => {
		const view = await drawn();
		view.scrim.click();
		view.sheet.querySelector<HTMLButtonElement>('.close')!.click();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		await pause(700);
		expect(view.onclose).toHaveBeenCalledOnce();
		await view.screen.unmount();
	});
});

describe('BottomSheet — focus', () => {
	it('moves focus in, keeps Tab inside, and gives it back to the opener', async () => {
		const view = await drawn();
		expect(view.sheet.contains(document.activeElement)).toBe(true);
		const buttons = [...view.sheet.querySelectorAll<HTMLButtonElement>('button')];
		const last = buttons[buttons.length - 1];
		last.focus();
		window.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true })
		);
		expect(document.activeElement).toBe(buttons[0]);
		buttons[0].focus();
		window.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true, cancelable: true })
		);
		expect(document.activeElement).toBe(last);
		await view.screen.unmount();
		expect(document.activeElement).toBe(view.opener);
	});
});

/**
 * `reveal` (owner, 2026-10-28): what lands late is brought into the body's
 * window — but the body is the person's while they use it. Their hand on it
 * defers the scroll until it has settled; once they have scrolled it
 * themselves, a cue says where the new thing is instead, and a tap on it
 * goes there. Only the body moves: the frame and its foot stay.
 */
describe('BottomSheet — reveal', () => {
	const CUE = { below: 'New check result below ↓', above: 'New check result above ↑' };
	const TALL =
		'<div style="height: 900px">Top</div>' +
		'<div data-verdict style="height: 120px">Verdict</div>' +
		'<div style="height: 600px">After</div>';
	const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

	/** Reduced motion asked for, or not: `reveal` reads it each time it scrolls. */
	const realMatchMedia = window.matchMedia.bind(window);
	function motion(reduced: boolean) {
		return vi.spyOn(window, 'matchMedia').mockImplementation((query: string) => {
			if (!query.includes('prefers-reduced-motion')) return realMatchMedia(query);
			return {
				matches: reduced,
				media: query,
				onchange: null,
				addListener: () => {},
				removeListener: () => {},
				addEventListener: () => {},
				removeEventListener: () => {},
				dispatchEvent: () => false
			} as MediaQueryList;
		});
	}
	afterEach(() => vi.restoreAllMocks());

	async function revealing(props: { cue?: boolean } = {}) {
		const screen = render(BottomSheet, {
			props: {
				title: 'Signing',
				hideTitle: true,
				variant: 'signing',
				dismissible: 'explicit',
				onclose: vi.fn(),
				revealCue: props.cue === false ? undefined : CUE,
				footer: body('<button type="button" data-testid="confirm">Confirm</button>'),
				children: body(`<div>${TALL}</div>`)
			}
		});
		await tick();
		await pause(300);
		const sheet = document.querySelector<HTMLElement>('[role="dialog"]')!;
		const content = sheet.querySelector<HTMLElement>('.content')!;
		const verdict = sheet.querySelector<HTMLElement>('[data-verdict]')!;
		const confirm = sheet.querySelector<HTMLElement>('[data-testid="confirm"]')!;
		const cue = () => sheet.querySelector<HTMLButtonElement>('[data-testid="sheet-reveal-cue"]');
		const inSight = () => {
			const box = verdict.getBoundingClientRect();
			const view = content.getBoundingClientRect();
			return box.top >= view.top - 0.5 && box.bottom <= view.bottom + 0.5;
		};
		/** What must never move: the frame, and the confirm in its foot. */
		const still = () => ({
			sheet: sheet.getBoundingClientRect().top,
			confirm: confirm.getBoundingClientRect().top
		});
		/** Where the body is scrolled to when the verdict is at its window's top. */
		const verdictAt = () =>
			content.scrollTop + verdict.getBoundingClientRect().top - content.getBoundingClientRect().top;
		const reveal = () => screen.component.reveal('[data-verdict]');
		expect(content.scrollHeight).toBeGreaterThan(content.clientHeight);
		expect(inSight()).toBe(false);
		return { screen, sheet, content, verdict, confirm, cue, inSight, still, reveal, verdictAt };
	}

	/** The person scrolls the body to `top` themselves: the wheel, then the scroll it makes. */
	async function scrollByHand(content: HTMLElement, top: number) {
		content.dispatchEvent(
			new WheelEvent('wheel', { deltaY: top - content.scrollTop, bubbles: true })
		);
		content.scrollTop = top;
		await frame();
		await frame();
		// Past the settle.
		await pause(400);
	}

	it('with nobody touching the body, scrolls the verdict into sight — and only the body moves', async () => {
		const reduced = motion(true);
		const view = await revealing();
		const was = view.still();
		view.reveal();
		await tick();
		expect(view.inSight()).toBe(true);
		expect(view.cue()).toBeNull();
		expect(view.still()).toEqual(was);
		reduced.mockRestore();
		await view.screen.unmount();
	});

	it('reduced motion scrolls at once; otherwise a short smooth scroll', async () => {
		const reduced = motion(true);
		const first = await revealing();
		const instant = vi.spyOn(first.content, 'scrollTo');
		first.reveal();
		await tick();
		expect(instant).toHaveBeenCalledOnce();
		expect(instant.mock.calls[0][0]).toMatchObject({ behavior: 'instant' });
		expect(first.inSight()).toBe(true);
		reduced.mockRestore();
		await first.screen.unmount();

		const full = motion(false);
		const second = await revealing();
		const smooth = vi.spyOn(second.content, 'scrollTo');
		const was = second.still();
		second.reveal();
		await tick();
		expect(smooth).toHaveBeenCalledOnce();
		expect(smooth.mock.calls[0][0]).toMatchObject({ behavior: 'smooth' });
		await vi.waitFor(() => expect(second.inSight()).toBe(true), { timeout: 2000 });
		expect(second.still()).toEqual(was);
		full.mockRestore();
		await second.screen.unmount();
	});

	const holds: [string, (content: HTMLElement) => () => void][] = [
		[
			'a finger on it',
			(content) => {
				const touch = new Touch({ identifier: 1, target: content, clientX: 10, clientY: 10 });
				content.dispatchEvent(
					new TouchEvent('touchstart', {
						touches: [touch],
						changedTouches: [touch],
						bubbles: true
					})
				);
				return () =>
					content.dispatchEvent(
						new TouchEvent('touchend', { touches: [], changedTouches: [touch], bubbles: true })
					);
			}
		],
		[
			'a press on it (its scrollbar, a mouse, a pen)',
			(content) => {
				content.dispatchEvent(new PointerEvent('pointerdown', { pointerId: 2, bubbles: true }));
				return () =>
					window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 2, bubbles: true }));
			}
		],
		[
			'a scrolling key held down',
			(content) => {
				content.tabIndex = -1;
				content.focus();
				content.dispatchEvent(new KeyboardEvent('keydown', { key: 'PageDown', bubbles: true }));
				return () =>
					content.dispatchEvent(new KeyboardEvent('keyup', { key: 'PageDown', bubbles: true }));
			}
		],
		[
			'the wheel turning',
			(content) => {
				content.dispatchEvent(new WheelEvent('wheel', { deltaY: 0, bubbles: true }));
				return () => {};
			}
		]
	];

	for (const [name, hold] of holds) {
		it(`waits while ${name}, then reveals once it has settled`, async () => {
			const reduced = motion(true);
			const view = await revealing();
			const scrollTo = vi.spyOn(view.content, 'scrollTo');
			const letGo = hold(view.content);
			view.reveal();
			await tick();
			await pause(100);
			// Mid-gesture: nothing is scrolled under the person's hand.
			expect(scrollTo).not.toHaveBeenCalled();
			expect(view.content.scrollTop).toBe(0);
			letGo();
			await pause(100);
			// Let go, not yet settled.
			expect(scrollTo).not.toHaveBeenCalled();
			await vi.waitFor(() => expect(view.inSight()).toBe(true), { timeout: 2000 });
			// Nothing they did was a scroll: the reveal is carried out, no cue.
			expect(scrollTo).toHaveBeenCalledOnce();
			expect(view.cue()).toBeNull();
			reduced.mockRestore();
			await view.screen.unmount();
		});
	}

	it('the person scrolled away themselves: no scroll, a cue — and their own scroll to it clears it', async () => {
		const reduced = motion(true);
		const view = await revealing();
		await scrollByHand(view.content, 100);
		const scrollTo = vi.spyOn(view.content, 'scrollTo');
		const was = view.still();
		view.reveal();
		await tick();
		await pause(400);
		expect(scrollTo).not.toHaveBeenCalled();
		expect(view.content.scrollTop).toBe(100);
		expect(view.still()).toEqual(was);
		// The cue, pointing down, on the body's edge — outside the scroll, over no confirm.
		const cue = view.cue();
		expect(cue).not.toBeNull();
		expect(cue!.textContent?.trim()).toBe(CUE.below);
		expect(view.content.contains(cue)).toBe(false);
		expect(cue!.getBoundingClientRect().bottom).toBeLessThanOrEqual(
			view.content.getBoundingClientRect().bottom + 0.5
		);
		expect(cue!.getBoundingClientRect().bottom).toBeLessThanOrEqual(
			view.confirm.getBoundingClientRect().top
		);
		// They scroll to it themselves: the cue goes.
		await scrollByHand(view.content, view.verdictAt() - 50);
		expect(view.inSight()).toBe(true);
		await tick();
		expect(view.cue()).toBeNull();
		reduced.mockRestore();
		await view.screen.unmount();
	});

	it('a tap on the cue scrolls the verdict into sight, and the cue goes', async () => {
		const reduced = motion(true);
		const view = await revealing();
		await scrollByHand(view.content, 100);
		view.reveal();
		await tick();
		const was = view.still();
		view.cue()!.click();
		await frame();
		await frame();
		expect(view.inSight()).toBe(true);
		expect(view.still()).toEqual(was);
		await tick();
		expect(view.cue()).toBeNull();
		reduced.mockRestore();
		await view.screen.unmount();
	});

	it('points up at a verdict above where the person has scrolled to', async () => {
		const reduced = motion(true);
		const view = await revealing();
		await scrollByHand(view.content, view.content.scrollHeight);
		expect(view.inSight()).toBe(false);
		view.reveal();
		await tick();
		expect(view.cue()!.textContent?.trim()).toBe(CUE.above);
		expect(view.cue()!.dataset.side).toBe('above');
		view.cue()!.click();
		await frame();
		await frame();
		expect(view.inSight()).toBe(true);
		reduced.mockRestore();
		await view.screen.unmount();
	});

	it('with no words for a cue, it scrolls even after the person did: never out of sight with nothing said', async () => {
		const reduced = motion(true);
		const view = await revealing({ cue: false });
		await scrollByHand(view.content, 100);
		view.reveal();
		await tick();
		expect(view.inSight()).toBe(true);
		expect(view.cue()).toBeNull();
		reduced.mockRestore();
		await view.screen.unmount();
	});

	it('a verdict already in sight moves nothing and raises no cue', async () => {
		const reduced = motion(true);
		const view = await revealing();
		await scrollByHand(view.content, view.verdictAt() - 50);
		expect(view.inSight()).toBe(true);
		const at = view.content.scrollTop;
		view.reveal();
		await tick();
		expect(view.content.scrollTop).toBe(at);
		expect(view.cue()).toBeNull();
		reduced.mockRestore();
		await view.screen.unmount();
	});
});
