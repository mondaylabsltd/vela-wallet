/**
 * R22 (review 1-3): the held confirm, in a browser — a pointer holds (the
 * fill grows, an early release does nothing, a hold that completes confirms
 * once); a keyboard or screen-reader activation arms it, says "Press again to
 * confirm", and a second inside the window confirms; past it, it disarms.
 * `holdMs` 0 is the plain button.
 */
import { createRawSnippet, tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import HoldConfirm from './HoldConfirm.svelte';
import type { HoldTimers } from './hold-confirm';

const label = createRawSnippet(() => ({ render: () => '<span>Sign anyway</span>' }));

function handClock() {
	let now = 0;
	let id = 1;
	const due = new Map<number, { at: number; run: () => void }>();
	const timers: HoldTimers = {
		set(run, ms) {
			due.set(id, { at: now + ms, run });
			return id++;
		},
		clear(handle) {
			due.delete(handle as number);
		}
	};
	return {
		timers,
		advance(ms: number) {
			now += ms;
			for (const [key, t] of [...due]) {
				if (t.at <= now && due.has(key)) {
					due.delete(key);
					t.run();
				}
			}
		}
	};
}

function mount(holdMs = 1500) {
	const clock = handClock();
	const onconfirm = vi.fn();
	const screen = render(HoldConfirm, {
		props: {
			variant: 'danger',
			holdMs,
			holdLabel: 'Hold to confirm',
			againLabel: 'Press again to confirm',
			windowMs: 5000,
			testid: 'confirm',
			timers: clock.timers,
			onconfirm,
			children: label
		}
	});
	const button = screen.container.querySelector('button') as HTMLButtonElement;
	const hint = () =>
		screen.container.querySelector('[data-testid="confirm-hint"]')?.textContent?.trim();
	const press = () =>
		button.dispatchEvent(
			new PointerEvent('pointerdown', { bubbles: true, isPrimary: true, button: 0, pointerId: 1 })
		);
	const release = () => {
		button.dispatchEvent(
			new PointerEvent('pointerup', { bubbles: true, isPrimary: true, pointerId: 1 })
		);
		// The click a press-and-release sends.
		button.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }));
	};
	return { clock, onconfirm, button, hint, press, release, screen };
}

describe('HoldConfirm (R22)', () => {
	it('a hold that completes confirms — once — and the fill runs for the core’s hold', async () => {
		const { clock, onconfirm, button, press, release, screen } = mount();
		await tick();
		expect(button.getAttribute('aria-describedby')).toBeTruthy();
		press();
		await tick();
		expect(screen.container.querySelector('[data-phase="holding"]')).not.toBeNull();
		clock.advance(1499);
		expect(onconfirm).not.toHaveBeenCalled();
		clock.advance(1);
		expect(onconfirm).toHaveBeenCalledTimes(1);
		release();
		expect(onconfirm).toHaveBeenCalledTimes(1);
	});

	it('letting go early does nothing', async () => {
		const { clock, onconfirm, press, release } = mount();
		await tick();
		press();
		clock.advance(700);
		release();
		clock.advance(5000);
		expect(onconfirm).not.toHaveBeenCalled();
	});

	it('the keyboard / screen-reader path: the first activation arms and says so, the second confirms', async () => {
		const { onconfirm, button, hint } = mount();
		await tick();
		expect(hint()).toBe('Hold to confirm');
		// Enter, Space or an assistive activation: a click no pointer press preceded.
		button.click();
		await tick();
		expect(onconfirm).not.toHaveBeenCalled();
		expect(hint()).toBe('Press again to confirm');
		button.click();
		expect(onconfirm).toHaveBeenCalledTimes(1);
	});

	it('past the window the arming lapses: the next activation arms again', async () => {
		const { clock, onconfirm, button, hint } = mount();
		await tick();
		button.click();
		clock.advance(5000);
		await tick();
		expect(hint()).toBe('Hold to confirm');
		button.click();
		expect(onconfirm).not.toHaveBeenCalled();
	});

	it('holdMs 0: the plain button, one activation confirms, nothing said beside it', async () => {
		const { onconfirm, button, hint } = mount(0);
		await tick();
		button.click();
		expect(onconfirm).toHaveBeenCalledTimes(1);
		expect(hint()).toBeUndefined();
	});
});
