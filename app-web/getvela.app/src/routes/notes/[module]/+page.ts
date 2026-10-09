import { error } from '@sveltejs/kit';
import { getNoteModule } from '$lib/content/notes-modules';
import { notesIn, partsWithNotes } from '$lib/content/notes';
import type { EntryGenerator, PageLoad } from './$types';

// Every published module is prerendered from `entries`. 'auto', not `true`:
// while every note is a draft there is nothing to list, and `true` would fail
// the build for a route it never saw. Anything not listed is rendered on
// request, where `getNote` hides drafts, so it is a 404.
export const prerender = 'auto';

export const entries: EntryGenerator = () =>
	partsWithNotes().flatMap(({ modules }) => modules.map(({ module }) => ({ module: module.slug })));

export const load: PageLoad = ({ params }) => {
	const module = getNoteModule(params.module);
	// A module with nothing to read is not a page yet.
	if (!module || notesIn(module.slug).length === 0) error(404, 'Not found');
	return { module: module.slug };
};
