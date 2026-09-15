<script lang="ts">
	/**
	 * A slowly turning globe, drawn as dots on a 2D canvas — continents only, so
	 * the land reads as land and the oceans are simply empty.
	 *
	 * One colour, the page's own ink. Country borders were tried twice, as
	 * hairlines and as a four-colour political map, and cut on 2026-09-15: both
	 * were louder than anything else on the screen, and the screen's job is one
	 * headline and one button.
	 *
	 * Deliberately NOT a WebGL globe library: this is one element on a marketing
	 * page that is prerendered for fifteen locales, and none of them should pay
	 * several hundred kilobytes for it. Which lattice points are land was decided
	 * at build time (scripts/gen-globe-data.mjs), so the page ships a ~1.8 kB
	 * bitmask instead of a world map and the runtime never parses any geography.
	 *
	 * The dots say where land is, and nothing more. They are not wallets: we do
	 * not know where the wallets in the count below were created, and pretending
	 * to would be inventing data.
	 */
	import { LAND_DOTS, LAND_MASK } from './globe-data';
	import { angleFacing, latticePoint, project, type Vec } from './globe-geometry';

	let { size = 380 }: { size?: number } = $props();

	let canvas: HTMLCanvasElement | undefined = $state();

	const SPIN = 0.0022; // radians per frame — one turn in roughly 48 seconds
	/** The Atlantic, so the first glance gets two coastlines and an ocean. */
	const START = angleFacing(-40);

	/**
	 * The land points of the lattice, in unit-sphere coordinates. The lattice
	 * must be built exactly as the generator built it, or the mask would light up
	 * the wrong points and the continents would dissolve into noise.
	 */
	function landPoints(): Vec[] {
		const bits = Uint8Array.from(atob(LAND_MASK), (c) => c.charCodeAt(0));
		const pts: Vec[] = [];
		for (let i = 0; i < LAND_DOTS; i++) {
			if (bits[i >> 3] & (1 << (i & 7))) pts.push(latticePoint(i, LAND_DOTS));
		}
		return pts;
	}

	$effect(() => {
		const el = canvas;
		if (!el) return;
		const ctx = el.getContext('2d');
		if (!ctx) return;

		const points = landPoints();
		const reduced = window.matchMedia('(prefers-reduced-motion: reduce)');

		// The palette is read off the page rather than hardcoded, so the globe
		// follows the theme. It is read from RESOLVED colour properties (`color`
		// and `outline-color`, neither of which paints anything on a canvas)
		// rather than from the custom properties themselves: a custom property
		// computes to its own text — `light-dark(#23211c, #edeae2)` — and only a
		// real colour property resolves that to the rgb() this needs.
		let ink = '0,0,0';
		let sea = '0,0,0';
		function readColours() {
			const s = getComputedStyle(el as HTMLCanvasElement);
			ink = rgb(s.color) || ink;
			sea = rgb(s.outlineColor) || sea;
		}
		function rgb(value: string): string {
			const m = value.match(/-?[\d.]+/g);
			return m && m.length >= 3 ? `${m[0]},${m[1]},${m[2]}` : '';
		}

		let dpr = 1;
		let px = 0;
		function resize() {
			dpr = Math.min(window.devicePixelRatio || 1, 2);
			px = (el as HTMLCanvasElement).clientWidth;
			(el as HTMLCanvasElement).width = px * dpr;
			(el as HTMLCanvasElement).height = px * dpr;
		}

		let angle = START;

		function draw() {
			const c = ctx as CanvasRenderingContext2D;
			const half = px / 2;
			const radius = half * 0.86;
			const scale = px / 380;
			c.setTransform(dpr, 0, 0, dpr, 0, 0);
			c.clearRect(0, 0, px, px);

			// The water. Barely there — enough that the continents sit ON something
			// and the unlit half still reads as a sphere.
			c.beginPath();
			c.arc(half, half, radius, 0, Math.PI * 2);
			c.fillStyle = `rgba(${sea},0.05)`;
			c.fill();

			// A small globe gets half the dots: the lattice is even, so every other
			// point is still an even lattice — and a phone should not spend fifteen
			// hundred fills a frame on decoration.
			const step = px < 260 ? 2 : 1;
			for (let i = 0; i < points.length; i += step) {
				const v = project(points[i], angle);
				// Only the half facing the reader is drawn. Land on the far side of a
				// globe is hidden by the globe, and drawing it through the sphere
				// gives two overlapping Africas.
				if (v.z <= 0.02) continue;
				c.beginPath();
				c.arc(half + v.x * radius, half - v.y * radius, (0.85 + v.z * 0.75) * scale, 0, 7);
				c.fillStyle = `rgba(${ink},${(0.18 + v.z * 0.52).toFixed(3)})`;
				c.fill();
			}
		}

		let frame = 0;
		let running = true;
		function loop() {
			if (!running) return;
			angle += SPIN;
			draw();
			frame = requestAnimationFrame(loop);
		}
		function start() {
			if (frame || !running) return;
			frame = requestAnimationFrame(loop);
		}
		function stop() {
			cancelAnimationFrame(frame);
			frame = 0;
		}

		readColours();
		resize();
		draw();

		// Three reasons to stop spinning: the reader asked for less motion, the
		// tab is in the background, or the globe has scrolled off the screen.
		// None of them should cost a repaint.
		const still = () => reduced.matches;
		const onVisible = () => (document.hidden || still() ? stop() : start());
		const io = new IntersectionObserver(([e]) =>
			e.isIntersecting && !document.hidden && !still() ? start() : stop()
		);
		io.observe(el);

		const ro = new ResizeObserver(() => {
			resize();
			draw();
		});
		ro.observe(el);

		// Two ways the palette can flip under us: the toggle pins a theme on
		// <html>, or the reader is following the OS and the OS changes.
		const repaint = () => {
			readColours();
			draw();
		};
		const themes = new MutationObserver(repaint);
		themes.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['data-theme']
		});
		const dark = window.matchMedia('(prefers-color-scheme: dark)');
		dark.addEventListener('change', repaint);

		document.addEventListener('visibilitychange', onVisible);
		reduced.addEventListener('change', onVisible);

		return () => {
			running = false;
			stop();
			io.disconnect();
			ro.disconnect();
			themes.disconnect();
			dark.removeEventListener('change', repaint);
			document.removeEventListener('visibilitychange', onVisible);
			reduced.removeEventListener('change', onVisible);
		};
	});
</script>

<canvas
	bind:this={canvas}
	class="globe-canvas"
	style="--globe-size-default: {size}px"
	aria-hidden="true"
></canvas>

<style>
	.globe-canvas {
		display: block;
		/* The prop is the default, not the law: a parent can set `--globe-size` —
		   a phone needs a much smaller globe than a desktop — and an inline style
		   would have outranked any stylesheet that tried. */
		width: min(100%, var(--globe-size, var(--globe-size-default)));
		aspect-ratio: 1;
		/* The three colours the script reads back, resolved. None of these
		   properties paints anything on a canvas — they are here purely as the
		   channel the theme travels down. */
		color: var(--text);
		outline: 0 solid var(--text-muted);
	}
</style>
