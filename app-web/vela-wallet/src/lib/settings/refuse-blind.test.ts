/**
 * R22: "Refuse blind signing" — off unless switched on (the core's default,
 * `prefs::refuse_blind_signing`), stored under the core's key with the core's
 * spelling, and switched on for the rest of the visit even where storage
 * refuses it.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { refuseBlindSigning, refuseBlindSigningKey } from '$lib/core/kernels';
import { refuseBlind } from './refuse-blind.svelte';

describe('Refuse blind signing (R22)', () => {
	it('is the core’s key and reading: off unless the stored value is `on`', () => {
		expect(refuseBlindSigningKey()).toBe('vela.refuseBlindSigning');
		expect(refuseBlindSigning(null)).toBe(false);
		expect(refuseBlindSigning('off')).toBe(false);
		expect(refuseBlindSigning('yes')).toBe(false);
		expect(refuseBlindSigning('on')).toBe(true);
	});

	it('is off by default, and on once the person switches it on', () => {
		expect(refuseBlind.current()).toBe(false);
		refuseBlind.set(true);
		expect(refuseBlind.current()).toBe(true);
		refuseBlind.set(false);
		expect(refuseBlind.current()).toBe(false);
	});
});
