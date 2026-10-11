/**
 * A figure follows the page's language (device walk of 2026-10-11, item 8).
 *
 * The symbol of a fiat figure is the core's, and the one thing about it that
 * turns on language is which of the yen and the yuan is the bare "¥". The
 * formatter reads the language from a rune the root layout keeps; this proves
 * the half a node test cannot — that a model DERIVED from a figure is redrawn
 * when the language changes, the way it is when the rate does.
 */
import { flushSync } from 'svelte';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { loadCore } from '$lib/core/client';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import { moneyText, unitAdornment } from '$lib/wallet/live';
import { pageLanguage, setPageLanguage } from './page-language.svelte';

const CNY: CurrencyView = { code: 'CNY', rate: 7, committed: true, pending: null };

beforeAll(() => loadCore());
afterEach(() => setPageLanguage('en'));

describe('the page language', () => {
	it('is English until a layout has said otherwise', () => {
		expect(pageLanguage()).toBe('en');
	});

	it('redraws every figure derived under it when it changes', () => {
		const drawn: string[] = [];
		const stop = $effect.root(() => {
			const figure = $derived(moneyText(1, CNY));
			const unit = $derived(unitAdornment('JPY', 'ETH').prefix);
			$effect.pre(() => {
				drawn.push(`${figure} | ${unit}`);
			});
		});
		flushSync();
		setPageLanguage('zh');
		flushSync();
		setPageLanguage('ja');
		flushSync();
		// Saying the same language again draws nothing again.
		setPageLanguage('ja');
		flushSync();
		stop();
		expect(drawn).toEqual(['CN¥7.00 | ¥', '¥7.00 | JP¥', 'CN¥7.00 | ¥']);
	});
});
