/**
 * Where a signature goes (spec 071): the Clear Signer when this request's
 * "Sign with" says so — Settings' default, or the sheet's pick over it — and
 * the passkey otherwise, byte for byte as before.
 *
 * Driven end to end below the UI: the stored preference is read by the real
 * `sign_pref` core, the page's request is built by the real core
 * (`clearSignerRequest`), the page is a stand-in popup, and its answer — a
 * real P-256 signature — is judged by the real core. What comes back is what
 * a passkey would have returned, so the caller goes on exactly as it did.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

const store = vi.hoisted(() => new Map<string, string>());
const passkeys = vi.hoisted(() => ({ signWithAny: vi.fn() }));
/** The account record this device holds — a test may give its key a page. */
const accounts = vi.hoisted(() => ({ list: [] as Record<string, unknown>[] }));

vi.mock('$lib/core/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/client')>()),
	// The module is initialised synchronously above; nothing to fetch in node.
	loadCore: async () => {}
}));
vi.mock('$lib/services/storage', () => ({
	getItem: async (key: string) => store.get(key) ?? null,
	setItem: async (key: string, value: string) => void store.set(key, value),
	removeItem: async (key: string) => void store.delete(key)
}));
vi.mock('$lib/onboarding/core/storage', () => ({
	loadAccounts: () => accounts.list
}));
vi.mock('$lib/onboarding/core/passkey', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/onboarding/core/passkey')>()),
	signWithAny: passkeys.signWithAny
}));

import { PasskeyError, setSignMethod } from '$lib/onboarding/core/passkey';
import { signPreference } from '$lib/settings/core/sign-pref.svelte';
import { answer, fakeBrowser, passkey } from './__fixtures__/clear-signer-page';
import { ClearSignerRefusedError } from './clear-signer';
import { clearSignerSession } from './core/clear-signer.svelte';
import { cancelChallenge, signChallenge, type ChallengeSigner } from './sign-challenge';

const SAFE = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
const OWNER = passkey(7, [0x11, 0x22, 0x33]);
const STRANGER = passkey(9, [0x99]);
const DIGEST = new Uint8Array(32).fill(0x5a);

const OPERATION = {
	userOp: {
		sender: SAFE,
		nonce: '0x1a',
		initCode: '0x',
		callData: '0x7bb37428',
		verificationGasLimit: '300000',
		callGasLimit: '250000',
		preVerificationGas: '110000',
		maxFeePerGas: '0',
		maxPriorityFeePerGas: '0',
		paymasterAndData: '0x'
	},
	calls: [{ to: '0x' + 'ab'.repeat(20), value: '1000000000000000', data: '0x' }]
};

function signer(request: ChallengeSigner['request']): ChallengeSigner {
	return {
		account: SAFE,
		keys: [STRANGER.key, OWNER.key],
		credentials: [{ id: STRANGER.key.credentialId }, { id: OWNER.key.credentialId }],
		request
	};
}

const DAPP_TX = {
	method: 'eth_sendTransaction',
	params: [{ from: SAFE, to: '0x' + 'ab'.repeat(20), value: '0x38d7ea4c68000' }],
	origin: 'https://app.example',
	chainId: 100
};

/**
 * Start a signature, answer "where is your Clear Signer?" with this device,
 * and wait for the page to be opened and handed the intent (spec 075: every
 * Clear Signer request asks where first, and that tap is also the user
 * activation a popup needs).
 */
async function opened(
	request: ChallengeSigner['request'],
	operation?: typeof OPERATION,
	origin = 'http://127.0.0.1:8137'
): Promise<{ page: ReturnType<typeof fakeBrowser>; signing: Promise<unknown> }> {
	const page = fakeBrowser({ origin });
	clearSignerSession.host = page.host;
	const signing = signChallenge(DIGEST, signer(request), operation);
	signing.catch(() => {});
	await vi.waitFor(() => expect(clearSignerSession.view.asking).toBe(true));
	clearSignerSession.answerWhere('this_device');
	await vi.waitFor(() => expect(page.opened).toHaveLength(1));
	page.say({ vela: 'ready', v: 1 });
	return { page, signing };
}

beforeAll(async () => {
	accounts.list = [{ address: SAFE.toLowerCase(), name: 'Savings' }];
	// What Settings stored on this device: the Clear Signer, on a page of the
	// person's own (normalised by the core when it reads it back).
	store.set('vela.signMethod', 'clear_signer');
	store.set('vela.clearSignerUrl', 'http://127.0.0.1:8137');
	await signPreference.ready();
});

afterEach(() => {
	setSignMethod(null);
	passkeys.signWithAny.mockReset();
	accounts.list = [{ address: SAFE.toLowerCase(), name: 'Savings' }];
	clearSignerSession.dismiss();
});

describe('the stored default', () => {
	it('is read by the core: the Clear Signer, on the person’s own page, normalised', () => {
		expect(signPreference.view.method).toBe('clear_signer');
		expect(signPreference.view.signer_url).toBe('http://127.0.0.1:8137/');
	});
});

describe('the Clear Signer', () => {
	it('a dApp transaction: its own method, params and origin, and the ASSEMBLED operation', async () => {
		const { page, signing } = await opened(DAPP_TX, OPERATION);
		expect(page.opened).toEqual(['http://127.0.0.1:8137/sign.html?ch=post']);
		expect(clearSignerSession.view.waiting).toBe(true);
		const intent = page.intent();
		expect(page.posted[0].targetOrigin).toBe('http://127.0.0.1:8137');
		expect(intent?.intent).toEqual({
			method: 'eth_sendTransaction',
			params: DAPP_TX.params,
			origin: 'https://app.example'
		});
		expect(intent?.context).toMatchObject({
			chainId: 100,
			account: SAFE,
			signer: { name: 'Savings', letter: 'S' },
			// The fee leg is the one after the calls — always last.
			operation: { userOp: OPERATION.userOp, feeLegIndex: 1 }
		});

		page.say({ vela: 'result', id: page.id(), result: answer(OWNER, DIGEST) });
		const assertion = (await signing) as { credentialId: string; signatureHex: string };
		// A passkey's shape: the caller builds the Safe signature from it unchanged.
		expect(assertion.credentialId).toBe('112233');
		expect(assertion.signatureHex.startsWith('30')).toBe(true);
		expect(passkeys.signWithAny).not.toHaveBeenCalled();
		expect(clearSignerSession.view).toMatchObject({ waiting: false, notice: null });
	});

	it('the wallet’s own send: no method, no site — its calls are the intent', async () => {
		const { page, signing } = await opened(
			{ method: '', params: [], origin: '', chainId: 100 },
			OPERATION
		);
		const intent = page.intent();
		expect(intent?.intent).toMatchObject({ method: 'wallet_sendCalls', origin: '' });
		expect((intent?.intent.params as { calls: unknown[] }[])[0].calls).toEqual([
			{ to: OPERATION.calls[0].to, value: '0x38d7ea4c68000', data: '0x' }
		]);
		page.say({ vela: 'result', id: page.id(), result: answer(OWNER, DIGEST) });
		await expect(signing).resolves.toMatchObject({ credentialId: '112233' });
	});

	it('a message carries no operation', async () => {
		const { page, signing } = await opened({
			method: 'personal_sign',
			params: ['0x68656c6c6f', SAFE],
			origin: 'https://app.example',
			chainId: 100
		});
		expect(page.intent()?.context).not.toHaveProperty('operation');
		page.say({ vela: 'result', id: page.id(), result: answer(OWNER, DIGEST) });
		await expect(signing).resolves.toMatchObject({ credentialId: '112233' });
	});

	it('an answer the core refuses signs nothing — the request stays open, with the sentence', async () => {
		const { page, signing } = await opened(DAPP_TX, OPERATION);
		page.say({ vela: 'result', id: page.id(), result: answer(OWNER, new Uint8Array(32)) });
		const error = await signing.catch((e: unknown) => e);
		expect(error).toBeInstanceOf(ClearSignerRefusedError);
		// The kind every pipeline already reads as "nothing signed, still open".
		expect((error as PasskeyError).kind).toBe('cancelled');
		expect((error as ClearSignerRefusedError).code).toBe('wrong_challenge');
		expect(clearSignerSession.view).toMatchObject({ waiting: false, notice: 'mismatch' });
	});

	it('the page closed: declined, and said so', async () => {
		const { page, signing } = await opened(DAPP_TX, OPERATION);
		page.say({ vela: 'error', id: page.id(), code: 'user_rejected' });
		await expect(signing).rejects.toMatchObject({ code: 'declined', kind: 'cancelled' });
		expect(clearSignerSession.view.notice).toBe('closed');
	});

	it('the person’s own Cancel ends the wait with nothing to read', async () => {
		const { signing } = await opened(DAPP_TX, OPERATION);
		cancelChallenge();
		await expect(signing).rejects.toMatchObject({ code: 'declined' });
		expect(clearSignerSession.view).toMatchObject({ waiting: false, notice: null });
	});

	it('backing out of the question signs nothing, and says nothing either', async () => {
		const page = fakeBrowser({ origin: 'http://127.0.0.1:8137' });
		clearSignerSession.host = page.host;
		const signing = signChallenge(DIGEST, signer(DAPP_TX), OPERATION);
		signing.catch(() => {});
		await vi.waitFor(() => expect(clearSignerSession.view.asking).toBe(true));
		cancelChallenge();
		await expect(signing).rejects.toMatchObject({ code: 'declined' });
		expect(page.opened).toEqual([]);
		expect(clearSignerSession.view).toMatchObject({ asking: false, notice: null });
	});
});

describe('a key that lives behind a page (spec 075)', () => {
	it('is signed on ITS page, not the one Settings names — even on `auto`', async () => {
		accounts.list = [
			{
				address: SAFE.toLowerCase(),
				name: 'Savings',
				keys: [
					{
						credential_id: OWNER.key.credentialId,
						public_key_hex: OWNER.key.publicKeyHex,
						name: 'Savings',
						transports: '',
						signer_origin: 'http://localhost:8199'
					}
				]
			}
		];
		setSignMethod('auto');
		const { page, signing } = await opened(DAPP_TX, OPERATION, 'http://localhost:8199');
		expect(page.opened).toEqual(['http://localhost:8199/sign.html?ch=post']);
		page.say({ vela: 'result', id: page.id(), result: answer(OWNER, DIGEST) });
		await expect(signing).resolves.toMatchObject({ credentialId: '112233' });
		expect(passkeys.signWithAny).not.toHaveBeenCalled();
	});
});

describe('a passkey', () => {
	it('this request’s pick lies over the default: the passkey signs, as it always did', async () => {
		setSignMethod('platform');
		passkeys.signWithAny.mockResolvedValue({ credentialId: 'aa' });
		const page = fakeBrowser();
		clearSignerSession.host = page.host;
		await signChallenge(DIGEST, signer(DAPP_TX), OPERATION);
		expect(passkeys.signWithAny).toHaveBeenCalledWith(
			'5a'.repeat(32),
			[{ id: STRANGER.key.credentialId }, { id: OWNER.key.credentialId }],
			'platform'
		);
		expect(page.opened).toEqual([]);
	});

	it('with no sheet, Settings’ default is where the passkey is looked for', async () => {
		signPreference.chooseMethod('security_key');
		passkeys.signWithAny.mockResolvedValue({ credentialId: 'aa' });
		await signChallenge(DIGEST, signer({ method: '', params: [], origin: '', chainId: 100 }));
		expect(passkeys.signWithAny.mock.calls[0][2]).toBe('security_key');
		signPreference.chooseMethod('clear_signer');
	});
});
