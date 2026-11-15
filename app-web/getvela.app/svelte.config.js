import adapter from '@sveltejs/adapter-cloudflare';
import { relative, sep } from 'node:path';
import { mdsvex, escapeSvelte } from 'mdsvex';
import headingIds from './scripts/markdown/heading-ids.js';
import { highlight } from './scripts/markdown/highlight.js';

/** @type {import('mdsvex').MdsvexOptions} */
const mdsvexOptions = {
	extensions: ['.md'],
	// Give every heading a stable `id` so the docs table-of-contents and
	// in-page anchor links work.
	rehypePlugins: [headingIds],
	// Build-time highlighting (scripts/markdown/highlight.js): static HTML with
	// a class per token, colors in tokens.css — no JavaScript to the client.
	highlight: {
		highlighter: (code, lang = '') => `{@html \`${escapeSvelte(highlight(code, lang ?? ''))}\`}`
	}
};

/** @type {import('@sveltejs/kit').Config} */
const config = {
	// Treat `.md` files as compilable components (handled by mdsvex below).
	extensions: ['.svelte', '.md'],
	preprocess: [mdsvex(mdsvexOptions)],
	compilerOptions: {
		// defaults to rune mode for the project, execept for `node_modules`. Can be removed in svelte 6.
		runes: ({ filename }) => {
			const relativePath = relative(import.meta.dirname, filename);
			const pathSegments = relativePath.toLowerCase().split(sep);
			const isExternalLibrary = pathSegments.includes('node_modules');

			return isExternalLibrary ? undefined : true;
		}
	},
	kit: {
		// Deploy the site to Cloudflare Workers.
		adapter: adapter()
	}
};

export default config;
