/**
 * r10-10: a list in the person's language ("#1, #2, and #3", "#1、#2和#3"),
 * never an English comma list inside a translated sentence. The locale is the
 * page's (`<html lang>`) unless one is given; a browser without a formatter
 * gets the plain comma list.
 */
export function listOf(items: readonly string[], locale?: string): string {
	if (items.length < 2) return items.join('');
	const tag =
		locale ??
		(typeof document === 'undefined' ? undefined : document.documentElement.lang || undefined);
	try {
		return new Intl.ListFormat(tag, { type: 'conjunction' }).format(items);
	} catch {
		return items.join(', ');
	}
}
