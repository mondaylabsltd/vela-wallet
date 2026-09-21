/**
 * The Clear Signer's web channel (spec 071, contract §4), against a stand-in
 * popup and the REAL verdict: the answers below are signed by a P-256 key
 * standing in for the passkey, exactly as the core's own suite builds them
 * (`rust/crates/vela-core/tests/clear_signer.rs`), and judged by the core over
 * wasm. What is pinned here is the transport's half — who may speak (the
 * signer's exact origin AND the window this wallet opened), which answer is
 * this request's, and that a closed page, a timeout and a cancel all end the
 * wait instead of leaving a sheet spinning.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ClearSignerKey, ClearSignerRequest } from '$lib/core/kernels';
import {
	answer as signed,
	fakeBrowser as browser,
	passkey,
	SIGNER_ORIGIN as ORIGIN
} from './__fixtures__/clear-signer-page';
import { noticeOf, openClearSigner, signerPageUrl, type ClearSignerOutcome } from './clear-signer';

const SIGNER = 'https://sign.getvela.app/';
const REQUEST: ClearSignerRequest = {
	intent: { method: 'personal_sign', params: ['0x68656c6c6f'], origin: 'https://app.example' },
	context: { chainId: 1, account: '0x88cCA0EeDbF2C4426110bbFc998F048689266894' }
};
const DIGEST = new Uint8Array(32).fill(0xab);

const OWNER = passkey(7, [0x11, 0x22, 0x33]);
const STRANGER = passkey(9, [0x99]);
const answer = (signer: typeof OWNER, digest: Uint8Array, flags = 0x05) =>
	signed(signer, digest, { flags });

function ceremony(page: ReturnType<typeof browser>, keys: ClearSignerKey[] = [OWNER.key]) {
	const opened = openClearSigner({
		signerUrl: SIGNER,
		request: REQUEST,
		digest: DIGEST,
		keys,
		host: page.host
	});
	let outcome: ClearSignerOutcome | null = null;
	void opened.outcome.then((value) => (outcome = value));
	return { ...opened, settled: () => outcome };
}

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
});

describe('the page address', () => {
	it('is built as the core builds every channel’s: sign.html, then the channel', () => {
		expect(signerPageUrl('https://sign.getvela.app/')).toBe(
			'https://sign.getvela.app/sign.html?ch=post'
		);
		expect(signerPageUrl('https://me.example/signer')).toBe(
			'https://me.example/signer/sign.html?ch=post'
		);
		expect(signerPageUrl('http://127.0.0.1:8137/sign.html')).toBe(
			'http://127.0.0.1:8137/sign.html?ch=post'
		);
	});
});

describe('the conversation', () => {
	it('opens the page, and hands the intent over only when it says ready — to its origin alone', () => {
		const page = browser();
		ceremony(page);
		expect(page.opened).toEqual(['https://sign.getvela.app/sign.html?ch=post']);
		expect(page.posted).toEqual([]);
		page.say({ vela: 'ready', v: 1 });
		expect(page.posted).toHaveLength(1);
		expect(page.posted[0].targetOrigin).toBe(ORIGIN);
		expect(page.posted[0].message).toMatchObject({
			vela: 'intent',
			intent: REQUEST.intent,
			context: REQUEST.context
		});
		expect(typeof page.id()).toBe('string');
	});

	it('a signature by this wallet over this digest comes back as a passkey’s would', async () => {
		const page = browser();
		const run = ceremony(page, [STRANGER.key, OWNER.key]);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'result', id: page.id(), result: answer(OWNER, DIGEST) });
		const outcome = await run.outcome;
		expect(outcome.kind).toBe('accepted');
		if (outcome.kind !== 'accepted') return;
		expect(outcome.assertion.credentialId).toBe('112233');
		// DER, bare hex — what `fromHex` and the Safe envelope take downstream.
		expect(outcome.assertion.signatureHex.startsWith('30')).toBe(true);
		expect(outcome.assertion.clientDataJSONHex).toBe(answer(OWNER, DIGEST).clientDataJSON.slice(2));
		// The page has nothing left to do: the wallet closes it and stops listening.
		expect(page.popup.closed).toBe(true);
		expect(page.listeners.size).toBe(0);
	});

	it('a page that reloaded and says ready again is given the intent again', () => {
		const page = browser();
		ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'ready', v: 1 });
		expect(page.posted.filter((entry) => entry.message.vela === 'intent')).toHaveLength(2);
		expect(page.posted[0].message.id).toBe(page.posted[1].message.id);
	});
});

describe('who may speak', () => {
	it('a ready from another origin gets nothing', () => {
		const page = browser();
		ceremony(page);
		page.say({ vela: 'ready', v: 1 }, { origin: 'https://evil.example' });
		expect(page.posted).toEqual([]);
	});

	it('a result from another origin, or from another window of the same one, is not an answer', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		const result = { vela: 'result', id: page.id(), result: answer(OWNER, DIGEST) };
		page.say(result, { origin: 'https://evil.example' });
		page.say(result, { source: { postMessage() {} } });
		page.say(result, { source: null });
		await Promise.resolve();
		expect(run.settled()).toBeNull();
		// The real one still lands.
		page.say(result);
		expect((await run.outcome).kind).toBe('accepted');
	});

	it('an answer to another request is not this one’s', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'result', id: 'someone-else', result: answer(OWNER, DIGEST) });
		page.say({ vela: 'error', id: 'someone-else', code: 'user_rejected' });
		await Promise.resolve();
		expect(run.settled()).toBeNull();
	});
});

describe('what the core refuses', () => {
	it('a key that is not this wallet’s', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'result', id: page.id(), result: answer(STRANGER, DIGEST) });
		const outcome = await run.outcome;
		expect(outcome).toMatchObject({ kind: 'refused', code: 'foreign_key' });
		expect(noticeOf((outcome as { code: string }).code)).toBe('mismatch');
	});

	it('a signature over anything but the digest this wallet computed', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		const other = new Uint8Array(32).fill(0xcd);
		page.say({ vela: 'result', id: page.id(), result: answer(OWNER, other) });
		expect(await run.outcome).toMatchObject({ kind: 'refused', code: 'wrong_challenge' });
	});

	it('a user who was not verified', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'result', id: page.id(), result: answer(OWNER, DIGEST, 0x01) });
		expect(await run.outcome).toMatchObject({ kind: 'refused', code: 'not_verified' });
	});

	it('an answer that is not an answer at all', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'result', id: page.id() });
		expect(await run.outcome).toMatchObject({ kind: 'refused', code: 'malformed' });
	});
});

describe('how a wait ends without a signature', () => {
	it('the page said the person declined', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'error', id: page.id(), code: 'user_rejected' });
		const outcome = await run.outcome;
		expect(outcome).toMatchObject({ kind: 'refused', code: 'declined' });
		expect(noticeOf('declined')).toBe('closed');
	});

	it('the page’s own rules refused — distinct from the person', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'error', id: page.id(), code: 'refused' });
		expect(await run.outcome).toEqual({ kind: 'refused', code: 'refused', detail: 'refused' });
		expect(noticeOf('refused')).toBe('refused');
	});

	it('a page closed without answering is declined — never a spinner', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		page.popup.closed = true;
		vi.advanceTimersByTime(500);
		await Promise.resolve();
		// One beat of grace: an answer posted as the page closed may be queued.
		expect(run.settled()).toBeNull();
		vi.advanceTimersByTime(500);
		expect(await run.outcome).toMatchObject({ kind: 'refused', code: 'declined' });
	});

	it('an answer that arrives inside the grace beat still wins', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		page.popup.closed = true;
		vi.advanceTimersByTime(500);
		page.say({ vela: 'result', id: page.id(), result: answer(OWNER, DIGEST) });
		expect((await run.outcome).kind).toBe('accepted');
	});

	it('five minutes without an answer is a timeout', async () => {
		const page = browser();
		const run = ceremony(page);
		page.say({ vela: 'ready', v: 1 });
		vi.advanceTimersByTime(5 * 60_000 - 1);
		await Promise.resolve();
		expect(run.settled()).toBeNull();
		vi.advanceTimersByTime(1);
		expect(await run.outcome).toMatchObject({ kind: 'refused', code: 'timeout' });
		expect(page.popup.closed).toBe(true);
	});

	it('cancel on the waiting sheet is declined, and the page is closed', async () => {
		const page = browser();
		const run = ceremony(page);
		run.cancel();
		expect(await run.outcome).toMatchObject({ kind: 'refused', code: 'declined' });
		expect(page.popup.closed).toBe(true);
	});
});

describe('open the page again', () => {
	it('a blocked popup is not a closed one: the wait goes on, and a tap opens it', async () => {
		const page = browser({ blocked: true });
		const run = ceremony(page);
		vi.advanceTimersByTime(5_000);
		await Promise.resolve();
		expect(run.settled()).toBeNull();
		page.unblock();
		run.reopen();
		expect(page.opened).toHaveLength(2);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'result', id: page.id(), result: answer(OWNER, DIGEST) });
		expect((await run.outcome).kind).toBe('accepted');
	});

	it('an open page is brought back, not opened twice', () => {
		const page = browser();
		const run = ceremony(page);
		run.reopen();
		expect(page.opened).toHaveLength(1);
		expect(page.popup.focused).toBe(1);
	});
});
