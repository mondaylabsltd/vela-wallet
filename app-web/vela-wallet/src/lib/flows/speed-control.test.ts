/**
 * The speed control's one rule of its own: what the dead `rapid` is CALLED.
 *
 * Nothing constructs `rapid` (spec 068) and the relay refuses it, but the
 * wire type still has it, so a quote or a view that names it must be named as
 * one of the three speeds this build offers. The core's answer is its factory
 * default (`fee_speed::offered`) — `standard` since the Ethereum fee fix — and
 * iOS, Android and the desktop all draw that. This shell drew `fast` until
 * 2026-10-09, so a quote at `rapid` read as Fast here and Standard everywhere
 * else.
 */
import { describe, expect, it } from 'vitest';
import '$lib/i18n/wasm-init.server';
import { FeeTierPrefCore } from '$lib/core/client';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeTierPrefView } from '$lib/core/generated/FeeTierPrefView';
import { offeredTier, speedControlModel, type SpeedWords } from './speed-control';

const WORDS: SpeedWords = {
	label: 'Speed',
	once: 'Just this one',
	free: 'Free here',
	single: 'One speed here',
	gasPriceLabel: 'Gas price',
	names: { fast: 'Fast', standard: 'Standard', slow: 'Slow' }
};

function viewAt(tier: FeeSpeedView['tier']): FeeSpeedView {
	return {
		tier,
		preferred: tier,
		previews: [],
		open: true,
		picked: false,
		free: false,
		free_note: false,
		single: false,
		gas_price_line: false,
		options: [{ tier, selected: true, fee: null, measuring: false, gas_price: null }]
	};
}

describe('the dead `rapid` tier', () => {
	it('reads as Standard, and every offered tier as itself', () => {
		expect(offeredTier('rapid')).toBe('standard');
		for (const tier of ['fast', 'standard', 'slow'] as const) {
			expect(offeredTier(tier)).toBe(tier);
		}
	});

	// The core's own answer, read from the core rather than restated: a
	// stored `rapid` is "never chose", which is the factory default.
	it('is the factory default the core gives a preference nobody chose', () => {
		const core = new FeeTierPrefCore();
		try {
			const view = JSON.parse(core.view()) as FeeTierPrefView;
			expect(view.committed).toBe(false);
			expect(offeredTier('rapid')).toBe(view.tier);
		} finally {
			core.free();
		}
	});

	it('names the folded control and its option Standard, never Fast', () => {
		const model = speedControlModel(viewAt('rapid'), WORDS, () => null);
		expect(model.value).toBe('Standard');
		expect(model.options.map((option) => [option.id, option.label])).toEqual([
			['standard', 'Standard']
		]);
	});
});
