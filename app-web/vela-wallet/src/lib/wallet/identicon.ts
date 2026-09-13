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
import { preferences } from '$lib/services/preferences.svelte';
import { IDENTICON_PLACEHOLDER_SVG } from './identicon-placeholder';

export { IDENTICON_PLACEHOLDER_SVG };

/**
 * Circular identicon SVG for a raw seed; placeholder when unrenderable.
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

/**
 * The avatar a person chose (spec 028 T431/T432).
 *
 * Two styles, and the preference decides which. `identicon` is the address's
 * own artwork — the anti-forgery mark the receive card leans on, and the
 * default for that reason. `initials` is a letter on a soft accent disc, ported
 * from `WalletAvatar.tsx`'s non-identicon branch: same 0.34 letter-to-diameter
 * ratio, same fallback to `V` when there is no name to take a letter from.
 *
 * It is composed here rather than drawn as a component because every avatar in
 * the app already takes an SVG STRING — the identicon comes from vela-core that
 * way — and a second shape would mean every call site learning about both.
 */
export function avatarSvgForClient(rawSeed: string, name?: string): string {
	if (preferences.avatarStyle === 'identicon') return identiconSvgForClient(rawSeed);
	return initialsSvg(name);
}

/** XML-escape: a contact's name is a person's own text, not our markup. */
function escapeXml(text: string): string {
	return text
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;');
}

/**
 * A letter on an accent disc. Colours are token `var(--…)` references rather
 * than literals: this markup is inlined into the page, so it wears the palette
 * the page is wearing and follows a theme change without being rebuilt.
 */
export function initialsSvg(name?: string): string {
	const letter = escapeXml(((name ?? '').trim()[0] ?? 'V').toUpperCase());
	return (
		'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" role="presentation">' +
		'<circle cx="50" cy="50" r="50" fill="var(--color-accent-soft)"/>' +
		'<text x="50" y="50" text-anchor="middle" dominant-baseline="central"' +
		' font-family="var(--font-ui)" font-weight="700" font-size="34"' +
		' fill="var(--color-accent-base)">' +
		letter +
		'</text></svg>'
	);
}
