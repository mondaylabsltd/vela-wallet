/**
 * The raw call data laid out to be read (`raw-json.ts`): indentation, the
 * kinds of its tokens, a long hex value in its pieces — and a site's hostile
 * text, which is drawn as text and nothing else.
 */
import { describe, expect, it } from 'vitest';
import {
	HEX_SPLIT_OVER,
	RAW_CAP,
	capLines,
	hexPieces,
	hiddenMarker,
	rawView,
	rawWords,
	type RawLine
} from './raw-json';

const WORDS = { jsonText: '(a string, shown as JSON)' };

/** A line as it reads: its indent, then its runs. */
const text = (line: RawLine) => '  '.repeat(line.depth) + line.runs.map((r) => r.text).join('');
const drawn = (raw: string) => rawView(raw, WORDS).lines.map(text);
const runs = (raw: string) => rawView(raw, WORDS).lines.flatMap((line) => line.runs);

const SELECTOR = '0x3593564c';
const WORD_A = '0'.repeat(63) + '6';
const WORD_B = 'f'.repeat(64);
const WORD_C = 'ab'.repeat(32);
const CALLDATA = SELECTOR + WORD_A + WORD_B + WORD_C;

/** The Universal Router request of the owner's report, shortened. */
const UNIVERSAL_ROUTER = JSON.stringify([
	{
		chainId: '0x38',
		gas: '0x24f2b',
		value: '0x2386f26fc10000',
		from: '0x14fb0000000000000000000000000000000000aa',
		to: '0xdc2600000000000000000000000000000000000b',
		data: CALLDATA
	}
]);

describe('laid out', () => {
	it('two spaces a level, one field a line, objects and arrays nested', () => {
		expect(drawn(UNIVERSAL_ROUTER)).toEqual([
			'[',
			'  {',
			'    "chainId": "0x38",',
			'    "gas": "0x24f2b",',
			'    "value": "0x2386f26fc10000",',
			'    "from": "0x14fb0000000000000000000000000000000000aa",',
			'    "to": "0xdc2600000000000000000000000000000000000b",',
			`    "data": "${SELECTOR}`,
			`      ${WORD_A}`,
			`      ${WORD_B}`,
			`      ${WORD_C}"`,
			'  }',
			']'
		]);
		expect(rawView(UNIVERSAL_ROUTER, WORDS).json).toBe(true);
	});

	it('an empty object or array stays on its line', () => {
		expect(drawn('{"a":{},"b":[],"c":[1,true,null]}')).toEqual([
			'{',
			'  "a": {},',
			'  "b": [],',
			'  "c": [',
			'    1,',
			'    true,',
			'    null',
			'  ]',
			'}'
		]);
	});

	it('each token in its kind', () => {
		const kinds = runs('{"k":"v","n":-1.5e3,"t":true,"f":false,"z":null}')
			.filter((run) => run.kind !== 'punct')
			.map((run) => [run.kind, run.text]);
		expect(kinds).toEqual([
			['key', '"k"'],
			['string', '"v"'],
			['key', '"n"'],
			['number', '-1.5e3'],
			['key', '"t"'],
			['literal', 'true'],
			['key', '"f"'],
			['literal', 'false'],
			['key', '"z"'],
			['literal', 'null']
		]);
	});

	it('the source as sent: a duplicate key, a number past double precision, an escape', () => {
		const raw = '{"a":1,"a":2,"big":123456789012345678901234567890,"s":"x\\u202ey"}';
		expect(drawn(raw)).toEqual([
			'{',
			'  "a": 1,',
			'  "a": 2,',
			'  "big": 123456789012345678901234567890,',
			'  "s": "x\\u202ey"',
			'}'
		]);
	});

	it('a scalar JSON text is itself', () => {
		expect(drawn('"0xdeadbeef"')).toEqual(['"0xdeadbeef"']);
		expect(drawn('42')).toEqual(['42']);
	});
});

describe('long hex', () => {
	it('stays whole up to a 32-byte word with its 0x', () => {
		const hash = '0x' + 'e3'.repeat(32);
		expect(hash.length).toBe(HEX_SPLIT_OVER);
		expect(hexPieces(hash)).toBeNull();
		expect(hexPieces('0x' + 'e3'.repeat(32) + '0')).not.toBeNull();
	});

	it('calldata: its selector, then a 32-byte word a line', () => {
		expect(hexPieces(CALLDATA)).toEqual([SELECTOR, WORD_A, WORD_B, WORD_C]);
		// Under `data`, the selector leads even when the words are not whole.
		expect(hexPieces(CALLDATA + 'cafe', 'data')).toEqual([
			SELECTOR,
			WORD_A,
			WORD_B,
			WORD_C,
			'cafe'
		]);
	});

	it('a signature: words from its start, so r, s and v fall on their boundaries', () => {
		const r = '1'.repeat(64);
		const s = '2'.repeat(64);
		const signature = `0x${r}${s}1b`;
		expect(hexPieces(signature, 'signature')).toEqual(['0x', r, s, '1b']);
	});

	it('not hex is no hex: words, a 0x with other letters', () => {
		expect(hexPieces('0x' + 'g'.repeat(80))).toBeNull();
		expect(hexPieces('ab'.repeat(50))).toBeNull();
	});

	it('the pieces put together are the value, character for character', () => {
		const pieces = hexPieces(CALLDATA + 'cafe', 'data')!;
		expect(pieces.join('')).toBe(CALLDATA + 'cafe');
	});

	it('a comma after a broken value lands on its last word', () => {
		const lines = drawn(JSON.stringify({ data: CALLDATA, to: '0x01' }));
		expect(lines).toContain(`    ${WORD_C}",`);
		expect(lines.at(-2)).toBe('  "to": "0x01"');
	});
});

describe('typed data: a string holding JSON is drawn as that JSON', () => {
	const typed = {
		types: { Permit: [{ name: 'owner', type: 'address' }] },
		primaryType: 'Permit',
		message: { owner: '0x01', sig: '0x' + '9'.repeat(130) }
	};
	const raw = JSON.stringify(['0x14fb0000000000000000000000000000000000aa', JSON.stringify(typed)]);

	it('under its note, nested where the string stood', () => {
		const lines = drawn(raw);
		expect(lines[0]).toBe('[');
		expect(lines[1]).toBe('  "0x14fb0000000000000000000000000000000000aa",');
		expect(lines[2]).toBe(`  ${WORDS.jsonText} {`);
		expect(lines).toContain('    "primaryType": "Permit",');
		expect(lines).toContain('          "name": "owner",');
		expect(lines.at(-1)).toBe(']');
		expect(lines.at(-2)).toBe('  }');
		expect(runs(raw).find((run) => run.kind === 'note')?.text).toBe(WORDS.jsonText);
	});

	it('its long hex broken too', () => {
		expect(drawn(raw)).toContain(`      "sig": "0x`);
	});

	it('a string that only looks like JSON stays a string', () => {
		expect(drawn('["{not json", "[1,"]')).toEqual(['[', '  "{not json",', '  "[1,"', ']']);
	});

	it('no deeper than two strings in', () => {
		let inner = '{"a":1}';
		for (let i = 0; i < 4; i++) inner = JSON.stringify([inner]);
		const notes = runs(inner).filter((run) => run.kind === 'note');
		expect(notes).toHaveLength(2);
	});
});

describe('not JSON: drawn as it came', () => {
	it('the bytes of a call, as they are', () => {
		const view = rawView(CALLDATA, WORDS);
		expect(view.json).toBe(false);
		expect(view.lines.map(text)).toEqual([CALLDATA]);
		expect(view.lines[0].runs.every((run) => run.kind === 'plain')).toBe(true);
	});

	it('broken JSON, its own line breaks kept', () => {
		expect(drawn('{"a": 1,\n"b":')).toEqual(['{"a": 1,', '"b":']);
	});
});

describe('hostile text is text', () => {
	const SCRIPT = '<script>alert(1)</script>';
	const IMG = '"><img src=x onerror=alert(1)>';

	it('markup in a key or a value is a run of characters, kept whole', () => {
		const raw = JSON.stringify({ [SCRIPT]: IMG });
		const all = runs(raw);
		expect(all.find((run) => run.kind === 'key')?.text).toBe(JSON.stringify(SCRIPT));
		expect(all.find((run) => run.kind === 'string')?.text).toBe(JSON.stringify(IMG));
	});

	it('markup that is not JSON is drawn as it came', () => {
		expect(drawn(`${SCRIPT}${IMG}`)).toEqual([`${SCRIPT}${IMG}`]);
	});

	it('a right-to-left override is its marker, never itself — and the view says so', () => {
		const raw = JSON.stringify({ to: 'Bob \u202Egpj.exe\u202C ok' });
		const view = rawView(raw, WORDS);
		expect(view.hidden).toBe(true);
		const all = view.lines.flatMap((line) => line.runs);
		expect(all.filter((run) => run.kind === 'hidden').map((run) => run.text)).toEqual([
			'⟨U+202E⟩',
			'⟨U+202C⟩'
		]);
		expect(all.some((run) => /[\u202A-\u202E]/.test(run.text))).toBe(false);
	});

	it('every hidden kind: zero-width, bidi isolates, separators, the BOM, tags, fillers', () => {
		const hostile = [
			'\u200B',
			'\u200D',
			'\u200E',
			'\u2066',
			'\u2069',
			'\u2028',
			'\u2029',
			'\uFEFF',
			'\u00AD',
			'\u{E0041}',
			'\u3164',
			'\u0085'
		];
		const view = rawView(JSON.stringify({ k: `a${hostile.join('b')}c` }), WORDS);
		const markers = view.lines.flatMap((line) =>
			line.runs.filter((run) => run.kind === 'hidden').map((run) => run.text)
		);
		expect(markers).toEqual(hostile.map(hiddenMarker));
		const drawnText = view.lines.flatMap((line) => line.runs.map((run) => run.text)).join('');
		for (const char of hostile) expect(drawnText.includes(char)).toBe(false);
	});

	it('in a key, in a string holding JSON, and in text that is not JSON', () => {
		expect(rawView(JSON.stringify({ 'a\u202Eb': 1 }), WORDS).hidden).toBe(true);
		expect(rawView(JSON.stringify([JSON.stringify({ x: '\u202E' })]), WORDS).hidden).toBe(true);
		const plain = rawView('\uFEFF{"a":1}', WORDS);
		expect(plain.json).toBe(false);
		expect(plain.hidden).toBe(true);
		expect(plain.lines[0].runs[0]).toEqual({ kind: 'hidden', text: '⟨U+FEFF⟩' });
	});

	it('tab, line feed and carriage return are not hidden', () => {
		expect(rawView('a\tb\r', WORDS).hidden).toBe(false);
	});

	it('the marker is the core’s: ⟨U+XXXX⟩, four digits or more', () => {
		expect(hiddenMarker('\u202E')).toBe('⟨U+202E⟩');
		expect(hiddenMarker('\u{E0041}')).toBe('⟨U+E0041⟩');
	});

	it('never throws: deep nesting, a lone surrogate, nothing at all', () => {
		const deep = '['.repeat(100_000) + ']'.repeat(100_000);
		expect(() => rawView(deep, WORDS)).not.toThrow();
		expect(() => rawView('"\uD800"', WORDS)).not.toThrow();
		expect(rawView('', WORDS)).toEqual({
			json: false,
			lines: [{ depth: 0, runs: [] }],
			hidden: false
		});
	});
});

describe('a huge payload asks before it is drawn whole', () => {
	const huge = JSON.stringify([{ to: '0x01', data: SELECTOR + 'ab'.repeat(100_000) }]);

	it('200 KB of calldata lays out quickly and is capped for drawing', () => {
		expect(huge.length).toBeGreaterThan(200_000);
		const started = performance.now();
		const view = rawView(huge, WORDS);
		expect(performance.now() - started).toBeLessThan(500);
		expect(view.lines.length).toBeGreaterThan(3000);
		const shown = capLines(view.lines);
		expect(shown.cut).toBe(true);
		expect(shown.lines.length).toBeLessThanOrEqual(RAW_CAP.lines);
		const chars = shown.lines.reduce(
			(sum, line) => sum + line.runs.reduce((n, run) => n + run.text.length, 0),
			0
		);
		expect(chars).toBeLessThanOrEqual(RAW_CAP.chars);
	});

	it('a single line past the budget is cut at it, and says so', () => {
		const blob = 'x'.repeat(RAW_CAP.chars * 3);
		const shown = capLines(rawView(blob, WORDS).lines);
		expect(shown.cut).toBe(true);
		const line = shown.lines[0].runs.map((run) => run.text).join('');
		expect(line.length).toBe(RAW_CAP.chars + 1);
		expect(line.endsWith('…')).toBe(true);
	});

	it('a short one is drawn whole', () => {
		expect(capLines(rawView(UNIVERSAL_ROUTER, WORDS).lines).cut).toBe(false);
	});
});

describe('its words', () => {
	it('the count’s template split around its place, so no {{…}} stays in a model', () => {
		const words = rawWords({
			techRawCopy: 'Copy raw data',
			techRawCopied: 'Copied',
			techRawShowAll: 'Show all ({{lines}} lines)',
			techRawJsonText: '(a string, shown as JSON)',
			warnHiddenChars: 'Hidden characters.'
		} as Parameters<typeof rawWords>[0]);
		expect(words.showAll).toEqual(['Show all (', ' lines)']);
		expect(JSON.stringify(words)).not.toContain('{{');
	});
});
