/**
 * The bottom sheet's drag rules (founder, 2026-09-27: one sheet, like Android
 * and iOS). Numbers only — the component's wiring is covered by
 * BottomSheet.svelte.test.ts.
 */
import { describe, expect, it } from 'vitest';
import {
	contentTakesDrag,
	DISMISS_FRACTION,
	FLICK_MIN_DISTANCE,
	FLICK_VELOCITY,
	rubberBand,
	scrimOpacity,
	sheetOffset,
	shouldDismiss,
	SLOP,
	VelocityTracker
} from './sheet-gesture';

const H = 600;

describe('sheetOffset', () => {
	it('follows a downward finger 1:1 when the sheet may close', () => {
		expect(sheetOffset(0, H, true)).toBe(0);
		expect(sheetOffset(120, H, true)).toBe(120);
		expect(sheetOffset(700, H, true)).toBe(700);
	});

	it('resists downward when the sheet may not close — less and less, never a full sheet', () => {
		const a = sheetOffset(100, H, false);
		const b = sheetOffset(400, H, false);
		const c = sheetOffset(5000, H, false);
		expect(a).toBeGreaterThan(0);
		expect(a).toBeLessThan(100);
		expect(b).toBeGreaterThan(a);
		expect(b - a).toBeLessThan(300);
		expect(c).toBeLessThan(H);
	});

	it('rubber-bands upward, whether or not the sheet may close', () => {
		for (const dismissible of [true, false]) {
			const up = sheetOffset(-200, H, dismissible);
			expect(up).toBeLessThan(0);
			expect(up).toBeGreaterThan(-200);
			expect(sheetOffset(-5000, H, dismissible)).toBeGreaterThan(-H);
		}
	});
});

describe('rubberBand', () => {
	it('is zero at rest, grows monotonically and stays under the dimension', () => {
		expect(rubberBand(0, H)).toBe(0);
		expect(rubberBand(-10, H)).toBe(0);
		let last = 0;
		for (const d of [1, 10, 100, 1000, 10000]) {
			const r = rubberBand(d, H);
			expect(r).toBeGreaterThan(last);
			expect(r).toBeLessThan(H);
			last = r;
		}
	});

	it('follows nearly 1:1 at first and ever less after', () => {
		// The slope at 0 is c = 0.55; at a full dimension it has fallen away.
		expect(rubberBand(1, H)).toBeCloseTo(0.55, 1);
		expect(rubberBand(H + 1, H) - rubberBand(H, H)).toBeLessThan(0.3);
	});
});

describe('shouldDismiss', () => {
	const base = { height: H, velocity: 0, dismissible: true };

	it(`closes past ${DISMISS_FRACTION * 100}% of the height, springs back short of it`, () => {
		expect(shouldDismiss({ ...base, offset: H * DISMISS_FRACTION + 1 })).toBe(true);
		expect(shouldDismiss({ ...base, offset: H * DISMISS_FRACTION - 1 })).toBe(false);
		expect(shouldDismiss({ ...base, offset: 40 })).toBe(false);
	});

	it('closes on a fast downward flick even when short', () => {
		expect(
			shouldDismiss({ ...base, offset: FLICK_MIN_DISTANCE + 4, velocity: FLICK_VELOCITY + 0.1 })
		).toBe(true);
		// Too slow: back.
		expect(shouldDismiss({ ...base, offset: 60, velocity: FLICK_VELOCITY - 0.1 })).toBe(false);
		// Fast but it barely moved (a tap that jittered): back.
		expect(shouldDismiss({ ...base, offset: 3, velocity: 2 })).toBe(false);
		// Fast UPWARD: back.
		expect(shouldDismiss({ ...base, offset: 60, velocity: -2 })).toBe(false);
	});

	it('never closes a sheet that may not close — however far or fast', () => {
		const gate = { ...base, dismissible: false };
		expect(shouldDismiss({ ...gate, offset: H * 0.9 })).toBe(false);
		expect(shouldDismiss({ ...gate, offset: 300, velocity: 5 })).toBe(false);
	});

	it('never closes from above rest', () => {
		expect(shouldDismiss({ ...base, offset: -100, velocity: 3 })).toBe(false);
	});
});

describe('scrimOpacity', () => {
	it('dims fully at rest and above it, fading as the sheet goes down', () => {
		expect(scrimOpacity(0, H)).toBe(1);
		expect(scrimOpacity(-50, H)).toBe(1);
		expect(scrimOpacity(H / 2, H)).toBeCloseTo(0.5);
		expect(scrimOpacity(H * 2, H)).toBe(0);
	});
});

describe('VelocityTracker', () => {
	it('measures the recent stretch of the gesture, in px/ms', () => {
		const v = new VelocityTracker();
		v.reset(0, 0);
		// Slow for a long while, then fast at the end: the end is what counts.
		for (let t = 10; t <= 500; t += 10) v.add(t, t * 0.05);
		for (let t = 510; t <= 600; t += 10) v.add(t, 25 + (t - 500) * 2);
		expect(v.velocity()).toBeGreaterThan(1.5);
	});

	it('is zero with nothing to measure', () => {
		const v = new VelocityTracker();
		expect(v.velocity()).toBe(0);
		v.reset(5, 100);
		expect(v.velocity()).toBe(0);
	});
});

describe('contentTakesDrag', () => {
	const at = { atTop: true };

	it('waits until the finger has moved past the slop', () => {
		expect(contentTakesDrag({ ...at, dx: 0, dy: SLOP - 1 })).toBe('undecided');
	});

	it('takes a pull-down that began with the content at its top', () => {
		expect(contentTakesDrag({ ...at, dx: 2, dy: 20 })).toBe('sheet');
	});

	it('leaves a pull-down to the content when it is scrolled', () => {
		expect(contentTakesDrag({ atTop: false, dx: 0, dy: 20 })).toBe('content');
	});

	it('never takes a push UP — scrollable or not (no leftover fling reaches the sheet)', () => {
		expect(contentTakesDrag({ atTop: true, dx: 0, dy: -20 })).toBe('content');
		expect(contentTakesDrag({ atTop: false, dx: 0, dy: -200 })).toBe('content');
	});

	it('leaves a sideways move to the content', () => {
		expect(contentTakesDrag({ ...at, dx: 30, dy: 12 })).toBe('content');
	});
});
