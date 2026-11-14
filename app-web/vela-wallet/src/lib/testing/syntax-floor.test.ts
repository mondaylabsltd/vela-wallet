/** The syntax-floor check refuses what it names and nothing a floor browser runs. */
import { describe, expect, it } from 'vitest';
import { belowFloor } from './syntax-floor';

describe('the extension syntax floor', () => {
	it('passes the modern syntax Safari 17.4 and Chrome 122 run', () => {
		const ok = `(() => {
			class A { #x = 1; static { this.y = 2; } get x() { return this.#x; } }
			const o = { a: 1 }; const v = o?.a ?? 0; let n = 1_000n; n **= 2n;
			const re = /(?:a|b)\\(?i:x\\)[\\p{L}]/v; const g = /(?<year>\\d{4})/d;
			const arr = [3, 1].toSorted().at(-1); const ok = Object.hasOwn(o, 'a');
			async function* gen() { for await (const x of [1]) yield x; }
			label: for (;;) { try { break label; } catch { } }
		})();`;
		expect(belowFloor(ok)).toEqual([]);
	});

	it.each([
		['import { a } from "./a.js";', 'import'],
		['export const a = 1;', 'export'],
		['const a = 1; export { a };', 'export'],
		['export default 1;', 'export'],
		['console.log(import.meta.url);', 'import.meta'],
		['{ using r = open(); }', 'using'],
		['async function f() { await using r = open(); }', 'await using'],
		['@sealed class A {}', 'decorator'],
		['const re = /(?i:abc)/;', 'regex modifiers'],
		['const re = /(?-s:a.b)/;', 'regex modifiers'],
		['const a = ;', 'syntax error']
	])('refuses %s', (source, what) => {
		expect(belowFloor(source).some((found) => found.startsWith(what))).toBe(true);
	});

	it('lets a module import, export and read import.meta, and holds it to the rest', () => {
		const chunk = 'import { a } from "./a.js"; export const b = a; export default import.meta.url;';
		expect(belowFloor(chunk, 'chunk.js', { module: true })).toEqual([]);
		expect(belowFloor(chunk, 'chunk.js')).not.toEqual([]);
		expect(belowFloor('{ using r = open(); }', 'chunk.js', { module: true })).toEqual([
			'using at 1'
		]);
		expect(belowFloor('const re = /(?i:a)/;', 'chunk.js', { module: true })).toEqual([
			'regex modifiers at 1'
		]);
	});
});
