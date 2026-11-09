/**
 * §37 (review 1-11, ERC-6492): every MESSAGE signature the wallet returns —
 * `personal_sign` (a sign-in too) and typed data, through the passkey and
 * the trusted page alike — is the EIP-1271 signature as built when the
 * account holds code on the request's chain, and otherwise the core's
 * ERC-6492 envelope (`messageSignature`), read with the wallet's own RPC.
 * A fresh account has no code for `isValidSignature` to run; without the
 * envelope its sign-in verifies nowhere.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { hashTypedData, keccak256 } from '$lib/core/kernels';

/** The parallel space's fixed keys and their Safes (`golden-addresses.test.ts`). */
const FIXTURE_PUBLIC_KEYS = vi.hoisted(() => [
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b',
	'047802f2cc39cc6ed85c41268a580c5f0df36df3f065facfb40f84265927b7ed678438b2084cb84f0d00708e40d44ed8c9901d979bbcb390117089847672c12eec',
	'043eb2ae2f4e8090837820048baca2db04a7e7ca7dc6742f342a30c6855e7d96947f36ac5a4538dc3a8a7cbf0deea1c0ecb80bfb96a3f8ca57c98f8f400548cd56'
]);
const FIXTURE_SAFE = vi.hoisted(() => '0xD400866e00B055B20752a826CD5C89b811de130b');
const GOLDEN_MULTI_SAFE = vi.hoisted(() => '0x88cCA0EeDbF2C4426110bbFc998F048689266894');

const SINGLE = vi.hoisted(() => ({
	id: 'cred-1',
	address: FIXTURE_SAFE.toLowerCase(),
	publicKeyHex: FIXTURE_PUBLIC_KEYS[0]
}));
const MULTI = vi.hoisted(() => ({
	id: 'cred-1',
	address: GOLDEN_MULTI_SAFE.toLowerCase(),
	publicKeyHex: FIXTURE_PUBLIC_KEYS[0],
	keys: FIXTURE_PUBLIC_KEYS.map((publicKeyHex, i) => ({
		credentialId: `cred-${i + 1}`,
		publicKeyHex
	}))
}));
vi.mock('./accounts', () => ({
	findAccountByAddress: (address: string) =>
		[SINGLE, MULTI].find((a) => a.address === address.toLowerCase()),
	findAccountByCredentialId: () => undefined
}));
const rpc = vi.hoisted(() => ({
	code: '0x' as string | null,
	calls: [] as { method: string; params: unknown[]; chainId: number }[]
}));
vi.mock('./rpc-adapter', () => ({
	rpcCall: vi.fn(async (method: string, params: unknown[], chainId: number) => {
		rpc.calls.push({ method, params, chainId });
		if (rpc.code === null) throw new Error('[fault] chain unreachable');
		return { jsonrpc: '2.0', id: 1, result: rpc.code };
	})
}));
vi.mock('./networks', async (importOriginal) => ({
	...(await importOriginal<typeof import('./networks')>()),
	spaceNetworksSync: () => [{ chainId: 100 }, { chainId: 8453 }]
}));
/** The passkey: a well-formed assertion over exactly the challenge it is asked to sign. */
vi.mock('$lib/signing/sign-challenge', () => ({
	signChallenge: vi.fn(async (challenge: Uint8Array) => {
		const b64 = btoa(String.fromCharCode(...challenge))
			.replace(/\+/g, '-')
			.replace(/\//g, '_')
			.replace(/=+$/, '');
		const clientData = JSON.stringify({
			type: 'webauthn.get',
			challenge: b64,
			origin: 'https://getvela.app',
			crossOrigin: false
		});
		const hex = (bytes: Uint8Array) =>
			[...bytes].map((b) => b.toString(16).padStart(2, '0')).join('');
		const int = '01'.repeat(32);
		return {
			signatureHex: `3044022${'0'}${int}022${'0'}${int}`,
			authenticatorDataHex:
				'49960de5880e8c687434170f6476605b8fe4aeb9a28632c7995cf3ba831d97630500000000',
			clientDataJSONHex: hex(new TextEncoder().encode(clientData)),
			credentialId: 'cred-1'
		};
	})
}));

import { handleDAppRequest, messageSignatureFor } from './dapp-submit';

/** ERC-6492's magic suffix: 0x6492 repeated to 32 bytes. */
const MAGIC = '6492'.repeat(16);
const SIGNATURE = '0x' + 'ab'.repeat(97);

beforeEach(() => {
	rpc.code = '0x';
	rpc.calls.length = 0;
});

describe('messageSignatureFor', () => {
	it('an account with no code on the chain: the envelope, around the very signature', async () => {
		const out = await messageSignatureFor(SIGNATURE, SINGLE, SINGLE.address, 8453);
		expect(out.endsWith(MAGIC)).toBe(true);
		expect(out).toContain(SIGNATURE.slice(2));
		// Read on the REQUEST's chain, through the wallet's own RPC.
		expect(rpc.calls).toEqual([
			{ method: 'eth_getCode', params: [SINGLE.address, 'latest'], chainId: 8453 }
		]);
	});

	it('an account deployed there: the EIP-1271 signature as it was built', async () => {
		rpc.code = '0x6080604052';
		expect(await messageSignatureFor(SIGNATURE, SINGLE, SINGLE.address, 100)).toBe(SIGNATURE);
	});

	it('the code unread: wrapped — every 6492 verifier accepts the envelope from a deployed account too', async () => {
		rpc.code = null;
		expect(
			(await messageSignatureFor(SIGNATURE, SINGLE, SINGLE.address, 100)).endsWith(MAGIC)
		).toBe(true);
	});

	it('a multi-key account: its founding keys, in order, make the deployment', async () => {
		const out = await messageSignatureFor(SIGNATURE, MULTI, MULTI.address, 100);
		expect(out.endsWith(MAGIC)).toBe(true);
		// Out of order they are another wallet: the core refuses rather than wrap one.
		const shuffled = { ...MULTI, keys: [MULTI.keys[1], MULTI.keys[0], MULTI.keys[2]] };
		await expect(messageSignatureFor(SIGNATURE, shuffled, MULTI.address, 100)).rejects.toThrow();
	});

	it('no stored record: nothing is guessed', async () => {
		await expect(messageSignatureFor(SIGNATURE, undefined, SINGLE.address, 100)).rejects.toThrow(
			'Public key not found'
		);
	});
});

describe('every message the dApp path signs goes through it', () => {
	/**
	 * R22 (review 1-5): the review the core froze at the confirm — the digest
	 * the sheet read, for this account. Nothing is signed without it.
	 */
	const hex = (bytes: Uint8Array) =>
		'0x' + [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('');
	const reviewOf = (digest: Uint8Array, chainId: number) =>
		JSON.stringify({
			kind: 'message',
			chain_id: chainId,
			account: SINGLE.address,
			digest: hex(digest)
		});
	const eip191 = (bytes: Uint8Array) => {
		const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${bytes.length}`);
		const all = new Uint8Array(prefix.length + bytes.length);
		all.set(prefix);
		all.set(bytes, prefix.length);
		return keccak256(all);
	};
	const sign = (method: string, params: unknown[], review: string) =>
		handleDAppRequest(
			{ id: 'r1', method, params, origin: 'https://app.example' },
			{ id: 'cred-1' },
			SINGLE.address,
			100,
			undefined,
			undefined,
			undefined,
			undefined,
			'core',
			undefined,
			review
		) as Promise<string>;

	it('personal_sign (a sign-in among them) from an undeployed account: the envelope', async () => {
		const out = await sign(
			'personal_sign',
			['0x68656c6c6f', SINGLE.address],
			reviewOf(eip191(new TextEncoder().encode('hello')), 100)
		);
		expect(out.endsWith(MAGIC)).toBe(true);
		expect(rpc.calls[0]).toMatchObject({ method: 'eth_getCode', chainId: 100 });
	});

	it('typed data: read on the chain its domain names', async () => {
		rpc.code = '0x6080604052';
		const typed = {
			types: {
				EIP712Domain: [
					{ name: 'name', type: 'string' },
					{ name: 'chainId', type: 'uint256' }
				],
				Mail: [{ name: 'contents', type: 'string' }]
			},
			primaryType: 'Mail',
			domain: { name: 'Example', chainId: 8453 },
			message: { contents: 'hello' }
		};
		const out = await sign(
			'eth_signTypedData_v4',
			[SINGLE.address, JSON.stringify(typed)],
			reviewOf(hashTypedData(typed as never), 8453)
		);
		// Deployed there: no envelope.
		expect(out.endsWith(MAGIC)).toBe(false);
		expect(rpc.calls[0]).toMatchObject({ method: 'eth_getCode', chainId: 8453 });
	});
});
