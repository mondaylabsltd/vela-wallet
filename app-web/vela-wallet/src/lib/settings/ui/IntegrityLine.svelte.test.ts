/**
 * A signing page's integrity line never breaks badly (the 102 device run).
 *
 * Two things were seen on a phone: the time split across lines — 「检查于 下午
 * / 10:07」 — and a line that STARTED with "·". Both are fixed where the words
 * are made: the core's `checked_time` binds its own spaces (U+00A0), and the
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

async function sweep(text: string): Promise<{ width: number; chars: { char: string; top: number }[] }[]> {
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
		expect(afternoon('zh')).toBe(`下午${NBSP}2:32`);
		expect(afternoon('en')).toBe(`2:32${NBSP}PM`);
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
					const at = drawn.lastIndexOf(time);
					expect(at, `${language}: the time is drawn`).toBeGreaterThanOrEqual(0);
					const gap = at + time.indexOf(NBSP);
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
	 * NOT held yet, and not this shell's to fix (a core gap, reported with the
	 * round): in Chinese and Japanese the time can still come apart, BETWEEN
	 * the two ideographs of its day period — 「…检查于 下 / 午 2:32」. U+00A0
	 * binds 午 to the digits; nothing binds 下 to 午, and a browser may break
	 * between any two ideographs. Measured here: zh 「下午 2:32」 on two lines
	 * at 162–172 px and 294–304 px of this line's width. `checked_time` would
	 * need a word joiner (U+2060) inside the period, or the shells a no-wrap
	 * span around `{{time}}`.
	 */
	it.todo('zh / ja: the day period itself (下午, 午後) never splits across lines');

	it('…and a time with a plain space does split at it', async () => {
		const loose = line(zh, 'matches', afternoon('zh').replaceAll(NBSP, ' '));
		const time = afternoon('zh').replaceAll(NBSP, ' ');
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
