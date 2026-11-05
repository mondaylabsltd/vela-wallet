/**
 * r14-11a: the short address forms inside a line of text, found so they can
 * be drawn as one unbreakable run (`AddressText.svelte`, `.short-address`).
 *
 * Unicode line breaking allows a break after "…" (U+2026), so "0x5fb7e1…47B271"
 * broke into "0x5fb7e1…" and "47B271" wherever a line ran out — a CJK line
 * readily does. The text around an address still wraps; the address moves to
 * the next line whole.
 *
 * A short form is `0x`, hex, an ellipsis ("…" or "..."), hex: the core's
 * 6+6 (`names.short`), its 6+4, and the three-dot form a decoded call uses.
 * A whole address has no ellipsis and is left alone — it is long enough that
 * it may have to break.
 */
export interface AddressRun {
	text: string;
	/** A short address form: never broken. */
	address: boolean;
}

const SHORT_ADDRESS = /0x[0-9a-fA-F]{2,}(?:…|\.\.\.)[0-9a-fA-F]{2,}/g;

export function addressRuns(text: string): AddressRun[] {
	const runs: AddressRun[] = [];
	let at = 0;
	for (const match of text.matchAll(SHORT_ADDRESS)) {
		const start = match.index ?? 0;
		if (start > at) runs.push({ text: text.slice(at, start), address: false });
		runs.push({ text: match[0], address: true });
		at = start + match[0].length;
	}
	if (at < text.length) runs.push({ text: text.slice(at), address: false });
	return runs;
}
