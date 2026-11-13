import { regex as excludedCharacters } from './slug-characters.js';

/**
 * @typedef {{ type: string, tagName?: string, value?: string,
 *   properties?: Record<string, unknown>, children?: HeadingNode[] }} HeadingNode
 */

/** Only text descendants contribute to a heading, not image alt text or comments.
 * @param {HeadingNode} node
 * @returns {string}
 */
function headingText(node) {
	if (node.type === 'text') return node.value ?? '';
	return node.children?.map(headingText).join('') ?? '';
}

/**
 * Build-time rehype plugin. IDs match the previous GitHub slug rules, including
 * Unicode, repeated spaces and collisions with naturally suffixed headings.
 * Each document gets its own counter; existing IDs are left untouched.
 * @param {{ prefix?: string }} [options]
 */
export default function headingIds(options = {}) {
	const prefix = options.prefix ?? '';
	/** @param {HeadingNode} tree */
	return (tree) => {
		/** @type {Map<string, number>} */
		const occurrences = new Map();
		/** @param {HeadingNode} node */
		function visit(node) {
			if (node.type === 'element' && /^h[1-6]$/.test(node.tagName ?? '')) {
				const properties = (node.properties ??= {});
				if (!properties.id) {
					const base = headingText(node)
						.toLowerCase()
						.replace(excludedCharacters, '')
						.replace(/ /g, '-');
					let slug = base;
					while (occurrences.has(slug)) {
						const next = (occurrences.get(base) ?? 0) + 1;
						occurrences.set(base, next);
						slug = `${base}-${next}`;
					}
					occurrences.set(slug, 0);
					properties.id = prefix + slug;
				}
			}
			for (const child of node.children ?? []) visit(child);
		}
		visit(tree);
	};
}
