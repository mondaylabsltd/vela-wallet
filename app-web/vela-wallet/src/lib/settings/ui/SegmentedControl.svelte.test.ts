/**
 * The theme picker's labels, in a browser (founder, 2026-09-26: 浅色 / 深色 /
 * 跟随系统 were cut short in es and it). Whether a word fits is the browser's
 * to answer, so this renders the real control at the widths it really gets and
 * the largest text size, with the corpus's own words.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { SegmentedModel } from '../model';
import SegmentedControl from './SegmentedControl.svelte';

const theme = (labels: [string, string, string]): SegmentedModel => ({
	label: 'Theme',
	selected: 'auto',
	segments: [
		{ id: 'light', label: labels[0], icon: 'sun' },
		{ id: 'dark', label: labels[1], icon: 'moon' },
		{ id: 'auto', label: labels[2], icon: 'monitor' }
	]
});

const settle = async () => {
	await tick();
	await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
};

async function drawn(labels: [string, string, string], width: string, scale: string) {
	const screen = render(SegmentedControl, { props: { model: theme(labels) } });
	screen.container.style.width = width;
	screen.container.style.setProperty('--text-scale', scale);
	await settle();
	const group = screen.container.querySelector('[role="radiogroup"]') as HTMLElement;
	const buttons = [...group.querySelectorAll<HTMLElement>('button')];
	const spans = buttons.map((b) => b.querySelector('.label') as HTMLElement);
	return { screen, group, buttons, spans };
}

/** Every label whole: inside its box and its segment, one line, never "…". */
function allWhole(buttons: HTMLElement[], spans: HTMLElement[], context: string) {
	for (const [i, span] of spans.entries()) {
		const label = `${span.textContent} ${context}`;
		expect(span.scrollWidth, label).toBeLessThanOrEqual(span.clientWidth + 1);
		expect(getComputedStyle(span).textOverflow, label).not.toBe('ellipsis');
		const [box, segment] = [span.getBoundingClientRect(), buttons[i].getBoundingClientRect()];
		expect(box.left, label).toBeGreaterThanOrEqual(segment.left - 0.5);
		expect(box.right, label).toBeLessThanOrEqual(segment.right + 0.5);
	}
	// Three equal thirds, whatever the words.
	const widths = buttons.map((b) => Math.round(b.getBoundingClientRect().width));
	expect(Math.max(...widths) - Math.min(...widths), context).toBeLessThanOrEqual(1);
}

const size = (el: HTMLElement) => parseFloat(getComputedStyle(el).fontSize);

describe('SegmentedControl — every label whole, in every language', () => {
	// The corpus's words (settings.appearance.theme*), after the founder's
	// one-word "System" — and the longest of the old ones, which still has to
	// fit wherever it is still shipped.
	const LOCALES: Record<string, [string, string, string]> = {
		'es-MX': ['Claro', 'Oscuro', 'Sistema'],
		it: ['Chiaro', 'Scuro', 'Sistema'],
		fr: ['Clair', 'Sombre', 'Système'],
		de: ['Hell', 'Dunkel', 'System'],
		ja: ['ライト', 'ダーク', '自動'],
		ru: ['Светлая', 'Тёмная', 'Авто'],
		vi: ['Sáng', 'Tối', 'Hệ thống'],
		zh: ['浅色', '深色', '跟随系统']
	};

	it('keeps icon and label side by side, full size, where there is room', async () => {
		const { screen, group, buttons, spans } = await drawn(LOCALES.fr, '342px', '1');
		expect(group.classList.contains('stacked')).toBe(false);
		allWhole(buttons, spans, 'fr 342px');
		for (const span of spans) expect(size(span)).toBe(size(buttons[0]));
		screen.unmount();
	});

	// The phone (342 at 390, 272 at 320), the desktop's 280-pixel control
	// column, and the largest text size.
	for (const [locale, labels] of Object.entries(LOCALES)) {
		it(`fits ${locale} whole at every width, at the largest text size`, async () => {
			for (const width of ['342px', '280px', '272px']) {
				for (const scale of ['1', '1.35']) {
					const { screen, buttons, spans } = await drawn(labels, width, scale);
					allWhole(buttons, spans, `${locale} ${width} ×${scale}`);
					// Set smaller, if at all, never below 60% of what was asked.
					for (const span of spans) {
						expect(size(span)).toBeGreaterThanOrEqual(size(buttons[0]) * 0.6 - 0.1);
					}
					// One line each — the control stacks its icons before any label wraps.
					for (const span of spans) {
						const line = parseFloat(getComputedStyle(span).lineHeight);
						expect(
							span.getBoundingClientRect().height,
							`${span.textContent} ${width}`
						).toBeLessThan(line * 1.5);
					}
					screen.unmount();
				}
			}
		});
	}

	it('stacks every icon over its label together, never one segment alone', async () => {
		const { screen, group, buttons } = await drawn(LOCALES.fr, '272px', '1.35');
		expect(group.classList.contains('stacked')).toBe(true);
		for (const button of buttons) expect(getComputedStyle(button).flexDirection).toBe('column');
		screen.unmount();
	});

	it('follows the text size when it changes under it', async () => {
		const { screen, group, buttons, spans } = await drawn(LOCALES.fr, '280px', '1');
		expect(group.classList.contains('stacked')).toBe(false);
		// The document's own text size, the way the Settings slider sets it.
		document.documentElement.style.setProperty('--text-scale', '1.35');
		screen.container.style.removeProperty('--text-scale');
		await settle();
		try {
			allWhole(buttons, spans, 'fr after the slider');
			expect(group.classList.contains('stacked')).toBe(true);
		} finally {
			document.documentElement.style.removeProperty('--text-scale');
			screen.unmount();
		}
	});
});
