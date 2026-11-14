import { describe, expect, it } from 'vitest';
import { mdsvex } from 'mdsvex';
import headingIds from '../../../scripts/markdown/heading-ids.js';

type Node = Parameters<ReturnType<typeof headingIds>>[0];
const text = (value: string): Node => ({ type: 'text', value });
const heading = (value: string, properties: Record<string, unknown> = {}): Node => ({
	type: 'element',
	tagName: 'h2',
	properties,
	children: [text(value)]
});

function ids(values: string[]) {
	const tree: Node = { type: 'root', children: values.map((value) => heading(value)) };
	headingIds()(tree);
	return tree.children?.map((node) => node.properties?.id);
}

describe('Markdown heading IDs', () => {
	it('keeps multilingual letters, accents, underscores and hyphens', () => {
		expect(
			ids(['你好，世界！', '日本語 FAQ', 'Café déjà vu', 'Привет мир', 'İstanbul', 'a_b-c'])
		).toEqual(['你好世界', '日本語-faq', 'café-déjà-vu', 'привет-мир', 'i̇stanbul', 'a_b-c']);
	});

	it('preserves the old punctuation, whitespace and emoji rules', () => {
		expect(ids([' Hello  World! ', 'a\tb\nc', '🔐 Safe & secure', 'a—b', '!!!', '!!!'])).toEqual([
			'-hello--world-',
			'abc',
			'-safe--secure',
			'ab',
			'',
			'-1'
		]);
	});

	it('avoids collisions with both repeated and naturally suffixed headings', () => {
		expect(ids(['Foo', 'Foo', 'Foo-1', 'Foo', '__proto__', '__proto__', 'constructor'])).toEqual([
			'foo',
			'foo-1',
			'foo-1-1',
			'foo-2',
			'__proto__',
			'__proto__-1',
			'constructor'
		]);
		expect(ids(['Foo-1', 'Foo', 'Foo'])).toEqual(['foo-1', 'foo', 'foo-2']);
	});

	it('extracts nested inline text without including image alt text or comments', () => {
		const node = heading('');
		node.children = [
			text('Use '),
			{ type: 'element', tagName: 'code', children: [text('eth_call')] },
			text(' & '),
			{ type: 'element', tagName: 'a', children: [text('links')] },
			{ type: 'element', tagName: 'img', properties: { alt: 'ignored' } },
			{ type: 'comment', value: 'ignored' }
		];
		headingIds()(node);
		expect(node.properties?.id).toBe('use-eth_call--links');
	});

	it('preserves explicit IDs and only assigns IDs to h1 through h6', () => {
		const tree: Node = {
			type: 'root',
			children: [
				heading('Custom', { id: 'existing' }),
				heading('Custom'),
				{ type: 'element', tagName: 'p', children: [text('Paragraph')] }
			]
		};
		headingIds({ prefix: 'section-' })(tree);
		expect(tree.children?.map((node) => node.properties?.id)).toEqual([
			'existing',
			'section-custom',
			undefined
		]);
	});

	it('resets counters for every document even when a transformer is reused', () => {
		const transform = headingIds();
		const trees = Array.from({ length: 2 }, (): Node => ({
			type: 'root',
			children: [heading('Same'), heading('Same')]
		}));
		for (const tree of trees) transform(tree);
		expect(trees.map((tree) => tree.children?.map((node) => node.properties?.id))).toEqual([
			['same', 'same-1'],
			['same', 'same-1']
		]);
	});

	it('works in the actual mdsvex pipeline with links, inline code and Chinese', async () => {
		const processor = mdsvex({
			extensions: ['.md'],
			rehypePlugins: [headingIds],
			highlight: false
		});
		const output = await processor.markup({
			content: '# 你好，世界！\n\n## Use `eth_call` & [links](/docs)\n\n## 你好，世界！',
			filename: 'heading-regression.md'
		});
		expect(
			[...(output?.code ?? '').matchAll(/<h[1-6]\b[^>]*\bid="([^"]*)"/g)].map((match) => match[1])
		).toEqual(['你好世界', 'use-eth_call--links', '你好世界-1']);
	});
});
