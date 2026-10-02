/**
 * The key-method rows' words are the core's (087 F01, F02): `methodCopy` is the
 * web's mirror of `vela_core::app::method_words` for a browser — which cannot
 * tell what unlocks the device it runs on — and this pins the mirror to the
 * real core, method by method, in both choosers.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { keyMethodWords } from '../../../../../../rust/pkg-web/vela_core.js';
import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
import { methodCopy, type KeyChooser } from './copy';

const METHODS: KeyMethod[] = ['platform', 'hybrid', 'security_key', 'trusted_signer'];
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
		expect(promptCopy({ type: 'create_failed', detail: SITE_ACCESS_WITHHELD }, t).message).toBe(
			'onboarding.common.siteAccessBody'
		);
		expect(promptCopy({ type: 'sign_in_failed', detail: SITE_ACCESS_WITHHELD }, t).message).toBe(
			'onboarding.common.siteAccessBody'
		);
		// Anything else still carries the platform's own words, for the report.
		expect(promptCopy({ type: 'create_failed', detail: 'boom' }, t).message).toBe('boom');
	});
});
