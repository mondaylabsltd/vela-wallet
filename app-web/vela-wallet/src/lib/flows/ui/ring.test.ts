/**
 * The receipt's ring (spec 038 #D3, shared with spec 077's dApp landing).
 *
 * The curve used to live inside `SendReceipt.svelte`, where it could only be
 * checked by looking at it. These are the promises it makes to a person who is
 * watching a transaction, stated as numbers.
 */
import { describe, expect, it } from 'vitest';
import { ringProgress } from './ring';

describe('the ring around a transaction on its way', () => {
	it('starts empty', () => {
		expect(ringProgress(0, 20)).toBe(0);
	});

	it('is about 70% at the time the chain usually takes, and 86% at twice it', () => {
		expect(ringProgress(20, 20)).toBeCloseTo(0.69, 2);
		expect(ringProgress(40, 20)).toBeCloseTo(0.86, 2);
	});

	it('NEVER closes, however long it waits — only the confirmation does', () => {
		// A full ring beside "Submitted" reads as a finished transaction that is
		// not finished. The ceiling is what keeps that from ever being drawn.
		expect(ringProgress(3_600, 20)).toBeLessThan(0.921);
		expect(ringProgress(86_400, 2)).toBeLessThan(0.921);
	});

	it('keeps moving visibly past the typical time', () => {
		// A three-minute transaction on a four-second chain must not look stuck.
		const a = ringProgress(30, 4) ?? 0;
		const b = ringProgress(180, 4) ?? 0;
		expect(b).toBeGreaterThan(a);
	});

	it('circles rather than filling when the chain has no estimate', () => {
		// `undefined` is what makes `StatusHero` roam. A 0 here would draw an
		// empty ring that never moves, which looks like a stuck transaction.
		expect(ringProgress(10, 0)).toBeUndefined();
		expect(ringProgress(10, -1)).toBeUndefined();
		expect(ringProgress(10, Number.NaN)).toBeUndefined();
	});

	it('draws nothing negative when a clock goes backwards', () => {
		expect(ringProgress(-5, 20)).toBe(0);
	});
});
