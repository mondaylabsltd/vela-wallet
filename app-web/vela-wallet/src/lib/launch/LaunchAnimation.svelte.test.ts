/**
 * The launch animation in a real browser, on the browser's own animation
 * clock: the Web Animations the component starts are paused and seeked to the
 * shared timeline's sample times, and what is drawn — the mark's transform,
 * each letter's opacity — must be what `launch-timeline.vectors.json` says,
 * the reference iOS, Android and desktop are held to as well.
 *
 * Then the contract around it: every outcome finishes exactly once, any input
 * cuts to the dissolve, and reduced motion shows the finished lockup at once.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import timeline from '../../../../../docs/design/onboarding/launch/launch-timeline.vectors.json';
import { SCENE } from './generated/scene';
import LaunchAnimation from './LaunchAnimation.svelte';

type Case = { ms: number; mark: { x: number; y: number }; letters: number[] };

const frame = () => new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)));

function drawn(container: HTMLElement) {
	const svg = container.querySelector('svg[data-launch-scene]') as SVGSVGElement;
	const mark = svg.querySelector('[data-launch-mark]') as SVGGElement;
	const letters = [...svg.querySelectorAll(':scope > g > path')] as SVGPathElement[];
	return { svg, mark, letters };
}

/** The translation a computed `transform` carries (a matrix or `none`). */
function translation(el: Element): { x: number; y: number } {
	const m = new DOMMatrix(getComputedStyle(el).transform);
	return { x: m.e, y: m.f };
}

const reduceMotion = (on: boolean) => {
	const real = window.matchMedia.bind(window);
	return vi
		.spyOn(window, 'matchMedia')
		.mockImplementation((query: string) =>
			query.includes('prefers-reduced-motion')
				? ({ matches: on, media: query } as MediaQueryList)
				: real(query)
		);
};

afterEach(() => {
	vi.restoreAllMocks();
});

describe('LaunchAnimation — the scene on the browser clock', () => {
	it('draws every sample of the shared timeline', async () => {
		reduceMotion(false);
		const screen = render(LaunchAnimation, { props: { appearance: 'dark' } });
		await frame();
		const { mark, letters } = drawn(screen.container);
		expect(letters).toHaveLength(SCENE.letters.length);
		const animations = [mark, ...letters].map((el) => el.getAnimations()[0]);
		expect(animations.every(Boolean)).toBe(true);
		for (const a of animations) a.pause();

		for (const sample of timeline.cases as Case[]) {
			for (const a of animations) a.currentTime = sample.ms;
			const at = translation(mark);
			expect(at.x, `mark at ${sample.ms} ms`).toBeCloseTo(sample.mark.x, 1);
			expect(at.y, `mark at ${sample.ms} ms`).toBeCloseTo(sample.mark.y, 1);
			for (const [i, letter] of letters.entries()) {
				expect(
					Number(getComputedStyle(letter).opacity),
					`letter ${i} ("${SCENE.letters[i].glyph}") at ${sample.ms} ms`
				).toBeCloseTo(sample.letters[i], 2);
			}
		}
		screen.unmount();
	});

	it('draws each appearance in its own colours, the framing per form factor', async () => {
		reduceMotion(false);
		for (const appearance of ['dark', 'light'] as const) {
			const screen = render(LaunchAnimation, { props: { appearance } });
			const { svg, mark, letters } = drawn(screen.container);
			const fills = [...mark.querySelectorAll('path')].map((p) => p.getAttribute('fill'));
			expect(fills).toEqual(SCENE.mark.paths.map((p) => SCENE.colors[appearance][p.fill]));
			for (const letter of letters) {
				expect(letter.getAttribute('fill')).toBe(SCENE.colors[appearance].wordmark);
			}
			// The test viewport is a phone or a large screen; the viewBox is that
			// form factor's canvas either way.
			const form = svg.dataset.launchScene;
			const canvas = form === 'largeScreen' ? SCENE.largeScreen : SCENE.canvas;
			expect(svg.getAttribute('viewBox')).toBe(`0 0 ${canvas.w} ${canvas.h}`);
			screen.unmount();
		}
	});
});

describe('LaunchAnimation — the contract around it', () => {
	it('plays, holds, dissolves and finishes exactly once', async () => {
		reduceMotion(false);
		const onfinished = vi.fn();
		const progress: number[] = [];
		const screen = render(LaunchAnimation, {
			props: { appearance: 'dark', onfinished, onprogress: (v: number) => progress.push(v) }
		});
		await frame();
		const { mark, letters } = drawn(screen.container);
		// Run the scene to its end at once; the hold and dissolve then follow.
		for (const el of [mark, ...letters]) el.getAnimations()[0].finish();
		await vi.waitFor(() => expect(onfinished).toHaveBeenCalledTimes(1), { timeout: 3000 });
		expect(progress.at(-1)).toBe(1);
		await new Promise((resolve) => setTimeout(resolve, 200));
		expect(onfinished).toHaveBeenCalledTimes(1);
		screen.unmount();
	});

	it('cuts to the dissolve on any input', async () => {
		reduceMotion(false);
		const onfinished = vi.fn();
		const screen = render(LaunchAnimation, { props: { appearance: 'light', onfinished } });
		await frame();
		const started = performance.now();
		window.dispatchEvent(new PointerEvent('pointerdown'));
		await vi.waitFor(() => expect(onfinished).toHaveBeenCalledTimes(1), { timeout: 2000 });
		// The dissolve alone, not the 1.7 s scene and its hold.
		expect(performance.now() - started).toBeLessThan(1200);
		screen.unmount();
	});

	it('under reduced motion shows the finished lockup, still, and moves on without a hold', async () => {
		reduceMotion(true);
		const onfinished = vi.fn();
		const screen = render(LaunchAnimation, { props: { appearance: 'dark', onfinished } });
		await frame();
		const { mark, letters } = drawn(screen.container);
		expect(mark.getAnimations()).toHaveLength(0);
		expect(translation(mark)).toEqual(SCENE.mark.to);
		for (const letter of letters) expect(getComputedStyle(letter).opacity).toBe('1');
		const started = performance.now();
		await vi.waitFor(() => expect(onfinished).toHaveBeenCalledTimes(1), { timeout: 2000 });
		expect(performance.now() - started).toBeLessThan(1200);
		screen.unmount();
	});
});
