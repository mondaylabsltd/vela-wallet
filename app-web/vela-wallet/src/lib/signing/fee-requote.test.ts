/**
 * The signing sheet asks for a failed fee again by itself (spec 079, FR-008).
 *
 * Measured on the Xiaomi with the relay dropped at the proxy: the row said
 * "点击重试" and stayed that way after the relay came back. The schedule is the
 * core's (`fee_policy::requote_delay_ms`), read here through the same wasm the
 * sheet calls; these tests are about the timer honouring it and STOPPING —
 * on a quote, on approval, on close.
 */
import { describe, expect, it } from 'vitest';
import '$lib/i18n/wasm-init.server';
import { feeRequoteDelayMs } from '$lib/core/kernels';
import type { FeeFailure } from '$lib/core/generated/FeeFailure';
import { FeeRequoteTimer, heldFeeFailure } from './fee-requote';

/** A timer whose clock is the test's: `advance(ms)` fires what is due. */
function harness() {
	let now = 0;
	let requotes = 0;
	const due: { at: number; run: () => void; id: number }[] = [];
	let next = 0;
	const timer = new FeeRequoteTimer({
		delayMs: feeRequoteDelayMs,
		requote: () => (requotes += 1),
		schedule: (run, ms) => {
			const id = (next += 1);
			due.push({ at: now + ms, run, id });
			return id;
		},
		cancel: (id) => {
			const at = due.findIndex((entry) => entry.id === id);
			if (at !== -1) due.splice(at, 1);
		}
	});
	return {
		timer,
		requotes: () => requotes,
		pending: () => due.map((entry) => entry.at - now),
		advance(ms: number) {
			now += ms;
			for (const entry of due.filter((e) => e.at <= now)) {
				due.splice(due.indexOf(entry), 1);
				entry.run();
			}
		}
	};
}

const failed = (failure: FeeFailure) => ({ failed: failure, busy: false });
const BUSY = { failed: null, busy: true };
const QUOTED = { failed: null, busy: false };

describe('the core’s schedule, through the wasm the sheet calls', () => {
	it('3 s, 6 s, 12 s, then every 15 s for a relay out of reach', () => {
		expect([1, 2, 3, 4, 5, 9].map((n) => feeRequoteDelayMs('quote_unavailable', n))).toEqual([
			3000, 6000, 12000, 15000, 15000, 15000
		]);
	});

	it('never for a failure no retry can clear', () => {
		expect(feeRequoteDelayMs('missing_public_key', 1)).toBeNull();
		expect(feeRequoteDelayMs('calculation_failed', 1)).toBeNull();
	});
});

describe('FeeRequoteTimer', () => {
	it('asks again on the schedule while the failure lasts, with no tap', () => {
		const h = harness();
		h.timer.observe(failed('quote_unavailable'), true);
		expect(h.pending()).toEqual([3000]);
		h.advance(2999);
		expect(h.requotes()).toBe(0);
		h.advance(1);
		expect(h.requotes()).toBe(1);
		// The re-ask runs, and fails again: the next wait is longer.
		h.timer.observe(BUSY, true);
		expect(h.pending()).toEqual([]);
		h.timer.observe(failed('quote_unavailable'), true);
		expect(h.pending()).toEqual([6000]);
		h.advance(6000);
		h.timer.observe(BUSY, true);
		h.timer.observe(failed('quote_unavailable'), true);
		expect(h.pending()).toEqual([12000]);
		h.advance(12000);
		h.timer.observe(BUSY, true);
		h.timer.observe(failed('quote_unavailable'), true);
		expect(h.pending()).toEqual([15000]);
		expect(h.requotes()).toBe(3);
	});

	it('a quote ends it: nothing more is asked, and the count starts over', () => {
		const h = harness();
		h.timer.observe(failed('quote_unavailable'), true);
		h.advance(3000);
		h.timer.observe(BUSY, true);
		h.timer.observe(failed('quote_unavailable'), true);
		expect(h.timer.attempt).toBe(2);
		// The person tapped refresh and it answered before the timer.
		h.timer.observe(QUOTED, true);
		expect(h.pending()).toEqual([]);
		expect(h.timer.attempt).toBe(0);
		h.advance(60_000);
		expect(h.requotes()).toBe(1);
		// A later failure starts at 3 s again.
		h.timer.observe(failed('estimate_failed'), true);
		expect(h.pending()).toEqual([3000]);
	});

	it('stops the moment the person approves — no re-price under a committed slide', () => {
		const h = harness();
		h.timer.observe(failed('quote_unavailable'), true);
		h.timer.observe(failed('quote_unavailable'), false); // approved: signing
		expect(h.pending()).toEqual([]);
		h.advance(60_000);
		expect(h.requotes()).toBe(0);
	});

	it('does not fire if the sheet went between scheduling and firing', () => {
		// A cancel that raced the timer, or a host that forgot to cancel: the
		// fire checks again.
		let requotes = 0;
		let fire: (() => void) | null = null;
		const timer = new FeeRequoteTimer({
			delayMs: feeRequoteDelayMs,
			requote: () => (requotes += 1),
			schedule: (run) => ((fire = run), 1),
			cancel: () => {}
		});
		timer.observe(failed('quote_unavailable'), true);
		timer.stop();
		(fire as (() => void) | null)?.();
		expect(requotes).toBe(0);
	});

	it('never schedules for a failure the core will not retry', () => {
		const h = harness();
		h.timer.observe(failed('missing_public_key'), true);
		h.timer.observe(failed('calculation_failed'), true);
		expect(h.pending()).toEqual([]);
	});

	it('a tap on refresh while a re-ask waits takes its place — one question, not two', () => {
		const h = harness();
		h.timer.observe(failed('quote_unavailable'), true);
		expect(h.pending()).toEqual([3000]);
		h.timer.observe(BUSY, true); // the person tapped ⟳
		expect(h.pending()).toEqual([]);
		h.advance(10_000);
		expect(h.requotes()).toBe(0);
		// It failed too: the schedule carries on from where it was.
		h.timer.observe(failed('quote_unavailable'), true);
		expect(h.pending()).toEqual([6000]);
	});

	it('one timer at a time: a repeated view does not stack re-asks', () => {
		const h = harness();
		h.timer.observe(failed('quote_unavailable'), true);
		h.timer.observe(failed('quote_unavailable'), true);
		h.timer.observe(failed('quote_unavailable'), true);
		expect(h.pending()).toEqual([3000]);
	});

	it('a closed sheet, or no fee at all, stops it', () => {
		const h = harness();
		h.timer.observe(failed('quote_unavailable'), true);
		h.timer.observe(null, true);
		expect(h.pending()).toEqual([]);
		h.timer.observe(failed('quote_unavailable'), true);
		h.timer.stop();
		expect(h.pending()).toEqual([]);
		expect(h.timer.attempt).toBe(0);
	});
});

describe('heldFeeFailure — the reason stays put while the sheet asks again', () => {
	it('a failure sets it, a measurement keeps it, an answer clears it', () => {
		let held = heldFeeFailure(null, failed('quote_unavailable'));
		expect(held).toBe('quote_unavailable');
		held = heldFeeFailure(held, BUSY);
		expect(held).toBe('quote_unavailable');
		held = heldFeeFailure(held, QUOTED);
		expect(held).toBeNull();
	});

	it('a first measurement with no failure behind it holds nothing', () => {
		expect(heldFeeFailure(null, BUSY)).toBeNull();
		expect(heldFeeFailure('quote_unavailable', null)).toBeNull();
	});
});
