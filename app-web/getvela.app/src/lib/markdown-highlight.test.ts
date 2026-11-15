import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { LANGUAGES, PLAIN, highlight } from '../../scripts/markdown/highlight.js';

const CONTENT = join(import.meta.dirname, '../content');

function markdownFiles(dir: string): string[] {
	return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
		e.isDirectory()
			? markdownFiles(join(dir, e.name))
			: e.name.endsWith('.md')
				? [join(dir, e.name)]
				: []
	);
}

describe('docs code highlighting', () => {
	it('marks commands, flags, strings, variables and comments', () => {
		const html = highlight('FOO=1 docker run -d --name x "a b" $HOME # note', 'sh');
		expect(html).toContain('<span class="t-v">FOO</span><span class="t-op">=</span>');
		expect(html).toContain('<span class="t-fn">docker</span>');
		expect(html).toContain('<span class="t-s">run</span>');
		expect(html).toContain('<span class="t-f">--name</span>');
		expect(html).toContain('<span class="t-s">&quot;a b&quot;</span>');
		expect(html).toContain('<span class="t-v">$HOME</span>');
		expect(html).toContain('<span class="t-c"># note</span>');
	});

	it('starts a new command after a pipe, but not across a line continuation', () => {
		const html = highlight('curl -s x | shasum -a 256\ndocker run \\\n  --rm y', 'bash');
		expect(html).toContain('<span class="t-fn">shasum</span>');
		expect(html).toContain('<span class="t-f">--rm</span>');
		expect(html.match(/t-fn/g)).toHaveLength(3);
	});

	it('escapes HTML and keeps backslashes alive through the template literal', () => {
		const html = highlight('echo "<script>" \\\n  x', 'sh');
		expect(html).not.toContain('<script>');
		expect(html).toContain('&lt;script&gt;');
		expect(html).not.toContain('\\');
		expect(html).toContain('&#92;');
	});

	it('shows an unknown language as plain, escaped text', () => {
		expect(highlight('a < b', 'cobol')).toBe(
			'<pre class="code" tabindex="0"><code><span class="line">a &lt; b</span></code></pre>'
		);
	});

	it('reads .env files', () => {
		const html = highlight('# relay\nOPERATOR_SECRET=abc # keep it', 'dotenv');
		expect(html).toContain('<span class="t-c"># relay</span>');
		expect(html).toContain('<span class="t-v">OPERATOR_SECRET</span>');
		expect(html).toContain('<span class="t-c"> # keep it</span>');
	});

	it('knows every fence language the docs use (a new one gets added here first)', () => {
		const used = new Set<string>();
		for (const file of markdownFiles(CONTENT)) {
			for (const m of readFileSync(file, 'utf8').matchAll(/^\s*```([^\s`]*)/gm)) {
				used.add(m[1].toLowerCase());
			}
		}
		const known = new Set([...Object.keys(LANGUAGES), ...PLAIN]);
		expect([...used].filter((l) => !known.has(l))).toEqual([]);
	});
});
