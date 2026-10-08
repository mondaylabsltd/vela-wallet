/**
 * Issue 462: a press turns the hero's refresh glyph for at least 650 ms,
 * however fast the core answers; the core's own flag covers the rest.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { REFRESH_AGE_TICK_MS, REFRESH_MIN_SPIN_MS, RefreshHold } from './refresh-hold.svelte';

describe('RefreshHold', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('holds a press for 650 ms, and no longer', () => {
		expect(REFRESH_MIN_SPIN_MS).toBe(650);
		const hold = new RefreshHold();
		expect(hold.held).toBe(false);
		hold.press();
		expect(hold.held).toBe(true);
		vi.advanceTimersByTime(649);
		expect(hold.held).toBe(true);
		vi.advanceTimersByTime(1);
		expect(hold.held).toBe(false);
	});

	it('a second press starts the hold again', () => {
		const hold = new RefreshHold();
		hold.press();
		vi.advanceTimersByTime(500);
		hold.press();
		vi.advanceTimersByTime(500);
		expect(hold.held).toBe(true);
		vi.advanceTimersByTime(150);
		expect(hold.held).toBe(false);
	});

	it('leaves nothing running once the page has gone', () => {
		const hold = new RefreshHold();
		hold.press();
		hold.dispose();
		expect(hold.held).toBe(false);
		expect(vi.getTimerCount()).toBe(0);
	});

	it('words the age again well inside the 30 s the label may lag', () => {
		expect(REFRESH_AGE_TICK_MS).toBeLessThanOrEqual(30_000);
	});
});
