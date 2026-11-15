import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { missingGlyphs, renderOg } from '../../../scripts/og/render.js';
import { getAllPosts } from './blog';
import { getDoc } from './docs';
import { formatDate } from '../format';
import { docsPath, ogCards } from './og-cards';

const APP = join(import.meta.dirname, '../../..');

describe('social-preview cards', () => {
	const cards = ogCards(APP, false);
	const byPath = new Map(cards.map((c) => [c.path, c]));

	it('one card per page path', () => {
		expect(byPath.size).toBe(cards.length);
	});

	it('says what each published post says', () => {
		const posts = getAllPosts();
		expect(cards.filter((c) => c.path.startsWith('blog/')).length).toBe(posts.length);
		for (const post of posts) {
			const card = byPath.get(`blog/${post.slug}`)!;
			expect(card.title).toBe(post.meta.title);
			expect(card.description).toBe(post.meta.description ?? '');
			expect(card.meta).toBe(`${post.meta.author ?? 'Vela'} · ${formatDate(post.meta.date)}`);
		}
	});

	it('says what each docs page says, in the language it is written in', () => {
		const docs = cards.filter((c) => /^([a-zA-Z-]+\/)?docs(\/|$)/.test(c.path));
		expect(docs.length).toBeGreaterThan(100);
		for (const card of docs) {
			const slug = card.path.replace(/^([a-zA-Z-]+\/)?docs\/?/, '') || 'introduction';
			const doc = getDoc(card.lang, slug)!;
			expect(doc.renderedLocale, card.path).toBe(card.lang);
			expect(docsPath(card.lang, slug).slice(1)).toBe(card.path);
			expect(card.title, card.path).toBe(doc.meta.title);
			expect(card.description, card.path).toBe(doc.meta.description ?? '');
		}
		expect(byPath.get('zh/docs/why-vela')?.section).toBe('文档');
	});

	it('can draw every card with the committed fonts (else: bun run og:fonts)', () => {
		const lacking = ogCards(APP, true)
			.map((c) => [c.path, missingGlyphs(c)] as const)
			.filter(([, m]) => m);
		expect(lacking).toEqual([]);
	});

	it('draws a 1200×630 PNG, in every script', () => {
		for (const path of [
			'blog/why-no-seed-phrase',
			'zh/docs/why-vela',
			'ja/docs',
			'ko/docs',
			'ru/docs'
		]) {
			const card = byPath.get(path);
			expect(card, path).toBeDefined();
			const png: Uint8Array = renderOg(card!);
			expect(Array.from(png.subarray(0, 8))).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
			const view = new DataView(png.buffer, png.byteOffset);
			expect([view.getUint32(16), view.getUint32(20)]).toEqual([1200, 630]);
		}
	});

	it('names the characters it cannot draw instead of drawing stand-ins', () => {
		const card = { lang: 'en', site: 'Vela Wallet', domain: 'getvela.app', title: '为什么 Vela' };
		expect(missingGlyphs(card)).toBe('为什么');
		expect(() => renderOg(card)).toThrow(/为 \(U\+4E3A\), 什 \(U\+4EC0\), 么 \(U\+4E48\)/);
	});

	it('draws decomposed accents as their precomposed letters', () => {
		const card = { lang: 'vi', site: 'V', domain: 'g', title: 'Vì sao' };
		expect(renderOg({ ...card, title: 'Vì sao' })).toEqual(renderOg(card));
	});

	it('refuses a field that would break the request framing', () => {
		expect(() => renderOg({ lang: 'en', site: 'a', domain: 'b', title: 'x\0y' })).toThrow();
	});
});
