<script module lang="ts">
	/** One image the viewer can show: the PROCESSED JPEG, what will be sent. */
	export interface ViewerImage {
		id: number;
		url: string;
		/** "View screenshot 2" — the dialog's name while this one is on screen. */
		label: string;
	}
</script>

<script lang="ts">
	/**
	 * A screenshot, large (078 §C; founder 2026-09-27: "上传的截图要能点击放大
	 * 预览吧"). What it shows is the PROCESSED image — the JPEG this device
	 * re-encoded, the bytes that will go public — never the file that was
	 * picked, so what the person inspects is exactly what is sent.
	 *
	 * Two containers, one behaviour underneath:
	 *
	 * - **Phone width** — full screen on pure black (both themes). ✕ at the
	 *   trailing edge (web habit, as on Android), a "2 / 5" counter when there
	 *   is more than one, Remove at the bottom in a light-on-dark red. Swipe
	 *   sideways to page; pinch or double-tap to zoom (and drag to pan while
	 *   zoomed); swipe down to dismiss, the black fading as the picture follows
	 *   the finger. The numbers are the bottom sheet's (`viewer-gesture.ts`).
	 * - **Desktop width** — a lightbox, never full screen (founder ruling
	 *   2026-09-05: nothing phone-shaped on a desktop): a dimmed scrim, the
	 *   picture at most ~90% × ~85% of the window, ✕ top-right, the counter,
	 *   ← / → at the sides, Remove under the picture. The scrim closes it.
	 *
	 * Both: Esc closes, ← / → page, focus moves in and Tab stays in. The keys
	 * are taken in the CAPTURE phase and stopped there — the viewer usually
	 * sits inside a bottom sheet, whose own Escape and Tab trap would
	 * otherwise close the sheet or pull focus back into the form behind.
	 *
	 * Removing shows the next image (or the previous when it was the last);
	 * removing the only one closes. Where focus lands afterwards is the host's
	 * to decide (it knows the tiles). Reduced motion keeps the fade and drops
	 * every scale and slide.
	 */
	import { onDestroy, onMount, untrack } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import { BREAKPOINT_DESKTOP } from '$lib/tokens/tokens';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import {
		clampPan,
		clampScale,
		dismissOffset,
		dismissScale,
		doubleTapZoom,
		fitSize,
		isDoubleTap,
		pageAfter,
		pageOffset,
		shouldDismissViewer,
		UNZOOMED,
		VelocityTracker,
		viewerBackdrop,
		viewerTakesDrag,
		zoomAbout,
		ZOOM_EPSILON,
		type Point,
		type Size,
		type Zoom
	} from './viewer-gesture';
	import { tapGuard } from './tap-guard';

	interface Props {
		images: ViewerImage[];
		/** The image the tile opened on. */
		startId: number;
		closeLabel: string;
		removeLabel: string;
		onremove: (id: number) => void;
		/**
		 * After the exit plays — however the viewer was dismissed. `via` says
		 * whether that was the keyboard (focus returns with its ring) or a
		 * pointer (focus returns quietly).
		 */
		onclose: (via: 'keyboard' | 'pointer') => void;
	}

	let { images, startId, closeLabel, removeLabel, onremove, onclose }: Props = $props();

	const desktop = new MediaQuery(`(min-width: ${BREAKPOINT_DESKTOP}px)`, false);

	let currentId = $state(untrack(() => startId));
	const index = $derived(
		Math.max(
			0,
			images.findIndex((image) => image.id === currentId)
		)
	);
	const current = $derived<ViewerImage | undefined>(images[index]);
	const count = $derived(images.length);

	let root = $state<HTMLElement>();
	let stage = $state<HTMLElement>();
	let boxWidth = $state(0);
	let boxHeight = $state(0);
	const box = $derived<Size>({ width: boxWidth, height: boxHeight });
	/** Each picture's own size, once it has loaded — what "fit" is computed from. */
	let naturals = $state<Record<number, Size>>({});

	let zoom = $state<Zoom>(UNZOOMED);
	/** The strip's offset while a finger pages it. */
	let pageDx = $state(0);
	/** The picture's offset while a finger drags it down. */
	let dismissDy = $state(0);
	/** A settle is easing back to rest, to the next page, or out. */
	let easing = $state(false);
	let closing = $state(false);
	/**
	 * The ✕, the counter and Remove step away the moment a swipe down starts
	 * (as the platform viewers do), so they never float over the sheet the
	 * black is fading to; they come back if the picture springs back.
	 */
	let chromeHidden = $state(false);

	let alive = true;
	let easeTimer: ReturnType<typeof setTimeout> | undefined;
	let closeTimer: ReturnType<typeof setTimeout> | undefined;
	const EASE_MS = 260;
	const CLOSE_MS = 180;

	const reducedMotion = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches;

	function fittedOf(id: number | undefined): Size {
		const natural = id === undefined ? undefined : naturals[id];
		return natural === undefined ? box : fitSize(natural, box);
	}
	const fitted = $derived(fittedOf(current?.id));
	const backdrop = $derived(closing ? 0 : viewerBackdrop(dismissDy, boxHeight));

	function ease(): void {
		if (reducedMotion()) {
			easing = false;
			return;
		}
		easing = true;
		clearTimeout(easeTimer);
		easeTimer = setTimeout(() => (easing = false), EASE_MS);
	}

	// --- the one close path ---------------------------------------------------

	let closedVia: 'keyboard' | 'pointer' = 'pointer';

	function finish(): void {
		clearTimeout(closeTimer);
		if (alive) onclose(closedVia);
	}

	/** A click from the keyboard (Enter / Space) carries no pointer. */
	const viaOf = (event?: MouseEvent): 'keyboard' | 'pointer' =>
		event !== undefined && event.detail === 0 ? 'keyboard' : 'pointer';

	/** Close, the way every door does: the exit plays from wherever the picture is, then `onclose`. */
	function close(via: 'keyboard' | 'pointer' = 'pointer'): void {
		if (closing) return;
		closing = true;
		closedVia = via;
		chromeHidden = true;
		if (reducedMotion()) {
			finish();
			return;
		}
		// A picture already on its way down carries on down; otherwise it
		// shrinks back a little as the black fades.
		if (dismissDy > 0) {
			easing = true;
			dismissDy = boxHeight;
		}
		closeTimer = setTimeout(finish, CLOSE_MS);
	}

	// --- paging and removing ----------------------------------------------------

	function go(target: number): void {
		if (target < 0 || target >= count) {
			ease();
			pageDx = 0;
			return;
		}
		const next = images[target];
		if (next.id !== currentId) zoom = UNZOOMED;
		ease();
		currentId = next.id;
		pageDx = 0;
	}

	/** Remove acts on a tap only — never on a drag that happened to end on it. */
	const removeTap = tapGuard();

	function removeCurrent(event: MouseEvent): void {
		if (!removeTap.accept(event)) return;
		if (current === undefined || closing) return;
		const gone = current.id;
		const neighbour = images[index + 1]?.id ?? images[index - 1]?.id;
		onremove(gone);
		if (neighbour === undefined) {
			close(viaOf(event));
			return;
		}
		zoom = UNZOOMED;
		pageDx = 0;
		currentId = neighbour;
	}

	// --- the finger (phone width only) ----------------------------------------

	type Gesture = 'idle' | 'undecided' | 'pan' | 'page' | 'dismiss' | 'none' | 'pinch';
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- bookkeeping for the gesture, never drawn
	const pointers = new Map<number, Point>();
	let gesture: Gesture = 'idle';
	let start = { x: 0, y: 0, t: 0 };
	let startZoom: Zoom = UNZOOMED;
	let pinch = { distance: 1, mid: { x: 0, y: 0 } as Point, zoom: UNZOOMED };
	let lastTap: { t: number; x: number; y: number } | null = null;
	const trackX = new VelocityTracker();
	const trackY = new VelocityTracker();

	/** A screen point as px from the stage's centre. */
	function fromCentre(point: Point): Point {
		const rect = stage?.getBoundingClientRect();
		if (rect === undefined) return { x: 0, y: 0 };
		return { x: point.x - (rect.left + rect.width / 2), y: point.y - (rect.top + rect.height / 2) };
	}

	function beginSingle(point: Point, t: number): void {
		gesture = 'undecided';
		start = { x: point.x, y: point.y, t };
		startZoom = zoom;
		trackX.reset(t, point.x);
		trackY.reset(t, point.y);
	}

	function beginPinch(): void {
		const [a, b] = [...pointers.values()];
		gesture = 'pinch';
		pinch = {
			distance: Math.max(1, Math.hypot(b.x - a.x, b.y - a.y)),
			mid: fromCentre({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }),
			zoom
		};
		// A pinch that began mid-page or mid-dismiss takes over from it.
		pageDx = 0;
		dismissDy = 0;
	}

	function onPointerDown(event: PointerEvent): void {
		if (closing || desktop.current || stage === undefined) return;
		if (event.pointerType === 'mouse' && event.button !== 0) return;
		try {
			stage.setPointerCapture(event.pointerId);
		} catch {
			// the pointer is already gone
		}
		pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
		easing = false;
		if (pointers.size === 1) beginSingle({ x: event.clientX, y: event.clientY }, event.timeStamp);
		else if (pointers.size === 2) beginPinch();
	}

	function onPointerMove(event: PointerEvent): void {
		if (!pointers.has(event.pointerId)) return;
		pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
		if (gesture === 'pinch') {
			if (pointers.size < 2) return;
			const [a, b] = [...pointers.values()];
			const mid = fromCentre({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
			const scale = clampScale(
				(pinch.zoom.scale * Math.hypot(b.x - a.x, b.y - a.y)) / pinch.distance
			);
			// The point of the picture that was under the fingers stays under them.
			const about = zoomAbout(pinch.zoom, scale, pinch.mid);
			zoom = clampPan(
				{ scale, x: about.x + mid.x - pinch.mid.x, y: about.y + mid.y - pinch.mid.y },
				fitted,
				box
			);
			return;
		}
		if (pointers.size !== 1) return;
		const dx = event.clientX - start.x;
		const dy = event.clientY - start.y;
		trackX.add(event.timeStamp, event.clientX);
		trackY.add(event.timeStamp, event.clientY);
		if (gesture === 'undecided') {
			const verdict = viewerTakesDrag({ dx, dy, zoomed: zoom.scale > ZOOM_EPSILON });
			if (verdict === 'undecided') return;
			gesture = verdict;
			if (verdict === 'dismiss') chromeHidden = true;
		}
		if (gesture === 'pan') {
			zoom = clampPan(
				{ scale: startZoom.scale, x: startZoom.x + dx, y: startZoom.y + dy },
				fitted,
				box
			);
		} else if (gesture === 'page') {
			pageDx = pageOffset(dx, index, count, boxWidth);
		} else if (gesture === 'dismiss') {
			dismissDy = dismissOffset(dy, boxHeight);
		}
	}

	function onPointerUp(event: PointerEvent): void {
		if (!pointers.has(event.pointerId)) return;
		pointers.delete(event.pointerId);
		const cancelled = event.type === 'pointercancel';
		if (gesture === 'pinch') {
			if (pointers.size === 1) {
				// One finger stays: it goes on panning a zoomed picture, and does
				// nothing else — a sloppy lift must not page or dismiss.
				const [rest] = [...pointers.values()];
				beginSingle(rest, event.timeStamp);
				gesture = zoom.scale > ZOOM_EPSILON ? 'pan' : 'none';
			} else if (pointers.size === 0) {
				gesture = 'idle';
				if (zoom.scale <= ZOOM_EPSILON) {
					ease();
					zoom = UNZOOMED;
				}
			}
			return;
		}
		if (pointers.size > 0) return;
		const ended = gesture;
		gesture = 'idle';
		if (ended === 'undecided') {
			if (!cancelled) tapped(event);
		} else if (ended === 'page') {
			go(
				cancelled
					? index
					: pageAfter({
							index,
							count,
							offset: pageDx,
							width: boxWidth,
							velocity: trackX.velocity()
						})
			);
		} else if (ended === 'dismiss') {
			const away =
				!cancelled &&
				shouldDismissViewer({ offset: dismissDy, height: boxHeight, velocity: trackY.velocity() });
			if (away) close('pointer');
			else {
				ease();
				dismissDy = 0;
				chromeHidden = false;
			}
		}
	}

	/** A tap that was not a drag: the second of two quick ones toggles the zoom. */
	function tapped(event: PointerEvent): void {
		const now = { t: event.timeStamp, x: event.clientX, y: event.clientY };
		if (now.t - start.t > 400) {
			lastTap = null;
			return;
		}
		if (isDoubleTap(lastTap, now)) {
			lastTap = null;
			ease();
			zoom = doubleTapZoom(zoom, fromCentre(now), fitted, box);
		} else {
			lastTap = now;
		}
	}

	// --- keys, focus ----------------------------------------------------------

	function focusables(): HTMLElement[] {
		if (root === undefined) return [];
		return Array.from(root.querySelectorAll<HTMLElement>('button:not([disabled])')).filter(
			(el) => el.offsetParent !== null
		);
	}

	function onKeydown(event: KeyboardEvent): void {
		if (root === undefined || event.isComposing) return;
		const stop = () => {
			event.preventDefault();
			event.stopPropagation();
		};
		switch (event.key) {
			case 'Escape':
				stop();
				close('keyboard');
				return;
			case 'ArrowLeft':
				stop();
				go(index - 1);
				return;
			case 'ArrowRight':
				stop();
				go(index + 1);
				return;
			case 'Tab': {
				stop();
				const items = focusables();
				if (items.length === 0) {
					root.focus({ preventScroll: true });
					return;
				}
				const at = items.indexOf(document.activeElement as HTMLElement);
				const next = event.shiftKey
					? at <= 0
						? items.length - 1
						: at - 1
					: at === -1 || at === items.length - 1
						? 0
						: at + 1;
				items[next].focus();
				return;
			}
		}
	}

	onMount(() => {
		// Capture phase, stopped: see the module doc.
		window.addEventListener('keydown', onKeydown, true);
		root?.focus({ preventScroll: true });
		// A touch that starts here is the viewer's: the sheet under it must not
		// read it as a pull on its content.
		const node = root;
		const own = (event: TouchEvent) => event.stopPropagation();
		node?.addEventListener('touchstart', own, { passive: true });
		node?.addEventListener('touchmove', own, { passive: true });
		return () => {
			window.removeEventListener('keydown', onKeydown, true);
			node?.removeEventListener('touchstart', own);
			node?.removeEventListener('touchmove', own);
		};
	});

	// The stage's finger, listened for natively: it is a surface for gestures
	// whose keyboard equivalents are the buttons and the arrow keys, not a
	// control of its own.
	$effect(() => {
		const node = stage;
		if (node === undefined) return;
		node.addEventListener('pointerdown', onPointerDown);
		node.addEventListener('pointermove', onPointerMove);
		node.addEventListener('pointerup', onPointerUp);
		node.addEventListener('pointercancel', onPointerUp);
		return () => {
			node.removeEventListener('pointerdown', onPointerDown);
			node.removeEventListener('pointermove', onPointerMove);
			node.removeEventListener('pointerup', onPointerUp);
			node.removeEventListener('pointercancel', onPointerUp);
		};
	});

	onDestroy(() => {
		alive = false;
		clearTimeout(easeTimer);
		clearTimeout(closeTimer);
	});

	function loaded(id: number, event: Event): void {
		const img = event.currentTarget as HTMLImageElement;
		if (img.naturalWidth > 0) naturals[id] = { width: img.naturalWidth, height: img.naturalHeight };
	}

	/**
	 * The picture on screen: its zoom, the dismiss drag's slide and shrink,
	 * and — closing any other way — a small shrink as the black fades.
	 */
	const pictureTransform = $derived.by(() => {
		const shrink = closing && dismissDy <= 0 ? 0.96 : dismissScale(dismissDy, boxHeight);
		return `translate(${zoom.x}px, ${zoom.y + dismissDy}px) scale(${zoom.scale * shrink})`;
	});
</script>

<div
	class="viewer"
	class:lightbox={desktop.current}
	class:phone={!desktop.current}
	class:closing
	class:easing
	role="dialog"
	aria-modal="true"
	aria-label={current?.label}
	tabindex="-1"
	data-focus-inner
	bind:this={root}
>
	{#if desktop.current}
		<!-- The lightbox: the window behind stays in view, dimmed, and a click on
		     it closes — a desktop dialog, not a phone screen. -->
		<div class="scrim" role="presentation" onclick={() => close('pointer')}></div>
		<button
			type="button"
			class="chrome close"
			aria-label={closeLabel}
			onclick={(event) => close(viaOf(event))}
		>
			<Icon icon={UTILITY_ICONS.x} size="lg" />
		</button>
		{#if index > 0}
			<button
				type="button"
				class="chrome nav prev"
				aria-label={images[index - 1].label}
				onclick={() => go(index - 1)}
			>
				<Icon icon={UTILITY_ICONS['chevron-left']} size="lg" />
			</button>
		{/if}
		{#if index < count - 1}
			<button
				type="button"
				class="chrome nav next"
				aria-label={images[index + 1].label}
				onclick={() => go(index + 1)}
			>
				<Icon icon={UTILITY_ICONS['chevron-right']} size="lg" />
			</button>
		{/if}
		{#if current !== undefined}
			<figure class="frame">
				<!-- The counter rides with the picture, inside the frame — never out
				     on the dimmed page where it met the panel's heading. -->
				{#if count > 1}
					<p class="counter" aria-hidden="true">{index + 1} / {count}</p>
				{/if}
				{#key current.id}
					<img class="picture" src={current.url} alt="" draggable="false" />
				{/key}
				<button
					type="button"
					class="remove-text"
					onpointerdown={removeTap.down}
					onpointermove={removeTap.move}
					onpointercancel={removeTap.cancel}
					onclick={removeCurrent}>{removeLabel}</button
				>
			</figure>
		{/if}
	{:else}
		<div class="ground" style:opacity={backdrop}></div>
		<div class="stage" bind:this={stage} bind:clientWidth={boxWidth} bind:clientHeight={boxHeight}>
			<div class="strip" style:transform={`translateX(calc(${-index * 100}% + ${pageDx}px))`}>
				{#each images as image, i (image.id)}
					{@const size = fittedOf(image.id)}
					<div class="page" aria-hidden={i === index ? undefined : 'true'}>
						<img
							src={image.url}
							alt=""
							draggable="false"
							style:width={naturals[image.id] ? `${size.width}px` : undefined}
							style:height={naturals[image.id] ? `${size.height}px` : undefined}
							style:transform={i === index ? pictureTransform : undefined}
							onload={(event) => loaded(image.id, event)}
						/>
					</div>
				{/each}
			</div>
		</div>
		<div class="bar top" class:hidden={chromeHidden}>
			{#if count > 1}
				<p class="counter" aria-hidden="true">{index + 1} / {count}</p>
			{/if}
			<button
				type="button"
				class="chrome close"
				aria-label={closeLabel}
				onclick={(event) => close(viaOf(event))}
			>
				<Icon icon={UTILITY_ICONS.x} size="lg" />
			</button>
		</div>
		<div class="bar bottom" class:hidden={chromeHidden}>
			<button
				type="button"
				class="remove-text"
				onpointerdown={removeTap.down}
				onpointermove={removeTap.move}
				onpointercancel={removeTap.cancel}
				onclick={removeCurrent}>{removeLabel}</button
			>
		</div>
	{/if}
</div>

<style>
	.viewer {
		/* The height of each bar's own row: one 44 control and its air. */
		--viewer-bar: calc(var(--size-control-md) + var(--space-lg));
		position: fixed;
		inset: 0;
		/* Above every sheet and dialog it can be opened from. */
		z-index: 40;
		outline: none;
		touch-action: none;
		color: var(--color-onViewer);
		animation: viewer-in var(--motion-duration-fast) ease-out;
		transition: opacity var(--motion-duration-fast) ease-in;
	}

	.viewer.closing {
		opacity: 0;
		pointer-events: none;
	}

	/* --- phone: full screen, pure black ------------------------------------- */

	.ground {
		position: absolute;
		inset: 0;
		background: var(--color-viewerInk);
	}

	/* The pictures fit BETWEEN the two bars, on the black. A phone screenshot
	   is the screen's own shape: laid full-bleed it read as the app itself,
	   and a light one swallowed the white ✕ and counter. Letterboxed, the
	   whole picture is in view with nothing over it, and the chrome is always
	   on black. (Zoomed, it fills this area and pans inside it.) */
	.stage {
		position: absolute;
		inset: calc(env(safe-area-inset-top, 0%) + var(--viewer-bar)) env(safe-area-inset-right, 0%)
			calc(env(safe-area-inset-bottom, 0%) + var(--viewer-bar) + var(--space-lg))
			env(safe-area-inset-left, 0%);
		overflow: hidden;
		cursor: grab;
	}

	.strip {
		display: flex;
		width: 100%;
		height: 100%;
	}

	.page {
		display: flex;
		flex: 0 0 100%;
		align-items: center;
		justify-content: center;
		min-width: 0;
		height: 100%;
	}

	.page img {
		display: block;
		max-width: 100%;
		max-height: 100%;
		object-fit: contain;
		user-select: none;
		-webkit-user-drag: none;
		animation: picture-in var(--motion-duration-fast) ease-out;
	}

	.easing .strip,
	.easing .page img {
		transition: transform var(--motion-sheet-drag) ease-out;
	}

	.phone.closing .page img {
		transition: transform var(--motion-duration-fast) ease-in;
	}

	.bar {
		position: absolute;
		inset-inline: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		padding-inline: var(--space-md);
		pointer-events: none;
	}

	.bar > * {
		pointer-events: auto;
	}

	.bar {
		transition: opacity var(--motion-hover) ease-out;
	}

	.bar.hidden {
		opacity: 0;
	}

	.bar.hidden > * {
		pointer-events: none;
	}

	.bar.top {
		top: 0;
		box-sizing: border-box;
		height: calc(var(--viewer-bar) + env(safe-area-inset-top, 0%));
		padding-top: env(safe-area-inset-top, 0%);
	}

	.bar.bottom {
		bottom: 0;
		padding-bottom: calc(var(--space-lg) + env(safe-area-inset-bottom, 0%));
	}

	/* ✕ at the trailing edge — the web's (and Android's) habit — centred on
	   the counter's line. */
	.bar.top .close {
		position: absolute;
		top: calc(env(safe-area-inset-top, 0%) + var(--space-lg) / 2);
		inset-inline-end: var(--space-sm);
	}

	.counter {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
		color: color-mix(in srgb, var(--color-onViewer) 70%, transparent);
	}

	.chrome {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-md);
		height: var(--size-control-md);
		padding: 0;
		border: none;
		border-radius: var(--radius-full);
		background: none;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-onViewer);
		cursor: pointer;
		transition: background var(--motion-hover) ease-out;
	}

	/* 20 at the standard size, growing with the text (v2 A8). */
	.chrome :global(svg) {
		width: 1.25em;
		height: 1.25em;
	}

	.chrome:active {
		transform: scale(var(--motion-press-button));
	}

	/* Remove: a text button in the danger colour, the light-on-dark one in
	   both themes (the light theme's red fails on black). */
	.remove-text {
		min-height: var(--size-control-md);
		padding-inline: var(--space-xl);
		border: none;
		border-radius: var(--radius-full);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-viewerDanger);
		cursor: pointer;
		transition: background var(--motion-hover) ease-out;
	}

	.remove-text:active {
		transform: scale(var(--motion-press-button));
	}

	@media (hover: hover) {
		.chrome:hover,
		.remove-text:hover {
			background: color-mix(in srgb, var(--color-onViewer) 14%, transparent);
		}
	}

	/* The focus ring, visible on black in both themes. */
	.chrome:focus-visible,
	.remove-text:focus-visible {
		box-shadow:
			0 0 0 var(--space-xs) var(--color-viewerInk),
			0 0 0 var(--space-sm) var(--color-fixed-focusRingOuter);
		border-radius: var(--radius-full);
	}

	/* --- desktop: a lightbox --------------------------------------------------- */

	.scrim {
		position: absolute;
		inset: 0;
		background: color-mix(in srgb, var(--color-viewerInk) 92%, transparent);
		/* The page under it dissolves rather than reads: at 92% its headings
		   still showed, faintly, around the counter. */
		backdrop-filter: blur(var(--space-md));
	}

	.lightbox .counter {
		pointer-events: none;
	}

	.lightbox .close {
		position: absolute;
		top: var(--space-xl);
		inset-inline-end: var(--space-xl);
	}

	.nav {
		position: absolute;
		top: 50%;
		translate: 0 -50%;
		background: color-mix(in srgb, var(--color-onViewer) 10%, transparent);
	}

	.nav.prev {
		inset-inline-start: var(--space-2xl);
	}

	.nav.next {
		inset-inline-end: var(--space-2xl);
	}

	/* The picture and its Remove, centred; clicks around them reach the scrim. */
	.frame {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-lg);
		margin: 0;
		pointer-events: none;
	}

	.frame > * {
		pointer-events: auto;
	}

	.picture {
		display: block;
		max-width: 90vw;
		/* The frame's other rows — the counter above, Remove below — come out
		   of the 85%, so the whole frame fits it. */
		max-height: calc(
			85dvh - var(--size-control-md) - 2 * var(--space-lg) - var(--text-base) *
				var(--text-scale, 1) * var(--leading-normal)
		);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-lg);
		object-fit: contain;
		user-select: none;
		animation: picture-in var(--motion-duration-fast) ease-out;
	}

	.lightbox.closing .picture {
		transform: scale(0.96);
		transition: transform var(--motion-duration-fast) ease-in;
	}

	/* --- motion ----------------------------------------------------------------- */

	@keyframes viewer-in {
		from {
			opacity: 0;
		}
	}

	@keyframes picture-in {
		from {
			transform: scale(0.96);
		}
	}

	/* Reduced motion: the fade stays, every scale and slide goes. (A zoom is
	   the person's own doing and stays — it just does not animate.) */
	@media (prefers-reduced-motion: reduce) {
		.page img,
		.picture {
			animation: none;
		}

		.easing .strip,
		.easing .page img,
		.phone.closing .page img,
		.lightbox.closing .picture {
			transition: none;
		}

		.lightbox.closing .picture {
			transform: none;
		}
	}
</style>
