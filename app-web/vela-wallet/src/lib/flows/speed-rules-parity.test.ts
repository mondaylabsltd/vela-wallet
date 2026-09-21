/**
 * The two speed constants the web still spells out, pinned to the core.
 *
 * Every speed DECISION is the core's (`fee_policy`'s speed rules, spec 068 —
 * moved out of this shell on 2026-09-21). Two names stay literal here only
 * because modules read them at import time, before the core has loaded: the
 * offered tiers (the picker's order) and the tier a free upgrade goes to. A
 * literal that quietly disagreed with the core would be a second answer, so
 * both are compared with the core's own.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { feeSpeedRule } from '$lib/core/kernels';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import { OFFERED_TIERS } from './live-send';
import { FASTEST_TIER } from './speed-choice';

describe('the speed constants the web spells out', () => {
	it('offers exactly the core’s tiers, in the core’s order', () => {
		expect([...OFFERED_TIERS]).toEqual(feeSpeedRule<FeeTier[]>({ rule: 'offered_tiers' }));
	});

	it('upgrades to the core’s fastest tier', () => {
		// The fastest tier is the one a Slow default with a free partner is
		// judged against.
		expect(
			feeSpeedRule<FeeTier | null>({
				rule: 'free_speed_partner',
				picked: null,
				preferred: 'slow',
				in_force: 'slow',
				on_form: true
			})
		).toBe(FASTEST_TIER);
	});

	it('keeps the fee-signal window the core names (issue 212)', () => {
		expect(feeSpeedRule<number>({ rule: 'fee_signals_cache_ttl_ms' })).toBe(15_000);
	});
});
