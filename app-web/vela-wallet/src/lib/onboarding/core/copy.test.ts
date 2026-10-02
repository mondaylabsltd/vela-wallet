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
