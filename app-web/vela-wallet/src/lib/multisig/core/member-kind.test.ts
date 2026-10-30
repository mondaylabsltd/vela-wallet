/**
 * r7: a member's kind, declared then verified — what the add screens' choice
 * shows from what the wallet can tell and what the person said.
 */
import { describe, expect, it } from 'vitest';
import { kindChoice } from './member-kind';

describe('a member’s kind choice (r7)', () => {
	it('evidence of a Vela account: shown as found, not asked — over any word', () => {
		const found = { kind: 'passkey', locked: true, note: 'known' };
		expect(kindChoice('vela', undefined)).toEqual(found);
		expect(kindChoice('vela', 'wallet'), 'evidence beats the label').toEqual(found);
	});

	it('the person’s word stands where the wallet has no finding', () => {
		expect(kindChoice(null, 'passkey')).toEqual({ kind: 'passkey', locked: false, note: null });
		expect(kindChoice('wallet', 'passkey')).toEqual({
			kind: 'passkey',
			locked: false,
			note: null
		});
	});

	it('no Vela account found: "Other wallet" pre-filled, for the person to confirm', () => {
		expect(kindChoice('wallet', undefined)).toEqual({
			kind: 'wallet',
			locked: false,
			note: 'likely'
		});
	});

	it('it can’t tell: nothing picked — the person is asked', () => {
		expect(kindChoice(null, undefined)).toEqual({ kind: null, locked: false, note: 'ask' });
	});
});
