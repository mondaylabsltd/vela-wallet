/**
 * The whole translation surface of the onboarding cores.
 *
 * The Rust machines emit semantic variants (`syncing_key`, `recover_offer`) and
 * never a word of user-facing text — that is what keeps 15 locales out of the
 * wasm and makes "the copy did not change" a diff a reviewer can read on one
 * screen.
 *
 * Every mapping below is exhaustive with a `never` fallback, so adding a
 * variant in Rust without adding its copy here is a compile error rather than a
 * blank status line in production.
 */

import type { KeyMethod } from '../generated/KeyMethod';
import { SELECTOR_UNRESPONSIVE } from './passkey';
import type { PromptKind } from '../generated/PromptKind';
import type { StatusKey } from '../generated/StatusKey';
import type { SubmitLabel } from '../generated/SubmitLabel';

/** Resolve one corpus key, with optional `{{var}}` fills. */
export type Translate = (key: string, params?: Record<string, string | number>) => string;

function unreachable(value: never): never {
	// Cannot happen while the generated types are in sync; if the generator was
	// skipped, fail loudly here rather than silently rendering nothing.
	throw new Error(`unhandled onboarding variant: ${JSON.stringify(value)}`);
}

/** The transient status line. */
export function statusKeyToI18n(status: StatusKey): string {
	switch (status) {
		case 'setting_up_identity':
			return 'onboarding.create.statusSettingUpIdentity';
		case 'verifying_identity':
			return 'onboarding.create.statusVerifyingIdentity';
		case 'extracting_key':
			return 'onboarding.create.statusExtractingKey';
		case 'computing_address':
			return 'onboarding.create.statusComputingAddress';
		case 'syncing_key':
			return 'onboarding.create.statusSyncingKey';
		case 'setup_cancelled':
			return 'onboarding.create.statusSetupCancelled';
		case 'verify_cancelled':
			return 'onboarding.create.statusVerifyCancelled';
		default:
			return unreachable(status);
	}
}

/**
 * The progress screen's three task rows, and how far along it is.
 *
 * Derived from the core's reported stage, never from elapsed time: a bar that
 * advances on a timer tells the person something the wallet does not know. The
 * percentage exists because the design shows one; it is a rendering of the same
 * three-step fact, not a second source of truth.
 *
 * `setting_up_identity` is absent on purpose — it happens before the key list
 * exists, so it belongs to the form's status line rather than to this screen.
 */
export const PROGRESS_TASKS = [
	'onboarding.create.taskVerifyKey',
	'onboarding.create.taskDeriveAddress',
	'onboarding.create.taskWriteIndex'
] as const;

export type ProgressPosition = { activeTask: number; percent: number };

export function progressFor(status: StatusKey | null): ProgressPosition | null {
	switch (status) {
		case 'verifying_identity':
		case 'extracting_key':
			return { activeTask: 0, percent: 33 };
		case 'computing_address':
			return { activeTask: 1, percent: 62 };
		case 'syncing_key':
			return { activeTask: 2, percent: 100 };
		default:
			return null;
	}
}

/** The form's primary button. */
export function submitLabelToI18n(label: SubmitLabel): string {
	switch (label) {
		case 'create':
			return 'onboarding.create.nextBtn';
		case 'finish_verify':
			return 'onboarding.create.finishVerifyBtn';
		default:
			return unreachable(label);
	}
}

/** A key row's title and caption in the add-method picker. */
export function methodCopy(method: KeyMethod): { title: string; body: string } {
	switch (method) {
		case 'platform':
			return {
				title: 'onboarding.create.methodPlatformTitle',
				body: 'onboarding.create.methodPlatformBody'
			};
		case 'hybrid':
			return {
				title: 'onboarding.create.methodHybridTitle',
				body: 'onboarding.create.methodHybridBody'
			};
		case 'security_key':
			return {
				title: 'onboarding.create.methodSecurityKeyTitle',
				body: 'onboarding.create.methodSecurityKeyBody'
			};
		default:
			return unreachable(method);
	}
}

/**
 * The provider line under a key's name, when the AAGUID catalog cannot name the
 * vault: WHERE THIS KEY LIVES, in the same three words the method picker used.
 *
 * Takes `key.kind` — what the AUTHENTICATOR reported — never `key.method`, the
 * tap in the picker (issue 207). The tap does not reach the web ceremony at
 * all: `navigator.credentials` shows its own sheet, so somebody who taps
 * "Phone or tablet" and then touches the YubiKey in the port was being told
 * they had a phone. The report is the only signal that knows.
 *
 * "This device" is true HERE — this key was minted seconds ago on the machine
 * in front of the person. It would not be true in settings, whose list can hold
 * a key that lives on a computer the person is not sitting at; that surface
 * keeps its own location-neutral wording (`settings/live.ts`).
 *
 * The design draws a richer line still («macOS · 密码 App», «YubiKey 5C · USB»),
 * which needs the AAGUID resolved to a provider name and model — when the
 * catalog or the directory CAN name it, the row shows that instead.
 */
export function providerLineFor(kind: KeyMethod): string {
	switch (kind) {
		case 'platform':
			return 'onboarding.create.methodPlatformTitle';
		case 'hybrid':
			return 'onboarding.create.methodHybridTitle';
		case 'security_key':
			return 'onboarding.create.providerSecurityKey';
		default:
			return unreachable(kind);
	}
}

/**
 * The one badge a key row wears, in the one place that decides it (issue 207).
 *
 * It answers EXACTLY ONE question — is this key backed up? — and never again
 * says anything about where the key lives. That was the contradiction the
 * reporter saw: a YubiKey badged "This device only", which a security key is by
 * definition not.
 *
 * `undefined` when nobody can vouch for the answer: an attestation this build
 * cannot read makes `synced` fail open to `true` for the second-key GATE, and a
 * gate is not a badge. Drawing a green "Synced" from that would be telling
 * somebody their wallet is backed up on the strength of a guess.
 */
export function keyBadge(
	key: { synced: boolean; synced_known: boolean },
	t: Translate
): { text: string; tone: 'synced' | 'local' } | undefined {
	if (!key.synced_known) return undefined;
	return key.synced
		? { text: t('onboarding.create.keySyncedBadge'), tone: 'synced' }
		: { text: t('onboarding.create.keyDeviceOnlyBadge'), tone: 'local' };
}

export type PromptCopy = {
	title: string;
	message: string;
	/** Present only for the one prompt whose answer changes the flow. */
	confirm?: { confirmLabel: string; cancelLabel: string };
};

/** One entry per notice or question the core can raise. */
export function promptCopy(kind: PromptKind, t: Translate): PromptCopy {
	switch (kind.type) {
		case 'not_supported_create':
			return {
				title: t('onboarding.create.alertNotSupportedTitle'),
				message: t('onboarding.create.alertNotSupportedBody')
			};
		case 'not_supported_login':
			return {
				title: t('onboarding.login.alertNotSupportedTitle'),
				message: t('onboarding.login.alertNotSupportedBody')
			};
		case 'not_discoverable':
			return {
				title: t('onboarding.common.notDiscoverableTitle'),
				message: t('onboarding.common.notDiscoverableBody')
			};
		case 'incompatible_create':
			return {
				title: t('onboarding.login.alertIncompatibleTitle'),
				message: t('onboarding.login.alertIncompatibleBodyCreate')
			};
		case 'incompatible_login':
			return {
				title: t('onboarding.login.alertIncompatibleTitle'),
				message: t('onboarding.login.alertIncompatibleBody')
			};
		case 'create_failed':
			// The platform's own words. Opaque by nature — it goes straight into
			// the bug report, and inventing friendlier text here would lose the
			// detail that makes the report worth filing.
			return { title: t('onboarding.create.alertErrorTitle'), message: kind.detail };
		case 'recover_offer':
			return {
				title: t('onboarding.login.recoverOfferTitle'),
				message: t('onboarding.login.recoverOfferBody'),
				confirm: {
					confirmLabel: t('onboarding.login.recoverConfirm'),
					cancelLabel: t('onboarding.login.recoverCancel')
				}
			};
		case 'recover_failed':
			return {
				title: t('onboarding.login.recoverFailedTitle'),
				message: t('onboarding.login.recoverFailedBody')
			};
		case 'sign_in_failed':
			return {
				title: t('onboarding.login.alertSignInFailedTitle'),
				// The one failure whose words are the shell's, not the platform's:
				// the system's passkey sheet never appeared (see `passkey.ts`).
				message:
					kind.detail === SELECTOR_UNRESPONSIVE
						? t('onboarding.login.alertSelectorUnresponsive')
						: t('onboarding.login.alertSignInFailedBody', { message: kind.detail })
			};
		default:
			return unreachable(kind);
	}
}
