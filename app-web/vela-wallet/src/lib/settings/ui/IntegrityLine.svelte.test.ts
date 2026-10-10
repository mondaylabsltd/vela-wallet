/**
 * A signing page's integrity line never breaks badly (the 102 device run).
 *
 * Two things were seen on a phone: the time split across lines — 「检查于 下午
 * / 10:07」 — and a line that STARTED with "·". Both are fixed where the words
 * are made: the core's `checked_time` binds its own spaces (U+00A0) and joins
 * its own CJK characters (U+2060, so 「下午」 cannot come apart either), and the
 * corpus binds each "·" to the word before it. This draws the real strings —
 * the corpus's line, the core's time — at every width a phone or a narrow
 * column could give them, and looks at where the browser actually broke.
 *
 * A `.svelte.test.ts`: where a line breaks is the browser's to answer.
 */
import { tick } from 'svelte';
import { beforeAll, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import { signerIntegrityTime } from '$lib/core/kernels';
import en from '../../../../../../assets/i18n/en.json';
import ja from '../../../../../../assets/i18n/ja.json';
import zh from '../../../../../../assets/i18n/zh.json';
import IntegrityLine from './IntegrityLine.svelte';

beforeAll(() => loadCore());

const NBSP = '\u00a0';
/**
 * U+2060 WORD JOINER: the core puts one between two neighbours of the time
 * when either is a Han, kana or Hangul character, so the day period itself
 * (下午, 午後) cannot come apart. It draws as nothing — which is why the
 * helpers below, which read what was DRAWN, compare against the time without
 * it. Product code neither strips nor adds one.
 */
const WJ = '\u2060';
/** A time as it is drawn: the joiners take no room and leave no glyph. */
const drawnForm = (time: string) => time.replaceAll(WJ, '');

type Catalog = { componentsUi: { signing: { integrity: Record<string, string> } } };

/** The corpus's line for `state`, with the core's own time in it. */
function line(catalog: unknown, state: 'matches' | 'trusted', time: string): string {
	return (catalog as Catalog).componentsUi.signing.integrity[state]
		.replace('{{version}}', '0ba8ee8c')
		.replace('{{time}}', time);
}

/** 2:32 in the afternoon, read half an hour later the same day, on a 12-hour clock. */
const afternoon = (language: string) =>
	signerIntegrityTime(
		Date.UTC(2026, 9, 9, 14, 32),
		Date.UTC(2026, 9, 9, 15, 2),
		0,
		{ date: 'dmy_slash', time: 'h12' },
		language
	);

/** The same check read the NEXT day: the date and the time. */
const yesterday = (language: string) =>
	signerIntegrityTime(
		Date.UTC(2026, 9, 9, 14, 32),
		Date.UTC(2026, 9, 10, 9, 0),
		0,
		{ date: 'dmy_slash', time: 'h12' },
		language
	);

/** The line each character of `.text` was drawn on (its top, rounded), in reading order. */
function drawnLines(root: HTMLElement): { char: string; top: number }[] {
	const text = root.querySelector('.text') as HTMLElement;
	const walker = document.createTreeWalker(text, NodeFilter.SHOW_TEXT);
	const out: { char: string; top: number }[] = [];
	for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
		const value = node.nodeValue ?? '';
		for (let i = 0; i < value.length; i += 1) {
			const range = document.createRange();
			range.setStart(node, i);
			range.setEnd(node, i + 1);
			const rect = range.getClientRects()[0];
			if (rect === undefined || rect.width === 0) continue;
			out.push({ char: value[i], top: Math.round(rect.top + rect.height / 2) });
		}
	}
	return out;
}

/** Characters whose centre sits on a later line than the one before them. */
function lineStarts(chars: { char: string; top: number }[]): string[] {
	const starts: string[] = [];
	for (let i = 1; i < chars.length; i += 1) {
		// A mono version and the prose around it differ by a pixel or two on one line.
		if (chars[i].top - chars[i - 1].top > 6) starts.push(chars[i].char);
	}
	return starts;
}

/** Every width from a narrow column to a wide phone, in tokens of 4 px. */
const WIDTHS = Array.from({ length: 56 }, (_, i) => 150 + i * 4);

async function sweep(
	text: string
): Promise<{ width: number; chars: { char: string; top: number }[] }[]> {
	const screen = render(IntegrityLine, { props: { line: { text, tone: 'ok' } } });
	const out = [];
	for (const width of WIDTHS) {
		screen.container.style.width = `${width}px`;
		await tick();
		out.push({ width, chars: drawnLines(screen.container) });
	}
	await screen.unmount();
	return out;
}

describe('the core’s check time binds its own spaces', () => {
	it('「下午 2:32」, "2:32 PM", a date and a time: no breaking space in any of them', () => {
		for (const language of ['zh', 'en', 'ja']) {
			for (const time of [afternoon(language), yesterday(language)]) {
				expect(time, `${language}: ${time}`).not.toMatch(/[ \\t]/);
			}
		}
		// CJK neighbours are joined too (PR 3 note 14): U+00A0 binds 午 to the
		// digits, and nothing bound 下 to 午 — a browser may break between any
		// two ideographs.
		expect(afternoon('zh')).toBe(`下${WJ}午${NBSP}2:32`);
		expect(afternoon('ja')).toBe(`午${WJ}後${NBSP}2:32`);
		// A Latin-script moment is byte for byte what it was.
		expect(afternoon('en')).toBe(`2:32${NBSP}PM`);
		expect(afternoon('en')).not.toContain(WJ);
		expect(yesterday('en')).toContain(NBSP);
	});
});

describe('the integrity line, at every width', () => {
	for (const [language, catalog] of [
		['zh', zh],
		['ja', ja],
		['en', en]
	] as const) {
		for (const state of ['matches', 'trusted'] as const) {
			it(`${language} · ${state}: no line starts with "·", and the time never breaks at its space`, async () => {
				const time = afternoon(language);
				const text = line(catalog, state, time);
				// The corpus binds each "·" to the word before it.
				expect(text).not.toContain(' · ');
				expect(text.split(`${NBSP}· `).length).toBe(3);
				let wrapped = 0;
				for (const { width, chars } of await sweep(text)) {
					const starts = lineStarts(chars);
					if (starts.length > 0) wrapped += 1;
					expect(starts, `${language} ${state} at ${width}px`).not.toContain('·');
					// The time's own space never ends a line: what follows it is
					// on the line it is on.
					const drawn = chars.map((c) => c.char).join('');
					const shown = drawnForm(time);
					const at = drawn.lastIndexOf(shown);
					expect(at, `${language}: the time is drawn`).toBeGreaterThanOrEqual(0);
					const gap = at + shown.indexOf(NBSP);
					expect(
						Math.abs(chars[gap + 1].top - chars[gap - 1].top),
						`${language} ${state} at ${width}px: 「${time}」 split at its space`
					).toBeLessThanOrEqual(6);
				}
				// The sweep is a real one: the line did wrap at some of these widths.
				expect(wrapped).toBeGreaterThan(10);
			});
		}
	}

	it('the same words with a plain space DO start a line with "·" — the binding is what holds it', async () => {
		const plain = line(zh, 'matches', afternoon('zh')).replaceAll(`${NBSP}· `, ' · ');
		const broken = (await sweep(plain)).filter(({ chars }) => lineStarts(chars).includes('·'));
		expect(broken.length).toBeGreaterThan(0);
	});

	/*
	 * PR 3 note 14. In Chinese and Japanese the time still came apart, BETWEEN
	 * the two ideographs of its day period — 「…检查于 下 / 午 2:32」: U+00A0
	 * binds 午 to the digits, nothing bound 下 to 午, and a browser may break
	 * between any two ideographs. It was measured here, at 162–172 px and
	 * 294–304 px of this line's width, and left as a `todo` for the core. The
	 * core's `checked_time` now joins those neighbours (U+2060 WORD JOINER),
	 * so the whole time is one unbreakable unit — and this holds it, at every
	 * width, where the browser actually put each character.
	 */
	for (const [language, catalog, period] of [
		['zh', zh, '下午'],
		['ja', ja, '午後']
	] as const) {
		for (const state of ['matches', 'trusted'] as const) {
			it(`${language} · ${state}: the day period (${period}) and its time stay on one line, at every width`, async () => {
				const time = afternoon(language);
				const shown = drawnForm(time);
				expect(shown.startsWith(period)).toBe(true);
				let wrapped = 0;
				// The time arriving at the head of a line is where a break inside
				// it would be taken: it must be seen to move there whole.
				let headed = 0;
				for (const { width, chars } of await sweep(line(catalog, state, time))) {
					if (lineStarts(chars).length > 0) wrapped += 1;
					const drawn = chars.map((c) => c.char).join('');
					const at = drawn.lastIndexOf(shown);
					expect(at, `${language}: the time is drawn`).toBeGreaterThanOrEqual(0);
					const tops = chars.slice(at, at + shown.length).map((c) => c.top);
					expect(
						Math.max(...tops) - Math.min(...tops),
						`${language} ${state} at ${width}px: 「${shown}」 is on two lines`
					).toBeLessThanOrEqual(6);
					if (at > 0 && chars[at].top - chars[at - 1].top > 6) headed += 1;
				}
				// A real sweep: the line wrapped at many of these widths, and at
				// some of them the break fell right before the time.
				expect(wrapped).toBeGreaterThan(10);
				expect(headed).toBeGreaterThan(0);
			});
		}
	}

	it('zh: the same line WITHOUT the joiner does split 下 from 午 — the joiner is what holds it', async () => {
		const time = afternoon('zh');
		const loose = line(zh, 'matches', drawnForm(time));
		const shown = drawnForm(time);
		const split = (await sweep(loose)).filter(({ chars }) => {
			const drawn = chars.map((c) => c.char).join('');
			const at = drawn.lastIndexOf(shown);
			return at >= 0 && chars[at + 1].top - chars[at].top > 6;
		});
		expect(split.length).toBeGreaterThan(0);
	});

	it('the joiner draws as nothing: no box, no gap — 「下午」 is as wide with it as without', async () => {
		const measure = async (text: string) => {
			const screen = render(IntegrityLine, { props: { line: { text, tone: 'ok' } } });
			// Wide enough for one line: the width measured is the text's own.
			screen.container.style.width = '600px';
			await tick();
			const node = screen.container.querySelector('.text') as HTMLElement;
			const range = document.createRange();
			range.selectNodeContents(node);
			const { width } = range.getBoundingClientRect();
			// Every character the browser gave room to — a missing-glyph box
			// for U+2060 would be one more of them.
			const drawn = drawnLines(screen.container).map((c) => c.char);
			await screen.unmount();
			return { width, drawn };
		};
		for (const language of ['zh', 'ja']) {
			const time = afternoon(language);
			const joined = await measure(time);
			const plain = await measure(drawnForm(time));
			expect(time).toContain(WJ);
			expect(joined.drawn.join(''), language).toBe(drawnForm(time));
			expect(joined.drawn, language).not.toContain(WJ);
			expect(joined.width, language).toBeCloseTo(plain.width, 1);
		}
	});

	it('…and a time with a plain space does split at it', async () => {
		const time = drawnForm(afternoon('zh')).replaceAll(NBSP, ' ');
		const loose = line(zh, 'matches', time);
		let split = 0;
		for (const { chars } of await sweep(loose)) {
			const drawn = chars.map((c) => c.char).join('');
			// A space at a line's end is not drawn, so the time is found by its digits.
			const digits = drawn.lastIndexOf(time.slice(time.indexOf(' ') + 1));
			if (digits > 0 && chars[digits].top - chars[digits - 1].top > 6) split += 1;
		}
		expect(split).toBeGreaterThan(0);
	});
});
