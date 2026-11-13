/**
 * A multisig member's approval on the member's trusted signing page
 * (2026-10-29), asked of the real core: the page is checked before it is
 * opened (R6, on the bytes it serves), the request the page is handed is the
 * core's, and the answer is accepted only as the core accepts it — a
 * user-verified assertion over exactly the approval bytes, by the member's
 * key. The browser (fetch, the window, its messages) is stood in for.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { createHash, webcrypto } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Account } from '$lib/core/generated/Account';
import { multisigApprovalRoute } from '$lib/core/kernels';
import {
	approvalChallenge,
	safeTxHash,
	velaAccountGenesis,
	verifyApproval,
	type SafeTx
} from './kernel';
import {
	approveOnPage,
	checkPage,
	forgetPageAnswers,
	PageApprovalError,
	pageHandoff,
	type PageApprovalDeps
} from './page-approval.svelte';

/** A version this build ships and `dist/` holds (`integrity::BUILD_ALLOWED`). */
const VERSION = '6d7172b4ba59735e09eaa8ea6c3d3c020ba7cd26399e6ab85be3619a23424d0a';
const PAGE_BYTES = readFileSync(
	fileURLToPath(
		new URL(`../../../../../trusted-signer/dist/b/${VERSION}/sign.html`, import.meta.url)
	)
);
const OFFICIAL = 'https://sign.getvela.app/';
const CHAIN = 5042002;
const MULTISIG = '0x4c3FF6Bc7d1a4b7f4e0e93B4C61b8b1a7e5dF2A9';
const TX: SafeTx = {
	to: '0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69',
	value: '0xde0b6b3a7640000',
	data: '0x',
	operation: 0,
	safeTxGas: '0x0',
	baseGas: '0x0',
	gasPrice: '0x0',
	gasToken: '0x0000000000000000000000000000000000000000',
	refundReceiver: '0x0000000000000000000000000000000000000000',
	nonce: '0x3'
};

const hex = (bytes: Uint8Array | ArrayBuffer) => Buffer.from(new Uint8Array(bytes)).toString('hex');

/** A member: a P-256 key, its Vela account and the record a device keeps. */
async function member() {
	const pair = await webcrypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, [
		'sign',
		'verify'
	]);
	const raw = new Uint8Array(await webcrypto.subtle.exportKey('raw', pair.publicKey));
	const credentialId = hex(webcrypto.getRandomValues(new Uint8Array(16)));
	const keys = [{ credentialId, publicKeyHex: hex(raw) }];
	const { address } = velaAccountGenesis(keys);
	const account = {
		id: credentialId,
		name: 'Alice',
		address,
		public_key_hex: hex(raw),
		created_at_iso: '2026-10-29T00:00:00.000Z',
		keys: [
			{
				credential_id: credentialId,
				public_key_hex: hex(raw),
				name: 'Alice',
				transports: 'internal'
			}
		],
		sign_in_key: { credential_id: credentialId, method: 'platform', transports: 'internal' },
		signing_domain: 'getvela.app',
		signing_venue: { type: 'page', url: OFFICIAL }
	} as unknown as Account;
	/** What the page would answer: a `webauthn.get` over `challenge` (hex). */
	const answer = async (challengeHex: string, uv = true) => {
		const auth = Buffer.concat([
			createHash('sha256').update('getvela.app').digest(),
			Buffer.from([uv ? 0x05 : 0x01, 0, 0, 0, 1])
		]);
		const client = Buffer.from(
			JSON.stringify({
				type: 'webauthn.get',
				challenge: Buffer.from(challengeHex.replace(/^0x/, ''), 'hex').toString('base64url'),
				origin: 'https://sign.getvela.app',
				crossOrigin: false
			})
		);
		const signed = Buffer.concat([auth, createHash('sha256').update(client).digest()]);
		const signature = await webcrypto.subtle.sign(
			{ name: 'ECDSA', hash: 'SHA-256' },
			pair.privateKey,
			signed
		);
		return {
			credentialId: Buffer.from(credentialId, 'hex').toString('base64url'),
			signature: hex(signature),
			authenticatorData: hex(auth),
			clientDataJSON: hex(client)
		};
	};
	return { account, keys, answer };
}

/** A stand-in browser: the page's host, one window, its messages. */
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
	const fetched: string[] = [];
	const deps: PageApprovalDeps = {
		fetch: (async (input: RequestInfo | URL) => {
			const url = String(input);
			fetched.push(url);
			if (url.endsWith('/index.json')) {
				return new Response(JSON.stringify({ versions: [VERSION] }), { status: 200 });
			}
			return new Response(new Uint8Array(served), { status: 200 });
		}) as typeof fetch,
		open: vi.fn(() => win as unknown as Window),
		now: () => 1_793_000_000_000,
		lang: () => 'en',
		listen: (h) => {
			handler = h;
			return () => (handler = null);
		},
		sha256: async (bytes) => createHash('sha256').update(Buffer.from(bytes)).digest('hex')
	};
	const from = (data: unknown, origin = 'https://sign.getvela.app', source: unknown = win) =>
		handler?.({ data, origin, source } as MessageEvent);
	return { deps, win, sent, from, fetched };
}

async function until(test: () => boolean) {
	for (let i = 0; i < 200 && !test(); i++) await new Promise((r) => setTimeout(r, 5));
	expect(test()).toBe(true);
}

afterEach(() => {
	forgetPageAnswers();
	pageHandoff.current = null;
});

describe('where a member approves', () => {
	it('is the page the member chose, else Vela', async () => {
		const { account } = await member();
		expect(multisigApprovalRoute(account)).toEqual({ type: 'page', url: OFFICIAL });
		expect(multisigApprovalRoute({ ...account, signing_venue: { type: 'in_vela' } })).toEqual({
			type: 'in_vela'
		});
	});
});

describe('the page is checked before it opens', () => {
	it('opens the version the core picked, when its bytes are that version', async () => {
		const { deps, fetched } = browser();
		const checked = await checkPage(OFFICIAL, deps);
		expect(fetched).toEqual([`${OFFICIAL}index.json`, `${OFFICIAL}b/${VERSION}/sign`]);
		expect(checked.line.opens).toBe(true);
		expect(checked.launch).toBe(`${OFFICIAL}b/${VERSION}/sign?ch=post&lang=en`);
	});

	it('opens nothing for other bytes, and says why', async () => {
		const { deps } = browser(Buffer.from('<!doctype html><p>not the page</p>'));
		const checked = await checkPage(OFFICIAL, deps);
		expect(checked.launch).toBeNull();
		expect(checked.line.opens).toBe(false);
		expect(checked.line.key).toBe('componentsUi.signing.integrity.mismatch');
	});
});

describe('an approval on the page', () => {
	it("hands the page the SafeTx and the core's hash, and takes back only the member's approval", async () => {
		const { account, keys, answer } = await member();
		const hash = safeTxHash(CHAIN, MULTISIG, TX);
		const b = browser();
		const pending = approveOnPage(
			{
				member: account,
				page: OFFICIAL,
				chainId: CHAIN,
				multisig: MULTISIG,
				multisigName: 'Team fund',
				owners: [account.address, MULTISIG],
				threshold: 2,
				tx: TX,
				safeTxHash: hash,
				keys
			},
			b.deps
		);
		await until(() => pageHandoff.current?.phase === 'ready');
		expect(pageHandoff.current?.official).toBe(true);
		pageHandoff.current?.open();
		expect(b.deps.open).toHaveBeenCalledWith(`${OFFICIAL}b/${VERSION}/sign?ch=post&lang=en`);
		expect(pageHandoff.current?.phase).toBe('waiting');
		// A message from anywhere but the page's own window is not heard.
		b.from({ vela: 'ready' }, 'https://evil.example');
		b.from({ vela: 'ready' }, 'https://sign.getvela.app', {});
		expect(b.sent).toHaveLength(0);
		b.from({ vela: 'ready' });
		expect(b.sent).toHaveLength(1);
		const intent = b.sent[0].message as {
			vela: string;
			id: string;
			intent: { method: string; params: { safeTxHash: string; safeTx: SafeTx; member: string }[] };
			context: {
				multisig: { name: string; owners: string[]; threshold: number };
				account: string;
			};
		};
		expect(b.sent[0].origin).toBe('https://sign.getvela.app');
		expect(intent.vela).toBe('intent');
		expect(intent.intent.method).toBe('vela_multisigApprove');
		expect(intent.intent.params[0].safeTxHash).toBe(hash);
		expect(intent.intent.params[0].safeTx).toEqual(TX);
		expect(intent.context.multisig.name).toBe('Team fund');
		// r10 page: the members and approvals now, for a member change's
		// before → after (the page marks them as the app's word).
		expect(intent.context.multisig.owners).toEqual([account.address, MULTISIG]);
		expect(intent.context.multisig.threshold).toBe(2);
		b.from({
			vela: 'result',
			id: intent.id,
			result: await answer(approvalChallenge(CHAIN, account.address, hash))
		});
		const approval = await pending;
		expect(approval.owner.toLowerCase()).toBe(account.address.toLowerCase());
		expect(verifyApproval(CHAIN, hash, approval)).toBe(true);
		expect(pageHandoff.current).toBeNull();
	});

	it('refuses an answer over anything but the approval bytes', async () => {
		const { account, keys, answer } = await member();
		const hash = safeTxHash(CHAIN, MULTISIG, TX);
		const b = browser();
		const pending = approveOnPage(
			{
				member: account,
				page: OFFICIAL,
				chainId: CHAIN,
				multisig: MULTISIG,
				tx: TX,
				safeTxHash: hash,
				keys
			},
			b.deps
		);
		await until(() => pageHandoff.current?.phase === 'ready');
		pageHandoff.current?.open();
		b.from({ vela: 'ready' });
		const id = (b.sent[0].message as { id: string }).id;
		// The bare safeTxHash: what a page that skipped the member's SafeMessage would sign.
		b.from({ vela: 'result', id, result: await answer(hash) });
		await expect(pending).rejects.toMatchObject({ failure: 'unverified' });
	});

	it("is not opened when the page's check fails, and says why", async () => {
		const { account, keys } = await member();
		const b = browser(Buffer.from('tampered'));
		const pending = approveOnPage(
			{
				member: account,
				page: OFFICIAL,
				chainId: CHAIN,
				multisig: MULTISIG,
				tx: TX,
				safeTxHash: safeTxHash(CHAIN, MULTISIG, TX),
				keys
			},
			b.deps
		);
		await until(() => pageHandoff.current?.phase === 'refused');
		expect(pageHandoff.current?.line?.opens).toBe(false);
		pageHandoff.current?.cancel();
		const error = await pending.catch((e: unknown) => e);
		expect(error).toBeInstanceOf(PageApprovalError);
		expect((error as PageApprovalError).failure).toBe('not_opened');
		expect(b.deps.open).not.toHaveBeenCalled();
	});

	it('ends when the page refuses, or its window is closed', async () => {
		const { account, keys } = await member();
		const hash = safeTxHash(CHAIN, MULTISIG, TX);
		const input = {
			member: account,
			page: OFFICIAL,
			chainId: CHAIN,
			multisig: MULTISIG,
			tx: TX,
			safeTxHash: hash,
			keys
		};

		const refused = browser();
		const one = approveOnPage(input, refused.deps);
		await until(() => pageHandoff.current?.phase === 'ready');
		pageHandoff.current?.open();
		refused.from({ vela: 'ready' });
		refused.from({
			vela: 'error',
			id: (refused.sent[0].message as { id: string }).id,
			code: 'refused'
		});
		await expect(one).rejects.toMatchObject({ failure: 'declined' });

		const closed = browser();
		const two = approveOnPage(input, closed.deps);
		await until(() => pageHandoff.current?.phase === 'ready');
		pageHandoff.current?.open();
		closed.win.closed = true;
		await expect(two).rejects.toMatchObject({ failure: 'closed' });
	});

	it('is asked once per transaction: the same approval is not asked for again', async () => {
		const { account, keys, answer } = await member();
		const hash = safeTxHash(CHAIN, MULTISIG, TX);
		const input = {
			member: account,
			page: OFFICIAL,
			chainId: CHAIN,
			multisig: MULTISIG,
			tx: TX,
			safeTxHash: hash,
			keys
		};
		const b = browser();
		const first = approveOnPage(input, b.deps);
		await until(() => pageHandoff.current?.phase === 'ready');
		pageHandoff.current?.open();
		b.from({ vela: 'ready' });
		b.from({
			vela: 'result',
			id: (b.sent[0].message as { id: string }).id,
			result: await answer(approvalChallenge(CHAIN, account.address, hash))
		});
		const approval = await first;
		const again = browser();
		await expect(approveOnPage(input, again.deps)).resolves.toEqual(approval);
		expect(again.deps.open).not.toHaveBeenCalled();
	});
});
