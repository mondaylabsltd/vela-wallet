/**
 * The golden addresses — the FIFTH surface (spec 026 T210).
 *
 * The same public keys must derive the same Safe addresses in Rust
 * (`app-desktop` links vela-core directly), uniffi Kotlin (`app-android`),
 * uniffi Swift (`app-ios`) and the wasm the Expo app links. This file makes
 * the SvelteKit web the fifth place that pins them, so a derivation change is
 * a conscious edit in five places rather than a silent drift in one.
 *
 * The keys are the parallel space's fixed keyset (public by design — their
 * addresses must never hold real funds). Phase 3's fixture module derives
 * these same three keys from its seeds and asserts against these constants.
 */
import '#lib/i18n/wasm-init.server.ts';
import { describe, expect, it } from 'vitest';
import { computeAddress, computeAddressMulti, computeSafeAddressMulti } from './kernels';

/** Uncompressed P-256 points `04‖x‖y` for `vela-fixture-01..03`. */
export const FIXTURE_PUBLIC_KEYS = [
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b',
	'047802f2cc39cc6ed85c41268a580c5f0df36df3f065facfb40f84265927b7ed678438b2084cb84f0d00708e40d44ed8c9901d979bbcb390117089847672c12eec',
	'043eb2ae2f4e8090837820048baca2db04a7e7ca7dc6742f342a30c6855e7d96947f36ac5a4538dc3a8a7cbf0deea1c0ecb80bfb96a3f8ca57c98f8f400548cd56'
];

/** The single-key fixture Safes — these are the addresses funded on-chain. */
export const FIXTURE_SAFES = [
	'0xD400866e00B055B20752a826CD5C89b811de130b',
	'0x031d7D57c99CAF891e1C250554691Fd12D84772b',
	'0x58cd0ce6A27099220543b31710d7860d75Ba1d3d'
];

/** All three keys founding ONE wallet — the multi-key conformance anchor. */
export const GOLDEN_MULTI_SAFE = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';

describe('the golden Safes', () => {
	it('derives each single-key fixture Safe', () => {
		expect(FIXTURE_PUBLIC_KEYS.map((key) => computeAddress(key))).toEqual(FIXTURE_SAFES);
	});

	it('derives the multi-key golden Safe from the three founding keys, in order', () => {
		expect(computeAddressMulti(FIXTURE_PUBLIC_KEYS)).toBe(GOLDEN_MULTI_SAFE);
	});

	it('key ORDER is part of the address — a reordered set is a different wallet', () => {
		const swapped = [FIXTURE_PUBLIC_KEYS[1], FIXTURE_PUBLIC_KEYS[0], FIXTURE_PUBLIC_KEYS[2]];
		expect(computeAddressMulti(swapped)).not.toBe(GOLDEN_MULTI_SAFE);
	});

	it('a one-key set derives exactly what the single-key path derives (N=1 equivalence)', () => {
		const key = FIXTURE_PUBLIC_KEYS[0];
		expect(computeAddressMulti([key])).toBe(computeAddress(key));
		const info = computeSafeAddressMulti([key]);
		expect(info.address).toBe(FIXTURE_SAFES[0]);
		expect(info.setupData.length).toBeGreaterThan(0);
		expect(info.saltNonce.length).toBe(32);
	});

	it('rejects a malformed public key rather than deriving a plausible address', () => {
		expect(() => computeAddress(FIXTURE_PUBLIC_KEYS[0].slice(0, 40))).toThrow();
		expect(() => computeAddress('0x' + 'ff'.repeat(65))).toThrow();
	});
});
