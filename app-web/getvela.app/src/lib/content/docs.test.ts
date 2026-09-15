import { describe, expect, it } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

/**
 * Frontmatter has to PARSE, in every locale (spec 059, T057).
 *
 * Found the hard way on 2026-09-16: a Spanish and a Russian description each
 * contained a bare `colon + space` — `Vela corre en tu navegador: sin
 * instalación`. That is a YAML mapping, not a sentence, so the frontmatter
 * failed to parse, mdsvex exported the compiled body with NO `metadata`, and
 * the page threw `Cannot read properties of undefined (reading 'title')` —
 * but only when that one page prerendered, in that one language. Five pages
 * across two locales were broken this way and the English source was fine, so
 * nothing in the English test suite could have caught it.
 *
 * Every language whose punctuation puts a colon mid-sentence walks into this,
 * which is most of them. The rule is the fix: a value with a colon in it must
 * be quoted.
 */

const DOCS = join(process.cwd(), 'src/content/docs');

function markdownFiles(dir: string): string[] {
	return readdirSync(dir).flatMap((entry) => {
		const path = join(dir, entry);
		if (statSync(path).isDirectory()) return markdownFiles(path);
		return entry.endsWith('.md') ? [path] : [];
	});
}

describe('every docs file has frontmatter a YAML parser can read', () => {
	const files = markdownFiles(DOCS);

	it('found the docs', () => {
		expect(files.length).toBeGreaterThan(100);
	});

	for (const file of files) {
		const relative = file.slice(DOCS.length + 1);

		it(`${relative} declares a title and a description, and quotes what it must`, () => {
			const lines = readFileSync(file, 'utf8').split('\n');
			expect(lines[0], 'file does not open with `---`').toBe('---');
			const end = lines.indexOf('---', 1);
			expect(end, 'frontmatter is never closed').toBeGreaterThan(0);

			const keys = new Set<string>();
			for (const line of lines.slice(1, end)) {
				const match = /^([A-Za-z_]+): (.*)$/.exec(line);
				if (!match) continue;
				const [, key, value] = match;
				keys.add(key);
				if (value.startsWith('"') || value.startsWith("'")) continue;
				// An unquoted scalar containing `: ` (or ending in `:`) is read as a
				// nested mapping and blows up the whole block.
				expect(
					value.includes(': ') || value.trimEnd().endsWith(':'),
					`${key} has an unquoted colon — wrap the value in double quotes`
				).toBe(false);
			}
			expect([...keys]).toContain('title');
			expect([...keys]).toContain('description');
		});
	}
});
