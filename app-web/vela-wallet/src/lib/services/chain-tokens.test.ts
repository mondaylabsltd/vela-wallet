/**
 * PR 2 polish: the chain registry document has three outcomes, not two.
 *
 * `fetchChainTokens` answered `null` for a 404 and for a network failure
 * alike, so a chain with no native coin of its own (Tempo) whose document
 * could not be fetched read as a chain that answered and holds nothing. The
 * read is now `doc` (2xx, parsed), `absent` (HTTP 404 — the server answered
 * that there is none) or `unread` (no definitive answer: a network failure, a
 * timeout, 5xx, 429 or any other non-2xx, an unreadable body). `doc` and
 * `absent` are kept 30 min; `unread` never is.
 *
 * Only `fetch` is stubbed: the classification and the cache are the real code.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fetchChainTokens, readChainTokens } from './chain-tokens';
import { NET_TIMEOUTS } from './net';

/** Each test asks about its own chain: the cache is the module's, and lives across tests. */
let nextChain = 900_000;
const freshChain = () => (nextChain += 1);

const DOC = {
	nativeCurrency: { name: 'Tempo USD', symbol: 'USD', decimals: 18 },
	stables: [{ symbol: 'USDC', type: 'native', contract: '0x' + '20'.repeat(20) }],
	wrappedNativeToken: null
};

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
	fetchMock = vi.fn();
	vi.stubGlobal('fetch', fetchMock);
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

const json = (body: unknown, status = 200) =>
	new Response(JSON.stringify(body), {
		status,
		headers: { 'content-type': 'application/json' }
	});

describe('the registry document’s three outcomes', () => {
	it('2xx with a document: `doc`, parsed — and kept, the next ask is not a fetch', async () => {
		const chain = freshChain();
		fetchMock.mockResolvedValue(json(DOC));
		const read = await readChainTokens(chain);
		expect(read.kind).toBe('doc');
		if (read.kind !== 'doc') return;
		expect(read.data.chainId).toBe(chain);
		expect(read.data.stables).toEqual(DOC.stables);
		expect(String(fetchMock.mock.calls[0][0])).toContain(`/chains/eip155-${chain}.json`);
		await readChainTokens(chain);
		expect(fetchMock).toHaveBeenCalledTimes(1);
	});

	it('404: `absent` — the server answered there is none — and kept', async () => {
		const chain = freshChain();
		fetchMock.mockResolvedValue(new Response('not found', { status: 404 }));
		expect(await readChainTokens(chain)).toEqual({ kind: 'absent' });
		expect(await readChainTokens(chain)).toEqual({ kind: 'absent' });
		expect(fetchMock).toHaveBeenCalledTimes(1);
	});

	it.each([500, 502, 503, 429, 403])(
		'HTTP %i: `unread`, never kept — the next ask reads again',
		async (status) => {
			const chain = freshChain();
			fetchMock.mockResolvedValueOnce(new Response('busy', { status }));
			expect((await readChainTokens(chain)).kind).toBe('unread');
			fetchMock.mockResolvedValueOnce(json(DOC));
			expect((await readChainTokens(chain)).kind).toBe('doc');
			expect(fetchMock).toHaveBeenCalledTimes(2);
		}
	);

	it('a network failure: `unread`, never kept', async () => {
		const chain = freshChain();
		fetchMock.mockRejectedValueOnce(new TypeError('Failed to fetch'));
		expect((await readChainTokens(chain)).kind).toBe('unread');
		fetchMock.mockResolvedValueOnce(new Response('', { status: 404 }));
		expect((await readChainTokens(chain)).kind).toBe('absent');
	});

	it('a timeout: `unread`', async () => {
		vi.useFakeTimers();
		const chain = freshChain();
		// A server that holds the connection open until the request is aborted.
		fetchMock.mockImplementation(
			(_url: string, init: RequestInit) =>
				new Promise((_resolve, reject) => {
					init.signal?.addEventListener('abort', () =>
						reject(new DOMException('aborted', 'AbortError'))
					);
				})
		);
		const read = readChainTokens(chain);
		await vi.advanceTimersByTimeAsync(NET_TIMEOUTS.ethereumData);
		expect((await read).kind).toBe('unread');
	});

	it('a 200 whose body is not the document: `unread`, never kept', async () => {
		const chain = freshChain();
		fetchMock.mockResolvedValueOnce(new Response('<html>captive portal</html>', { status: 200 }));
		expect((await readChainTokens(chain)).kind).toBe('unread');
		fetchMock.mockResolvedValueOnce(json(null));
		expect((await readChainTokens(chain)).kind).toBe('unread');
		fetchMock.mockResolvedValueOnce(json(DOC));
		expect((await readChainTokens(chain)).kind).toBe('doc');
	});

	it('`fetchChainTokens` (token trust) still reads both misses as "no facts"', async () => {
		const absent = freshChain();
		fetchMock.mockResolvedValueOnce(new Response('', { status: 404 }));
		expect(await fetchChainTokens(absent)).toBeNull();
		const unread = freshChain();
		fetchMock.mockResolvedValueOnce(new Response('', { status: 500 }));
		expect(await fetchChainTokens(unread)).toBeNull();
		const doc = freshChain();
		fetchMock.mockResolvedValueOnce(json(DOC));
		expect((await fetchChainTokens(doc))?.stables).toEqual(DOC.stables);
	});
});
