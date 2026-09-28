/**
 * The network filter reaches the feed's core (spec 082 T211, G62).
 *
 * The device pass filtered the sidebar to Gnosis on a wallet with no Gnosis
 * history: home still said 暂无交易记录 and History 暂无交易, never 该网络暂无
 * 交易记录 / 此网络暂无交易. The core picks those lines (`history_empty_key`,
 * `home_empty_key`, RG5) from ITS chain filter — and the web never told it:
 * the sidebar and the phone's sheet set `chainFilter` and nothing else.
 *
 * Two halves: the real feed, over the real core, turns a filter into the
 * filtered keys; and the wallet page tells it, from an effect over the
 * filter (read from its source, the way `one-surface.test.ts` pins wiring).
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { feed } from '$lib/wallet/core/feed.svelte';

const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

describe('the filtered empty lines (G62)', () => {
	it('filter Gnosis → "none on this network"; all networks → "no activity"', async () => {
		await feed.boot();
		feed.chainFilter(100);
		await settle();
		expect(feed.view.history_empty_key).toBe('history.emptyFilter');
		expect(feed.view.home_empty_key).toBe('home.emptyNoActivityNetwork');

		feed.chainFilter(null);
		await settle();
		expect(feed.view.history_empty_key).toBe('history.emptyTitle');
		expect(feed.view.home_empty_key).toBe('home.emptyNoActivity');
	});

	it('the wallet page tells the feed whenever the filter changes', () => {
		const source = readFileSync(join(import.meta.dirname, '+page.svelte'), 'utf8')
			.replace(/<!--[\s\S]*?-->/g, '')
			.replace(/\/\*[\s\S]*?\*\//g, '')
			.replace(/^\s*\/\/.*$/gm, '');
		const effect = /\$effect\(\(\) => \{\s*const chainId = chainFilter\.chainId;[\s\S]*?\}\);/.exec(
			source
		);
		expect(effect, 'an effect over chainFilter.chainId').not.toBeNull();
		expect(effect![0]).toMatch(/feed\.chainFilter\(chainId\)/);
	});
});
