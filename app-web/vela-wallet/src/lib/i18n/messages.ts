/**
 * Welcome-page message manifest (spec 006, data-model.md PageMessages) plus
 * the onboarding-flow message manifest (spec 014, contracts/i18n-keys.md).
 *
 * Keys live in the vela-core corpus (`onboarding.welcomeWeb.*` plus two reused
 * `onboarding.welcome.*` actions). This module is client-safe: it names keys
 * and shapes; resolution happens only in `engine.server.ts`.
 */

export interface WelcomeMessages {
	metaTitle: string;
	metaDescription: string;
	tagline: string;
	/** The v2 headline and its one supporting line (spec 019). */
	heroTitle: string;
	/**
	 * Which rung of the hero type ladder this locale's headline needs —
	 * `'regular'` or `'long'`. It lives in the corpus beside the string it
	 * describes because it is a property of the translation: measured at the
	 * shipped font, the widest authored line runs from 6.9em (zh) to 15.0em
	 * (id), and no single size serves both.
	 */
	heroTitleFit: string;
	heroSubtitle: string;
	createWallet: string;
	alreadyHaveWallet: string;
}

/**
 * Every corpus key the Welcome page consumes (tests iterate this).
 *
 * `onboarding.welcomeWeb.features.*` left the page with the v2 design (spec
 * 019, founder direction 2026-08-25): the design is one column — brand,
 * headline, two ways in — and the six feature cards have no place in it. The
 * twelve strings stay in the corpus rather than being deleted, because they are
 * written marketing copy and the page they belong on may yet exist; nothing
 * resolves them today.
 *
 * `onboarding.welcomeWeb.passkeyIndexLink` likewise stays for the future
 * settings screen (founder direction 2026-08-01).
 */
export const WELCOME_KEYS = [
	'onboarding.welcomeWeb.meta.title',
	'onboarding.welcomeWeb.meta.description',
	'onboarding.welcomeWeb.tagline',
	'onboarding.welcome.heroTitle',
	'onboarding.welcome.heroTitleFit',
	'onboarding.welcome.heroSubtitle',
	'onboarding.welcome.createWallet',
	'onboarding.welcome.alreadyHaveWallet'
] as const;

/* ------------------------------------------------------------------------ */
/* Onboarding flow messages (spec 014, T025)                                 */
/* ------------------------------------------------------------------------ */

/** Scaffold chrome + pattern furniture the panels resolve outside the outcome catalog. */
const FLOW_CHROME_KEYS = [
	'onboarding.common.close',
	// Spec 038: the front door's sentence for a probe that never left the
	// machine — an existing key, newly served here.
	'onboarding.common.networkBody',
	// Spec 038: the sign-in sheet's title on Welcome.
	'onboarding.login.header',
	'onboarding.common.copyAddress',
	'onboarding.common.copied',
	'onboarding.create.technicalDetails',
	'onboarding.common.stepCounter',
	'onboarding.common.confirmInPrompt',
	'onboarding.common.waitedSeconds'
] as const;

/**
 * The desktop rail's one slot (spec 019): a numbered step per screen, and the
 * brand line outside the journey. `CreateFlow.svelte` builds the step keys
 * from a template (`step${Naming|Keys|Create}${Label|Detail}`), so all six
 * expansions must be listed here — a template hides its keys from grep.
 */
const FLOW_RAIL_KEYS = [
	'onboarding.create.stepNamingLabel',
	'onboarding.create.stepNamingDetail',
	'onboarding.create.stepKeysLabel',
	'onboarding.create.stepKeysDetail',
	'onboarding.create.stepCreateLabel',
	'onboarding.create.stepCreateDetail',
	'onboarding.welcome.desktopTagline'
] as const;

/**
 * The v2 name screen. Three gates: where the PUBLIC key goes, where the PRIVATE
 * key stays, and the legal assent — whose fragments are named `ack2*` because
 * they render at index 2, and a fragment key that disagrees with its row is how
 * the `ack3` -> `ack1` confusion started.
 *
 * `accountNameLabel` is gone from this screen: the field sat under a heading
 * that already said "name your wallet", so the label restated it. Its helper
 * line went too — what it said (the name is stored on-chain) is now `ack0`,
 * where a person has to actually look at it.
 */
const FLOW_FORM_KEYS = [
	'onboarding.create.headerDefault',
	'onboarding.create.nameTitle',
	'onboarding.create.accountNamePlaceholder',
	'onboarding.create.nameTooLong',
	'onboarding.create.ack0',
	'onboarding.create.ack1',
	'onboarding.create.ack2',
	'onboarding.create.ack2PrivacyPolicy',
	'onboarding.create.ack2And',
	'onboarding.create.ack2Terms',
	'onboarding.create.ack2Period',
	'onboarding.create.nextBtn',
	'onboarding.create.finishVerifyBtn',
	'onboarding.create.startOverBtn',
	'onboarding.create.createWalletBtn'
] as const;

/** The founding-key list and its three add methods (spec 019). */
const FLOW_KEYS_SCREEN_KEYS = [
	'onboarding.create.keysTitle',
	'onboarding.create.keysTitleBlocked',
	'onboarding.create.keysSubtitle',
	'onboarding.create.keysSubtitleBlocked',
	'onboarding.create.keysSubtitleFull',
	'onboarding.create.keysLabel',
	'onboarding.create.keysHint',
	'onboarding.create.keyCount',
	'onboarding.create.keyLimitReached',
	'onboarding.create.keySyncedBadge',
	'onboarding.create.keyDeviceOnlyBadge',
	'onboarding.create.keyHardwareBadge',
	'onboarding.create.providerPlatform',
	'onboarding.create.providerGeneric',
	'onboarding.create.providerSecurityKey',
	'onboarding.create.needSecondKeyHint',
	'onboarding.create.addKeyBtn',
	'onboarding.create.addSecondKeyBtn',
	'onboarding.create.addMethodLabel',
	'onboarding.create.confirmKeyBtn',
	'onboarding.create.removeKeyBtn',
	'onboarding.create.methodPlatformTitle',
	'onboarding.create.methodPlatformBody',
	'onboarding.create.methodHybridTitle',
	'onboarding.create.methodHybridBody',
	'onboarding.create.methodHybridUnavailable',
	'onboarding.create.methodSecurityKeyTitle',
	'onboarding.create.methodSecurityKeyBody',
	// Spec 075: the Clear Signer, the fourth route beside the three above.
	'componentsUi.signing.clearSignerTitle',
	'componentsUi.signing.clearSignerBody'
] as const;

/** The progress, retry and done screens (spec 019). */
const FLOW_OUTCOME_SCREEN_KEYS = [
	'onboarding.create.progressTitle',
	'onboarding.create.progressSubtitle',
	'onboarding.create.progressMeterLabel',
	'onboarding.create.taskVerifyKey',
	'onboarding.create.taskDeriveAddress',
	'onboarding.create.taskWriteIndex',
	'onboarding.create.syncFailedTitle',
	'onboarding.create.syncFailedMessage',
	'onboarding.create.syncFailedHint',
	'onboarding.create.retryUploadBtn',
	'onboarding.create.successTitle',
	'onboarding.create.successMessage',
	'onboarding.create.walletAddressLabel',
	'onboarding.create.identiconHint',
	'onboarding.create.enterWalletBtn'
] as const;

/** Every prompt the two machines can raise (spec 019). */
const FLOW_PROMPT_KEYS = [
	'onboarding.create.alertErrorTitle',
	'onboarding.create.alertNotSupportedTitle',
	'onboarding.create.alertNotSupportedBody',
	'onboarding.common.notDiscoverableTitle',
	'onboarding.common.notDiscoverableBody',
	'onboarding.common.recreateWallet',
	'onboarding.common.retry',
	'onboarding.common.back',
	'onboarding.login.alertNotSupportedTitle',
	'onboarding.login.alertNotSupportedBody',
	'onboarding.login.alertIncompatibleTitle',
	'onboarding.login.alertIncompatibleBody',
	'onboarding.login.alertIncompatibleBodyCreate',
	'onboarding.login.alertSignInFailedTitle',
	'onboarding.login.alertSignInFailedBody',
	'onboarding.login.alertSelectorUnresponsive',
	'onboarding.login.recoverOfferTitle',
	'onboarding.login.recoverOfferBody',
	'onboarding.login.recoverConfirm',
	'onboarding.login.recoverCancel',
	'onboarding.login.recoverFailedTitle',
	'onboarding.login.recoverFailedBody',
	'onboarding.login.switchDeviceBtn',
	'onboarding.login.statusCancelledTitle',
	'onboarding.login.statusCancelledBody',
	'onboarding.settings.warningText',
	'onboarding.storage.unreadableTitle',
	'onboarding.storage.unreadableBody',
	'onboarding.storage.signInAgain',
	'onboarding.storage.resetCopy'
] as const;

/** The transient status line the create machine reports. */
const FLOW_STATUS_KEYS = [
	'onboarding.create.statusSettingUpIdentity',
	'onboarding.create.statusVerifyingIdentity',
	'onboarding.create.statusExtractingKey',
	'onboarding.create.statusComputingAddress',
	'onboarding.create.statusSyncingKey',
	'onboarding.create.statusSetupCancelled',
	'onboarding.create.statusVerifyCancelled'
] as const;

/** Login waiting pattern. */
const FLOW_LOGIN_WAIT_KEYS = [
	'onboarding.login.statusAwaitingPasskey',
	'onboarding.login.statusAwaitingPasskeyHint'
] as const;

/**
 * Every corpus key the v2 onboarding screens can resolve at runtime.
 *
 * Spelled out rather than derived from a catalog, because the catalog it used
 * to be derived from is gone: spec 014's eighteen `OutcomeKind`s were a design
 * taxonomy the core does not express. A transport failure and a 503 both reach
 * a shell as `CreateFailed { detail }` with the platform's own words, so
 * rendering them as distinct screens would mean classifying error strings in
 * TypeScript — the one thing the architecture exists to prevent. What the core
 * actually emits is nine prompts, and those are listed above.
 */
export const FLOW_KEYS: readonly string[] = [
	...new Set([
		...FLOW_CHROME_KEYS,
		...FLOW_RAIL_KEYS,
		...FLOW_FORM_KEYS,
		...FLOW_KEYS_SCREEN_KEYS,
		...FLOW_OUTCOME_SCREEN_KEYS,
		...FLOW_PROMPT_KEYS,
		...FLOW_STATUS_KEYS,
		...FLOW_LOGIN_WAIT_KEYS
	])
];

/**
 * Serialized flow copy: dotted corpus key → resolved template. Interpolation
 * placeholders (`{{seconds}}`, `{{current}}`…) ship raw and are filled
 * client-side by `fillTemplate` from frozen presentation state (FR-011).
 */
export type FlowMessages = Readonly<Record<string, string>>;
