/**
 * A list's gesture, as the incoming check's cadence hears it (r10
 * follow-up): one `on` when it starts, one `off` once it has settled —
 * `scrollend`, or still for `GESTURE_SETTLE_MS` with no finger down.
 */
import { describe, expect, it } from 'vitest';
import { GESTURE_SETTLE_MS, GestureTracker, type GestureHost } from './list-gesture';

function clock() {
	let now = 0;
	let seq = 0;
	const timers = new Map<number, { at: number; run: () => void }>();
	const host: GestureHost = {
		setTimeout: (run, ms) => {
			seq += 1;
			timers.set(seq, { at: now + ms, run });
			return seq;
		},
		clearTimeout: (handle) => void timers.delete(handle as number)
	};
	const advance = (by: number) => {
		const to = now + by;
		for (;;) {
			const due = [...timers.entries()]
				.filter(([, t]) => t.at <= to)
				.sort((a, b) => a[1].at - b[1].at)[0];
			if (!due) break;
			timers.delete(due[0]);
			now = due[1].at;
			due[1].run();
		}
		now = to;
	};
	return { host, advance, pending: () => timers.size };
}

function tracked() {
	const c = clock();
	const edges: boolean[] = [];
	const tracker = new GestureTracker((on) => void edges.push(on), c.host);
	return { ...c, edges, tracker };
}

describe('GestureTracker', () => {
	it('reports a burst of scroll events as one on and one off', () => {
		const { tracker, advance, edges } = tracked();
		for (let i = 0; i < 50; i++) {
			tracker.input('moved');
			advance(16);
		}
		expect(edges).toEqual([true]);
		advance(GESTURE_SETTLE_MS);
		expect(edges).toEqual([true, false]);
	});

	it('scrollend ends it at once', () => {
		const { tracker, edges, pending } = tracked();
		tracker.input('moved');
		tracker.input('settled');
		expect(edges).toEqual([true, false]);
		expect(pending()).toBe(0);
	});

	it('a finger down holds the gesture until it lifts and the list settles', () => {
		const { tracker, advance, edges } = tracked();
		tracker.input('finger-down');
		tracker.input('moved');
		advance(5_000);
		tracker.input('settled'); // a scrollend under a resting finger
		expect(edges).toEqual([true]);
		tracker.input('finger-up');
		advance(GESTURE_SETTLE_MS - 1);
		expect(edges).toEqual([true]);
		advance(1);
		expect(edges).toEqual([true, false]);
	});

	it('a tap is no gesture', () => {
		const { tracker, advance, edges } = tracked();
		tracker.input('finger-down');
		tracker.input('finger-up');
		tracker.input('settled');
		advance(1_000);
		expect(edges).toEqual([]);
	});

	it('a list that goes away mid-gesture ends it, and nothing is left armed', () => {
		const { tracker, edges, pending } = tracked();
		tracker.input('moved');
		tracker.stop();
		tracker.stop();
		expect(edges).toEqual([true, false]);
		expect(pending()).toBe(0);
	});
});
