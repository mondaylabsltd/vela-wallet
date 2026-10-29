/**
 * r4C-11: a wallet's refusal in words a person acts on, never its raw text.
 */
import { describe, expect, it } from 'vitest';
import { fillTemplate } from '$lib/i18n/fill';
import { problemText, shortReason, walletProblem } from './errors';

const WORDS = {
	walletDeclined: 'Declined in the wallet.',
	walletUnauthorized: 'Connect this account in your wallet first, then try again.',
	walletUnsupported: "{{wallet}} can't do this request.",
	otherChain: 'Switch it to {{chain}}.',
	walletGone: '{{wallet}} disconnected.',
	walletFailed: "{{wallet}} couldn't sign: {{reason}}"
};

const say = (error: unknown) =>
	problemText(walletProblem(error), WORDS, fillTemplate, 'Rabby', 'Arc Testnet');

describe('a wallet error, said', () => {
	it('by its EIP-1193 code, with what to do', () => {
		expect(say({ code: 4001, message: 'User rejected the request.' })).toBe(
			'Declined in the wallet.'
		);
		expect(say({ code: 4100, message: 'Unauthorized' })).toBe(WORDS.walletUnauthorized);
		// Some wallets say it without the code.
		expect(say(new Error('Unauthorized'))).toBe(WORDS.walletUnauthorized);
		expect(say({ code: 4200, message: 'x' })).toBe("Rabby can't do this request.");
		expect(say({ code: -32601, message: 'the method eth_signTypedData_v4 does not exist' })).toBe(
			"Rabby can't do this request."
		);
		expect(say({ code: 4901, message: 'x' })).toBe('Switch it to Arc Testnet.');
		expect(say({ code: 4900, message: 'x' })).toBe('Rabby disconnected.');
	});

	it("anything else in the wallet's own words, cut to one readable line", () => {
		expect(
			say({ code: -32603, message: '{"message":"Ledger: \\"0x6985\\" denied by the user"}' })
		).toBe('Rabby couldn\'t sign: Ledger: "0x6985" denied by the user');
		expect(shortReason('Error: Error: something\nstack line 1\nstack line 2')).toBe('something');
		expect(shortReason('x'.repeat(400))).toHaveLength(120);
	});
});
