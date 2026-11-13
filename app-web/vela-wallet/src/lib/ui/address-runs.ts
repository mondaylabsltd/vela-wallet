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

/** Hex longer than this is split in two (a 6+6 short form never is). */
const SPLIT_FROM = 24;
/** Longer hex (calldata) wraps as text does: two halves would not fit. */
const SPLIT_UP_TO = 66;

/**
 * r16-21: the two halves a full address (or hash) is drawn as — a line may
 * break between them and nowhere else, the desktop's rule (r15-10,
 * `ui::address::address_halves`): `0x` with the first half, the halves as
 * near even as hex allows (22 + 20 for an address). `null`: drawn whole —
 * a short form, words, calldata.
 */
export function addressHalves(text: string): [string, string] | null {
	if (!/^0[xX][0-9a-fA-F]+$/.test(text)) return null;
	if (text.length < SPLIT_FROM || text.length > SPLIT_UP_TO) return null;
	const mid = 2 + Math.ceil((text.length - 2) / 2);
	return [text.slice(0, mid), text.slice(mid)];
}
