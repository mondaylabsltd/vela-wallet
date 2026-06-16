import type { PageLoad } from './$types';
import { getAllPosts } from '$lib/content/blog';

export const prerender = true;

export const load: PageLoad = () => {
	const posts = getAllPosts().map((post) => ({
		slug: post.slug,
		meta: post.meta,
		readingMinutes: post.readingMinutes
	}));
	return { posts };
};
