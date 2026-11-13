import adapter from '@sveltejs/adapter-cloudflare';
import { relative, sep } from 'node:path';
import { mdsvex, escapeSvelte } from 'mdsvex';
import { createHighlighter } from 'shiki';
import headingIds from './scripts/markdown/heading-ids.js';

// Build-time syntax highlighter. Shiki runs only during the build (Node), and
// emits self-contained, inline-styled HTML — so highlighted code ships zero JS
// to the client. Both themes are emitted per token and resolved with CSS
// `light-dark()` (see tokens.css), so code blocks follow the site theme.
const SHIKI_THEMES = { light: 'github-light', dark: 'vesper' };
const highlighter = await createHighlighter({
	themes: Object.values(SHIKI_THEMES),
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
	rehypePlugins: [headingIds],
	highlight: {
		highlighter: async (code, lang = 'text') => {
			const known = highlighter.getLoadedLanguages();
			const safeLang = known.includes(lang) ? lang : 'text';
			const html = escapeSvelte(
				highlighter.codeToHtml(code, { lang: safeLang, themes: SHIKI_THEMES, defaultColor: false })
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
		// Deploy the site to Cloudflare Workers.
		adapter: adapter()
	}
};

export default config;
