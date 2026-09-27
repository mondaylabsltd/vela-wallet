/**
 * A tap is a tap; a touch that turned into a scroll is not (iPhone,
 * 2026-09-27: a scroll that began on a remove badge deleted the screenshot).
 */
import { describe, expect, it } from 'vitest';
import { SLOP } from '$lib/wallet/ui/sheet-gesture';
import { tapGuard } from './tap-guard';

const pointer = (x: number, y: number, pointerId = 1) =>
	({ pointerId, clientX: x, clientY: y }) as PointerEvent;
const click = (detail = 1) => ({ detail }) as MouseEvent;

describe('tapGuard', () => {
	it('a still press is a tap', () => {
		const guard = tapGuard();
		guard.down(pointer(10, 10));
		guard.move(pointer(12, 11));
		expect(guard.accept(click())).toBe(true);
	});

	it('a press that moved past the slop is not', () => {
		const guard = tapGuard();
		guard.down(pointer(10, 10));
		guard.move(pointer(10, 10 + SLOP + 1));
		// Back where it started: it was still a drag.
		guard.move(pointer(10, 10));
		expect(guard.accept(click())).toBe(false);
	});

	it('a press the browser took for a scroll (pointercancel) is not', () => {
		const guard = tapGuard();
		guard.down(pointer(10, 10));
		guard.cancel(pointer(10, 12));
		expect(guard.accept(click())).toBe(false);
	});

	it('the keyboard (a click with no pointer) and a script always are', () => {
		const guard = tapGuard();
		guard.down(pointer(10, 10));
		guard.cancel(pointer(10, 10));
		expect(guard.accept(click(0))).toBe(true);
		expect(tapGuard().accept(click())).toBe(true);
	});

	it('forgets each press once its click is answered', () => {
		const guard = tapGuard();
		guard.down(pointer(10, 10));
		guard.move(pointer(90, 10));
		expect(guard.accept(click())).toBe(false);
		guard.down(pointer(10, 10));
		expect(guard.accept(click())).toBe(true);
	});

	it('another finger’s movement does not spoil this press', () => {
		const guard = tapGuard();
		guard.down(pointer(10, 10, 1));
		guard.move(pointer(200, 200, 2));
		guard.cancel(pointer(200, 200, 2));
		expect(guard.accept(click())).toBe(true);
	});
});
