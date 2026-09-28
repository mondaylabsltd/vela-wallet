/**
 * How long a logo that did not load stays failed (spec 082 RE10, W20): the
 * core's `remote_mark` rule, not a session-long set — a miss during a network
 * blip used to keep every token a letter until the wallet was reopened.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it } from 'vitest';
import { markMissTtlMs } from '$lib/core/kernels';
import { hasFailed, markFailed, missTtlMs, resetLogoCacheForTests } from './logo-cache';

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
