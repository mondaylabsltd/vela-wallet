// The notes index made by the plugin in vite.config.ts (spec 101).
declare module 'virtual:notes-index' {
	import type { NoteEntry } from './lib/content/notes-front';
	const entries: NoteEntry[];
	export default entries;
}
