/**
 * The key ceremonies on a Clear Signer page (spec 075), driven below the UI.
 *
 * The REAL core builds every request and judges every answer here: the page is
 * a stand-in popup, but what it sends back is a real WebAuthn-shaped answer
 * (a real P-256 signature over a real clientDataJSON), and the verdict is
 * `clearSignerVerifyCeremony` over wasm. What is pinned:
 *
 * - each of the four operations becomes the page request the contract names,
 *   with the wallet's own name on it;
 * - a create and the member proof that follows it ride ONE page visit;
 * - the member proof is accepted only over the challenge THIS wallet fetched
 *   from the registry — the one thing the page does not invent;
 * - a sign-in answer cannot be passed off as a proof (the core reads the
 *   challenge's form);
 * - every refusal reaches the machine as a cancelled ceremony, which is what
 *   leaves the flow where it was instead of raising an error.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

const store = vi.hoisted(() => new Map<string, string>());
const registry = vi.hoisted(() => ({
	challenge: new Uint8Array(32).fill(0x33),
	url: 'https://registry.example',
	asked: [] as unknown[]
}));

vi.mock('$lib/core/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
vi.mock('$lib/services/storage', () => ({
	getItem: async (key: string) => store.get(key) ?? null,
	setItem: async (key: string, value: string) => void store.set(key, value),
	removeItem: async (key: string) => void store.delete(key)
}));
vi.mock('./storage', () => ({ loadAccounts: () => [] }));
vi.mock('./registry', () => ({
	registryUrl: () => registry.url,
	memberChallenge: async (body: unknown) => {
		registry.asked.push(body);
		return {
			challenge: `0x${Buffer.from(registry.challenge).toString('hex')}`,
			challengeBase64url: Buffer.from(registry.challenge).toString('base64url')
		};
	}
}));

import { p256 } from '@noble/curves/p256';
import { sha256 } from '@noble/hashes/sha256';
import { signPreference } from '$lib/settings/core/sign-pref.svelte';
import { fakeBrowser } from '$lib/signing/__fixtures__/clear-signer-page';
import { ClearSignerRefusedError } from '$lib/signing/clear-signer';
import { clearSignerSession } from '$lib/signing/core/clear-signer.svelte';
import { buildMockRegistration } from '$lib/dev/passkey-fixture';
import {
	clearSignerCeremonyOf,
	endCeremonyFlow,
	rpIdOfSigner,
	runClearSignerCeremony
} from './clear-signer-ceremony';
import { createOnboardingExecutor } from './executor';
import type { ShellOperation } from '../generated/ShellOperation';

/** The page this wallet is pointed at; loopback http is a page the browser signs on. */
const PAGE = 'http://127.0.0.1:8137';
const ORIGIN = 'http://127.0.0.1:8137';
const WALLET = 'Savings';

const hex = (bytes: Uint8Array) =>
	Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
const b64url = (bytes: Uint8Array) => Buffer.from(bytes).toString('base64url');

/** A passkey the page holds: a P-256 key and the credential id it answers as. */
const KEY = (() => {
	const priv = new Uint8Array(32).fill(7);
	const credential = Uint8Array.from([0x11, 0x22, 0x33]);
	return { priv, credential, credentialHex: hex(credential), credentialB64: b64url(credential) };
})();

/** The page's answer to a ceremony: a real assertion over `challenge` (075 §10.3). */
function assertionAnswer(challenge: Uint8Array, options: { flags?: number; origin?: string } = {}) {
	const authenticatorData = Uint8Array.from([
		...sha256(new TextEncoder().encode('127.0.0.1')),
		options.flags ?? 0x05,
		0,
		0,
		0,
		9
	]);
	const clientDataJSON = new TextEncoder().encode(
		`{"type":"webauthn.get","challenge":"${b64url(challenge)}","origin":"${options.origin ?? ORIGIN}","crossOrigin":false}`
	);
	const signed = sha256(Uint8Array.from([...authenticatorData, ...sha256(clientDataJSON)]));
	const signature = p256.sign(signed, KEY.priv).toDERRawBytes();
	return {
		assertion: {
			credentialId: KEY.credentialB64,
			signatureDer: hex(signature),
			authenticatorData: hex(authenticatorData),
			clientDataJSON: hex(clientDataJSON),
			userHandle: hex(new TextEncoder().encode(`${WALLET}\u0000uuid`)),
			authenticatorAttachment: 'platform'
		},
		origin: ORIGIN
	};
}

/** The page's answer to a create: the fixture's attestation, at the page's origin. */
function registrationAnswer(origin = ORIGIN) {
	const made = buildMockRegistration({ rpId: '127.0.0.1', origin });
	const bare = (value: string) => value.replace(/^0x/i, '');
	return {
		registration: {
			credentialId: b64url(Uint8Array.from(Buffer.from(bare(made.credentialId), 'hex'))),
			attestationObject: bare(made.attestationObjectHex),
			clientDataJSON: bare(made.clientDataJSONHex),
			authenticatorAttachment: 'platform',
			transports: 'internal'
		},
		origin
	};
}

const REGISTER: ShellOperation = {
	type: 'register_passkey',
	name: WALLET,
	exclude_credential_ids: [],
	method: 'clear_signer'
};
const SIGN_IN: ShellOperation = { type: 'authenticate_passkey', method: 'clear_signer' };
const memberProof = (signerOrigin?: string): ShellOperation => ({
	type: 'sign_member_proof',
	credential_id: KEY.credentialHex,
	public_key_hex: `04${'ab'.repeat(64)}`,
	attestation_hex: '',
	transports: '',
	method: 'clear_signer',
	group_public_key_hex: `04${'cd'.repeat(64)}`,
	...(signerOrigin === undefined ? {} : { signer_origin: signerOrigin })
});

type FakePage = ReturnType<typeof fakeBrowser>;

const intentsOn = (page: FakePage) =>
	page.posted.filter((entry) => entry.message.vela === 'intent');

/**
 * Start a ceremony and hand the page its request.
 *
 * The "where" question comes up only when a page visit has to be OPENED: a
 * second ceremony in the same flow rides the visit the first one opened, which
 * is the whole point of a session — so this waits for whichever happens.
 */
async function asked(
	operation: ShellOperation,
	options: { origin?: string; page?: FakePage } = {}
) {
	const page = options.page ?? fakeBrowser({ origin: options.origin ?? ORIGIN });
	clearSignerSession.host = page.host;
	const before = intentsOn(page).length;
	const ran = runClearSignerCeremony(
		operation as Parameters<typeof runClearSignerCeremony>[0]
	).catch((error: unknown) => error);
	await vi.waitFor(() =>
		expect(clearSignerSession.view.asking || intentsOn(page).length > before).toBe(true)
	);
	if (clearSignerSession.view.asking) {
		clearSignerSession.answerWhere('this_device');
		await vi.waitFor(() => expect(page.opened.length).toBeGreaterThan(0));
		page.say({ vela: 'ready', v: 1 });
	}
	await vi.waitFor(() => expect(intentsOn(page)).toHaveLength(before + 1));
	const posted = intentsOn(page)[before].message as {
		id: string;
		intent: { method: string; params: unknown[] };
		context: Record<string, unknown>;
	};
	return { page, ran, id: posted.id, posted };
}

beforeAll(async () => {
	store.set('vela.clearSignerUrl', PAGE);
	await signPreference.ready();
});

beforeEach(() => {
	registry.asked.length = 0;
});

afterEach(() => {
	endCeremonyFlow();
	clearSignerSession.dismiss();
});

describe('which operations go to the page', () => {
	it('only the four passkey ones, and only when they name the Clear Signer', () => {
		expect(clearSignerCeremonyOf(REGISTER)).toBe(REGISTER);
		expect(clearSignerCeremonyOf(SIGN_IN)).toBe(SIGN_IN);
		expect(clearSignerCeremonyOf({ ...REGISTER, method: 'platform' })).toBeNull();
		expect(clearSignerCeremonyOf({ type: 'load_accounts' })).toBeNull();
	});

	it('the page’s rpId is its host, with every getvela.app page folded to one', () => {
		expect(rpIdOfSigner('https://sign.getvela.app/')).toBe('getvela.app');
		expect(rpIdOfSigner('https://getvela.app')).toBe('getvela.app');
		expect(rpIdOfSigner('http://localhost:4173/')).toBe('localhost');
		expect(rpIdOfSigner('not a url')).toBe('');
	});
});

describe('creating a key, and confirming its membership', () => {
	it('is one page visit: the create’s card, then the member proof’s', async () => {
		const { page, ran, posted: create } = await asked(REGISTER);
		expect(page.opened).toEqual([`${PAGE}/sign.html?ch=post`]);
		expect(create.intent).toMatchObject({
			method: 'vela_createPasskey',
			params: [{ name: WALLET, excludeCredentialIds: [] }],
			origin: ''
		});
		// The name is how a person recognises their own wallet on a page that is
		// deliberately not this app.
		expect(create.context).toMatchObject({ walletName: WALLET });

		page.say({ vela: 'result', id: page.id(), ...registrationAnswer() });
		const registered = await ran;
		expect(registered).toMatchObject({ type: 'passkey_registered' });
		// The key now lives behind that page, and the record says so.
		expect(
			(registered as { registration: { signer_origin?: string } }).registration.signer_origin
		).toBe(ORIGIN);

		// The member proof follows on the SAME window.
		const next = await asked(memberProof(), { page });
		const second = next.ran;
		expect(page.opened).toHaveLength(1);
		const member = next.posted;
		expect(member.intent.method).toBe('vela_memberProof');
		expect(member.intent.params[0]).toMatchObject({
			credentialId: KEY.credentialB64,
			registry: registry.url,
			groupPublicKey: `04${'cd'.repeat(64)}`
		});
		// The wallet fetched the challenge itself, under the PAGE's rpId.
		expect(registry.asked).toEqual([
			{
				rpId: '127.0.0.1',
				groupPublicKey: `04${'cd'.repeat(64)}`,
				publicKey: `04${'ab'.repeat(64)}`,
				attestation: ''
			}
		]);

		page.say({ vela: 'result', id: next.id, ...assertionAnswer(registry.challenge) });
		const proved = await second;
		expect(proved).toMatchObject({ type: 'member_proof_signed' });
		// The proof is the core's, built from the answer's own three fields.
		expect((proved as { proof: { r: string; s: string } }).proof.r).toMatch(/^0x[0-9a-f]{64}$/);
	});

	it('a member proof over any other challenge is refused, and nothing is reported', async () => {
		const { page, ran } = await asked(memberProof());
		page.say({ vela: 'result', id: page.id(), ...assertionAnswer(new Uint8Array(32).fill(9)) });
		const error = await ran;
		expect(error).toBeInstanceOf(ClearSignerRefusedError);
		expect((error as ClearSignerRefusedError).code).toBe('wrong_challenge');
		// The sheet says which way it failed.
		expect(clearSignerSession.view.notice).toBe('mismatch');
	});

	it('opens the page the KEY lives behind, not the one Settings names', async () => {
		const { page, ran } = await asked(memberProof('http://localhost:8199'), {
			origin: 'http://localhost:8199'
		});
		expect(page.opened).toEqual(['http://localhost:8199/sign.html?ch=post']);
		clearSignerSession.cancel();
		await ran;
	});
});

describe('signing in, and proving a key', () => {
	it('signs in over the challenge the page derived', async () => {
		const { page, ran } = await asked(SIGN_IN);
		expect(page.intent()?.intent).toMatchObject({ method: 'vela_signIn', params: [{}] });
		const challenge = new TextEncoder().encode('vela-signin-1790000000000-0102030405060708');
		page.say({ vela: 'result', id: page.id(), ...assertionAnswer(challenge) });
		const result = await ran;
		expect(result).toMatchObject({ type: 'passkey_authenticated' });
		const assertion = (result as { assertion: { signer_origin?: string; credential_id: string } })
			.assertion;
		expect(assertion.credential_id).toBe(KEY.credentialHex);
		expect(assertion.signer_origin).toBe(ORIGIN);
	});

	it('will not take a sign-in as a proof: the core reads the challenge’s form', async () => {
		const proof: ShellOperation = {
			type: 'sign_proof',
			credential_id: KEY.credentialHex,
			transports: '',
			method: 'clear_signer',
			purpose: 'verify'
		};
		const { page, ran } = await asked(proof);
		expect(page.intent()?.intent).toMatchObject({
			method: 'vela_proof',
			params: [{ credentialId: KEY.credentialB64, purpose: 'verify' }]
		});
		const signIn = new TextEncoder().encode('vela-signin-1790000000000-0102030405060708');
		page.say({ vela: 'result', id: page.id(), ...assertionAnswer(signIn) });
		expect((await ran) as ClearSignerRefusedError).toMatchObject({ code: 'wrong_challenge' });

		// The same ceremony, on the same page visit (a refusal does not end one),
		// answered with the proof's own challenge: taken.
		const again = await asked(proof, { page });
		const verify = new TextEncoder().encode('vela-verify-1790000000000');
		again.page.say({ vela: 'result', id: again.id, ...assertionAnswer(verify) });
		expect(await again.ran).toMatchObject({ type: 'proof_signed' });
	});

	it('a user who was not verified is not a ceremony', async () => {
		const { page, ran } = await asked(SIGN_IN);
		const challenge = new TextEncoder().encode('vela-signin-1790000000000-0102030405060708');
		page.say({ vela: 'result', id: page.id(), ...assertionAnswer(challenge, { flags: 0x01 }) });
		expect((await ran) as ClearSignerRefusedError).toMatchObject({ code: 'not_verified' });
	});

	it('an answer from another origin is not from the page this wallet opened', async () => {
		const { page, ran } = await asked(SIGN_IN);
		const challenge = new TextEncoder().encode('vela-signin-1790000000000-0102030405060708');
		// The transport lets it through (the window and origin are right) and the
		// CORE catches the origin the authenticator actually signed.
		page.say({
			vela: 'result',
			id: page.id(),
			...assertionAnswer(challenge, { origin: 'https://evil.example' })
		});
		expect((await ran) as ClearSignerRefusedError).toMatchObject({ code: 'malformed' });
	});
});

describe('how a ceremony ends without an answer', () => {
	it('a closed page is a cancelled ceremony — the flow stays where it was', async () => {
		const { page, ran } = await asked(SIGN_IN);
		page.say({ vela: 'error', id: page.id(), code: 'user_rejected' });
		const error = await ran;
		expect(error).toBeInstanceOf(ClearSignerRefusedError);
		// The kind every machine already reads as "nothing happened, still open".
		expect((error as ClearSignerRefusedError).kind).toBe('cancelled');
		expect(clearSignerSession.view.notice).toBe('closed');
	});

	it('the flow’s end says goodbye and lets the page go', async () => {
		const { page, ran } = await asked(SIGN_IN);
		const challenge = new TextEncoder().encode('vela-signin-1790000000000-0102030405060708');
		page.say({ vela: 'result', id: page.id(), ...assertionAnswer(challenge) });
		await ran;
		expect(page.popup.closed).toBe(false);
		endCeremonyFlow();
		expect(page.posted.at(-1)?.message).toMatchObject({ vela: 'bye' });
		expect(page.popup.closed).toBe(true);
	});
});

describe('the executor', () => {
	it('routes a `clear_signer` ceremony to the page instead of the browser’s sheet', async () => {
		const execute = createOnboardingExecutor({
			prompt: async () => true,
			complete: async () => {}
		});
		const page = fakeBrowser({ origin: ORIGIN });
		clearSignerSession.host = page.host;
		const ran = execute({ id: 1, operation: SIGN_IN }, new AbortController().signal);
		await vi.waitFor(() => expect(clearSignerSession.view.asking).toBe(true));
		clearSignerSession.answerWhere('this_device');
		await vi.waitFor(() => expect(page.opened).toHaveLength(1));
		page.say({ vela: 'ready', v: 1 });
		await vi.waitFor(() => expect(page.intent()).toBeDefined());
		const challenge = new TextEncoder().encode('vela-signin-1790000000000-0102030405060708');
		page.say({ vela: 'result', id: page.id(), ...assertionAnswer(challenge) });
		await expect(ran).resolves.toMatchObject({ type: 'passkey_authenticated' });
	});
});
