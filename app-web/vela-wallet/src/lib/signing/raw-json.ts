/**
 * The raw call data of Technical details, drawn to be read (owner, 2026-10-29).
 *
 * A dApp's request arrived as one unformatted blob —
 * `[{"chainId":"0x38","gas":"0x24f2b",…,"data":"0x3593564c000…"}]` — which
 * nobody reads. This lays it out: two spaces per level, one field per line,
 * each token in its kind (key, string, number, true/false/null, punctuation),
 * and a long hex value broken into its selector and its 32-byte words.
 *
 * The text is a site's: untrusted. Nothing here builds markup — the lines are
 * runs of plain text the component draws as text nodes. A character that
 * draws as nothing or turns the text around is its `⟨U+202E⟩` marker, as the
 * core draws it in a message (`invisible_chars.rs`), never itself.
 *
 * Purely the drawing: the request is the string the sheet was given, and the
 * copy button copies THAT, byte for byte. Laid out from the source text, not
 * from a parsed value, so a duplicate key, a number past double precision or
 * an escape as the site wrote it (`\u202E`) reads as it was sent — `JSON.parse`
 * only decides whether it is JSON at all. When it is not, it is drawn as it
 * came, markers only.
 */

import type { SigningMessages } from './messages';
import type { RawDataModel } from './model';

/** What a run of text is, for its ink. `hidden` is a marker, `note` the wallet's own words. */
export type RawRunKind =
	'key' | 'string' | 'number' | 'literal' | 'punct' | 'plain' | 'hidden' | 'note';

export interface RawRun {
	kind: RawRunKind;
	text: string;
}

export interface RawLine {
	/** Nesting level: the line is indented two characters per level. */
	depth: number;
	runs: RawRun[];
	/** One piece of a long hex value broken over lines (its selector, or a 32-byte word). */
	hex?: boolean;
}

export interface RawView {
	/** It parsed: laid out and highlighted. `false`: drawn as it came. */
	json: boolean;
	lines: RawLine[];
	/** Some character of it is drawn as its marker. */
	hidden: boolean;
}

/** The note over a string drawn as the JSON it holds — "(a string, shown as JSON)". */
export interface RawViewWords {
	jsonText: string;
}

/**
 * Longer than a 32-byte word (`0x` + 64): broken over lines. An address, a
 * hash, a short amount stay whole on their line.
 */
export const HEX_SPLIT_OVER = 66;
/** The selector's characters with its `0x`. */
const SELECTOR = 10;
/** One 32-byte word. */
const WORD = 64;
/** A string drawn as the JSON it holds, inside one, inside one — and no deeper. */
const MAX_EMBED = 2;

/*
 * The core's hidden characters (`invisible_chars::is_invisible`): controls
 * other than tab, line feed and carriage return, every format character
 * (bidi controls, zero-width characters, the BOM, tags), the line and
 * paragraph separators, and the letters that draw as blank space. Wider than
 * the core's in one way, on purpose: no joiner is spared between two emoji
 * here — this is the bytes, not prose.
 */
const HIDDEN = /(?![\t\n\r])[\p{Cc}\p{Cf}\p{Zl}\p{Zp}\u034F\u115F\u1160\u180E\u3164\uFFA0]/gu;

/** How a hidden character is drawn: `⟨U+202E⟩` (the core's `marker`). */
export function hiddenMarker(char: string): string {
	const code = char.codePointAt(0) ?? 0;
	return `⟨U+${code.toString(16).toUpperCase().padStart(4, '0')}⟩`;
}

/** `text` as runs of `kind`, each hidden character its own marker run. */
function marked(text: string, kind: RawRunKind): { runs: RawRun[]; hidden: boolean } {
	const runs: RawRun[] = [];
	let at = 0;
	let hidden = false;
	for (const found of text.matchAll(HIDDEN)) {
		const index = found.index ?? 0;
		if (index > at) runs.push({ kind, text: text.slice(at, index) });
		runs.push({ kind: 'hidden', text: hiddenMarker(found[0]) });
		at = index + found[0].length;
		hidden = true;
	}
	if (at < text.length) runs.push({ kind, text: text.slice(at) });
	return { runs, hidden };
}

/**
 * A `0x` hex string's pieces: its 4-byte selector, then one 32-byte word per
 * piece (the last may be shorter). Calldata — `data`/`input`, or anything
 * shaped as a selector and whole words — leads with its selector; any other
 * long value (a signature, packed bytes) is words from its start, so `r`, `s`
 * and `v` fall on their boundaries. Shorter than {@link HEX_SPLIT_OVER} with
 * its `0x`: `null`, it stays whole.
 */
export function hexPieces(hex: string, key?: string): string[] | null {
	if (hex.length <= HEX_SPLIT_OVER || !/^0x[0-9a-fA-F]+$/.test(hex)) return null;
	const body = hex.length - 2;
	const calldata = key === 'data' || key === 'input' || (body - (SELECTOR - 2)) % WORD === 0;
	const head = calldata ? SELECTOR : 2;
	const pieces = [hex.slice(0, head)];
	for (let at = head; at < hex.length; at += WORD) pieces.push(hex.slice(at, at + WORD));
	return pieces;
}

type Token =
	{ t: '{' | '}' | '[' | ']' | ':' | ',' } | { t: 'string' | 'number' | 'literal'; s: string };

const NUMBER = /-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/y;
const LITERAL = /true|false|null/y;

/** The tokens of a JSON text, each as its source. `null`: not JSON after all. */
function lex(text: string): Token[] | null {
	const out: Token[] = [];
	let i = 0;
	while (i < text.length) {
		const c = text[i];
		if (c === ' ' || c === '\t' || c === '\n' || c === '\r') {
			i++;
		} else if (c === '{' || c === '}' || c === '[' || c === ']' || c === ':' || c === ',') {
			out.push({ t: c });
			i++;
		} else if (c === '"') {
			let j = i + 1;
			while (j < text.length && text[j] !== '"') j += text[j] === '\\' ? 2 : 1;
			if (j >= text.length) return null;
			out.push({ t: 'string', s: text.slice(i, j + 1) });
			i = j + 1;
		} else {
			const pattern = c === '-' || (c >= '0' && c <= '9') ? NUMBER : LITERAL;
			pattern.lastIndex = i;
			const found = pattern.exec(text);
			if (!found) return null;
			out.push({ t: pattern === NUMBER ? 'number' : 'literal', s: found[0] });
			i += found[0].length;
		}
	}
	return out;
}

/** A string's JSON text when it holds an object or an array; otherwise `null`. */
function embeddedJson(source: string): string | null {
	let decoded: unknown;
	try {
		decoded = JSON.parse(source);
	} catch {
		return null;
	}
	if (typeof decoded !== 'string') return null;
	const lead = decoded.trimStart()[0];
	if (lead !== '{' && lead !== '[') return null;
	try {
		const value: unknown = JSON.parse(decoded);
		return value !== null && typeof value === 'object' ? decoded : null;
	} catch {
		return null;
	}
}

/** Lays tokens out as lines, two spaces per level. */
class Printer {
	lines: RawLine[] = [];
	hidden = false;
	depth = 0;
	private line: RawLine;

	constructor(private readonly words: RawViewWords) {
		this.line = { depth: 0, runs: [] };
		this.lines.push(this.line);
	}

	private push(kind: RawRunKind, text: string): void {
		if (kind === 'punct' || kind === 'number' || kind === 'literal') {
			this.line.runs.push({ kind, text });
			return;
		}
		const { runs, hidden } = marked(text, kind);
		this.line.runs.push(...runs);
		this.hidden ||= hidden;
	}

	private newline(depth = this.depth, hex = false): void {
		this.line = hex ? { depth, runs: [], hex } : { depth, runs: [] };
		this.lines.push(this.line);
	}

	/** One JSON text, from the current line on. `false`: it did not lay out. */
	print(text: string, embed: number): boolean {
		const tokens = lex(text);
		if (!tokens) return false;
		for (let i = 0; i < tokens.length; i++) {
			const token = tokens[i];
			const next = tokens[i + 1];
			switch (token.t) {
				case '{':
				case '[': {
					const close = token.t === '{' ? '}' : ']';
					if (next?.t === close) {
						this.push('punct', token.t + close);
						i++;
					} else {
						this.push('punct', token.t);
						this.depth++;
						this.newline();
					}
					break;
				}
				case '}':
				case ']':
					this.depth--;
					this.newline();
					this.push('punct', token.t);
					break;
				case ':':
					this.push('punct', ': ');
					break;
				case ',':
					this.push('punct', ',');
					this.newline();
					break;
				case 'number':
				case 'literal':
					this.push(token.t, token.s);
					break;
				case 'string':
					if (next?.t === ':') this.push('key', token.s);
					else this.value(token.s, keyOf(tokens, i), embed);
					break;
			}
		}
		return true;
	}

	/** A string value: whole, broken into its hex pieces, or the JSON it holds. */
	private value(source: string, key: string | undefined, embed: number): void {
		const pieces = hexPieces(source.slice(1, -1), key);
		if (pieces) {
			// Under its key, one level in: the selector, then a word a line.
			this.push('string', `"${pieces[0]}`);
			pieces.slice(1).forEach((piece, n) => {
				this.newline(this.depth + 1, true);
				this.push('string', n === pieces.length - 2 ? `${piece}"` : piece);
			});
			// The comma after it goes on the last word; what follows, back at this level.
			return;
		}
		const inner = embed < MAX_EMBED ? embeddedJson(source) : null;
		if (inner !== null) {
			this.push('note', this.words.jsonText);
			this.push('punct', ' ');
			const depth = this.depth;
			if (this.print(inner, embed + 1)) {
				this.depth = depth;
				return;
			}
		}
		this.push('string', source);
	}
}

/** The key a value at `i` is under (`"data": "0x…"`), decoded; absent in an array. */
function keyOf(tokens: Token[], i: number): string | undefined {
	const colon = tokens[i - 1];
	const key = tokens[i - 2];
	if (colon?.t !== ':' || key?.t !== 'string') return undefined;
	try {
		const decoded: unknown = JSON.parse(key.s);
		return typeof decoded === 'string' ? decoded : undefined;
	} catch {
		return undefined;
	}
}

/** Drawn as it came: its own line breaks, its hidden characters as markers. */
function asIs(raw: string): RawView {
	let hidden = false;
	const lines = raw.split('\n').map((text): RawLine => {
		const line = marked(text, 'plain');
		hidden ||= line.hidden;
		return { depth: 0, runs: line.runs };
	});
	return { json: false, lines, hidden };
}

/**
 * The raw text as lines to draw. JSON (what `JSON.parse` takes) is laid out
 * and highlighted; anything else, or anything this cannot lay out, is drawn
 * as it came. Never throws: the sheet must open whatever a site sends.
 */
export function rawView(raw: string, words: RawViewWords): RawView {
	try {
		JSON.parse(raw);
	} catch {
		return asIs(raw);
	}
	try {
		const printer = new Printer(words);
		if (!printer.print(raw, 0)) return asIs(raw);
		return { json: true, lines: printer.lines, hidden: printer.hidden };
	} catch {
		return asIs(raw);
	}
}

/** How much is drawn before "Show all": lines, and characters across them. */
export const RAW_CAP = { lines: 200, chars: 20_000 } as const;

/**
 * The first lines of a view, up to {@link RAW_CAP} — a 200 KB calldata is a
 * few thousand lines, and drawing them all before anyone asks only makes the
 * sheet slow to open. A single line longer than the whole budget (a blob that
 * is not JSON) is cut at it. `cut`: something was left out.
 */
export function capLines(
	lines: RawLine[],
	cap: { lines: number; chars: number } = RAW_CAP
): { lines: RawLine[]; cut: boolean } {
	const out: RawLine[] = [];
	let chars = 0;
	for (const line of lines) {
		if (out.length >= cap.lines) return { lines: out, cut: true };
		const length = line.runs.reduce((sum, run) => sum + run.text.length, 0);
		if (chars + length > cap.chars) {
			if (out.length === 0) out.push(cutLine(line, cap.chars));
			return { lines: out, cut: true };
		}
		chars += length;
		out.push(line);
	}
	return { lines: out, cut: false };
}

/** `line` with only its first `budget` characters (a marker is kept or dropped whole). */
function cutLine(line: RawLine, budget: number): RawLine {
	const runs: RawRun[] = [];
	let left = budget;
	for (const run of line.runs) {
		if (left <= 0) break;
		if (run.text.length <= left) {
			runs.push(run);
			left -= run.text.length;
		} else {
			if (run.kind !== 'hidden') runs.push({ kind: run.kind, text: run.text.slice(0, left) });
			break;
		}
	}
	runs.push({ kind: 'plain', text: '…' });
	return { ...line, runs };
}

/** The raw data's own words (`RawData`). */
export function rawWords(m: SigningMessages): RawDataModel['words'] {
	return {
		copy: m.techRawCopy,
		copied: m.techRawCopied,
		showAll: around(m.techRawShowAll, '{{lines}}'),
		jsonText: m.techRawJsonText,
		hidden: m.warnHiddenChars
	};
}

/** A template's words before and after its one placeholder (none: all before). */
function around(template: string, placeholder: string): [string, string] {
	const at = template.indexOf(placeholder);
	return at < 0 ? [template, ''] : [template.slice(0, at), template.slice(at + placeholder.length)];
}
