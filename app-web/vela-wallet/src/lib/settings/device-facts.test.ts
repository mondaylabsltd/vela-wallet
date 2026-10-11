/**
 * The feedback sheet's "Unreachable networks" line (device walk of
 * 2026-10-11, item 9).
 *
 * The owner's phone read 「无法连接的 RPC 1625」 — a chain id after a label that
 * made it look like a count. Two things were wrong and one of them was the
 * web's too: a network with no name was written in English ("Chain 1625")
 * whatever the page's language. Each unreachable network is now named, a
 * nameless one in the corpus's words (`assets.chainFallback`), and what leaves
 * the device is the same line the person saw.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';

const pool = vi.hoisted(() => ({ failed: new Set<number>() }));
vi.mock('$lib/services/rpc-pool', () => ({ getFailedRpcChains: () => pool.failed }));

import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { environmentLines } from '$lib/services/bug-report';
import { deviceFacts, unreachableNetworkNames } from './device-facts';
import { feedbackLabels } from './live';

const en = resolveSettingsMessages('en');
const zh = resolveSettingsMessages('zh');
/** The `.g` name service's chain: one this wallet has no network for. */
const NAMELESS = 1625;

describe('the networks a report lists as unreachable', () => {
	it('are named — a network the wallet has, by its name', () => {
		expect(unreachableNetworkNames([1, 56, 100], en.bugReport.chainFallback)).toEqual([
			'Ethereum',
			'BNB Chain',
			'Gnosis'
		]);
	});

	it('one with no name is said in words, in the page’s language', () => {
		expect(unreachableNetworkNames([NAMELESS], en.bugReport.chainFallback)).toEqual(['Chain 1625']);
		expect(unreachableNetworkNames([NAMELESS], zh.bugReport.chainFallback)).toEqual(['链 1625']);
	});

	it('is never a bare id, in any language', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const m = resolveSettingsMessages(locale);
			const [named, nameless] = unreachableNetworkNames([56, NAMELESS], m.bugReport.chainFallback);
			expect(named).toBe('BNB Chain');
			expect(nameless, locale).toContain('1625');
			expect(nameless, locale).not.toMatch(/^\d+$/);
			expect(nameless, locale).not.toContain('{{');
		}
	});
});

describe('the line the feedback sheet shows, which is the line that is sent', () => {
	const line = (m: typeof en, locale: string) => {
		const facts = deviceFacts(locale, [], m.bugReport.chainFallback);
		return environmentLines(feedbackLabels(m), facts)[3];
	};

	it('reads "Unreachable networks", then the names', () => {
		pool.failed = new Set([56, NAMELESS]);
		expect(line(en, 'en')).toBe('Unreachable networks: BNB Chain, Chain 1625');
		expect(line(zh, 'zh')).toBe('连不上的网络: BNB Chain, 链 1625');
		// The label is no longer "RPCs", under which a number read as a count.
		expect(en.bugReport.previewRpc).not.toMatch(/RPC/);
	});

	it('says "None" when every network answers', () => {
		pool.failed = new Set();
		expect(line(en, 'en')).toBe(`Unreachable networks: ${en.bugReport.previewNone}`);
	});
});
