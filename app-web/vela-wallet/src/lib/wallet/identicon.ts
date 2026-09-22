/**
 * Identicon rendering for the RUNTIME side of the app (spec 019).
 *
 * `identicon.server.ts` is the build-time twin: the gallery and every
 * prerendered fixture resolve their artwork there, in Node, so the deployed
 * Worker stays wasm-free. The signed-in wallet cannot use it: its seed is the
 * person's own address, which exists only in the browser — and by the time the
 * browser knows one it has already fetched vela-core, because the session
 * machine that holds the address IS that module.
 *
 * So this calls the same two kernels through the client that already loaded
 * them, and both paths share one placeholder.
 */
import { identiconNormalizeSeed, identiconSvgCircular } from '$lib/onboarding/core/wasm-client';
import { IDENTICON_PLACEHOLDER_SVG } from './identicon-placeholder';

export { IDENTICON_PLACEHOLDER_SVG };

/**
 * Circular identicon SVG for a raw seed; placeholder when unrenderable.
 *
 * Every account and contact avatar is this artwork — the address's own
 * anti-forgery mark; spec 074 retired the initials style that could stand in
 * for it.
 *
 * Only callable once `loadOnboardingCore()` has resolved — every caller is
 * downstream of a booted session, which is exactly that guarantee.
 *
 * Seeds go through `identiconNormalizeSeed` (never a local `toLowerCase()` —
 * spec 003's drift rule).
 */
export function identiconSvgForClient(rawSeed: string): string {
	const seed = identiconNormalizeSeed(rawSeed);
	if (seed.length === 0) return IDENTICON_PLACEHOLDER_SVG;
	try {
		return identiconSvgCircular(seed);
	} catch {
		return IDENTICON_PLACEHOLDER_SVG;
	}
}
