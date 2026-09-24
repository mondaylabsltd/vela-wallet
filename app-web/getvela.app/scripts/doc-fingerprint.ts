/**
 * Doc fingerprints (spec 080) — shared by `i18n:stamp` and `i18n:status`.
 *
 * A translated doc records, in its front matter, the fingerprint of the English
 * FILE it was made from: `source: 3fa9c1d2e4b5`. When the English changes, the
 * two disagree and the translation is STALE — present, renderable, and quietly
 * describing a page that has since changed. The English docs drifted for a
 * month under thirteen structurally perfect translations with nothing to say
 * so; message namespaces already had this (`_fingerprints`), docs did not.
 */
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

const SOURCE_LINE = /^source: ([0-9a-f]{12})$/m;

/** First 12 hex chars of SHA-256 over the English file's bytes. */
export function docFingerprint(englishFile: string): string {
	return createHash('sha256').update(readFileSync(englishFile)).digest('hex').slice(0, 12);
}

/** The front matter block of a markdown file, or null when it has none. */
function frontMatter(text: string): { body: string; end: number } | null {
	if (!text.startsWith('---\n')) return null;
	const end = text.indexOf('\n---', 4);
	return end === -1 ? null : { body: text.slice(4, end), end };
}

/** The fingerprint a translation says it was made from, or null. */
export function recordedSource(text: string): string | null {
	const fm = frontMatter(text);
	const match = fm ? SOURCE_LINE.exec(fm.body) : null;
	return match ? match[1] : null;
}

/** `text` with its `source:` line set to `fingerprint` (replaced or appended). */
export function withSource(text: string, fingerprint: string): string {
	const fm = frontMatter(text);
	if (!fm) throw new Error('doc has no front matter to stamp');
	const line = `source: ${fingerprint}`;
	const body = SOURCE_LINE.test(fm.body)
		? fm.body.replace(SOURCE_LINE, line)
		: `${fm.body}\n${line}`;
	return `---\n${body}${text.slice(fm.end)}`;
}
