import { error } from '@sveltejs/kit';
import type { EntryGenerator, PageLoad } from './$types';
import { getPost, getAllPostSlugs } from '$lib/content/blog';

export const prerender = true;

export const entries: EntryGenerator = () => getAllPostSlugs().map((slug) => ({ slug }));

export const load: PageLoad = ({ params }) => {
	const post = getPost(params.slug);
	if (!post || post.meta.draft) {
		error(404, 'Post not found');
	}
	return {
		slug: post.slug,
		meta: post.meta,
		readingMinutes: post.readingMinutes
	};
};
