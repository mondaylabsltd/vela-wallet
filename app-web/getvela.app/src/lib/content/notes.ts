import type { Component } from 'svelte';
import index from 'virtual:notes-index';
import type { NoteEntry } from './notes-front';
import { NOTE_MODULES, NOTE_PARTS, type NoteModule, type NotePart } from './notes-modules';
import { seoConfig } from '$lib/seo';

/**
 * Notes (spec 101): `src/content/notes/<module>/<slug>.md`, English only.
 *
 * Two halves, on purpose. What the lists, the sidebar and the search need comes
 * from `virtual:notes-index`: frontmatter only, made at build time (drafts
 * included under `vite dev` alone). A note's body is imported when that note's
 * page loads, so no page carries the text of every other note.
 */

export type Note = NoteEntry;

const bodies = import.meta.glob<{ default: Component }>('/src/content/notes/*/*.md');

const moduleOrder = new Map(NOTE_MODULES.map((m, i) => [m.slug, i]));

function compareNotes(a: Note, b: Note): number {
	const byModule = (moduleOrder.get(a.module) ?? 999) - (moduleOrder.get(b.module) ?? 999);
	if (byModule !== 0) return byModule;
	const ao = a.order ?? Number.MAX_SAFE_INTEGER;
	const bo = b.order ?? Number.MAX_SAFE_INTEGER;
	if (ao !== bo) return ao - bo;
	return a.title.localeCompare(b.title);
}

const notes: Note[] = [...index].sort(compareNotes);
const byId = new Map(notes.map((n) => [n.id, n]));

/** Every note a reader can see in this build, in reading order. */
export function getNotes(): Note[] {
	return notes;
}

export function getNote(module: string, slug: string): Note | undefined {
	return byId.get(`${module}/${slug}`);
}

export function notesIn(module: string): Note[] {
	return notes.filter((n) => n.module === module);
}

/** Notes marked `featured`, for the index's "start here" row. */
export function featuredNotes(): Note[] {
	return notes.filter((n) => n.featured);
}

/** The compiled body of one note. */
export async function loadNoteBody(note: Note): Promise<Component> {
	const load = bodies[`/src/content/notes/${note.module}/${note.slug}.md`];
	if (!load) throw new Error(`No body for note ${note.id}`);
	return (await load()).default;
}

/** Parts and modules that have at least one visible note, in registry order. */
export function partsWithNotes(): {
	part: NotePart;
	modules: { module: NoteModule; notes: Note[] }[];
}[] {
	return NOTE_PARTS.map((part) => ({
		part,
		modules: part.modules
			.map((module) => ({ module, notes: notesIn(module.slug) }))
			.filter((entry) => entry.notes.length > 0)
	})).filter((entry) => entry.modules.length > 0);
}

/** Previous and next note inside the same module. */
export function adjacentNotes(note: Note): { prev?: Note; next?: Note } {
	const siblings = notesIn(note.module);
	const i = siblings.findIndex((n) => n.id === note.id);
	return { prev: siblings[i - 1], next: siblings[i + 1] };
}

/** `related` resolved to notes; a missing id is a test failure, not an empty link. */
export function relatedNotes(note: Note): Note[] {
	return note.related.map((id) => byId.get(id)).filter((n): n is Note => !!n);
}

export interface SourceLink {
	href: string;
	label: string;
}

/**
 * A source as a link a reader can follow. Repository paths are pinned to the
 * commit the note was checked at, so the lines it points to are the lines the
 * note was written against, whatever happens to `main` afterwards.
 */
export function sourceLink(source: string, commit: string): SourceLink {
	if (/^https?:\/\//.test(source)) {
		return { href: source, label: source.replace(/^https?:\/\/(www\.)?/, '') };
	}
	const [path, fragment] = source.split('#');
	// GitHub renders Markdown, and a rendered page has no line anchors; the
	// plain view does.
	const plain = fragment && path.endsWith('.md') ? '?plain=1' : '';
	return {
		href: `${seoConfig.github}/blob/${commit}/${path}${plain}${fragment ? `#${fragment}` : ''}`,
		label: fragment ? `${path}, ${lineLabel(fragment)}` : path
	};
}

/** `L98-L103` → "lines 98–103"; `L98` → "line 98". */
function lineLabel(fragment: string): string {
	const [from, to] = fragment.replace(/L/g, '').split('-');
	return to ? `lines ${from}–${to}` : `line ${from}`;
}
