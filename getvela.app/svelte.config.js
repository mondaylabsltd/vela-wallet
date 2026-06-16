import adapter from '@sveltejs/adapter-cloudflare';
import { relative, sep } from 'node:path';
import { mdsvex, escapeSvelte } from 'mdsvex';
import { createHighlighter } from 'shiki';
import rehypeSlug from 'rehype-slug';

// Build-time syntax highlighter. Shiki runs only during the build (Node), and
// emits self-contained, inline-styled HTML — so highlighted code ships zero JS
// to the client. The dark theme is chosen to sit naturally on the site palette.
const SHIKI_THEME = 'vesper';
const highlighter = await createHighlighter({
	themes: [SHIKI_THEME],
	langs: [
		'text',
		'bash',
		'shell',
		'json',
		'jsonc',
		'javascript',
		'typescript',
		'tsx',
		'svelte',
		'html',
		'css',
		'diff',
		'yaml',
		'toml',
		'solidity',
		'markdown'
	]
});

/** @type {import('mdsvex').MdsvexOptions} */
const mdsvexOptions = {
	extensions: ['.md'],
	// Give every heading a stable `id` so the docs table-of-contents and
	// in-page anchor links work.
	rehypePlugins: [rehypeSlug],
	highlight: {
		highlighter: async (code, lang = 'text') => {
			const known = highlighter.getLoadedLanguages();
			const safeLang = known.includes(lang) ? lang : 'text';
			const html = escapeSvelte(
				highlighter.codeToHtml(code, { lang: safeLang, theme: SHIKI_THEME })
			);
			return `{@html \`${html}\`}`;
		}
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
		// adapter-auto only supports some environments, see https://svelte.dev/docs/kit/adapter-auto for a list.
		// If your environment is not supported, or you settled on a specific environment, switch out the adapter.
		// See https://svelte.dev/docs/kit/adapters for more information about adapters.
		adapter: adapter()
	}
};

export default config;
