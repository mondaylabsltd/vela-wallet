/**
 * Plural copy on the web (issue 409).
 *
 * Flow copy is resolved at BUILD time, with no count, so a plural key ships as
 * its forms — `key_one`, `key_other`, … exactly the categories the core says
 * the locale has (`resolveFlowMessages`) — and the form is chosen HERE, at
 * render, by the core's CLDR rule.
 *
 * Never `count === 1 ? _one : _other` in TypeScript: that is not Russian's
 * rule (2–4 is `_few`), and Chinese, Japanese, Korean, Vietnamese and
 * Indonesian have one form for every count.
 */
import { i18nPluralSuffix } from '$lib/core/client';

/**
 * The template for plural `key` at `count` in `locale`, read through `lookup`
 * (a flow-copy map, a raw catalog). `undefined` when neither the chosen form
 * nor `_other` is there.
 */
export function pluralTemplate(
	lookup: (key: string) => string | undefined,
	locale: string,
	key: string,
	count: number
): string | undefined {
	let suffix = '_other';
	try {
		suffix = i18nPluralSuffix(locale, count);
	} catch {
		// The core is not running on this page yet. `_other` is the one form
		// every locale has, so the sentence is still the locale's own.
	}
	return lookup(`${key}${suffix}`) ?? lookup(`${key}_other`);
}

/**
 * A plural key's forms in one locale, keyed by suffix (`_one`, `_few`,
 * `_many`, `_other` — exactly the categories the core says the locale has),
 * with the locale whose rule picks among them. The shape a message manifest
 * carries for copy whose count is only known where it is drawn.
 */
export interface PluralCopy {
	locale: string;
	forms: Record<string, string>;
}

/** The form of `copy` for `count`, `{{count}}` still unfilled; '' when it has none. */
export function pluralForm(copy: PluralCopy, count: number): string {
	return pluralTemplate((suffix) => copy.forms[suffix], copy.locale, '', count) ?? '';
}
