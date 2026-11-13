/**
 * r4C-11: a wallet's refusal in words a person acts on, never its raw text.
 * What an EIP-1193 error means is the core's reading (pinned by
 * `tests/app_multisig_outside.rs`: `a_wallet_error_by_its_eip1193_code`,
 * `anything_else_in_the_wallets_own_words_cut_to_one_readable_line`); this
 * pins how each is said.
 */
import { describe, expect, it } from 'vitest';
import { fillTemplate } from '$lib/i18n/fill';
import { problemText, type WalletProblem } from './errors';

const WORDS = {
	walletDeclined: 'Declined in the wallet.',
	walletUnauthorized: 'Connect this account in your wallet first, then try again.',
	walletUnsupported: "{{wallet}} can't do this request.",
	otherChain: 'Switch it to {{chain}}.',
	walletGone: '{{wallet}} disconnected.',
	walletFailed: "{{wallet}} couldn't sign: {{reason}}"
};

const say = (p: WalletProblem) => problemText(p, WORDS, fillTemplate, 'Rabby', 'Arc Testnet');

describe('a wallet error, said', () => {
	it('by what it means, with what to do', () => {
		expect(say({ kind: 'declined' })).toBe('Declined in the wallet.');
		expect(say({ kind: 'unauthorized' })).toBe(WORDS.walletUnauthorized);
		expect(say({ kind: 'unsupported' })).toBe("Rabby can't do this request.");
		expect(say({ kind: 'otherChain' })).toBe('Switch it to Arc Testnet.');
		expect(say({ kind: 'disconnected' })).toBe('Rabby disconnected.');
	});

	it("anything else in the wallet's own words", () => {
		expect(say({ kind: 'unknown', reason: 'Ledger: "0x6985" denied by the user' })).toBe(
			'Rabby couldn\'t sign: Ledger: "0x6985" denied by the user'
		);
	});
});
