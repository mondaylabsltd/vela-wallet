/**
 * The key-method rows' words are the core's (087 F01, F02): `methodCopy` is the
 * web's mirror of `vela_core::app::method_words` for a browser — which cannot
 * tell what unlocks the device it runs on — and this pins the mirror to the
 * real core, method by method, in both choosers.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { keyMethodWords, venueWords } from '../../../../../../rust/pkg-web/vela_core.js';
import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
import { methodCopy, OWN_PAGE_COPY, type KeyChooser } from './copy';

/** Spec 102: three places, and no fourth. */
const METHODS: KeyMethod[] = ['platform', 'hybrid', 'security_key'];
const CHOOSERS: KeyChooser[] = ['create', 'sign_in'];

type Words = { title_key: string; line_key: string | null; line_name: string | null };

function core(method: KeyMethod, chooser: KeyChooser): Words {
	const json = keyMethodWords(method, chooser, 'other');
	if (json === undefined || json === null) throw new Error(`the core does not know ${method}`);
	return JSON.parse(json) as Words;
}

describe('methodCopy', () => {
	it('is the core’s words for a browser, in every chooser', () => {
		for (const method of METHODS) {
			for (const chooser of CHOOSERS) {
				const words = core(method, chooser);
				expect(words.line_name, `${method} ${chooser}`).toBeNull();
				expect(methodCopy(method, chooser)).toEqual({
					title: words.title_key,
					body: words.line_key
				});
			}
		}
	});

	/**
	 * Spec 102: the Trusted Signer is not a place a key lives — the core has
	 * no words for it as one, in either chooser — and the choosers' advanced
	 * "Use my own signing page" is the core's `venue_words("own_page")`.
	 */
	it('knows no fourth method, and the own-page entry is the core’s words', () => {
		for (const chooser of CHOOSERS) {
			expect(keyMethodWords('trusted_signer', chooser, 'other') ?? null, chooser).toBeNull();
		}
		const own = JSON.parse(venueWords('own_page') ?? 'null') as Words | null;
		expect(own).not.toBeNull();
		expect(OWN_PAGE_COPY).toEqual({ title: own?.title_key, body: own?.line_key });
	});

	it('the sign-in sheet’s phone row scans; only the create picker creates (F02)', () => {
		expect(methodCopy('hybrid', 'sign_in').body).toBe('explore.scan');
		expect(methodCopy('hybrid', 'create').body).toBe('onboarding.create.methodHybridBody');
	});

	it('names a product only where the shell can tell which (F01)', () => {
		expect(JSON.parse(keyMethodWords('platform', 'sign_in', 'face_id') ?? 'null')).toEqual({
			title_key: 'onboarding.create.methodPlatformTitle',
			line_key: null,
			line_name: 'Face ID'
		});
		expect(keyMethodWords('carrier_pigeon', 'create', 'other')).toBeUndefined();
	});
});

/**
 * Spec 094 S2: a passkey ceremony the extension could not run because the
 * person limited its site access is said in the corpus's plain words and
 * points at the one-click grant — never Chrome's raw `SecurityError` text.
 */
describe('a ceremony Chrome refused for limited site access', () => {
	it('reads as the plain sentence on both prompts', async () => {
		const { promptCopy } = await import('./copy');
		const { SITE_ACCESS_WITHHELD } = await import('./passkey');
		const t = (key: string) => key;
		expect(
			promptCopy({ type: 'create_failed', detail: SITE_ACCESS_WITHHELD, phone_link: false }, t)
				.message
		).toBe('onboarding.common.siteAccessBody');
		expect(
			promptCopy({ type: 'sign_in_failed', detail: SITE_ACCESS_WITHHELD, phone_link: false }, t)
				.message
		).toBe('onboarding.common.siteAccessBody');
		// Anything else still carries the platform's own words, for the report.
		expect(
			promptCopy({ type: 'create_failed', detail: 'boom', phone_link: false }, t).message
		).toBe('boom');
	});
});

/**
 * Issue #446 — a ceremony that ran over the phone link and failed is said as
 * the link's failure, never "set up Face ID here": the core sets `phone_link`.
 */
describe('a failed phone ceremony', () => {
	it('reads as the dropped link on both prompts', async () => {
		const { promptCopy } = await import('./copy');
		const t = (key: string) => key;
		expect(promptCopy({ type: 'create_failed', detail: 'x', phone_link: true }, t).message).toBe(
			'onboarding.common.phoneLinkFailed'
		);
		expect(promptCopy({ type: 'sign_in_failed', detail: 'x', phone_link: true }, t).message).toBe(
			'onboarding.common.phoneLinkFailed'
		);
	});
});

/**
 * Issue #450 — a security key this device could not use is about the key;
 * the core sets `security_key`. A passkey route keeps the biometrics sheet.
 */
describe('a security key that cannot run', () => {
	it('is said as the key, on both prompts', async () => {
		const { promptCopy } = await import('./copy');
		const t = (key: string) => key;
		for (const type of ['not_supported_create', 'not_supported_login'] as const) {
			const copy = promptCopy({ type, security_key: true }, t);
			expect(copy.title).toBe('onboarding.common.keyUnavailableTitle');
			expect(copy.message).toBe('onboarding.common.keyUnavailableBody');
		}
		expect(promptCopy({ type: 'not_supported_login', security_key: false }, t).message).toBe(
			'onboarding.login.alertNotSupportedBody'
		);
	});
});
