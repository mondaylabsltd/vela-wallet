/**
 * R22 (review 1-3): the held confirm's rule — a hold that completes confirms,
 * an early release does nothing, and the assistive path (a click no pointer
 * press preceded: Enter, Space, a screen reader's activation) arms on the
 * first activation and confirms on a second inside the window; past it, it
 * disarms. `hold_ms == 0` is a plain press.
 */
import { describe, expect, it } from 'vitest';
import { holdConfirm, type HoldPhase, type HoldTimers } from './hold-confirm';

/** A clock moved by hand. */
function handClock() {
	let now = 0;
	let next = 1;
	const due = new Map<number, { at: number; run: () => void }>();
	const timers: HoldTimers = {
		set(run, ms) {
			const id = next++;
			due.set(id, { at: now + ms, run });
			return id;
		},
		clear(handle) {
			due.delete(handle as number);
		}
	};
	return {
		timers,
		advance(ms: number) {
			now += ms;
			for (const [id, timer] of [...due].sort((a, b) => a[1].at - b[1].at)) {
				if (timer.at <= now && due.has(id)) {
					due.delete(id);
					timer.run();
				}
			}
		}
	};
}

function setup(holdMs = 1500, windowMs = 5000) {
	const clock = handClock();
	let confirmed = 0;
	const phases: HoldPhase[] = [];
	const control = holdConfirm({
		holdMs: () => holdMs,
		windowMs: () => windowMs,
		onconfirm: () => (confirmed += 1),
		onphase: (phase) => phases.push(phase),
		timers: clock.timers
	});
	return { control, clock, phases, confirmed: () => confirmed };
}

describe('holdConfirm (R22, the held confirm)', () => {
	it('confirms once a hold completes — and the click after it is no second confirm', () => {
		const { control, clock, confirmed, phases } = setup();
		control.pressStart();
		expect(control.phase).toBe('holding');
		clock.advance(1499);
		expect(confirmed()).toBe(0);
		clock.advance(1);
		expect(confirmed()).toBe(1);
		control.pressEnd();
		control.activate();
		expect(confirmed()).toBe(1);
		expect(phases).toEqual(['holding', 'idle']);
	});

	it('does nothing when the press is let go of early', () => {
		const { control, clock, confirmed } = setup();
		control.pressStart();
		clock.advance(800);
		control.pressEnd();
		control.activate(); // the click a short press still sends
		clock.advance(5000);
		expect(confirmed()).toBe(0);
		expect(control.phase).toBe('idle');
	});

	it('arms on an assistive activation, and confirms on the second inside the window', () => {
		const { control, clock, confirmed, phases } = setup();
		control.activate();
		expect(control.phase).toBe('armed');
		expect(confirmed()).toBe(0);
		clock.advance(4999);
		control.activate();
		expect(confirmed()).toBe(1);
		expect(phases).toEqual(['armed', 'idle']);
	});

	it('disarms once the window has passed: the next activation arms again', () => {
		const { control, clock, confirmed } = setup();
		control.activate();
		clock.advance(5000);
		expect(control.phase).toBe('idle');
		control.activate();
		expect(confirmed()).toBe(0);
		expect(control.phase).toBe('armed');
	});

	it('is a plain press when the core holds nothing (hold_ms 0)', () => {
		const { control, confirmed } = setup(0);
		control.pressStart();
		control.pressEnd();
		control.activate();
		expect(confirmed()).toBe(1);
		control.activate();
		expect(confirmed()).toBe(2);
	});

	it('drops a hold and an arming when reset (shut, busy, gone)', () => {
		const { control, clock, confirmed } = setup();
		control.pressStart();
		control.reset();
		clock.advance(2000);
		control.activate();
		expect(confirmed()).toBe(0);
		expect(control.phase).toBe('armed');
		control.reset();
		control.activate();
		expect(confirmed()).toBe(0);
	});
});
