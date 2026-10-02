/**
 * The slide's label sits beside its knob and is never cut (spec 089).
 *
 * Centred across the whole track, "Slide to confirm · Confirm send" ran under
 * the knob in the 360 px side panel, and ru's label — two lines there and in
 * the 420 px request window at the largest text size — hid its first letters
 * under it. The label now starts past the knob, wraps, and the track grows.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import SlideToConfirm from './SlideToConfirm.svelte';

let host: HTMLDivElement | null = null;

afterEach(() => {
	host?.remove();
	host = null;
});

async function drawn(width: number, hint: string, action: string, textScale = 1) {
	host = document.createElement('div');
	host.style.cssText = `width:${width}px;--text-scale:${textScale};`;
	document.body.appendChild(host);
	render(SlideToConfirm, { target: host, props: { hint, action, enabled: true } });
	await tick();
	await document.fonts.ready;
	const track = host.querySelector<HTMLElement>('.track')!.getBoundingClientRect();
	const knob = host.querySelector<HTMLElement>('.knob')!.getBoundingClientRect();
	const label = host.querySelector<HTMLElement>('.label')!;
	const range = document.createRange();
	range.selectNodeContents(label);
	const lines = [...range.getClientRects()].filter((r) => r.width > 0);
	return { track, knob, lines };
}

/** The sheet's column in the side panel (360 − 2 × 24) and in the request window (420 − 2 × 24). */
const PANEL = 312;
const WINDOW = 372;

const CASES = [
	{ name: 'en, side panel', width: PANEL, hint: 'Slide to confirm', action: 'Confirm send' },
	{
		name: 'ru, side panel',
		width: PANEL,
		hint: 'Проведите для подтверждения',
		action: 'Подтвердить отправку'
	},
	{
		name: 'ru, request window, largest text',
		width: WINDOW,
		hint: 'Проведите для подтверждения',
		action: 'Подтвердить отправку',
		scale: 1.35
	}
];

describe('the slide label beside its knob (089)', () => {
	for (const c of CASES) {
		it(`${c.name}: no letter under the knob, none outside the track`, async () => {
			const { track, knob, lines } = await drawn(c.width, c.hint, c.action, c.scale);
			expect(lines.length).toBeGreaterThan(0);
			for (const line of lines) {
				expect(line.left, 'clear of the knob').toBeGreaterThanOrEqual(knob.right);
				expect(line.right).toBeLessThanOrEqual(track.right + 0.5);
				expect(line.top).toBeGreaterThanOrEqual(track.top - 0.5);
				expect(line.bottom).toBeLessThanOrEqual(track.bottom + 0.5);
			}
		});
	}

	it('an everyday label at the request window’s width is still one line, the track its own height', async () => {
		const { track, lines } = await drawn(WINDOW, 'Slide to confirm', 'Confirm');
		expect(new Set(lines.map((l) => Math.round(l.top))).size).toBe(1);
		expect(Math.round(track.height)).toBe(56);
	});
});
