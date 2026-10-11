/**
 * The language of the page in view — for the few rules outside the corpus
 * that turn on it.
 *
 * Every string a page draws is resolved at build time, per locale, and arrives
 * as page data; a view-model builder never has to ask what language it is in.
 * A fiat figure's SYMBOL is the exception (device walk of 2026-10-11, item 8):
 * the core writes "¥" for the reader's own of the yen and the yuan and gives
 * the other its country, so the money formatter — one function, called from
 * some thirty places with a number and a currency — has to know the reader's
 * language. Handing it down every one of those paths is thirty chances to
 * leave it out, and a figure formatted without it reads "CN¥" to a Chinese
 * reader: the very mistake the rule exists to end. So it is said once, by the
 * root layout, and read here.
 *
 * A rune, so a model derived from a figure follows a change of language the
 * way it follows a change of rate.
 */
import { FALLBACK_LOCALE } from './locales';

const state = $state({ tag: FALLBACK_LOCALE as string });

/** The page's locale tag ("en", "zh-HK"…). English until a layout has said. */
export function pageLanguage(): string {
	return state.tag;
}

/** The root layout's, on mount and whenever the locale in the path changes. */
export function setPageLanguage(tag: string): void {
	if (state.tag !== tag) state.tag = tag;
}
