/**
 * The two Clear Signer channels as SESSIONS (spec 075 §1.5, relay.md).
 *
 * 071 pinned the same-device transport for one request; what is new here is
 * that a flow's requests ride one page visit — create then member proof, sign
 * in then recover twice — and that there is a second way to reach a page at
 * all. Both are checked against a stand-in page: the same-device one through
 * the fake browser 071 already had, the relay one through a stand-in room
 * that runs the page's own half of the session.
 *
 * The relay's bytes themselves are pinned elsewhere, against the core's
 * vectors (`relay/secure-session.test.ts`). What is pinned here is the
 * conversation: nothing is sent before the person confirms the code, the
 * relay only ever carries two hellos in the clear, a page that drops out is
 * waited for, and every way a room can end reaches the caller as an answer
 * rather than a hang.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fakeBrowser } from './__fixtures__/clear-signer-page';
import { fakeRelay } from './__fixtures__/relay-page';
import {
	openPostMessageChannel,
	openRelayChannel,
	signerPageUrl,
	type ClearSignerReply
} from './clear-signer-channel';

const SIGNER = 'https://sign.getvela.app/';
const SIGN_IN = { intent: { method: 'vela_signIn', params: [{}], origin: '' }, context: {} };
const PROOF = {
	intent: { method: 'vela_proof', params: [{ purpose: 'verify' }], origin: '' },
	context: {}
};

describe('the page address', () => {
	it('is built as the core builds every channel’s: sign.html, then the channel', () => {
		expect(signerPageUrl('https://sign.getvela.app/')).toBe(
			'https://sign.getvela.app/sign.html?ch=post'
		);
		expect(signerPageUrl('https://me.example/signer', 'relay')).toBe(
			'https://me.example/signer/sign.html?ch=relay'
		);
	});
});

describe('this device: one page visit, several requests', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('carries a create and then its member proof, on one window, and says goodbye', async () => {
		const page = fakeBrowser();
		const channel = openPostMessageChannel({ signerUrl: SIGNER, host: page.host });
		const first = channel.ask(SIGN_IN);
		page.say({ vela: 'ready', v: 1 });
		const firstId = page.id();
		page.say({ vela: 'result', id: firstId, registration: { credentialId: 'aa' } });
		expect(await first).toMatchObject({ kind: 'answer' });

		// The page is still open and waiting: the next request goes straight to
		// it, with its own id, and no second window is opened.
		const second = channel.ask(PROOF);
		await Promise.resolve();
		const intents = page.posted.filter((entry) => entry.message.vela === 'intent');
		expect(intents).toHaveLength(2);
		const secondId = intents[1].message.id as string;
		expect(secondId).not.toBe(firstId);
		expect(page.opened).toHaveLength(1);
		page.say({ vela: 'result', id: secondId, assertion: { credentialId: 'aa' } });
		expect(await second).toMatchObject({ kind: 'answer' });

		channel.end();
		expect(page.posted.at(-1)?.message).toMatchObject({ vela: 'bye' });
		expect(page.popup.closed).toBe(true);
		expect(channel.ended).toBe(true);
		expect(await channel.ask(SIGN_IN)).toMatchObject({ kind: 'refused', code: 'declined' });
	});

	it('an answer to another request, or from another window, is not this one’s', async () => {
		const page = fakeBrowser();
		const channel = openPostMessageChannel({ signerUrl: SIGNER, host: page.host });
		let settled: ClearSignerReply | null = null;
		void channel.ask(SIGN_IN).then((reply) => (settled = reply));
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'result', id: 'someone-else', assertion: {} });
		page.say({ vela: 'result', id: page.id(), assertion: {} }, { origin: 'https://evil.example' });
		await Promise.resolve();
		expect(settled).toBeNull();
	});

	it('the page’s own goodbye ends the session, and what was in flight is declined', async () => {
		const page = fakeBrowser();
		const channel = openPostMessageChannel({ signerUrl: SIGNER, host: page.host });
		const asked = channel.ask(SIGN_IN);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'bye', v: 1, reason: 'idle' });
		expect(await asked).toMatchObject({ kind: 'refused', code: 'declined' });
		expect(channel.ended).toBe(true);
	});

	it('the page’s refusal is its own, and a person’s rejection is not one', async () => {
		const page = fakeBrowser();
		const channel = openPostMessageChannel({ signerUrl: SIGNER, host: page.host });
		const asked = channel.ask(SIGN_IN);
		page.say({ vela: 'ready', v: 1 });
		page.say({ vela: 'error', id: page.id(), code: 'refused' });
		expect(await asked).toEqual({ kind: 'refused', code: 'refused', detail: 'refused' });

		const second = channel.ask(PROOF);
		const id = page.posted.filter((entry) => entry.message.vela === 'intent')[1].message
			.id as string;
		page.say({ vela: 'error', id, code: 'user_rejected' });
		expect(await second).toMatchObject({ kind: 'refused', code: 'declined' });
		// A refusal does not end the session: the flow may still ask again.
		expect(channel.ended).toBe(false);
	});

	it('five minutes with no answer times out that request, not the session', async () => {
		const page = fakeBrowser();
		const channel = openPostMessageChannel({ signerUrl: SIGNER, host: page.host });
		const asked = channel.ask(SIGN_IN);
		page.say({ vela: 'ready', v: 1 });
		vi.advanceTimersByTime(5 * 60_000);
		expect(await asked).toMatchObject({ kind: 'refused', code: 'timeout' });
		expect(channel.ended).toBe(false);
	});
});

describe('another device: the relay', () => {
	/** A channel paired with the stand-in page, code in hand. */
	async function paired() {
		const room = fakeRelay();
		let connected = '';
		const codes: (string | null)[] = [];
		const links: string[] = [];
		const channel = await openRelayChannel({
			relayUrl: 'wss://relay.getvela.app',
			signerUrl: SIGNER,
			onLink: (link) => links.push(link),
			onCode: (code) => codes.push(code),
			sockets: (url) => {
				connected = url;
				return room.socket;
			}
		});
		await room.join();
		await settleAll();
		return { room, channel, codes, links, connected };
	}

	/**
	 * Let both sides' promise chains run out. WebCrypto resolves off the
	 * microtask queue, so a handful of real turns is what it takes.
	 */
	const settleAll = async () => {
		for (let i = 0; i < 8; i++) await new Promise((resolve) => setTimeout(resolve, 0));
	};

	it('draws a pairing link for the page, and joins the room as the requester', async () => {
		const { channel, links, connected, room, codes } = await paired();
		expect(connected).toMatch(
			/^wss:\/\/relay\.getvela\.app\/v1\/rooms\/[A-Za-z0-9_-]{22}\?role=requester$/
		);
		expect(links).toHaveLength(1);
		const link = new URL(links[0]);
		expect(link.origin + link.pathname).toBe('https://sign.getvela.app/sign.html');
		expect(link.searchParams.get('ch')).toBe('relay');
		const fragment = new URLSearchParams(link.hash.slice(1));
		expect(fragment.get('relay')).toBe('wss://relay.getvela.app');
		expect(fragment.get('room')).toMatch(/^[A-Za-z0-9_-]{22}$/);
		expect(fragment.get('rk')).toMatch(/^[A-Za-z0-9_-]{22}$/);
		expect(fragment.get('v')).toBe('1');
		// Both screens derived the same six digits.
		expect(codes.at(-1)).toMatch(/^\d{6}$/);
		expect(codes.at(-1)).toBe(room.code());
		expect(channel.link).toBe(links[0]);
	});

	it('sends nothing until the person says the codes match', async () => {
		const { channel, room } = await paired();
		const asked = channel.ask(SIGN_IN);
		await settleAll();
		expect(room.received).toHaveLength(0);

		channel.confirm();
		await settleAll();
		expect(room.received).toHaveLength(1);
		expect(room.received[0]).toMatchObject({ t: 'intent', v: 1, n: 1, intent: SIGN_IN.intent });

		const id = room.received[0].id as string;
		await room.answer(id, {
			assertion: { credentialId: 'aa' },
			origin: 'https://sign.getvela.app'
		});
		await settleAll();
		const reply = await asked;
		expect(reply.kind).toBe('answer');
		if (reply.kind !== 'answer') return;
		expect(reply.payload).toMatchObject({ assertion: { credentialId: 'aa' } });
	});

	it('carries a second request in the same room, with `n` going up, and ends with bye', async () => {
		const { channel, room } = await paired();
		const first = channel.ask(SIGN_IN);
		channel.confirm();
		await settleAll();
		await room.answer(room.received[0].id as string, { assertion: {} });
		await settleAll();
		await first;

		const second = channel.ask(PROOF);
		await settleAll();
		expect(room.received).toHaveLength(2);
		expect(room.received[1].n as number).toBeGreaterThan(room.received[0].n as number);
		await room.answer(room.received[1].id as string, { assertion: {} });
		await settleAll();
		expect((await second).kind).toBe('answer');

		channel.end();
		await settleAll();
		expect(room.received.at(-1)).toMatchObject({ t: 'bye', reason: 'done' });
		expect(channel.ended).toBe(true);
	});

	it('lets the relay see two hellos and nothing else', async () => {
		const { channel, room } = await paired();
		void channel.ask(SIGN_IN);
		channel.confirm();
		await settleAll();
		const clear = room.frames.filter((frame) => typeof frame === 'string') as string[];
		// Only this wallet's own hello crossed in the clear from this side.
		expect(clear).toHaveLength(1);
		expect(JSON.parse(clear[0])).toMatchObject({
			t: 'hello',
			role: 'requester',
			app: 'vela-web/1'
		});
		const sealed = room.frames.filter((frame) => frame instanceof Uint8Array) as Uint8Array[];
		expect(sealed).toHaveLength(1);
		const bytes = new TextDecoder().decode(sealed[0]);
		expect(bytes).not.toContain('vela_signIn');
		expect(bytes).not.toContain('intent');
	});

	it('a page that drops out is waited for, and its request is put again after a new code', async () => {
		const { channel, room, codes } = await paired();
		const asked = channel.ask(SIGN_IN);
		channel.confirm();
		await settleAll();
		const id = room.received[0].id as string;

		room.leave();
		await settleAll();
		// No code on screen while nobody is there to compare it with.
		expect(codes.at(-1)).toBeNull();

		await room.join();
		await settleAll();
		const fresh = codes.at(-1);
		expect(fresh).toMatch(/^\d{6}$/);
		expect(fresh).toBe(room.code());
		// The request is still waiting, and goes again once the NEW code is
		// confirmed — the old session could never answer it.
		expect(room.received).toHaveLength(1);
		channel.confirm();
		await settleAll();
		expect(room.received).toHaveLength(2);
		expect(room.received[1].id).toBe(id);
		await room.answer(id, { assertion: {} });
		await settleAll();
		expect((await asked).kind).toBe('answer');
	});

	it('a room that closes answers the person instead of leaving them on a code', async () => {
		const { channel, room } = await paired();
		const asked = channel.ask(SIGN_IN);
		channel.confirm();
		await settleAll();
		room.shut(4408);
		expect(await asked).toMatchObject({ kind: 'refused', code: 'relay_down' });
		expect(channel.ended).toBe(true);
		expect(await channel.ask(PROOF)).toMatchObject({ kind: 'refused', code: 'relay_down' });
	});

	it('cancelling leaves the room at once', async () => {
		const { channel, room } = await paired();
		const asked = channel.ask(SIGN_IN);
		channel.confirm();
		await settleAll();
		channel.cancel();
		expect(await asked).toMatchObject({ kind: 'refused', code: 'declined' });
		expect(room.received.some((message) => message.t === 'bye')).toBe(false);
		expect(channel.ended).toBe(true);
	});
});
