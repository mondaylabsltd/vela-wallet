/**
 * Vela Connect's popup channel (spec 109): who it takes requests from, and
 * where its answers go — the rules that make the requester the browser's fact.
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { CONNECT_ORIGIN_KEY, PopupChannel, RedirectChannel, type ChannelRequest } from './channel';
import { base64UrlDecodeJson, base64UrlEncodeJson } from './protocol';

class MemoryStorage {
	private items = new Map<string, string>();
	get length() {
		return this.items.size;
	}
	key(i: number) {
		return [...this.items.keys()][i] ?? null;
	}
	getItem(k: string) {
		return this.items.get(k) ?? null;
	}
	setItem(k: string, v: string) {
		this.items.set(k, v);
	}
	removeItem(k: string) {
		this.items.delete(k);
	}
	clear() {
		this.items.clear();
	}
}

interface Posted {
	message: unknown;
	target: string;
}

function fakeWindow() {
	const posted: Posted[] = [];
	const opener = {
		closed: false,
		postMessage(message: unknown, target: string) {
			posted.push({ message, target });
		}
	};
	let listener: ((event: MessageEvent) => void) | null = null;
	const win = {
		opener: opener as unknown,
		addEventListener(_type: string, fn: (event: MessageEvent) => void) {
			listener = fn;
		},
		removeEventListener() {
			listener = null;
		}
	};
	const deliver = (data: unknown, origin: string, source: unknown = opener) =>
		listener?.({ data, origin, source } as MessageEvent);
	return { win: win as unknown as Window, opener, posted, deliver };
}

const SITE = 'https://sub3.dev';
const request = (id: string, method = 'eth_requestAccounts') => ({
	vela: 1,
	type: 'request',
	id,
	method,
	params: []
});

beforeEach(() => {
	(globalThis as { sessionStorage?: unknown }).sessionStorage = new MemoryStorage();
});

describe('PopupChannel', () => {
	it('says ready to its opener the moment it listens', () => {
		const { win, posted } = fakeWindow();
		new PopupChannel(win).start(() => {});
		expect(posted).toEqual([{ message: { vela: 1, type: 'ready' }, target: '*' }]);
	});

	it('takes a request from its opener, pinned to the origin the browser reported', () => {
		const { win, deliver } = fakeWindow();
		const got: ChannelRequest[] = [];
		const channel = new PopupChannel(win);
		channel.start((r) => got.push(r));
		deliver(request('r1'), SITE);
		expect(got).toEqual([
			{ id: 'r1', method: 'eth_requestAccounts', params: [], rid: 'c0:r1', origin: SITE }
		]);
		expect(channel.origin).toBe(SITE);
		expect(sessionStorage.getItem(CONNECT_ORIGIN_KEY)).toBe(SITE);
	});

	it('never takes a message that is not from its opener', () => {
		const { win, deliver } = fakeWindow();
		const got: ChannelRequest[] = [];
		new PopupChannel(win).start((r) => got.push(r));
		deliver(request('r1'), SITE, { postMessage() {} });
		deliver(request('r2'), SITE, null);
		expect(got).toEqual([]);
	});

	it('once pinned, never serves another origin — not even after the window navigates', () => {
		const { win, deliver } = fakeWindow();
		const got: ChannelRequest[] = [];
		new PopupChannel(win).start((r) => got.push(r));
		deliver(request('r1'), SITE);
		deliver(request('r2'), 'https://evil.example');
		expect(got.map((r) => r.id)).toEqual(['r1']);

		// The window came back from creating a wallet: a new document, the same pin.
		const again = fakeWindow();
		const later: ChannelRequest[] = [];
		const channel = new PopupChannel(again.win);
		expect(channel.origin).toBe(SITE);
		channel.start((r) => later.push(r));
		again.deliver(request('r3'), 'https://evil.example');
		again.deliver(request('r4'), SITE);
		expect(later.map((r) => r.id)).toEqual(['r4']);
	});

	it('refuses an opaque origin (a sandboxed frame, a file)', () => {
		const { win, deliver } = fakeWindow();
		const got: ChannelRequest[] = [];
		const channel = new PopupChannel(win);
		channel.start((r) => got.push(r));
		deliver(request('r1'), 'null');
		expect(got).toEqual([]);
		expect(channel.origin).toBeNull();
	});

	it('takes an id once: the same id again is the site sending it twice', () => {
		const { win, deliver } = fakeWindow();
		const got: ChannelRequest[] = [];
		new PopupChannel(win).start((r) => got.push(r));
		deliver(request('r1'), SITE);
		deliver(request('r1'), SITE);
		expect(got).toHaveLength(1);
	});

	it('answers to the pinned origin only, under the site’s own id', async () => {
		const { win, deliver, posted } = fakeWindow();
		const channel = new PopupChannel(win);
		let rid = '';
		channel.start((r) => (rid = r.rid));
		deliver(request('abc:1'), SITE);
		expect(await channel.answer(rid, { result: ['0xab'] })).toBe(true);
		expect(posted.at(-1)).toEqual({
			message: { vela: 1, type: 'response', id: 'abc:1', result: ['0xab'] },
			target: SITE
		});
	});

	it('answers a request it cannot take with -32600, to that origin', () => {
		const { win, deliver, posted } = fakeWindow();
		const got: ChannelRequest[] = [];
		new PopupChannel(win).start((r) => got.push(r));
		deliver({ vela: 9, type: 'request', id: 'r1', method: 'eth_accounts' }, SITE);
		expect(got).toEqual([]);
		expect(posted.at(-1)).toMatchObject({
			message: { type: 'response', id: 'r1', error: { code: -32600 } },
			target: SITE
		});
	});

	it('knows when its site is gone, and does not answer into the void', async () => {
		const { win, deliver, opener } = fakeWindow();
		const channel = new PopupChannel(win);
		let rid = '';
		channel.start((r) => (rid = r.rid));
		deliver(request('r1'), SITE);
		opener.closed = true;
		expect(channel.alive()).toBe(false);
		expect(await channel.answer(rid, { result: null })).toBe(false);
	});
});

describe('RedirectChannel', () => {
	const STATE = 'abcdefghijklmnop_1234';
	const hash = `#${new URLSearchParams({
		request: base64UrlEncodeJson({ method: 'eth_requestAccounts', params: [] }),
		return: 'https://sub3.dev/login?x=1',
		state: STATE
	})}`;

	it('takes the one request, from the return address’s origin, unverified', () => {
		const opened = RedirectChannel.open(hash, () => {});
		expect(opened).toBeInstanceOf(RedirectChannel);
		const channel = opened as RedirectChannel;
		expect(channel.verified).toBe(false);
		expect(channel.origin).toBe(SITE);
		const got: ChannelRequest[] = [];
		channel.start((r) => got.push(r));
		expect(got).toEqual([
			{
				id: 'redirect',
				method: 'eth_requestAccounts',
				params: [],
				rid: 'r0:redirect',
				origin: SITE
			}
		]);
	});

	it('survives the trip through creating a wallet (kept in this window)', () => {
		RedirectChannel.open(hash, () => {});
		const again = RedirectChannel.open('', () => {});
		expect(again).toBeInstanceOf(RedirectChannel);
		expect((again as RedirectChannel).origin).toBe(SITE);
	});

	it('answers by going back, once, with the state — and forgets the request', async () => {
		const went: string[] = [];
		const channel = RedirectChannel.open(hash, (url) => went.push(url)) as RedirectChannel;
		expect(await channel.answer('r0:redirect', { result: ['0xab'] })).toBe(true);
		expect(await channel.answer('r0:redirect', { result: ['0xcd'] })).toBe(false);
		expect(went).toHaveLength(1);
		const back = new URL(went[0]);
		expect(back.origin + back.pathname + back.search).toBe('https://sub3.dev/login?x=1');
		expect(base64UrlDecodeJson(back.hash.slice('#vela='.length))).toEqual({
			vela: 1,
			state: STATE,
			result: ['0xab']
		});
		expect(RedirectChannel.open('', () => {})).toBeNull();
	});

	it('is invalid for a link it cannot read, and none for no link', () => {
		expect(RedirectChannel.open('#request=x&return=http%3A%2F%2Fevil.example&state=' + STATE)).toBe(
			'invalid'
		);
		expect(RedirectChannel.open('')).toBeNull();
	});
});
