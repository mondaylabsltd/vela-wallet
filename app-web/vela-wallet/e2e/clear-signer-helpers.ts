/**
 * The harness the Clear Signer's e2e suites share (spec 075).
 *
 * Everything here stands up the REAL page — `app-web/clearsigning`, served on
 * a second localhost origin — and the things a ceremony needs around it: a
 * virtual authenticator inside the page's window, the registry the page and
 * the wallet both ask for a member challenge, and the two taps that are the
 * person's (where is your signer, and the codes match).
 *
 * Nothing is stubbed on the wallet's side of the conversation: the page runs
 * real WebAuthn, and the wallet's core judges the real answer.
 */
import { createServer, type Server } from 'node:http';
import { readFileSync, statSync } from 'node:fs';
import { webcrypto } from 'node:crypto';
import { extname, join, normalize } from 'node:path';
import { createContext, runInContext } from 'node:vm';
import { expect, type BrowserContext, type Page, type Route } from '@playwright/test';
import { en } from './live-helpers';

export const PAGE_ROOT = join(import.meta.dirname, '..', '..', 'clearsigning');

const TYPES: Record<string, string> = {
	'.html': 'text/html; charset=utf-8',
	'.js': 'text/javascript; charset=utf-8',
	'.css': 'text/css; charset=utf-8',
	'.svg': 'image/svg+xml',
	'.png': 'image/png',
	'.json': 'application/json'
};

export interface ServedPage {
	/** `http://localhost:<port>/` — `localhost`, not an IP: WebAuthn will not
	 *  take an IP address as a relying party. */
	url: string;
	origin: string;
	close(): void;
}

/** The signer page on its own origin, on a port the OS picks (agents share this machine). */
export async function serveSignerPage(): Promise<ServedPage> {
	const server: Server = createServer((request, response) => {
		const path = normalize(decodeURIComponent(new URL(request.url ?? '/', 'http://x').pathname));
		const file = join(PAGE_ROOT, path === '/' ? 'index.html' : path);
		try {
			if (!file.startsWith(PAGE_ROOT) || !statSync(file).isFile()) throw new Error('no');
			response.writeHead(200, {
				'content-type': TYPES[extname(file)] ?? 'application/octet-stream'
			});
			response.end(readFileSync(file));
		} catch {
			response.writeHead(404);
			response.end();
		}
	});
	await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
	const address = server.address();
	if (address === null || typeof address === 'string') throw new Error('no port');
	const url = `http://localhost:${address.port}/`;
	return { url, origin: `http://localhost:${address.port}`, close: () => server.close() };
}

// ---------------------------------------------------------------------------
// The registry both ends ask for a member challenge
// ---------------------------------------------------------------------------

/**
 * The page's OWN challenge derivation, loaded into this process.
 *
 * The member challenge is the one thing a wallet and a page must compute to
 * the same 32 bytes (PROTOCOL.md §10.1), so a stand-in registry that derived
 * it any other way would make the page refuse for the right reason and the
 * test fail for the wrong one. `lib/ceremony.js` is what the page runs;
 * running it here is what keeps the stand-in honest. Read-only use of
 * `app-web/clearsigning`.
 */
function pageCeremony(): {
	memberBinding(groupPublicKey: string, attestation: string): Uint8Array;
	memberChallenge(facts: {
		chainId: number;
		registry: string;
		rpId: string;
		publicKey: string;
		binding: Uint8Array;
	}): Uint8Array;
} {
	const sandbox: Record<string, unknown> = {
		crypto: webcrypto,
		TextEncoder,
		TextDecoder,
		btoa,
		atob,
		navigator: {},
		console
	};
	sandbox.window = sandbox;
	const context = createContext(sandbox);
	for (const file of ['lib/keccak.js', 'lib/abi.js', 'lib/encode.js', 'lib/ceremony.js']) {
		runInContext(readFileSync(join(PAGE_ROOT, file), 'utf8'), context, { filename: file });
	}
	const ns = sandbox.VelaCS as { ceremony: ReturnType<typeof pageCeremony> };
	return ns.ceremony;
}

/** The registry's two identities and the chain it files keys on. */
const CHAIN_ID = 100;
const DOMAIN_REGISTRY = '0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf';
/** The endpoint the wallet asks (its factory default); intercepted, never reached. */
export const REGISTRY_URL = /p256-index-v2\.getvela\.app/;

const hex = (bytes: Uint8Array) => `0x${Buffer.from(bytes).toString('hex')}`;
const b64url = (bytes: Uint8Array) => Buffer.from(bytes).toString('base64url');

export interface RegistryStub {
	/** Every request the stand-in answered, for the assertions about who asked. */
	asked: { method: string; path: string; body: Record<string, unknown> | null }[];
	handle(route: Route): Promise<void>;
}

/**
 * A stand-in for the public-key registry: honest about the member challenge
 * (the page checks it against its own derivation), and simply accepting about
 * the publish — `done` up front, which the core reads as "this exact group is
 * already on chain".
 */
export function registryStub(): RegistryStub {
	const ceremony = pageCeremony();
	const asked: RegistryStub['asked'] = [];

	const json = (route: Route, body: unknown) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			headers: { 'access-control-allow-origin': '*' },
			body: JSON.stringify(body)
		});

	return {
		asked,
		async handle(route: Route) {
			const request = route.request();
			const url = new URL(request.url());
			let body: Record<string, unknown> | null;
			try {
				body = request.postData()
					? (JSON.parse(request.postData() ?? '{}') as Record<string, unknown>)
					: null;
			} catch {
				body = null;
			}
			asked.push({ method: request.method(), path: url.pathname, body });

			if (url.pathname === '/api/health') {
				return json(route, {
					service: 'webauthn-p256-publickey-registry',
					status: 'ok',
					chainId: CHAIN_ID,
					domainRegistry: DOMAIN_REGISTRY
				});
			}
			if (url.pathname === '/api/challenge' && body) {
				if (Array.isArray(body.members)) {
					// GROUP mode, at publish. The group's own challenge is signed by
					// the software group key, and every member replays the proof it
					// collected at creation, so these bytes need only be well formed.
					const filler = (seed: number) => new Uint8Array(32).fill(seed);
					return json(route, {
						contentHash: hex(filler(1)),
						groupChallenge: { challenge: hex(filler(2)), challengeBase64url: b64url(filler(2)) },
						members: (body.members as { publicKey: string }[]).map((member, index) => ({
							publicKey: member.publicKey,
							challenge: hex(filler(10 + index)),
							challengeBase64url: b64url(filler(10 + index))
						}))
					});
				}
				// MEMBER mode: the challenge the page will derive for itself.
				const binding = ceremony.memberBinding(
					String(body.groupPublicKey),
					String(body.attestation ?? '')
				);
				const challenge = ceremony.memberChallenge({
					chainId: CHAIN_ID,
					registry: DOMAIN_REGISTRY,
					rpId: String(body.rpId),
					publicKey: String(body.publicKey),
					binding
				});
				return json(route, {
					challenge: hex(challenge),
					challengeBase64url: b64url(challenge),
					binding: hex(binding)
				});
			}
			if (url.pathname === '/api/register') {
				// Idempotent by content hash: `done` means it is already on chain.
				return json(route, { status: 'done' });
			}
			if (url.pathname.startsWith('/api/key/') || url.pathname.startsWith('/api/unit/')) {
				return route.fulfill({ status: 404, body: '{}', contentType: 'application/json' });
			}
			return route.fulfill({ status: 404, body: '{}', contentType: 'application/json' });
		}
	};
}

/**
 * Point both ends at the stand-in registry. The wallet's own fetches are a
 * PAGE route (it must win over the deny-all net registered before it); the
 * signer page's are a CONTEXT route, because that page is a window of its own.
 */
export async function stubRegistry(
	page: Page,
	context: BrowserContext,
	stub: RegistryStub
): Promise<void> {
	await context.route(REGISTRY_URL, (route) => void stub.handle(route));
	await page.route(REGISTRY_URL, (route) => void stub.handle(route));
}

// ---------------------------------------------------------------------------
// The browser's side
// ---------------------------------------------------------------------------

export interface VirtualAuthenticator {
	authenticatorId: string;
	/** Put a credential this authenticator already holds (a wallet signing in again). */
	addCredential(credential: {
		credentialIdBase64: string;
		privateKeyBase64: string;
		rpId: string;
		userHandle?: string;
	}): Promise<void>;
}

/**
 * A platform authenticator inside `target`'s own window — CDP's virtual
 * authenticator belongs to the target that added it, so it must be added to
 * the SIGNER page's window, not the wallet's.
 *
 * Resident keys and user verification, because every ceremony the page runs
 * asks for both; backup eligibility and state, because a key that claims
 * neither reads as device-bound and the create flow then demands a second key
 * before it will finish.
 */
export async function holdAuthenticator(target: Page): Promise<VirtualAuthenticator> {
	const cdp = await target.context().newCDPSession(target);
	await cdp.send('WebAuthn.enable');
	const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', {
		options: {
			protocol: 'ctap2',
			transport: 'internal',
			hasResidentKey: true,
			hasUserVerification: true,
			isUserVerified: true,
			automaticPresenceSimulation: true,
			defaultBackupEligibility: true,
			defaultBackupState: true
		}
	});
	return {
		authenticatorId,
		async addCredential(credential) {
			await cdp.send('WebAuthn.addCredential', {
				authenticatorId,
				credential: {
					credentialId: credential.credentialIdBase64,
					isResidentCredential: true,
					rpId: credential.rpId,
					privateKey: credential.privateKeyBase64,
					userHandle: Buffer.from(credential.userHandle ?? 'vela-e2e').toString('base64'),
					signCount: 0
				}
			});
		}
	};
}

/** A passkey's halves: the private key for an authenticator, the public for a wallet. */
export async function passkey(): Promise<{
	credentialHex: string;
	credentialB64: string;
	privateKeyB64: string;
	publicKeyHex: string;
}> {
	const pair = (await webcrypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, [
		'sign',
		'verify'
	])) as CryptoKeyPair;
	const rawId = Buffer.from(webcrypto.getRandomValues(new Uint8Array(16)));
	return {
		credentialHex: rawId.toString('hex'),
		credentialB64: rawId.toString('base64'),
		privateKeyB64: Buffer.from(await webcrypto.subtle.exportKey('pkcs8', pair.privateKey)).toString(
			'base64'
		),
		publicKeyHex: Buffer.from(await webcrypto.subtle.exportKey('raw', pair.publicKey)).toString(
			'hex'
		)
	};
}

/** Seed the app's IndexedDB KV before it boots (the executors' storage). */
export async function seedKv(page: Page, entries: Record<string, string>): Promise<void> {
	await page.addInitScript((seeded: Record<string, string>) => {
		const open = indexedDB.open('vela', 1);
		open.onupgradeneeded = () => open.result.createObjectStore('kv');
		open.onsuccess = () => {
			const tx = open.result.transaction('kv', 'readwrite');
			for (const [key, value] of Object.entries(seeded)) tx.objectStore('kv').put(value, key);
		};
	}, entries);
}

/**
 * Open a method chooser and wait for its four rows.
 *
 * The trigger TOGGLES the sheet, so this only presses it while no row is
 * showing: a page whose scripts have not attached their handlers yet swallows
 * the first press, and pressing blindly a second time would close what the
 * first one opened.
 */
export async function openMethodPicker(page: Page, triggerName: string): Promise<void> {
	const trigger = page.getByRole('button', { name: triggerName });
	const methods = page.locator('button.method');
	await expect
		.poll(
			async () => {
				if ((await methods.count()) === 0) {
					await trigger.click({ timeout: 10_000 }).catch(() => {
						/* the next turn tries again */
					});
				}
				return methods.count();
			},
			{ timeout: 45_000, intervals: [250, 500, 1000] }
		)
		.toBe(4);
}

/** "Where is your Clear Signer?" — the tap that also gives a popup its activation. */
export async function answerWhere(page: Page, where: 'this' | 'other'): Promise<void> {
	const label =
		where === 'this'
			? en('componentsUi.signing.clearSignerThisDevice')
			: en('componentsUi.signing.clearSignerOtherDevice');
	await page.getByText(en('componentsUi.signing.clearSignerWhere')).waitFor({ timeout: 30_000 });
	await page.getByRole('button', { name: label, exact: true }).click();
}

/** The page rendered a card; sign it there (its automation hook is the slide's own confirm). */
export async function signOnPage(popup: Page): Promise<void> {
	await popup.waitForFunction(
		() => !!(window as unknown as { __slider?: unknown }).__slider,
		null,
		{
			timeout: 30_000
		}
	);
	await popup.evaluate(() =>
		(window as unknown as { __slider: { __confirm(): void } }).__slider.__confirm()
	);
}

/** What the page is showing, for the waits that are about its state machine. */
export async function pagePhase(popup: Page): Promise<string> {
	return popup.evaluate(
		() => (window as unknown as { __velaState?: { phase?: string } }).__velaState?.phase ?? ''
	);
}
