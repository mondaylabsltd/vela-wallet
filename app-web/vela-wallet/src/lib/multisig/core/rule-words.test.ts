/**
 * Round 6 (G4): how a multisig's rule and its addresses read on every screen.
 *
 * - r4D-13: the rule is a requirement ("Needs 2 of 3 members"), never "Needs
 *   1 of 1 members"; the header chip says what it needs, in every locale —
 *   "需 4 位中的 2 位", not a bare "4 位中的 2 位" that reads as a count so far.
 * - r4D-19: an address reads in one case, its EIP-55 checksum, wherever it
 *   came from (the chain's logs and the mailbox hand them over lowercased).
 */
import { describe, expect, it } from 'vitest';
import { resolveMultisigMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { ruleChipText, ruleTitle, shortenAddress } from './display';

const LOCALES = SUPPORTED_LOCALES;

describe('the rule, as a requirement (r4D-13)', () => {
	it('English: "Needs 2 of 3 members"; one member is never "1 of 1 members"', () => {
		const m = resolveMultisigMessages('en');
		expect(ruleTitle(m, { m: 2, n: 3 })).toBe('Needs 2 of 3 members');
		expect(ruleTitle(m, { m: 1, n: 1 })).toBe(m.panel.titleSolo);
		expect(ruleTitle(m, { m: 1, n: 1 })).not.toMatch(/1 of 1/);
		expect(ruleChipText(m, { m: 2, n: 4 })).toBe('Needs 2 of 4');
		expect(ruleChipText(m, { m: 1, n: 1 })).toBe('One member');
	});

	it('Chinese: the chip says what it needs — "需 4 位中的 2 位"', () => {
		expect(ruleChipText(resolveMultisigMessages('zh'), { m: 2, n: 4 })).toBe('需 4 位中的 2 位');
		expect(ruleChipText(resolveMultisigMessages('zh-TW'), { m: 2, n: 4 })).toBe('需 4 位中的 2 位');
	});

	for (const locale of LOCALES) {
		it(`${locale}: every rule word is filled, and one member reads as one`, () => {
			const m = resolveMultisigMessages(locale);
			for (const text of [
				ruleTitle(m, { m: 2, n: 3 }),
				ruleChipText(m, { m: 2, n: 3 }),
				ruleTitle(m, { m: 1, n: 1 }),
				ruleChipText(m, { m: 1, n: 1 })
			]) {
				expect(text.trim()).not.toBe('');
				expect(text).not.toContain('{{');
			}
			expect(ruleChipText(m, { m: 2, n: 3 })).toContain('2');
			expect(ruleChipText(m, { m: 2, n: 3 })).toContain('3');
			// The chip is no longer the bare "2 of 3" the stepper shows.
			expect(ruleChipText(m, { m: 2, n: 3 })).not.toBe(
				m.create.thresholdValue.replace('{{m}}', '2').replace('{{n}}', '3')
			);
			expect(m.proposal.withdraw.trim()).not.toBe('');
			expect(m.create.createdBodySolo.trim()).not.toBe('');
			expect(m.create.sharedSolo.trim()).not.toBe('');
		});
	}
});

describe('the review’s words are plain words (r5reg-10)', () => {
	for (const locale of LOCALES.filter((l) => l !== 'en')) {
		it(`${locale}: no raw "guard" or "handler" left in English`, () => {
			const m = resolveMultisigMessages(locale);
			for (const text of [
				m.review.sourceProbe,
				m.review.writeGuard,
				m.review.writeModuleGuard,
				m.review.writeHandler,
				m.authority.handler,
				m.authority.riskGuard,
				m.authority.riskHandler
			]) {
				expect(text).not.toMatch(/\bguards?\b|\bhandler\b/i);
			}
		});
	}
});

describe('addresses in one case (r4D-19)', () => {
	it('shortens an address in its EIP-55 case, however it came', () => {
		const checksummed = '0xd9c79Cc5C3DE9858E8767BB702DB5FfE51a20432';
		expect(shortenAddress(checksummed.toLowerCase())).toBe(shortenAddress(checksummed));
		expect(shortenAddress(checksummed.toUpperCase().replace('0X', '0x'))).toBe('0xd9c79C…a20432');
		// Not an address: as given.
		expect(shortenAddress('0x12')).toBe('0x12');
	});
});
