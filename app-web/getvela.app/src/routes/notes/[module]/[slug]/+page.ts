import { error } from '@sveltejs/kit';
import { getNote, getNotes, loadNoteBody } from '$lib/content/notes';
import type { EntryGenerator, PageLoad } from './$types';

// Every published note is prerendered from `entries`. 'auto', not `true`:
// while every note is a draft there is nothing to list, and `true` would fail
// the build for a route it never saw. Anything not listed is rendered on
// request, where the index has no drafts, so it is a 404.
export const prerender = 'auto';

export const entries: EntryGenerator = () =>
	getNotes().map((note) => ({ module: note.module, slug: note.slug }));

export const load: PageLoad = async ({ params }) => {
	const note = getNote(params.module, params.slug);
	if (!note) error(404, 'Not found');
	// Only this note's body is loaded; a universal load may return a component.
	return { note, body: await loadNoteBody(note) };
};
