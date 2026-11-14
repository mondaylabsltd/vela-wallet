<script lang="ts">
	/**
	 * The launch animation, drawn by the browser itself: the scene
	 * (`generated/scene.ts`, from `docs/design/onboarding/launch/launch-scene.json`)
	 * is inline SVG, and the Web Animations API moves it. No animation runtime and
	 * nothing fetched — the lockup is in the DOM the moment this mounts.
	 *
	 * Same shape as the other three platforms: play once → hold the finished
	 * lockup → cross-dissolve into the page, with any input cutting straight to
	 * the dissolve and every outcome ending on the page.
	 *
	 * It is mounted CLIENT-SIDE ONLY. The page is prerendered per locale and must
	 * be complete without this, so nothing here may reach the server output and
	 * it can never be the LCP element.
	 */
	import { onMount } from 'svelte';
	import {
		DURATION_MS,
		EXIT_CROSSFADE_MS,
		HARD_CEILING_MS,
		HOLD_MS,
		boxSize,
		formFactor,
		type Appearance
	} from './constants';
	import { SCENE } from './generated/scene';

	interface Props {
		appearance: Appearance;
		/** Fraction of the cross-dissolve completed, 0…1. The HOST fades the page
		 *  content in by the same fraction over a background that never fades. */
		onprogress?: (value: number) => void;
		/** Fires EXACTLY once, for every outcome. */
		onfinished?: () => void;
	}

	let { appearance, onprogress, onfinished }: Props = $props();

	let mark: SVGGElement;
	const letters: SVGPathElement[] = [];
	let dissolve = $state(0);
	let finished = false;

	// Viewport-driven, and re-evaluated on resize: the framing and the box both
	// follow the window without restarting playback (FR-011).
	let vw = $state(0);
	let vh = $state(0);
	const form = $derived(vw ? formFactor(vw, vh) : 'phone');
	const box = $derived(boxSize(vw || 390, form));
	const canvas = $derived(form === 'largeScreen' ? SCENE.largeScreen : SCENE.canvas);
	const framing = $derived(
		form === 'largeScreen'
			? `translate(${SCENE.largeScreen.offset.x} ${SCENE.largeScreen.offset.y}) scale(${SCENE.largeScreen.scale})`
			: undefined
	);
	const colors = $derived(SCENE.colors[appearance]);

	const at = (p: { x: number; y: number }) => `translate(${p.x}px, ${p.y}px)`;

	const prefersReducedMotion = () =>
		typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

	function finishOnce() {
		if (finished) return;
		finished = true;
		onprogress?.(1);
		onfinished?.();
	}

	function beginExit() {
		if (dissolve > 0 || finished) return;
		const start = performance.now();
		const step = (now: number) => {
			// Both layers move together: the overlay's opacity is 1 − dissolve and
			// the host applies dissolve to the page, over one continuous
			// background. Fading only the backdrop is the bug this shape avoids.
			const t = Math.min((now - start) / EXIT_CROSSFADE_MS, 1);
			dissolve = t;
			onprogress?.(t);
			if (t < 1) requestAnimationFrame(step);
			else finishOnce();
		};
		requestAnimationFrame(step);
	}

	/** The finished lockup, with no motion. */
	function settle() {
		mark.style.transform = at(SCENE.mark.to);
		for (const letter of letters) letter.style.opacity = '1';
	}

	onMount(() => {
		const timers: ReturnType<typeof setTimeout>[] = [];
		let animations: Animation[] = [];

		const measure = () => {
			vw = window.innerWidth;
			vh = window.innerHeight;
		};
		measure();
		window.addEventListener('resize', measure, { passive: true });

		// FR-016: any input cuts to the dissolve.
		const skip = () => beginExit();
		window.addEventListener('pointerdown', skip, { passive: true });
		window.addEventListener('keydown', skip);

		// FR-015: a ceiling on the whole sequence, whatever happens to the clock.
		timers.push(setTimeout(beginExit, HARD_CEILING_MS));

		if (prefersReducedMotion() || typeof mark.animate !== 'function') {
			// FR-019/FR-020: the finished lockup, statically, and NO hold — the
			// point of the setting is less time on motion. A browser without the
			// Web Animations API gets the same.
			settle();
			beginExit();
		} else {
			const timing = (start: number, end: number): KeyframeAnimationOptions => ({
				delay: start,
				duration: end - start,
				easing: SCENE.easing,
				fill: 'both'
			});
			animations = [
				mark.animate(
					[{ transform: at(SCENE.mark.from) }, { transform: at(SCENE.mark.to) }],
					timing(SCENE.mark.startMs, SCENE.mark.endMs)
				),
				...SCENE.letters.map((letter, i) =>
					letters[i].animate([{ opacity: 0 }, { opacity: 1 }], timing(letter.startMs, letter.endMs))
				)
			];
			// Play → hold → dissolve. The tweens end at `settledMs`; the rest of
			// the authored length is the finished lockup, then the hold.
			Promise.all(animations.map((a) => a.finished)).then(
				() => timers.push(setTimeout(beginExit, DURATION_MS - SCENE.settledMs + HOLD_MS)),
				() => {
					// Cancelled: unmounted, nothing to do.
				}
			);
		}

		return () => {
			timers.forEach(clearTimeout);
			for (const animation of animations) animation.cancel();
			window.removeEventListener('resize', measure);
			window.removeEventListener('pointerdown', skip);
			window.removeEventListener('keydown', skip);
		};
	});
</script>

<!--
	Opaque until the dissolve starts — the page must not be visible through it
	(FR-013). `aria-hidden` + `inert`: decoration, and it must not trap focus or
	be announced (FR-021).
-->
<div class="overlay" style:opacity={1 - dissolve} aria-hidden="true" inert>
	<svg
		class="box"
		width={box.w}
		height={box.h}
		viewBox="0 0 {canvas.w} {canvas.h}"
		data-launch-scene={form}
	>
		<g transform={framing}>
			<g bind:this={mark} style:transform={at(SCENE.mark.from)} data-launch-mark>
				{#each SCENE.mark.paths as path, i (i)}
					<path d={path.d} fill={colors[path.fill]} />
				{/each}
			</g>
			{#each SCENE.letters as letter, i (i)}
				<path bind:this={letters[i]} d={letter.d} fill={colors.wordmark} opacity="0" />
			{/each}
		</g>
	</svg>
</div>

<style>
	.overlay {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		/* The page's own identically-coloured background sits underneath, so the
		   backdrop stays continuous through the dissolve instead of washing out
		   to the bare document. */
		background: var(--color-bg-base);
		z-index: 100;
	}

	.box {
		/* The canvas is cropped to the motion, so the box IS the artwork —
		   nothing to letterbox. */
		flex: none;
		overflow: visible;
	}
</style>
