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
import { feeRequoteDelayMs, feeRequoteTimeoutMs } from '$lib/core/kernels';
import type { FeeFailure } from '$lib/core/generated/FeeFailure';
import { feeFailureCause, FeeRequoteTimer, heldFeeFailure, withLostContext } from './fee-requote';
import { _resetPanelFailuresForTest, panelFailureLines } from '$lib/services/bug-report';
import { IDLE_FEE_VIEW } from '$lib/flows/core/fee-quote.svelte';

/** A timer whose clock is the test's: `advance(ms)` fires what is due. */
function harness(bounded = false) {
	let now = 0;
	let requotes = 0;
	const due: { at: number; run: () => void; id: number }[] = [];
	const lines: string[] = [];
	let next = 0;
	const timer = new FeeRequoteTimer({
		delayMs: feeRequoteDelayMs,
		requote: () => (requotes += 1),
		timeoutMs: bounded ? feeRequoteTimeoutMs : undefined,
		chainId: () => 100,
		log: (line) => lines.push(line),
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
		lines,
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
	it('3 s, 6 s, then every 8 s for a relay out of reach (spec 082 RJ12)', () => {
		expect([1, 2, 3, 4, 5, 9].map((n) => feeRequoteDelayMs('quote_unavailable', n))).toEqual([
			3000, 6000, 8000, 8000, 8000, 8000
		]);
	});

	it('a chain read that got no answer is on the same schedule (RJ13)', () => {
		expect(feeRequoteDelayMs({ chain_read: { rate_limited: true } }, 2)).toBe(6000);
		expect(feeRequoteDelayMs({ chain_read: { rate_limited: false } }, 3)).toBe(8000);
	});

	it('every wait plus the bound on its ask stays within 15 s (SC-003)', () => {
		for (let n = 1; n <= 6; n += 1) {
			expect(
				(feeRequoteDelayMs('quote_unavailable', n) ?? 0) + feeRequoteTimeoutMs()
			).toBeLessThanOrEqual(15_000);
		}
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
		expect(h.pending()).toEqual([8000]);
		h.advance(8000);
		h.timer.observe(BUSY, true);
		h.timer.observe(failed('quote_unavailable'), true);
		expect(h.pending()).toEqual([8000]);
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

	it('stops the moment the person approves — no re-price under a committed confirm', () => {
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

describe('withLostContext — a chain that could not be read is a failure the sheet says and retries', () => {
	it('an idle view after a lost context reads as the chain read that failed, confirm shut (RJ13)', () => {
		const view = withLostContext(IDLE_FEE_VIEW, true);
		expect(view.failed).toEqual({ chain_read: { rate_limited: false } });
		expect(view.confirm_fee_ready).toBe(false);
		// …which the core's schedule retries, like every other one of its kind.
		expect(feeRequoteDelayMs(view.failed!, 1)).toBe(3000);
		// A node refusing for load is said as that, not as "unreachable".
		expect(withLostContext(IDLE_FEE_VIEW, true, true).failed).toEqual({
			chain_read: { rate_limited: true }
		});
	});

	it('changes nothing while measuring, over a quote, over a real failure, or with the context in hand', () => {
		const busy = { ...IDLE_FEE_VIEW, busy: true };
		expect(withLostContext(busy, true)).toBe(busy);
		const failed = { ...IDLE_FEE_VIEW, failed: 'missing_public_key' as const };
		expect(withLostContext(failed, true)).toBe(failed);
		expect(withLostContext(IDLE_FEE_VIEW, false)).toBe(IDLE_FEE_VIEW);
	});
});

describe('each automatic re-ask is bounded, and every step is a `fee:` line (spec 082 RJ12, G47)', () => {
	it('a re-ask still out at the bound is given up and the next one scheduled', () => {
		const h = harness(true);
		h.timer.observe(failed('quote_unavailable'), true);
		h.advance(3000);
		expect(h.requotes()).toBe(1);
		// The relay hangs: the view stays busy past the bound.
		h.timer.observe(BUSY, true);
		expect(h.pending()).toEqual([feeRequoteTimeoutMs()]);
		h.advance(feeRequoteTimeoutMs());
		// Given up; attempt 2 is on the schedule, and a busy view no longer holds it.
		expect(h.pending()).toEqual([6000]);
		h.timer.observe(BUSY, true);
		expect(h.pending()).toEqual([6000]);
		h.advance(6000);
		expect(h.requotes()).toBe(2);
	});

	it('an answer inside the bound cancels it', () => {
		const h = harness(true);
		h.timer.observe(failed('quote_unavailable'), true);
		h.advance(3000);
		h.timer.observe(BUSY, true);
		h.timer.observe(QUOTED, true);
		expect(h.pending()).toEqual([]);
		h.advance(60_000);
		expect(h.requotes()).toBe(1);
	});

	it('the relay back: the fee is back within wait + bound of it (≤ 15 s)', () => {
		const h = harness(true);
		h.timer.observe(failed('quote_unavailable'), true);
		for (const wait of [3000, 6000]) {
			h.advance(wait);
			h.timer.observe(BUSY, true);
			h.advance(feeRequoteTimeoutMs());
		}
		// The relay came back just after the last give-up: at most 8 s to the
		// next ask, and it answers inside its bound.
		expect(h.pending()).toEqual([8000]);
	});

	it('logs each failure with its chain, cause and next re-ask, and the recovery', () => {
		const h = harness(true);
		h.timer.observe(failed({ chain_read: { rate_limited: true } }), true);
		h.advance(3000);
		h.timer.observe(BUSY, true);
		h.advance(feeRequoteTimeoutMs());
		h.timer.observe(QUOTED, true);
		expect(h.lines).toEqual([
			'fee: quote failed chain=100 cause=chain_read_rate_limited re-quote #1 in 3000 ms',
			'fee: quote failed chain=100 cause=timeout re-quote #2 in 6000 ms',
			'fee: quote back chain=100 after 2 re-quotes'
		]);
	});

	it('a failure no retry can clear is said once, not on every view', () => {
		const h = harness(true);
		h.timer.observe(failed('missing_public_key'), true);
		h.timer.observe(failed('missing_public_key'), true);
		expect(h.lines).toEqual(['fee: quote failed chain=100 cause=missing_public_key no re-quote']);
	});

	it('each failure reaches the report as a class (G61)', () => {
		_resetPanelFailuresForTest();
		const h = harness(true);
		h.timer.observe(failed('quote_unavailable'), true);
		h.advance(3000);
		h.timer.observe(BUSY, true);
		h.advance(feeRequoteTimeoutMs());
		expect(panelFailureLines()).toEqual([
			'panel:fee.quote_failed.quote_unavailable ×1',
			'panel:fee.quote_failed.timeout ×1'
		]);
		expect(feeFailureCause({ chain_read: { rate_limited: false } })).toBe('chain_read');
		_resetPanelFailuresForTest();
	});
});
