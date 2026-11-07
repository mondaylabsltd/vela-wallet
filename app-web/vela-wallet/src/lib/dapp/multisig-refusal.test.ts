/**
 * §31: a multisig's refused request is said in the words its refusal is for
 * — the parent-Safe sentence when the core says `data.reason: "parent_safe"`
 * (by `data.key`), dapp-7's "signs no messages yet" for everything else —
 * through the request window's real build-time words.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { resolveRequestMessages, resolveSigningMessages } from '$lib/i18n/engine.server';
import { multisigRefusalWords } from './multisig-refusal';

const SAFE = '0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe';

function refusal(why: 'member' | 'owner' | 'unchecked') {
	return {
		code: -32603,
		message: 'refused',
		data: {
			reason: 'parent_safe',
			why,
			safe: SAFE,
			function: 'execTransaction',
			key:
				why === 'unchecked'
					? 'componentsUi.signing.parentSafeUnchecked'
					: 'componentsUi.signing.parentSafeBlocked'
		}
	};
}

describe('a multisig’s refusal, in its own words', () => {
	for (const locale of ['en', 'zh'] as const) {
		const m = resolveRequestMessages(locale);
		const sheet = resolveSigningMessages(locale);

		it(`${locale}: a parent-Safe refusal is the signing sheet’s sentence, naming the call and the Safe`, () => {
			for (const why of ['member', 'owner'] as const) {
				const words = multisigRefusalWords(refusal(why), 'app.example', m);
				expect(words.title).toBe(sheet.selfCallBlockedTitle);
				expect(words.body).toBe(
					sheet.parentSafeBlocked
						.replace('{{function}}', 'execTransaction')
						.replace('{{safe}}', '0x5afe5a…fe5AfE')
				);
				expect(words.body).not.toContain('{{');
				expect(words.body).not.toBe(m.multisigNoMessagesBody.replace('{{host}}', 'app.example'));
			}
		});

		it(`${locale}: one that could not be checked says so`, () => {
			const words = multisigRefusalWords(refusal('unchecked'), 'app.example', m);
			expect(words.body).toBe(
				sheet.parentSafeUnchecked
					.replace('{{function}}', 'execTransaction')
					.replace('{{safe}}', '0x5afe5a…fe5AfE')
			);
		});

		it(`${locale}: every other refusal is dapp-7’s message words`, () => {
			const words = multisigRefusalWords({ data: undefined }, 'app.example', m);
			expect(words).toEqual({
				title: m.multisigNoMessagesTitle,
				body: m.multisigNoMessagesBody.replace('{{host}}', 'app.example')
			});
		});
	}
});
