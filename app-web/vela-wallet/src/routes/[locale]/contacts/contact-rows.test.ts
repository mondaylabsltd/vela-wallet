/**
 * A contact's 最近往来 is the core's (spec 093): the page tells the feed whose
 * page is open (`contact_filter_changed`), and draws the `contact_rows` the
 * core hands back with Activity's own row words — no counterparty filter and
 * no "Sent"/"Received" of the page's own.
 *
 * Two halves, as `wallet/feed-filter.test.ts` pins the chain filter: the real
 * feed over the real core takes the contact and lets it go; and the page
 * tells it from an effect over the open contact (read from its source).
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { feed } from '$lib/wallet/core/feed.svelte';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import { load } from './+page.server';

const ALICE = '0x' + 'a1'.repeat(20);

describe("a contact's rows (spec 093)", () => {
	it('the feed is told whose page is open, and that it closed', async () => {
		await feed.contactFilter(ALICE);
		expect(feed.contactAddress).toBe(ALICE);
		expect(feed.view.contact_rows).toEqual([]);
		await feed.contactFilter(null);
		expect(feed.contactAddress).toBeNull();
		expect(feed.view.contact_rows).toEqual([]);
	});

	it('the page tells the feed from an effect over the open contact, and nobody on leaving', () => {
		const source = readFileSync(join(import.meta.dirname, '+page.svelte'), 'utf8')
			.replace(/<!--[\s\S]*?-->/g, '')
			.replace(/\/\*[\s\S]*?\*\//g, '')
			.replace(/^\s*\/\/.*$/gm, '');
		expect(source).toMatch(/\$effect\(\(\) => \{\s*void feed\.contactFilter\(openContact\);/);
		expect(source).toMatch(/onMount\(\(\) => \(\) => void feed\.contactFilter\(null\)\)/);
		// What the rows are drawn from: the core's rows, for the contact it
		// was told about, in Activity's own words.
		expect(source).toMatch(/contactAddress: feed\.contactAddress/);
		expect(source).toMatch(/rowMessages: data\.rowMessages/);
	});

	it("the route ships Activity's row words", () => {
		const data = load({ params: { locale: 'zh' } } as never) as {
			rowMessages: { activity: { dappRowTitle: string } };
		};
		expect(data.rowMessages.activity).toEqual(resolveWalletMessages('zh').activity);
		expect(data.rowMessages.activity.dappRowTitle).toContain('{{place}}');
	});
});
