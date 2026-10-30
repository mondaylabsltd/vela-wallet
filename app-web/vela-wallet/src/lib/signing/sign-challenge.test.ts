/**
 * Where a signature goes (founder, 2026-09-26): the key the account signed in
 * with, over the route it signed in over — never a choice made per signature.
 * The account record says which, the real core reads it (`signingPlan`, spec
 * 102 — migrating a record from before), and
 * this asserts what the browser is then actually asked: that one credential,
 * over the transports the core names, with the chosen method's hint only when
 * the sign-in found the key where it was chosen — a key found somewhere else is
 * not steered away from it.
 *
 * A record written before it named its sign-in key must sign exactly as it
 * always did: every founding key allowed with its own transports, no hint, the
 * browser left to pick.
 *
 * And where it is reviewed and signed is the core's one rule (`signingRoute`,
 * owner 2026-10-30: the page, everywhere): an account whose venue is a trusted
 * signing page is handed there — every kind, its own page for a custom-domain
 * account — and the page's answer is accepted only as the core accepts it:
 * over exactly the bytes the core computes from the request it built, which
 * must be the bytes this shell signs, by the account's key. A page that fails
 * its check is never opened. The browser (fetch, the window, its messages) is
 * stood in for; the core, and the page bytes it checks, are real.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { createHash, webcrypto } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { hashTypedData, keccak256 } from '$lib/core/kernels';
import { computeSafeMessageHash } from '$lib/services/safe-transaction';
import { AskerGoneError } from '$lib/signing/core/sign-types';
import { CHECK_MS, pageHandoff, type PageHandoffDeps } from './page-handoff.svelte';
import { safeTxHash } from '$lib/multisig/core/kernel';
import { bytesToHex } from '$lib/onboarding/core/passkey';
import { saveAccount } from '$lib/onboarding/core/storage';
import type { Account } from '$lib/onboarding/generated/Account';
import {
	PageDeclinedError,
	PageSignFailedError,
	setPageHandoffDeps,
	signChallenge,
	VenueBlockedError,
	type ChallengeSigner
} from './sign-challenge';

type Asked = {
	hints?: string[];
	allowCredentials?: { id: Uint8Array; transports?: string[] }[];
};
let asked: Asked[] = [];

class MemoryStorage implements Storage {
	#map = new Map<string, string>();
	get length() {
		return this.#map.size;
	}
	clear() {
		this.#map.clear();
	}
	getItem(key: string) {
		return this.#map.get(key) ?? null;
	}
	key(index: number) {
		return [...this.#map.keys()][index] ?? null;
	}
	removeItem(key: string) {
		this.#map.delete(key);
	}
	setItem(key: string, value: string) {
		this.#map.set(key, value);
	}
}

beforeEach(() => {
	asked = [];
	const location = { protocol: 'https:', hostname: 'getvela.app' };
	vi.stubGlobal('localStorage', new MemoryStorage());
	vi.stubGlobal('location', location);
	vi.stubGlobal('window', { location, PublicKeyCredential: function () {} });
	vi.stubGlobal('PublicKeyCredential', function () {});
	vi.stubGlobal('navigator', {
		credentials: {
			get: (options: { publicKey: Asked }) => {
				asked.push(options.publicKey);
				// The ceremony is not what is under test: refuse after recording.
				return Promise.reject(new DOMException('cancelled', 'NotAllowedError'));
			}
		}
	});
});

afterEach(() => {
	vi.unstubAllGlobals();
	setPageHandoffDeps(undefined);
	pageHandoff.current = null;
});

const ADDRESS = '0x2222222222222222222222222222222222222222';

/** The sign-in key as an older build wrote it (`signed_in_with`), which the core migrates. */
type LegacySignIn = {
	credential_id: string;
	method: 'platform' | 'hybrid' | 'security_key' | 'trusted_signer';
	transports?: string;
	signer_origin?: string;
};

/** Two founding keys: a built-in passkey, then a YubiKey — as an older build stored them. */
function wallet(signedInWith?: LegacySignIn): Account {
	return {
		id: 'aa01',
		name: 'Ann',
		address: ADDRESS,
		public_key_hex: '04' + '11'.repeat(64),
		created_at_iso: '2026-09-26T00:00:00.000Z',
		keys: [
			{
				credential_id: 'aa01',
				public_key_hex: '04' + '11'.repeat(64),
				name: 'Mac',
				transports: 'internal'
			},
			{
				credential_id: 'bb02',
				public_key_hex: '04' + '22'.repeat(64),
				name: 'YubiKey',
				transports: 'usb,nfc'
			}
		],
		...(signedInWith === undefined ? {} : { signed_in_with: signedInWith })
	} as unknown as Account;
}

/** What each path hands over, as it built it before: every key, each with what it registered. */
const SIGNER: ChallengeSigner = {
	account: ADDRESS,
	keys: [],
	credentials: [{ id: 'aa01', transports: 'internal' }, { id: 'bb02' }],
	request: { method: '', params: [], origin: '', chainId: 100 }
};

async function attempt(): Promise<{ asked: Asked | undefined; error: unknown }> {
	const error = await signChallenge(new Uint8Array([1, 2, 3]), SIGNER).then(
		() => null,
		(reason: unknown) => reason
	);
	return { asked: asked.at(-1), error };
}

/** A message, as a dApp asks it: what a page venue hands over (the browser stood in for). */
async function attemptOnPage(): Promise<{ error: unknown }> {
	setPageHandoffDeps(browser().deps);
	const error = await signChallenge(new Uint8Array(32), {
		...SIGNER,
		request: {
			method: 'personal_sign',
			params: ['0x68656c6c6f', ADDRESS],
			origin: '',
			chainId: 100
		}
	}).then(
		() => null,
		(reason: unknown) => reason
	);
	return { error };
}

const pinned = (request: Asked | undefined) =>
	(request?.allowCredentials ?? []).map((c) => ({
		id: bytesToHex(c.id),
		transports: c.transports
	}));

describe('an account that names its sign-in key', () => {
	it('signs with THAT key over THAT route — the second key, as a security key', async () => {
		saveAccount(
			wallet({ credential_id: 'bb02', method: 'security_key', transports: 'usb,nfc,ble,hybrid' })
		);
		const { asked: request } = await attempt();
		expect(asked).toHaveLength(1);
		// Not keys[0], and not every key for the browser to choose between.
		expect(pinned(request).map((c) => c.id)).toEqual(['bb02']);
	});

	it('found where it was chosen: the method’s hint, over the method’s transports', async () => {
		const cases = [
			['platform', 'internal', 'client-device', ['internal']],
			['hybrid', 'internal', 'hybrid', ['hybrid', 'internal']],
			['security_key', 'usb,nfc,ble', 'security-key', ['usb', 'nfc', 'ble']]
		] as const;
		for (const [method, found, hint, transports] of cases) {
			saveAccount(wallet({ credential_id: 'bb02', method, transports: found }));
			const { asked: request } = await attempt();
			expect(request?.hints, method).toEqual([hint]);
			expect(pinned(request), method).toEqual([{ id: 'bb02', transports: [...transports] }]);
		}
	});

	it('a record that says nothing about where it was found reads as found where chosen', async () => {
		saveAccount(wallet({ credential_id: 'aa01', method: 'hybrid' }));
		const { asked: request } = await attempt();
		expect(request?.hints).toEqual(['hybrid']);
		expect(pinned(request)).toEqual([{ id: 'aa01', transports: ['hybrid', 'internal'] }]);
	});

	it('"This device" answered by a phone or a key: every place named, and no hint', async () => {
		saveAccount(
			wallet({ credential_id: 'aa01', method: 'platform', transports: 'usb,nfc,ble,hybrid' })
		);
		const { asked: request } = await attempt();
		expect(asked).toHaveLength(1);
		expect('hints' in (request ?? {})).toBe(false);
		expect(pinned(request)).toEqual([
			{ id: 'aa01', transports: ['internal', 'usb', 'nfc', 'ble', 'hybrid'] }
		]);
	});

	it('a record this build wrote (sign_in_key, domain, venue) signs the same way', async () => {
		saveAccount({
			...wallet(),
			sign_in_key: { credential_id: 'bb02', method: 'security_key', transports: 'usb,nfc,ble' },
			signing_domain: 'getvela.app',
			signing_venue: { type: 'in_vela' }
		});
		const { asked: request } = await attempt();
		expect(request?.hints).toEqual(['security-key']);
		expect(pinned(request)).toEqual([{ id: 'bb02', transports: ['usb', 'nfc', 'ble'] }]);
	});
});

/**
 * R7 (owner, 2026-10-30): the venue applies on the web too. An account whose
 * venue is a page is handed there; nothing is asked of this browser's own
 * passkey prompt.
 */
describe('the page, everywhere: where a signature goes', () => {
	it('a getvela.app account whose venue is the official page is handed to it, not signed here', async () => {
		saveAccount({
			...wallet(),
			sign_in_key: { credential_id: 'aa01', method: 'platform', transports: 'internal' },
			signing_domain: 'getvela.app',
			signing_venue: { type: 'page', url: 'https://sign.getvela.app/' }
		});
		const pending = attemptOnPage();
		await until(() => pageHandoff.current !== null);
		expect(pageHandoff.current?.kind).toBe('signature');
		expect(pageHandoff.current?.host).toBe('sign.getvela.app');
		pageHandoff.current?.cancel();
		const { error } = await pending;
		expect(asked).toHaveLength(0);
		// Closing the hand-off is the person's "no".
		expect(error).toBeInstanceOf(PageDeclinedError);
	});

	it('an older record signed in on the official page migrates to it as its venue — the page', async () => {
		// ≤ 0.9.7 wrote the page as the method; the core reads it as the venue.
		saveAccount(
			wallet({
				credential_id: 'aa01',
				method: 'trusted_signer',
				signer_origin: 'https://sign.getvela.app'
			})
		);
		const pending = attemptOnPage();
		await until(() => pageHandoff.current !== null);
		pageHandoff.current?.cancel();
		await pending;
		expect(asked).toHaveLength(0);
	});

	it('a custom-domain account is handed to its OWN page — the only place its keys answer', async () => {
		saveAccount({
			...wallet(),
			sign_in_key: { credential_id: 'aa01', method: 'platform' },
			signing_domain: 'sign.example.com',
			signing_venue: { type: 'page', url: 'https://sign.example.com/' }
		});
		const pending = attemptOnPage();
		await until(() => pageHandoff.current !== null);
		expect(pageHandoff.current?.host).toBe('sign.example.com');
		expect(pageHandoff.current?.official).toBe(false);
		pageHandoff.current?.cancel();
		await pending;
		expect(asked).toHaveLength(0);
	});

	it('a custom-domain account whose page is unknown here is refused with the core’s reason', async () => {
		saveAccount({
			...wallet(),
			sign_in_key: { credential_id: 'aa01', method: 'platform' },
			signing_domain: 'sign.example.com',
			signing_venue: { type: 'in_vela' }
		});
		const { error } = await attempt();
		expect(asked).toHaveLength(0);
		expect(error).toBeInstanceOf(VenueBlockedError);
		expect((error as VenueBlockedError).block).toEqual({
			type: 'app_cannot_reach',
			domain: 'sign.example.com'
		});
	});

	it('a method no venue signs is never signed in Vela behind a page the person chose', async () => {
		saveAccount({
			...wallet(),
			sign_in_key: { credential_id: 'aa01', method: 'platform', transports: 'internal' },
			signing_domain: 'getvela.app',
			signing_venue: { type: 'page', url: 'https://sign.getvela.app/' }
		});
		const error = await signChallenge(new Uint8Array(32), {
			...SIGNER,
			request: { method: 'eth_sign', params: [], origin: 'https://x.example', chainId: 1 }
		}).then(
			() => null,
			(reason: unknown) => reason
		);
		expect(error).toBeInstanceOf(PageSignFailedError);
		expect(asked).toHaveLength(0);
		// …while an account in Vela signs it as before.
		saveAccount({
			...wallet(),
			sign_in_key: { credential_id: 'aa01', method: 'platform', transports: 'internal' },
			signing_domain: 'getvela.app',
			signing_venue: { type: 'in_vela' }
		});
		await signChallenge(new Uint8Array(32), {
			...SIGNER,
			request: { method: 'eth_sign', params: [], origin: 'https://x.example', chainId: 1 }
		}).catch(() => null);
		expect(asked).toHaveLength(1);
	});
});

describe('a record from before the sign-in key', () => {
	it('signs exactly as before: every key, its own transports, no hint', async () => {
		saveAccount(wallet());
		const { asked: request } = await attempt();
		expect(asked).toHaveLength(1);
		expect('hints' in (request ?? {})).toBe(false);
		expect(pinned(request)).toEqual([
			{ id: 'aa01', transports: ['internal'] },
			{ id: 'bb02', transports: undefined }
		]);
	});

	it('naming a key the wallet does not hold is read as no sign-in key at all', async () => {
		saveAccount(wallet({ credential_id: 'cc03', method: 'security_key' }));
		const { asked: request } = await attempt();
		expect('hints' in (request ?? {})).toBe(false);
		expect(pinned(request).map((c) => c.id)).toEqual(['aa01', 'bb02']);
	});

	it('with nothing stored for the address, the request is the path’s own', async () => {
		const { asked: request } = await attempt();
		expect(pinned(request).map((c) => c.id)).toEqual(['aa01', 'bb02']);
	});

	it('a first key minted on a custom-domain page: its venue is that page, and it goes there', async () => {
		const behind = wallet() as unknown as { keys: Record<string, unknown>[] };
		behind.keys[0] = { ...behind.keys[0], signer_origin: 'https://me.example' };
		saveAccount(behind as unknown as Account);
		const pending = attemptOnPage();
		await until(() => pageHandoff.current !== null);
		expect(pageHandoff.current?.host).toBe('me.example');
		pageHandoff.current?.cancel();
		await pending;
		expect(asked).toHaveLength(0);
	});

	it('a first key minted on the official page: the official page', async () => {
		const behind = wallet() as unknown as { keys: Record<string, unknown>[] };
		behind.keys[0] = { ...behind.keys[0], signer_origin: 'https://sign.getvela.app' };
		saveAccount(behind as unknown as Account);
		const pending = attemptOnPage();
		await until(() => pageHandoff.current !== null);
		expect(pageHandoff.current?.host).toBe('sign.getvela.app');
		pageHandoff.current?.cancel();
		await pending;
		expect(asked).toHaveLength(0);
	});
});

async function until(test: () => boolean) {
	for (let i = 0; i < 400 && !test(); i++) await new Promise((r) => setTimeout(r, 5));
	expect(test()).toBe(true);
}

// ---------------------------------------------------------------------------
// The hand-off itself: every kind, the answer the core accepts, the refusals
// ---------------------------------------------------------------------------

/** A version this build ships and `dist/` holds (`integrity::BUILD_ALLOWED`). */
const VERSION = '6d7172b4ba59735e09eaa8ea6c3d3c020ba7cd26399e6ab85be3619a23424d0a';
const PAGE_BYTES = readFileSync(
	fileURLToPath(new URL(`../../../../trusted-signer/dist/b/${VERSION}/sign.html`, import.meta.url))
);
const PAGE_ORIGIN = 'https://sign.getvela.app';
const CHAIN = 5042002;
const hexOf = (bytes: Uint8Array | ArrayBuffer) =>
	Buffer.from(new Uint8Array(bytes)).toString('hex');

/** An account with a real P-256 key whose venue is the official page. */
async function pageAccount() {
	const pair = await webcrypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, [
		'sign',
		'verify'
	]);
	const raw = new Uint8Array(await webcrypto.subtle.exportKey('raw', pair.publicKey));
	const credentialId = hexOf(webcrypto.getRandomValues(new Uint8Array(16)));
	saveAccount({
		id: credentialId,
		name: 'Ann',
		address: ADDRESS,
		public_key_hex: hexOf(raw),
		created_at_iso: '2026-10-30T00:00:00.000Z',
		keys: [
			{
				credential_id: credentialId,
				public_key_hex: hexOf(raw),
				name: 'Mac',
				transports: 'internal'
			}
		],
		sign_in_key: { credential_id: credentialId, method: 'platform', transports: 'internal' },
		signing_domain: 'getvela.app',
		signing_venue: { type: 'page', url: 'https://sign.getvela.app/' }
	} as unknown as Account);
	const keys = [{ credentialId, publicKeyHex: hexOf(raw) }];
	/** What the page would answer: a `webauthn.get` over `challenge`, by `key`. */
	const answer = async (challenge: Uint8Array, key: CryptoKey = pair.privateKey, uv = true) => {
		const auth = Buffer.concat([
			createHash('sha256').update('getvela.app').digest(),
			Buffer.from([uv ? 0x05 : 0x01, 0, 0, 0, 1])
		]);
		const client = Buffer.from(
			JSON.stringify({
				type: 'webauthn.get',
				challenge: Buffer.from(challenge).toString('base64url'),
				origin: PAGE_ORIGIN,
				crossOrigin: false
			})
		);
		const signed = Buffer.concat([auth, createHash('sha256').update(client).digest()]);
		const signature = await webcrypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, key, signed);
		return {
			credentialId: Buffer.from(credentialId, 'hex').toString('base64url'),
			signature: hexOf(signature),
			authenticatorData: hexOf(auth),
			clientDataJSON: hexOf(client)
		};
	};
	return { keys, answer, credentialId };
}

/** A stand-in browser: the page's host, one window, its messages, the timers. */
function browser(served: Buffer = PAGE_BYTES) {
	const sent: { message: unknown; origin: string }[] = [];
	let handler: ((event: MessageEvent) => void) | null = null;
	const win = {
		closed: false,
		postMessage: (message: unknown, origin: string) => sent.push({ message, origin }),
		focus: () => {},
		close() {
			this.closed = true;
		}
	};
	const ticks = new Map<number, () => void>();
	const deps: PageHandoffDeps = {
		fetch: (async (input: RequestInfo | URL) => {
			const url = String(input);
			if (url.endsWith('/index.json')) {
				return new Response(JSON.stringify({ versions: [VERSION] }), { status: 200 });
			}
			return new Response(new Uint8Array(served), { status: 200 });
		}) as typeof fetch,
		open: vi.fn(() => win as unknown as Window),
		now: () => Date.now(),
		lang: () => 'en',
		listen: (h) => {
			handler = h;
			return () => (handler = null);
		},
		sha256: async (bytes) => createHash('sha256').update(Buffer.from(bytes)).digest('hex'),
		// The keep-alive runs on the test's clock; the window watch on the real one.
		setInterval: (fn, ms) => {
			if (ms >= 60_000) {
				ticks.set(ms, fn);
				return `tick-${ms}`;
			}
			return setInterval(fn, ms);
		},
		clearInterval: (id) => {
			if (typeof id === 'string') ticks.delete(Number(id.slice(5)));
			else clearInterval(id as ReturnType<typeof setInterval>);
		}
	};
	const from = (data: unknown, origin = PAGE_ORIGIN, source: unknown = win) =>
		handler?.({ data, origin, source } as MessageEvent);
	const intent = () =>
		sent.find((s) => (s.message as { vela?: string }).vela === 'intent')?.message as
			| {
					id: string;
					intent: { method: string; params: unknown; origin: string };
					context: Record<string, unknown>;
			  }
			| undefined;
	const tickKeepAlive = () => ticks.get(60_000)?.();
	return { deps, win, sent, from, intent, tickKeepAlive };
}

/** The signature `kind` asks for, and the bytes this shell would sign for it. */
function signature(
	kind: 'personal_sign' | 'eth_signTypedData_v4' | 'eth_sendTransaction' | 'wallet_sendCalls' | ''
) {
	const userOp = {
		sender: ADDRESS,
		nonce: '0x7',
		initCode: '0x',
		callData: '0x7bb37428',
		verificationGasLimit: '300000',
		callGasLimit: '200000',
		preVerificationGas: '110000',
		maxFeePerGas: '1500000007',
		maxPriorityFeePerGas: '1500000000',
		paymasterAndData: '0x'
	};
	const operation = {
		userOp,
		calls: [{ to: '0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69', value: '1', data: '0x' }]
	};
	if (kind === 'personal_sign') {
		const text = new TextEncoder().encode('hello');
		const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${text.length}`);
		const original = keccak256(new Uint8Array([...prefix, ...text]));
		return {
			request: {
				method: kind,
				params: ['0x68656c6c6f', ADDRESS],
				origin: 'https://app.example',
				chainId: CHAIN
			},
			challenge: computeSafeMessageHash(original, CHAIN, ADDRESS),
			operation: undefined
		};
	}
	if (kind === 'eth_signTypedData_v4') {
		const typed = {
			types: {
				EIP712Domain: [
					{ name: 'name', type: 'string' },
					{ name: 'chainId', type: 'uint256' }
				],
				Mail: [{ name: 'contents', type: 'string' }]
			},
			primaryType: 'Mail',
			domain: { name: 'Ether Mail', chainId: CHAIN },
			message: { contents: 'Hello' }
		};
		return {
			request: {
				method: kind,
				params: [ADDRESS, JSON.stringify(typed)],
				origin: 'https://app.example',
				chainId: CHAIN
			},
			challenge: computeSafeMessageHash(hashTypedData(typed as never), CHAIN, ADDRESS),
			operation: undefined
		};
	}
	return {
		request: {
			method: kind,
			params:
				kind === 'eth_sendTransaction'
					? [{ to: operation.calls[0].to, value: '0x1' }]
					: kind === 'wallet_sendCalls'
						? [{ version: '1.0', chainId: `0x${CHAIN.toString(16)}`, from: ADDRESS, calls: [] }]
						: [],
			origin: kind === '' ? '' : 'https://app.example',
			chainId: CHAIN
		},
		// The SafeOp hash the shell signs, from the same operation.
		challenge: null as Uint8Array | null,
		operation
	};
}

/** The bytes a transaction kind signs: the SafeOp hash of the request's operation, from the core. */
async function safeOpOf(op: Record<string, string>): Promise<Uint8Array> {
	const { calculateSafeOpHash } = await import('$lib/services/safe-transaction');
	const bytes = (h: string) =>
		h === '0x' ? new Uint8Array() : new Uint8Array(Buffer.from(h.slice(2), 'hex'));
	return calculateSafeOpHash(
		{
			sender: op.sender,
			nonce: op.nonce,
			initCode: bytes(op.initCode),
			callData: bytes(op.callData),
			verificationGasLimit: BigInt(op.verificationGasLimit),
			callGasLimit: BigInt(op.callGasLimit),
			preVerificationGas: BigInt(op.preVerificationGas),
			maxFeePerGas: BigInt(op.maxFeePerGas),
			maxPriorityFeePerGas: BigInt(op.maxPriorityFeePerGas),
			paymasterAndData: bytes(op.paymasterAndData),
			signature: new Uint8Array()
		} as never,
		CHAIN
	);
}

describe('the page, everywhere: the hand-off and the answer the core accepts', () => {
	const KINDS = [
		'personal_sign',
		'eth_signTypedData_v4',
		'eth_sendTransaction',
		'wallet_sendCalls',
		''
	] as const;

	for (const kind of KINDS) {
		it(`${kind || 'the wallet’s own send'}: the core’s request goes to the checked page, and its signature comes back`, async () => {
			const { keys, answer } = await pageAccount();
			const { deps, from, intent, sent, win } = browser();
			setPageHandoffDeps(deps);
			const asks = signature(kind);
			const challenge = asks.challenge ?? (await safeOpOf(asks.operation!.userOp));
			const pending = signChallenge(
				challenge,
				{ account: ADDRESS, keys, credentials: [], request: asks.request },
				asks.operation
			);
			await until(() => pageHandoff.current?.phase === 'ready');
			expect(pageHandoff.current?.line?.key).toBe('componentsUi.signing.integrity.matches');
			pageHandoff.current?.open();
			expect(deps.open).toHaveBeenCalledWith(
				`https://sign.getvela.app/b/${VERSION}/sign?ch=post&lang=en`
			);
			from({ vela: 'ready', v: 1 });
			const handed = intent();
			expect(handed?.intent.method).toBe(kind === '' ? 'wallet_sendCalls' : kind);
			expect(handed?.context.account).toBe(ADDRESS);
			expect(handed?.context.chainId).toBe(CHAIN);
			if (asks.operation) {
				expect((handed?.context.operation as { userOp: unknown }).userOp).toEqual(
					asks.operation.userOp
				);
			}
			// Only to the page's own origin.
			expect(sent.every((s) => s.origin === PAGE_ORIGIN)).toBe(true);
			from({ vela: 'result', id: handed?.id, result: await answer(challenge) });
			const assertion = await pending;
			expect(assertion.credentialId).toBe(keys[0].credentialId);
			// DER, as the passkey gives it — every path continues unchanged.
			expect(assertion.signatureHex.startsWith('30')).toBe(true);
			expect(pageHandoff.current).toBeNull();
			expect(sent.some((s) => (s.message as { vela?: string }).vela === 'bye')).toBe(true);
			// S4: a finished page is not left behind.
			expect(win.closed).toBe(true);
		});
	}

	it('a multisig executor’s operation: the page is told its number and the core’s safeTxHash at it (r7-1)', async () => {
		const { keys, answer } = await pageAccount();
		const { deps, from, intent } = browser();
		setPageHandoffDeps(deps);
		const asks = signature('wallet_sendCalls');
		const multisig = '0x4c3fF6bC7D1A4b7f4e0E93B4C61b8b1a7E5dF2a9';
		const tx = {
			to: '0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69',
			value: '0x1',
			data: '0x',
			operation: 0,
			safeTxGas: '0x0',
			baseGas: '0x0',
			gasPrice: '0x0',
			gasToken: '0x0000000000000000000000000000000000000000',
			refundReceiver: '0x0000000000000000000000000000000000000000',
			nonce: '0x3'
		};
		const challenge = await safeOpOf(asks.operation!.userOp);
		const pending = signChallenge(
			challenge,
			{
				account: ADDRESS,
				keys,
				credentials: [],
				request: {
					method: 'vela_multisigExecute',
					params: { tx, multisig, name: 'R7 Team', threshold: 2 },
					origin: '',
					chainId: CHAIN
				}
			},
			asks.operation
		);
		await until(() => pageHandoff.current?.phase === 'ready');
		pageHandoff.current?.open();
		from({ vela: 'ready', v: 1 });
		const handed = intent();
		// The operation is the wallet's own (its calls), the execution's facts beside it.
		expect(handed?.intent.method).toBe('wallet_sendCalls');
		expect(handed?.context.multisig).toEqual({
			address: multisig,
			name: 'R7 Team',
			threshold: 2,
			nonce: '0x3',
			safeTxHash: safeTxHash(CHAIN, multisig, tx)
		});
		from({ vela: 'result', id: handed?.id, result: await answer(challenge) });
		expect((await pending).credentialId).toBe(keys[0].credentialId);
	});

	it('a page whose check never finishes is bounded: not answering, said so, nothing opened (r7-2)', async () => {
		const { keys } = await pageAccount();
		const { deps } = browser();
		setPageHandoffDeps({
			...deps,
			// A host that never answers.
			fetch: (() => new Promise<Response>(() => {})) as typeof fetch,
			// The check's deadline, at once.
			setTimeout: (fn, ms) => {
				if (ms === CHECK_MS) queueMicrotask(fn);
				return 0;
			},
			clearTimeout: () => {}
		});
		const asks = signature('personal_sign');
		const error = await signChallenge(asks.challenge!, {
			account: ADDRESS,
			keys,
			credentials: [],
			request: asks.request
		}).then(
			() => null,
			(e: unknown) => e
		);
		expect(error).toBeInstanceOf(PageSignFailedError);
		expect((error as PageSignFailedError).failure).toBe('silent');
		expect((error as PageSignFailedError).page).toBe('unanswered');
		expect(deps.open).not.toHaveBeenCalled();
		expect(pageHandoff.current).toBeNull();
		expect(CHECK_MS).toBeLessThanOrEqual(20_000);
	});

	it('an answer the wallet will not take withdraws the request: the page is told (r7-5)', async () => {
		const { keys, answer } = await pageAccount();
		const asks = signature('personal_sign');
		const { deps, from, intent, sent, win } = browser();
		setPageHandoffDeps(deps);
		const pending = signChallenge(asks.challenge!, {
			account: ADDRESS,
			keys,
			credentials: [],
			request: asks.request
		}).then(
			() => null,
			(e: unknown) => e
		);
		await until(() => pageHandoff.current?.phase === 'ready');
		pageHandoff.current?.open();
		from({ vela: 'ready', v: 1 });
		// A replayed answer: a real signature, over another request's bytes.
		from({ vela: 'result', id: intent()?.id, result: await answer(new Uint8Array(32).fill(3)) });
		const error = (await pending) as PageSignFailedError;
		// r7-7: said as what it is — the page's answer did not match.
		expect(error.page).toBe('mismatch');
		const bye = sent.find((s) => (s.message as { vela?: string }).vela === 'bye');
		expect(bye?.message).toEqual({ vela: 'bye', v: 1, reason: 'withdrawn' });
		expect(bye?.origin).toBe(PAGE_ORIGIN);
		// Left open, saying so — the person reads it there.
		expect(win.closed).toBe(false);
	});

	it('an answer over different bytes is refused (another operation, a bare EIP-191 hash)', async () => {
		const { keys, answer } = await pageAccount();
		const asks = signature('personal_sign');
		for (const other of [
			new Uint8Array(32).fill(7),
			keccak256(new TextEncoder().encode('hello'))
		]) {
			const { deps, from, intent } = browser();
			setPageHandoffDeps(deps);
			const pending = signChallenge(asks.challenge!, {
				account: ADDRESS,
				keys,
				credentials: [],
				request: asks.request
			}).then(
				() => null,
				(e: unknown) => e
			);
			await until(() => pageHandoff.current?.phase === 'ready');
			pageHandoff.current?.open();
			from({ vela: 'ready', v: 1 });
			from({ vela: 'result', id: intent()?.id, result: await answer(other) });
			const error = await pending;
			expect(error).toBeInstanceOf(PageSignFailedError);
			expect((error as PageSignFailedError).failure).toBe('unverified');
			expect((error as PageSignFailedError).code).toBe('wrong_challenge');
		}
	});

	it('the shell signing other bytes than the request describes is refused', async () => {
		const { keys, answer } = await pageAccount();
		const asks = signature('personal_sign');
		const { deps, from, intent } = browser();
		setPageHandoffDeps(deps);
		const tampered = new Uint8Array(32).fill(9);
		const pending = signChallenge(tampered, {
			account: ADDRESS,
			keys,
			credentials: [],
			request: asks.request
		}).then(
			() => null,
			(e: unknown) => e
		);
		await until(() => pageHandoff.current?.phase === 'ready');
		pageHandoff.current?.open();
		from({ vela: 'ready', v: 1 });
		// Even a page that signed exactly what the shell holds.
		from({ vela: 'result', id: intent()?.id, result: await answer(tampered) });
		expect(((await pending) as PageSignFailedError).code).toBe('wrong_challenge');
	});

	it('an answer by a key that is not the account’s is refused', async () => {
		const { keys, answer } = await pageAccount();
		const stranger = await webcrypto.subtle.generateKey(
			{ name: 'ECDSA', namedCurve: 'P-256' },
			true,
			['sign', 'verify']
		);
		const asks = signature('eth_signTypedData_v4');
		const { deps, from, intent } = browser();
		setPageHandoffDeps(deps);
		const pending = signChallenge(asks.challenge!, {
			account: ADDRESS,
			keys,
			credentials: [],
			request: asks.request
		}).then(
			() => null,
			(e: unknown) => e
		);
		await until(() => pageHandoff.current?.phase === 'ready');
		pageHandoff.current?.open();
		from({ vela: 'ready', v: 1 });
		from({
			vela: 'result',
			id: intent()?.id,
			result: await answer(asks.challenge!, stranger.privateKey)
		});
		const error = (await pending) as PageSignFailedError;
		expect(error.failure).toBe('unverified');
		expect(error.code).toBe('bad_signature');
	});

	it('a page that does not match its check is never opened, and the card says why', async () => {
		const { keys } = await pageAccount();
		const { deps } = browser(Buffer.from('<!doctype html><p>not the page</p>'));
		setPageHandoffDeps(deps);
		const asks = signature('personal_sign');
		const pending = signChallenge(asks.challenge!, {
			account: ADDRESS,
			keys,
			credentials: [],
			request: asks.request
		}).then(
			() => null,
			(e: unknown) => e
		);
		await until(() => pageHandoff.current?.phase === 'refused');
		expect(pageHandoff.current?.line?.key).toBe('componentsUi.signing.integrity.mismatch');
		pageHandoff.current?.open();
		expect(deps.open).not.toHaveBeenCalled();
		pageHandoff.current?.cancel();
		expect(((await pending) as PageSignFailedError).failure).toBe('not_opened');
	});

	it('only the window this opened, from the page’s own origin, is heard', async () => {
		const { keys, answer } = await pageAccount();
		const asks = signature('personal_sign');
		const { deps, from, intent, win } = browser();
		setPageHandoffDeps(deps);
		const pending = signChallenge(asks.challenge!, {
			account: ADDRESS,
			keys,
			credentials: [],
			request: asks.request
		});
		await until(() => pageHandoff.current?.phase === 'ready');
		pageHandoff.current?.open();
		// Another origin, another window: ignored — no intent handed to them.
		from({ vela: 'ready', v: 1 }, 'https://evil.example');
		from({ vela: 'ready', v: 1 }, PAGE_ORIGIN, {});
		expect(intent()).toBeUndefined();
		from({ vela: 'ready', v: 1 });
		const good = await answer(asks.challenge!);
		from({ vela: 'result', id: intent()?.id, result: good }, 'https://evil.example');
		from({ vela: 'result', id: 'someone-else', result: good });
		expect(pageHandoff.current?.phase).toBe('waiting');
		from({ vela: 'result', id: intent()?.id, result: good }, PAGE_ORIGIN, win);
		expect((await pending).credentialId).toBe(keys[0].credentialId);
	});

	it('declined on the page, or the page closed: the person’s “no”; refused by its rules: not signed', async () => {
		const { keys } = await pageAccount();
		const asks = signature('personal_sign');
		for (const [how, expected] of [
			['user_rejected', PageDeclinedError],
			['closed', PageDeclinedError],
			['refused', PageSignFailedError]
		] as const) {
			const { deps, from, win, intent } = browser();
			setPageHandoffDeps(deps);
			const pending = signChallenge(asks.challenge!, {
				account: ADDRESS,
				keys,
				credentials: [],
				request: asks.request
			}).then(
				() => null,
				(e: unknown) => e
			);
			await until(() => pageHandoff.current?.phase === 'ready');
			pageHandoff.current?.open();
			from({ vela: 'ready', v: 1 });
			if (how === 'closed') win.close();
			else from({ vela: 'error', id: intent()?.id, code: how });
			const error = await pending;
			expect(error, how).toBeInstanceOf(expected);
		}
	});

	it('while the page has it, the asker is kept; a request that is gone closes the page', async () => {
		const { keys } = await pageAccount();
		const asks = signature('personal_sign');
		const { deps, from, win, tickKeepAlive, sent } = browser();
		setPageHandoffDeps(deps);
		let live = true;
		const keepAlive = vi.fn(async () => live);
		const pending = signChallenge(asks.challenge!, {
			account: ADDRESS,
			keys,
			credentials: [],
			request: asks.request,
			keepAlive
		}).then(
			() => null,
			(e: unknown) => e
		);
		await until(() => pageHandoff.current?.phase === 'ready');
		pageHandoff.current?.open();
		from({ vela: 'ready', v: 1 });
		tickKeepAlive();
		await until(() => keepAlive.mock.calls.length === 1);
		expect(pageHandoff.current?.phase).toBe('waiting');
		live = false;
		tickKeepAlive();
		expect(await pending).toBeInstanceOf(AskerGoneError);
		expect(win.closed).toBe(true);
		// r7-5: told before it closed, in case it does not.
		expect(sent.some((s) => (s.message as { reason?: string }).reason === 'withdrawn')).toBe(true);
	});
});
