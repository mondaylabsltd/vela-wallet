/**
 * The dApp path signs for the wallet a request is FOR (spec 062, found on
 * Base): one passkey founds any number of wallets, so a credential id does
 * not name a wallet — an address does.
 */
import { describe, expect, it, vi } from 'vitest';

const ONE = { id: 'cred-a', address: '0x' + 'd4'.repeat(20), publicKeyHex: '04' + '11'.repeat(64) };
const MULTI = {
	id: 'cred-a', // the SAME founding credential as ONE
	address: '0x' + '88'.repeat(20),
	publicKeyHex: ONE.publicKeyHex,
	keys: [
		{ credentialId: 'cred-a', publicKeyHex: ONE.publicKeyHex },
		{ credentialId: 'cred-b', publicKeyHex: '04' + '22'.repeat(64) }
	]
};
vi.mock('./accounts', () => ({
	findAccountByAddress: (address: string) =>
		[ONE, MULTI].find((a) => a.address === address.toLowerCase()),
	findAccountByCredentialId: (id: string) => [ONE, MULTI].find((a) => a.id === id)
}));
vi.mock('$lib/onboarding/core/passkey', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/onboarding/core/passkey')>()),
	signWithAny: vi.fn()
}));
vi.mock('./rpc-adapter', () => ({ rpcCall: vi.fn() }));
vi.mock('./safe-transaction', () => ({
	keySetOf: vi.fn(),
	sendBatchCalls: vi.fn(),
	sendContractCall: vi.fn(),
	sendNative: vi.fn()
}));

import { storedWalletFor } from './dapp-submit';

describe('storedWalletFor', () => {
	it('two wallets founded by one passkey: the request names which by ADDRESS', () => {
		// By credential alone both requests would have resolved to ONE — and a
		// request for MULTI would have deployed ONE's Safe (AA14).
		expect(storedWalletFor({ id: 'cred-a' }, MULTI.address)?.keys).toHaveLength(2);
		expect(storedWalletFor({ id: 'cred-a' }, ONE.address)?.keys).toBeUndefined();
		expect(
			storedWalletFor({ id: 'cred-a' }, MULTI.address.toUpperCase().replace('0X', '0x'))?.address
		).toBe(MULTI.address);
	});

	it('an address no record carries falls back to the credential, as before', () => {
		expect(storedWalletFor({ id: 'cred-a' }, '0x' + '00'.repeat(20))?.address).toBe(ONE.address);
		expect(storedWalletFor({ id: 'nobody' }, '0x' + '00'.repeat(20))).toBeUndefined();
	});
});
