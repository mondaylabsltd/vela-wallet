/**
 * §37 (review 1-13): the genesis refusal `ownerNotBound` — a member that is
 * a contract nothing ties to its own creation on a network — is worded
 * wherever the other `authority.*` refusals are, in every locale; and the
 * proposal list's non-member words exist in every locale too.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { resolveMultisigMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { chainName } from '$lib/services/networks';
import { findingText, refusalText } from './findings';

const MEMBER = '0x1234567890AbcdEF1234567890aBcdef12345678';

describe('ownerNotBound', () => {
	it('names the member and the network, with no slot left unfilled', () => {
		const m = resolveMultisigMessages('en');
		const line = refusalText(m, { kind: 'ownerNotBound', address: MEMBER }, 100);
		expect(line).toContain(MEMBER);
		expect(line).toContain(chainName(100));
		expect(line).not.toMatch(/\{\{/);
		expect(
			findingText(m, {
				from: 'genesis',
				chainId: 100,
				reason: { kind: 'ownerNotBound', address: MEMBER }
			})
		).toBe(line);
	});

	it('every locale words it, and the list’s non-member lines', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const m = resolveMultisigMessages(locale);
			expect(m.authority.ownerNotBound, locale).toMatch(/\{\{address\}\}/);
			expect(m.panel.nonMembersShow, locale).toMatch(/\{\{total\}\}/);
			expect(m.panel.nonMembersHide, locale).toBeTruthy();
			expect(m.panel.nonMembersWarning, locale).toBeTruthy();
		}
	});
});
