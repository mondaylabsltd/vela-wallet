/**
 * Reading a note's frontmatter without a YAML library (spec 101).
 *
 * Shared by the build (vite.config.ts makes the notes index from it, so the
 * lists and the search never load a note's body) and by notes.test.ts (which
 * holds every note to the subset this understands). The subset is on purpose:
 * `key: value` and `key:` followed by `  - item` lines. Frontmatter that needs
 * more than that is doing too much.
 */

export type Front = Record<string, string | string[]>;

function unquote(value: string): string {
	return value.replace(/^"(.*)"$/, '$1').replace(/^'(.*)'$/, '$1');
}

export function parseFront(text: string): Front {
	const out: Front = {};
	let listKey: string | null = null;
	for (const line of text.split('\n')) {
		const item = /^\s+-\s+(.*)$/.exec(line);
		if (item && listKey) {
			(out[listKey] as string[]).push(unquote(item[1]));
			continue;
		}
		const pair = /^([A-Za-z_]+):\s*(.*)$/.exec(line);
		if (!pair) continue;
		const [, key, value] = pair;
		if (value === '') {
			out[key] = [];
			listKey = key;
		} else {
			out[key] = unquote(value);
			listKey = null;
		}
	}
	return out;
}

/** A note file split into its frontmatter and its body. */
export function splitNote(raw: string): { front: Front; body: string } {
	const match = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/.exec(raw);
	return match ? { front: parseFront(match[1]), body: match[2] } : { front: {}, body: raw };
}

/** What the lists, the sidebar and the search know about a note: never its body. */
export interface NoteEntry {
	/** `<module>/<slug>` */
	id: string;
	module: string;
	slug: string;
	title: string;
	/** Short label for the sidebar; the title when absent. */
	nav: string;
	description: string;
	/** Two to four "Label | value" facts, shown as a strip under the answer. */
	facts: { label: string; value: string }[];
	checked: string;
	commit: string;
	sources: string[];
	related: string[];
	docs?: string;
	order?: number;
	featured: boolean;
	draft: boolean;
}

export function toEntry(module: string, slug: string, front: Front): NoteEntry {
	const str = (k: string) => (typeof front[k] === 'string' ? (front[k] as string) : '');
	const list = (k: string) => (Array.isArray(front[k]) ? (front[k] as string[]) : []);
	const order = Number(str('order'));
	return {
		id: `${module}/${slug}`,
		module,
		slug,
		title: str('title'),
		nav: str('nav') || str('title'),
		description: str('description'),
		facts: list('facts').map((f) => {
			const [label, ...rest] = f.split('|');
			return { label: label.trim(), value: rest.join('|').trim() };
		}),
		checked: str('checked'),
		commit: str('commit'),
		sources: list('sources'),
		related: list('related'),
		docs: str('docs') || undefined,
		order: Number.isFinite(order) && str('order') !== '' ? order : undefined,
		featured: str('featured') === 'true',
		draft: str('draft') === 'true'
	};
}
