import { describe, expect, it } from 'vitest';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { splitNote, type Front } from './notes-front';
import { NOTE_MODULES } from './notes-modules';
import { flatSidebar } from './sidebar';

/**
 * The mechanical half of the notes contract (spec 101 FR-008; the other half is
 * docs/notes/STYLE.md, which a reviewer reads).
 *
 * The rule this file exists for is the first one: **a source a note cites must
 * exist.** `docs/requirements/` cites ~135 paths that a later refactor deleted,
 * and nothing noticed, because nothing checked. A note that points at nothing is
 * a claim nobody can verify, which is the one thing a note must never be.
 */

const APP = process.cwd();
const REPO = join(APP, '../..');
const NOTES = join(APP, 'src/content/notes');
const STYLE = join(REPO, 'docs/notes/STYLE.md');

interface Parsed {
	file: string;
	id: string;
	module: string;
	front: Front;
	body: string;
}

function readNotes(): Parsed[] {
	if (!existsSync(NOTES)) return [];
	return readdirSync(NOTES).flatMap((module) => {
		const dir = join(NOTES, module);
		if (!statSync(dir).isDirectory()) return [];
		return readdirSync(dir)
			.filter((f) => f.endsWith('.md'))
			.map((f) => {
				const { front, body } = splitNote(readFileSync(join(dir, f), 'utf8'));
				return {
					file: `${module}/${f}`,
					id: `${module}/${f.replace(/\.md$/, '')}`,
					module,
					front,
					body
				};
			});
	});
}

/** The ```banned block of STYLE.md, one phrase per line. */
function bannedPhrases(): string[] {
	const style = readFileSync(STYLE, 'utf8');
	const block = /```banned\n([\s\S]*?)```/.exec(style);
	if (!block) throw new Error('docs/notes/STYLE.md has no ```banned block');
	return block[1]
		.split('\n')
		.map((l) => l.trim())
		.filter(Boolean);
}

/** Prose only: no script blocks, no code, no HTML tags, no link targets. */
function prose(body: string): string {
	return body
		.replace(/<script[\s\S]*?<\/script>/g, ' ')
		.replace(/```[\s\S]*?```/g, ' ')
		.replace(/`[^`]*`/g, ' ')
		.replace(/<[^>]+>/g, ' ')
		.replace(/\]\([^)]*\)/g, ']');
}

const notes = readNotes();
const ids = new Set(notes.map((n) => n.id));
const published = new Set(notes.filter((n) => n.front.draft !== 'true').map((n) => n.id));
const moduleSlugs = new Set(NOTE_MODULES.map((m) => m.slug));
const docSlugs = new Set(flatSidebar.map((d) => d.slug));
const banned = bannedPhrases();

describe('the notes registry', () => {
	it('names every module folder', () => {
		const folders = existsSync(NOTES)
			? readdirSync(NOTES).filter((d) => statSync(join(NOTES, d)).isDirectory())
			: [];
		const unknown = folders.filter((f) => !moduleSlugs.has(f));
		expect(unknown, 'add these to notes-modules.ts, or move the notes').toEqual([]);
	});

	it('has no duplicate module slugs', () => {
		expect(moduleSlugs.size).toBe(NOTE_MODULES.length);
	});

	it('can read the banned list from STYLE.md', () => {
		expect(banned.length).toBeGreaterThan(10);
	});
});

describe('every note', () => {
	it('found the notes', () => {
		expect(notes.length).toBeGreaterThan(0);
	});

	for (const note of notes) {
		const { front, body } = note;
		const isDraft = front.draft === 'true';

		describe(note.file, () => {
			it('has the required frontmatter', () => {
				expect(typeof front.title, 'title').toBe('string');
				expect(typeof front.description, 'description').toBe('string');
				expect(String(front.checked), 'checked: an ISO date').toMatch(/^\d{4}-\d{2}-\d{2}$/);
				expect(String(front.commit), 'commit: a hex hash').toMatch(/^[0-9a-f]{7,40}$/);
				expect(Array.isArray(front.sources) && front.sources.length > 0, 'sources').toBe(true);
			});

			it('has a sidebar label short enough for one line (≤ 40 characters)', () => {
				expect(String(front.nav ?? front.title ?? '').length).toBeLessThanOrEqual(40);
			});

			it('keeps its description to one search result (≤ 160 characters)', () => {
				expect(String(front.description ?? '').length).toBeLessThanOrEqual(160);
			});

			it('quotes any frontmatter value that contains a colon', () => {
				for (const [key, value] of Object.entries(front)) {
					if (typeof value !== 'string') continue;
					const raw = new RegExp(`^${key}: (.*)$`, 'm').exec(
						readFileSync(join(NOTES, note.file), 'utf8')
					)?.[1];
					if (!raw || raw.startsWith('"') || raw.startsWith("'")) continue;
					expect(raw.includes(': '), `${key} has an unquoted colon`).toBe(false);
				}
			});

			it('cites sources that exist', () => {
				for (const source of (front.sources as string[]) ?? []) {
					if (/^https?:\/\//.test(source)) continue;
					const path = source.split('#')[0];
					expect(existsSync(join(REPO, path)), `${source} does not exist in the repository`).toBe(
						true
					);
				}
			});

			it('relates only to notes and docs that exist', () => {
				// A note with nothing related is fine; say so, so the test still asserts.
				expect(Array.isArray(front.related ?? [])).toBe(true);
				for (const id of (front.related as string[]) ?? []) {
					// A published page must not point at one the build leaves out.
					const pool = isDraft ? ids : published;
					expect(
						pool.has(id),
						`related note ${id} does not exist${isDraft ? '' : ' (or is a draft)'}`
					).toBe(true);
				}
				if (front.docs) {
					expect(docSlugs.has(String(front.docs)), `docs: ${front.docs} is not a docs page`).toBe(
						true
					);
				}
			});

			it('takes its title from frontmatter, not an # h1 in the body', () => {
				expect(/^# /m.test(body.replace(/```[\s\S]*?```/g, ''))).toBe(false);
			});

			// What protects reading ease (STYLE.md): a short note in paragraphs a
			// reader can take in at a glance. No template beyond that.
			it('is between 60 and 500 words', () => {
				const words = prose(body).split(/\s+/).filter(Boolean).length;
				expect(words, `${words} words`).toBeGreaterThanOrEqual(60);
				expect(words, `${words} words`).toBeLessThanOrEqual(500);
			});

			it('keeps every paragraph under 80 words', () => {
				// A list item counts on its own, as a reader takes it.
				const long = prose(body)
					.split(/\n\s*\n|\n(?=\s*[-*] )/)
					// A table is scanned, not read through; it isn't a paragraph.
					.filter((p) => !p.trim().startsWith('|'))
					.map((p) => p.replace(/^[>\-\s]+/gm, ' ').trim())
					.filter((p) => p.split(/\s+/).filter(Boolean).length > 80)
					.map((p) => `${p.slice(0, 60)}…`);
				expect(long, 'split it, or make it a list').toEqual([]);
			});

			it('has at most four facts, each "Label | value"', () => {
				const facts = (front.facts as string[] | undefined) ?? [];
				expect(facts.length).toBeLessThanOrEqual(4);
				for (const fact of facts) {
					const [label, value] = fact.split('|').map((x) => x.trim());
					expect(label && value, `"${fact}" needs a label and a value`).toBeTruthy();
					expect(label.length, `"${label}" is a label, keep it short`).toBeLessThanOrEqual(32);
					expect(value.length, `"${value}" is a value, keep it short`).toBeLessThanOrEqual(48);
				}
			});

			// One language per page (STYLE.md, founder 2026-10-09): a ruling written
			// in Chinese is quoted in translation here, in the original in the
			// Chinese edition. Code spans are exempt, so a test string can be named.
			it('is written in English only, with no CJK text in its prose', () => {
				const cjk = prose(`${front.title} ${front.description} ${body}`).match(
					/[\u3040-\u30ff\u3400-\u9fff\uac00-\ud7af\uff00-\uffef\u3000-\u303f]+/g
				);
				expect(cjk ?? [], 'quote it in translation; see docs/notes/STYLE.md').toEqual([]);
			});

			it('uses at most two em dashes', () => {
				expect((prose(body).match(/—/g) ?? []).length).toBeLessThanOrEqual(2);
			});

			it('uses nothing from the banned list', () => {
				const text = prose(`${front.title} ${front.description} ${body}`)
					.replace(/[‘’]/g, "'")
					.toLowerCase();
				const found = banned.filter((phrase) =>
					new RegExp(
						`(^|[^a-z'-])${phrase.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}($|[^a-z'-])`
					).test(text)
				);
				expect(found, 'see docs/notes/STYLE.md').toEqual([]);
			});
		});
	}
});
