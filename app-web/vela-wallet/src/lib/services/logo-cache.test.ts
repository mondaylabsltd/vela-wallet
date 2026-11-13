/**
 * How long a logo that did not load stays failed (spec 082 RE10, W20): the
 * core's `remote_mark` rule, not a session-long set — a miss during a network
 * blip used to keep every token a letter until the wallet was reopened.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { markMissTtlMs } from '$lib/core/kernels';
import { hasFailed, markFailed, missTtlMs, probeMiss, resetLogoCacheForTests } from './logo-cache';

const URL = 'https://data.example/logos/0xabc.png';
const NOW = 1_800_000_000_000;

beforeEach(() => resetLogoCacheForTests());

describe('a logo miss', () => {
	it('lasts what the core says a miss with no status lasts — 60 s', () => {
		expect(missTtlMs()).toBe(markMissTtlMs('unknown'));
		expect(missTtlMs()).toBe(60_000);
	});

	it('is skipped for 60 s, then tried again', () => {
		expect(markFailed(URL, NOW)).toBe(60_000);
		expect(hasFailed(URL, NOW)).toBe(true);
		expect(hasFailed(URL, NOW + 59_999)).toBe(true);
		expect(hasFailed(URL, NOW + 60_000)).toBe(false);
		// Forgotten once it ran out: a second miss starts a new 60 s.
		expect(hasFailed(URL, NOW + 60_001)).toBe(false);
	});

	it('is per URL', () => {
		markFailed(URL, NOW);
		expect(hasFailed(`${URL}?v=2`, NOW)).toBe(false);
	});
});

describe('a logo miss, probed (home-20)', () => {
	function answering(status: number): typeof fetch {
		return vi.fn(
			async () => new Response(null, { status, headers: { 'content-type': 'text/html' } })
		) as unknown as typeof fetch;
	}

	it('a page in place of a logo is no logo for the session', async () => {
		const fetchImpl = answering(200);
		markFailed(URL, NOW);
		expect(await probeMiss(URL, fetchImpl, () => NOW)).toBeNull();
		expect(fetchImpl).toHaveBeenCalledWith(URL, expect.objectContaining({ method: 'HEAD' }));
		// An hour on, still not asked for again.
		expect(hasFailed(URL, NOW + 3_600_000)).toBe(true);
	});

	it('a 404 is no logo for the session too', async () => {
		markFailed(URL, NOW);
		expect(await probeMiss(URL, answering(404), () => NOW)).toBeNull();
		expect(hasFailed(URL, NOW + 3_600_000)).toBe(true);
	});

	it('a server having a bad minute is tried again after it', async () => {
		markFailed(URL, NOW);
		expect(await probeMiss(URL, answering(503), () => NOW)).toBe(60_000);
		expect(hasFailed(URL, NOW + 60_001)).toBe(false);
	});

	it('no answer leaves the 60 s miss, and asks again next time', async () => {
		const fetchImpl = vi.fn(async () => {
			throw new TypeError('Failed to fetch');
		}) as unknown as typeof fetch;
		markFailed(URL, NOW);
		expect(await probeMiss(URL, fetchImpl, () => NOW)).toBe(60_000);
		expect(hasFailed(URL, NOW + 60_001)).toBe(false);
		await probeMiss(URL, fetchImpl, () => NOW);
		expect(fetchImpl).toHaveBeenCalledTimes(2);
	});

	it('is one request per URL, however many rows miss it', async () => {
		const fetchImpl = answering(200);
		const all = await Promise.all([1, 2, 3].map(() => probeMiss(URL, fetchImpl, () => NOW)));
		expect(all).toEqual([null, null, null]);
		await probeMiss(URL, fetchImpl, () => NOW);
		expect(fetchImpl).toHaveBeenCalledOnce();
	});
});
