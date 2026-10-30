/**
 * r7: a member's kind, declared then verified — what the add screens' choice
 * shows from what the wallet can tell and what the person said, and what is
 * kept: a Vela account only from the index or this device, a smart contract
 * account from code with no index record, "Other wallet" only as a hint.
 */
import { describe, expect, it } from 'vitest';
import { kindChoice } from './member-kind';

describe('a member’s kind choice (r7)', () => {
	it('a Vela account found (the index, this device): shown as found, not asked — over any word', () => {
		const found = { kind: 'passkey', suggested: null, locked: true, note: 'known' };
		expect(kindChoice('vela', undefined)).toEqual(found);
		expect(kindChoice('vela', 'wallet'), 'evidence beats the label').toEqual(found);
	});

	it('code and no index record (another Safe): a smart contract account, shown, never "Vela account"', () => {
		const contract = { kind: 'contract', suggested: null, locked: true, note: 'contract' };
		expect(kindChoice('contract', undefined)).toEqual(contract);
		expect(kindChoice('contract', 'passkey'), 'evidence beats the label').toEqual(contract);
		expect(kindChoice('contract', 'wallet')).toEqual(contract);
		// An invite's `c`, nothing read yet: the label stands.
		expect(kindChoice(null, 'contract')).toEqual(contract);
	});

	it('the person’s word stands where the wallet has no finding', () => {
		expect(kindChoice(null, 'passkey')).toEqual({
			kind: 'passkey',
			suggested: null,
			locked: false,
			note: null
		});
		expect(kindChoice('wallet', 'passkey')).toEqual({
			kind: 'passkey',
			suggested: null,
			locked: false,
			note: null
		});
		expect(kindChoice('wallet', 'wallet')).toEqual({
			kind: 'wallet',
			suggested: null,
			locked: false,
			note: null
		});
	});

	it('no Vela account found: "Other wallet" only hinted — nothing kept unless it is picked', () => {
		expect(kindChoice('wallet', undefined)).toEqual({
			kind: null,
			suggested: 'wallet',
			locked: false,
			note: 'likely'
		});
	});

	it('it can’t tell: nothing picked — the person is asked', () => {
		expect(kindChoice(null, undefined)).toEqual({
			kind: null,
			suggested: null,
			locked: false,
			note: 'ask'
		});
	});
});
