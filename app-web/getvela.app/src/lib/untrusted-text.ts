/**
 * On-chain names are written by whoever wrote the record. Svelte already
 * renders them as text (never markup); what is left is characters that change
 * how the text LOOKS without being visible themselves.
 */

/** Bidi controls (ALM, LRM/RLM, embeddings, overrides, isolates), zero-width
 *  characters, invisible operators, the BOM, and C0/C1 control characters. Done
 *  by code point to keep the source free of invisible characters. */
function isHidden(c: number): boolean {
	return (
		c < 0x20 ||
		(c >= 0x7f && c <= 0x9f) ||
		c === 0x061c ||
		(c >= 0x200b && c <= 0x200f) ||
		(c >= 0x202a && c <= 0x202e) ||
		(c >= 0x2060 && c <= 0x2064) ||
		(c >= 0x2066 && c <= 0x2069) ||
		c === 0xfeff
	);
}

export interface CleanText {
	/** The text with hidden characters removed and whitespace trimmed. */
	text: string;
	/** True when something was removed — a reader should be told. */
	hadHidden: boolean;
}

/** Strip characters that could visually reverse or hide a row label. */
export function cleanUntrusted(n: string): CleanText {
	let out = '';
	let hadHidden = false;
	for (const ch of n) {
		if (isHidden(ch.codePointAt(0) ?? 0)) hadHidden = true;
		else out += ch;
	}
	return { text: out.trim(), hadHidden };
}
