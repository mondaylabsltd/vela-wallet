/**
 * What a person typed into the send amount, as the text the core can read
 * (issue 231).
 *
 * The core reads ONE shape: ASCII digits and a `.`. In fiat mode it parses the
 * figure the way `parseFloat` does — the longest numeric prefix — so "4,5"
 * from a decimal-comma keyboard is read as 4 and a different sum is sent,
 * silently. In token mode the same text enables Continue and is then refused
 * when the call is built. Neither is something a person can work out from the
 * screen, so the text is made readable HERE, before it is dispatched, and
 * written back into the field so what is on screen is what was sent on.
 *
 * The field's own text is always dot-decimal: the core echoes what it stored,
 * and Max and the ⇄ swap write "0.00075" whatever the number preset. So a `.`
 * is never treated as grouping while typing — dropping it would turn the
 * "0.5" the wallet itself put there into "05" on the next keystroke. What the
 * preset decides is what a `,` means:
 *
 * - decimal-comma preset, no `.` yet → the `,` IS the decimal mark ("4,5" → "4.5");
 * - a single typed `,`, no `.` yet → the same, under any preset (below);
 * - otherwise → a `,` is grouping and is dropped ("1,234.5" → "1234.5").
 *
 * A `,` is never blindly mapped to `.`: a pasted "1,234.56" would become
 * "1.234.56", which reads as 1.234. A PASTED figure that is unmistakably
 * grouped — a decimal part after one kind of mark, thousands after the other —
 * is read for what it is under any preset. Only a paste gets that reading
 * (`pasted`): a stray `,` typed into the middle of "1.23456" can spell
 * "1.234,56" too, and one keystroke must not move a figure a thousandfold.
 *
 * A `,` typed as ONE keystroke into a figure with no `.` yet is a decimal mark
 * under any preset (`previous` is how a keystroke is told from anything
 * else). The number preset follows the browser's language and a phone's
 * decimal pad follows the device's region; where the two disagree the pad's
 * only separator is `,`, and dropping it left a person unable to type a
 * fraction at all — "4,5" became 45 with no sign a key had been refused.
 * Nobody groups thousands by hand one key at a time; a pasted or autofilled
 * "1,234" arrives whole, is not a keystroke, and is still grouping.
 *
 * A PASTE that cannot be read as one figure is refused whole (`null`), and the
 * field keeps what it had: "1.5e-7" — how many screens print a tiny balance —
 * salvaged digit by digit is 1.57, ten million times the figure copied; so is
 * "0x10", and "4.5.6" has no reading at all. While TYPING the same stray
 * character is simply dropped, because there the figure on screen does not
 * change and the person sees their key do nothing.
 *
 * Strings only. This is an amount of money; it never passes through a number.
 */
export function cleanAmountText(
	raw: string,
	presetDecimal: string,
	pasted = false,
	previous?: string
): string | null {
	// A letter BETWEEN digits is an exponent or a hex figure, not a label
	// beside a number ("$4.00", "4 USDT" are fine).
	if (pasted && /\d[a-zA-Z]+[-+]?\d/.test(raw)) return null;

	const text = [...raw]
		.map(plain)
		.join('')
		// Spaces and apostrophes are grouping in some presets; letters, signs
		// and exponents are nothing an amount is written with.
		.replace(/[^0-9.,]/g, '');

	// "1,234.56" / "1,234,567" — and their mirror, "1.234,56" / "1.234.567".
	if (pasted) {
		if (/^\d{1,3}(?:,\d{3})+\.\d+$/.test(text) || /^\d{1,3}(?:,\d{3}){2,}$/.test(text)) {
			return text.replaceAll(',', '');
		}
		if (/^\d{1,3}(?:\.\d{3})+,\d+$/.test(text) || /^\d{1,3}(?:\.\d{3}){2,}$/.test(text)) {
			return text.replaceAll('.', '').replace(',', '.');
		}
	}

	const keystroke =
		previous !== undefined &&
		text.length === previous.length + 1 &&
		text.replace(',', '') === previous;
	const commaIsDecimal = (presetDecimal === ',' || keystroke) && !text.includes('.');
	// Two decimal marks in a paste: there is no figure here to read.
	if (pasted && commaIsDecimal && text.split(',').length > 2) return null;
	const dotted = commaIsDecimal ? text.replace(',', '.') : text;
	// One decimal mark, the first; every later mark and every comma still
	// standing goes.
	const [whole, ...rest] = dotted.replaceAll(',', '').split('.');
	if (pasted && rest.length > 1) return null;
	return rest.length === 0 ? whole : `${whole}.${rest.join('')}`;
}

/**
 * One character, as the ASCII an amount is written with. Arabic-Indic and
 * Persian digits as `parseLocaleNumber` maps them, and the full-width forms a
 * ja / zh / ko keyboard produces — WITH their decimal marks: mapping the
 * digits and dropping the mark between them turned "٤٫٥" into 45.
 */
function plain(ch: string): string {
	const code = ch.charCodeAt(0);
	if (code >= 0x0660 && code <= 0x0669) return String(code - 0x0660);
	if (code >= 0x06f0 && code <= 0x06f9) return String(code - 0x06f0);
	if (code >= 0xff10 && code <= 0xff19) return String(code - 0xff10);
	// "٫" is the Arabic decimal separator and nothing else; "．" and "。" are
	// what the `.` key gives in a full-width / Chinese-punctuation keyboard.
	if (ch === '٫' || ch === '．' || ch === '。') return '.';
	if (ch === '，') return ',';
	return ch;
}

/**
 * Where the caret belongs in `clean`, having been at `caret` in `raw`: after
 * as many KEPT characters as stood before it. Cleaning the text before the
 * caret on its own gave a different answer than cleaning the whole — in
 * "1,2.5" the comma is dropped, but "1," alone reads it as a decimal mark —
 * and the caret landed a character late.
 */
export function caretAfterClean(raw: string, clean: string, caret: number): number {
	const marks = '.,';
	let kept = 0;
	for (let i = 0; i < raw.length && i < caret && kept < clean.length; i++) {
		const ch = plain(raw[i]);
		const next = clean[kept];
		if (ch === next || (marks.includes(ch) && marks.includes(next))) kept++;
	}
	return kept;
}
