/**
 * Compatibility address (spec 024): the runtime core loader moved to
 * `#lib/core/client.ts`, where every machine — not just onboarding — reaches
 * it. Onboarding's importers keep their old names; new code imports from
 * `#lib/core/client.ts` directly.
 */
export {
	buildGroupProof,
	buildMemberProof,
	CreateWalletCore,
	groupPublicKeyFromSeed,
	identiconNormalizeSeed,
	identiconSvgCircular,
	loadCore as loadOnboardingCore,
	LoginCore,
	passkeyDirectoryEntry,
	passkeyDirectoryUrl,
	passkeyFallbackIconDataUri,
	passkeyProviderIconDataUri,
	resetCoreForTests as resetOnboardingCoreForTests,
	SessionCore,
	toHex
} from '#lib/core/client.ts';
