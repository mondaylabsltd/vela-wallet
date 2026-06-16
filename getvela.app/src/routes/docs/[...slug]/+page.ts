import { error } from '@sveltejs/kit';
import type { EntryGenerator, PageLoad } from './$types';
import { getDoc, getDocSlugs } from '$lib/content/docs';

export const entries: EntryGenerator = () => getDocSlugs().map((slug) => ({ slug }));

export const load: PageLoad = ({ params }) => {
	const doc = getDoc(params.slug);
	if (!doc) {
		error(404, 'Documentation page not found');
	}
	return { slug: doc.slug, meta: doc.meta };
};
