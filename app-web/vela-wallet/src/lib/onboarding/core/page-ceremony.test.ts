/**
 * A key ceremony on the signing page (spec 102 R3, R8): what the onboarding
 * executor does with an operation the core sent to a page — every one in the
 * Safari package, whose pages cannot use a `getvela.app` passkey.
 *
 * The browser (the page's host, its window, its messages) is stood in for; the
 * core (the request it builds, the verdict on the answer) and the published
 * page bytes it checks are real. Asserted: the page is handed the core's own
 * request — the ceremony, the place the person chose and its hints — under the
 * ceremony's own card; an answer the core verifies comes back as the
 * machine's own `Assertion`, carrying the page's origin; the person's "no" is
 * a cancel; an answer from another origin, or over a challenge the page was
 * not allowed to sign, is refused — nothing reaches the machine.
 */
import '$lib/i18n/wasm-init.server';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { pageHandoff, type PageHandoffDeps } from '$lib/signing/page-handoff.svelte';
import { createOnboardingExecutor, operationFailure, type OnboardingEffect } from './executor';
import { setCeremonyHandoffDeps } from './page-ceremony';
import type { ShellOperation } from '../generated/ShellOperation';

/** The front of the core's BUILD_ALLOWED, as published in `dist/`. */
const VERSION = '1f9092c7278df85fcaf3ff02ece7b757a709c475dc72b9dee6a9ce44151c14c2';
const PAGE_BYTES = readFileSync(
	fileURLToPath(
		new URL(`../../../../../trusted-signer/dist/b/${VERSION}/sign.html`, import.meta.url)
	)
);
const OFFICIAL = 'https://sign.getvela.app/';
const PAGE_ORIGIN = 'https://sign.getvela.app';

const hex = (bytes: Uint8Array | Buffer) => Buffer.from(bytes).toString('hex');

/** A stand-in browser: the page's host, one window, its messages. */
function browser() {
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
	const deps: PageHandoffDeps = {
		fetch: (async (input: RequestInfo | URL) =>
			String(input).endsWith('/index.json')
				? new Response(JSON.stringify({ versions: [VERSION] }), { status: 200 })
				: new Response(new Uint8Array(PAGE_BYTES), { status: 200 })) as typeof fetch,
		open: vi.fn(() => win as unknown as Window),
		now: () => Date.now(),
		lang: () => 'en',
		listen: (h) => {
			handler = h;
			return () => (handler = null);
		},
		sha256: async (bytes) => createHash('sha256').update(Buffer.from(bytes)).digest('hex')
	};
	const from = (data: unknown, origin = PAGE_ORIGIN) =>
		handler?.({ data, origin, source: win } as unknown as MessageEvent);
	const intent = () =>
		sent.find((s) => (s.message as { vela?: string }).vela === 'intent')?.message as
			{ id: string; intent: { method: string; params: Record<string, unknown>[] } } | undefined;
	return { deps, win, sent, from, intent };
}

/** Run `operation` through the executor, opening the page when it is offered. */
async function onThePage(operation: ShellOperation, page: ReturnType<typeof browser>) {
	setCeremonyHandoffDeps(page.deps);
	const execute = createOnboardingExecutor({
		prompt: async () => false,
		complete: async () => {}
	});
	const effect: OnboardingEffect = { id: 1, operation };
	const running = execute(effect, new AbortController().signal).catch((error: unknown) =>
		operationFailure(effect, error)
	);
	await vi.waitFor(() => expect(pageHandoff.current?.phase).toBe('ready'));
	const card = pageHandoff.current;
	card?.open();
	page.from({ vela: 'ready', v: 1 });
	await vi.waitFor(() => expect(page.intent()).toBeDefined());
	return { running, card };
}

/** A `webauthn.get` the page would answer with, over `challenge`, seen at `origin`. */
function assertion(challenge: Uint8Array, origin = PAGE_ORIGIN) {
	const auth = Buffer.concat([
		createHash('sha256').update('getvela.app').digest(),
		Buffer.from([0x05, 0, 0, 0, 1])
	]);
	const client = Buffer.from(
		JSON.stringify({
			type: 'webauthn.get',
			challenge: Buffer.from(challenge).toString('base64url'),
			origin,
			crossOrigin: false
		})
	);
	return {
		credentialId: Buffer.from('c0ffee01', 'hex').toString('base64url'),
		signatureDer: '3006020101020101',
		authenticatorData: hex(auth),
		clientDataJSON: hex(client),
		userHandle: 'abcd',
		authenticatorAttachment: 'platform'
	};
}

beforeEach(() => {
	const location = { protocol: 'https:', hostname: 'wallet.getvela.app' };
	vi.stubGlobal('location', location);
	vi.stubGlobal('window', { location });
});

afterEach(() => {
	vi.unstubAllGlobals();
	setCeremonyHandoffDeps(undefined);
	pageHandoff.current = null;
});

describe('a ceremony the core sends to a page (R3, R8)', () => {
	it('hands a new key to the page: the core request, the place chosen, the ceremony card', async () => {
		const page = browser();
		const { running, card } = await onThePage(
			{
				type: 'register_passkey',
				name: 'Savings',
				exclude_credential_ids: [],
				method: 'hybrid',
				page: OFFICIAL
			},
			page
		);
		expect(card?.kind).toBe('ceremony');
		expect(card?.titleKey).toBe('componentsUi.signing.ceremonyCreate');
		expect(card?.keyLabel?.label_key).toBe('componentsUi.signing.newKeyOnLabel');
		expect(card?.keyLabel?.place_key).toBe('onboarding.create.methodHybridTitle');
		expect(card?.official).toBe(true);
		expect(page.deps.open).toHaveBeenCalledWith(`${OFFICIAL}b/${VERSION}/sign?ch=post&lang=en`);
		const asked = page.intent();
		expect(asked?.intent.method).toBe('vela_createPasskey');
		expect(asked?.intent.params[0]).toMatchObject({
			name: 'Savings',
			place: 'hybrid',
			hints: ['hybrid']
		});
		// The person closes the page unsigned: a cancel, said as one.
		page.win.closed = true;
		page.from({ vela: 'bye', v: 1 });
		const result = await running;
		expect(result).toMatchObject({ type: 'passkey_failed', kind: 'cancelled' });
	});

	it("brings a sign-in back as the machine's own assertion, from the page's origin", async () => {
		const page = browser();
		const { running, card } = await onThePage(
			{ type: 'authenticate_passkey', method: 'platform', page: OFFICIAL },
			page
		);
		expect(card?.titleKey).toBe('componentsUi.signing.ceremonySignIn');
		const asked = page.intent();
		expect(asked?.intent.method).toBe('vela_signIn');
		const challenge = new TextEncoder().encode(`vela-signin-${Date.now()}-0123456789abcdef`);
		page.from({
			vela: 'result',
			id: asked?.id,
			assertion: assertion(challenge),
			origin: PAGE_ORIGIN
		});
		const result = (await running) as {
			type: string;
			assertion: { credential_id: string; signer_origin?: string; user_id_hex?: string };
		};
		expect(result.type).toBe('passkey_authenticated');
		expect(result.assertion.credential_id).toBe('c0ffee01');
		expect(result.assertion.signer_origin).toBe(PAGE_ORIGIN);
		expect(result.assertion.user_id_hex).toBe('abcd');
		// The page is told it is done, and its tab goes.
		expect(page.sent.some((s) => (s.message as { vela?: string }).vela === 'bye')).toBe(true);
		expect(page.win.closed).toBe(true);
	});

	it('refuses an answer the authenticator saw on another origin', async () => {
		const page = browser();
		const { running } = await onThePage(
			{ type: 'authenticate_passkey', method: 'platform', page: OFFICIAL },
			page
		);
		const challenge = new TextEncoder().encode(`vela-signin-${Date.now()}-0123456789abcdef`);
		page.from({
			vela: 'result',
			id: page.intent()?.id,
			assertion: assertion(challenge, 'https://evil.example'),
			origin: PAGE_ORIGIN
		});
		const result = await running;
		expect(result).toMatchObject({ type: 'passkey_failed', kind: 'other' });
		expect((result as { message: string }).message).toMatch(/did not check out/);
	});

	it('refuses a sign-in over a challenge the page was not allowed to sign', async () => {
		const page = browser();
		const { running } = await onThePage(
			{ type: 'authenticate_passkey', method: 'platform', page: OFFICIAL },
			page
		);
		// 32 bytes: what an operation hash is — never a sign-in.
		page.from({
			vela: 'result',
			id: page.intent()?.id,
			assertion: assertion(new Uint8Array(32).fill(7)),
			origin: PAGE_ORIGIN
		});
		const result = await running;
		expect(result).toMatchObject({ type: 'passkey_failed', kind: 'other' });
		expect((result as { message: string }).message).toMatch(/wrong_challenge/);
	});

	it("the page's own refusal is the page's, and the person's decline is a cancel", async () => {
		for (const [code, kind] of [
			['refused', 'other'],
			['user_rejected', 'cancelled']
		] as const) {
			const page = browser();
			const { running } = await onThePage(
				{ type: 'authenticate_passkey', method: 'platform', page: OFFICIAL },
				page
			);
			page.from({ vela: 'error', id: page.intent()?.id, code });
			expect(await running).toMatchObject({ type: 'passkey_failed', kind });
			pageHandoff.current = null;
		}
	});
});
