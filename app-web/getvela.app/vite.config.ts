import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { searchForWorkspaceRoot, type Plugin } from 'vite';
import { defineConfig } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';
import { sveltekit } from '@sveltejs/kit/vite';
import { splitNote, toEntry, type NoteEntry } from './src/lib/content/notes-front';
import { ogCards } from './src/lib/content/og-cards';
import { renderOg } from './scripts/og/render.js';
import { renderAll } from './scripts/og/render-all.js';

const NOTES_DIR = join(import.meta.dirname, 'src/content/notes');

/**
 * Every note's frontmatter, read from the files (spec 101). Drafts are included
 * only under `vite dev`, the one place they render.
 */
function readNoteIndex(includeDrafts: boolean): NoteEntry[] {
	if (!existsSync(NOTES_DIR)) return [];
	const entries: NoteEntry[] = [];
	for (const dir of readdirSync(NOTES_DIR, { withFileTypes: true })) {
		if (!dir.isDirectory()) continue;
		for (const file of readdirSync(join(NOTES_DIR, dir.name))) {
			if (!file.endsWith('.md')) continue;
			const { front } = splitNote(readFileSync(join(NOTES_DIR, dir.name, file), 'utf8'));
			const entry = toEntry(dir.name, file.replace(/\.md$/, ''), front);
			if (entry.title && (includeDrafts || !entry.draft)) entries.push(entry);
		}
	}
	return entries;
}

/**
 * `virtual:notes-index` — what the notes lists, sidebar and search know about
 * each note, without its body. The bodies are imported one page at a time; an
 * eager glob of every compiled note would put all of them in the JavaScript of
 * every notes page, which at a few hundred notes is most of a megabyte.
 */
function notesIndex(): Plugin {
	const id = 'virtual:notes-index';
	const resolved = '\0' + id;
	let includeDrafts = false;
	return {
		name: 'vela-notes-index',
		configResolved(config) {
			includeDrafts = config.command === 'serve';
		},
		resolveId(source) {
			return source === id ? resolved : undefined;
		},
		load(source) {
			if (source !== resolved) return;
			return `export default ${JSON.stringify(readNoteIndex(includeDrafts))};`;
		},
		configureServer(server) {
			const refresh = (file: string) => {
				if (!file.startsWith(NOTES_DIR) || !file.endsWith('.md')) return;
				const mod = server.moduleGraph.getModuleById(resolved);
				if (mod) server.moduleGraph.invalidateModule(mod);
				server.ws.send({ type: 'full-reload' });
			};
			server.watcher.on('add', refresh);
			server.watcher.on('change', refresh);
			server.watcher.on('unlink', refresh);
		}
	};
}

/**
 * The social-preview images (`/og/<page path>.png`): drawn by rust/vela-og at
 * build time and shipped as static files; under `vite dev`, drawn on request.
 * The Worker never renders an image and fetches nothing to do so.
 */
function ogImages(): Plugin {
	return {
		name: 'vela-og-images',
		configureServer(server) {
			server.middlewares.use((req, res, next) => {
				const match = /^\/og\/(.+)\.png$/.exec(req.url?.split('?')[0] ?? '');
				const card = match && ogCards(import.meta.dirname, true).find((c) => c.path === match[1]);
				if (!card) return next();
				res.setHeader('content-type', 'image/png');
				res.setHeader('cache-control', 'no-store');
				res.end(Buffer.from(renderOg(card)));
			});
		},
		async generateBundle() {
			// The client build only: its output is what the site serves as files.
			if (this.environment?.config.consumer !== 'client') return;
			for (const { path, png } of await renderAll(ogCards(import.meta.dirname, false))) {
				this.emitFile({ type: 'asset', fileName: `og/${path}.png`, source: png });
			}
		}
	};
}

export default defineConfig(({ command }) => ({
	plugins: [notesIndex(), ogImages(), sveltekit()],
	server: {
		// The site's hashing/signing module is committed in the repository's
		// rust/ (rust/pkg-web-site-crypto), outside this app.
		fs: {
			allow: [
				searchForWorkspaceRoot(process.cwd()),
				join(import.meta.dirname, '../../rust/pkg-web-site-crypto')
			]
		}
	},
	define: {
		// Notes a reader can see in this build, so the header and footer link to
		// /notes only once there is something to read.
		__NOTES_PUBLISHED__: readNoteIndex(command === 'serve').length
	},
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'client',
					browser: {
						enabled: true,
						provider: playwright(),
						instances: [{ browser: 'chromium', headless: true }]
					},
					include: ['src/**/*.svelte.{test,spec}.{js,ts}'],
					exclude: ['src/lib/server/**']
				}
			},

			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					setupFiles: ['src/lib/evm/test-setup.ts'],
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
}));
