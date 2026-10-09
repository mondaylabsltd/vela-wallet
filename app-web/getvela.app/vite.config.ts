import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Plugin } from 'vite';
import { defineConfig } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';
import { sveltekit } from '@sveltejs/kit/vite';
import { splitNote, toEntry, type NoteEntry } from './src/lib/content/notes-front';

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

export default defineConfig(({ command }) => ({
	plugins: [notesIndex(), sveltekit()],
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
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
}));
