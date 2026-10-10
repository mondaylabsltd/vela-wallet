<script lang="ts">
	import type { Snippet } from 'svelte';
	/**
	 * The scanner (spec 021 component 27) — S1 full-screen on the phone, DS1L
	 * as a centred modal on the desktop.
	 *
	 * The camera feed is out of scope here, so the frame holds an inert
	 * surface. What IS in scope is the frame itself: four corner brackets and
	 * nothing else, so the thing being aimed at stays visible. A full border
	 * would compete with the code inside it.
	 */
	import { UTILITY_ICONS, type UtilityIconId } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { PLACEHOLDER_MODULES, PLACEHOLDER_SEED, qrPattern } from '$lib/wallet/qr-pattern';
	import type { ScanModel } from '../model';

	interface Props {
		model: ScanModel;
		/** Desktop draws a titled modal; the phone goes edge to edge. */
		variant?: 'screen' | 'modal';
		/**
		 * What fills the frame (spec 028 T422). Absent = the FIXTURE frame a
		 * board draws — a sample code in the viewfinder, a picture and nothing
		 * else (PR 3 note 7: a board never opens a camera); a live screen passes
		 * a `<video>`. The component stays pure either way — it owns no camera
		 * and knows of none.
		 */
		feed?: Snippet;
		/**
		 * Why there is nothing to look at, when there is nothing to look at.
		 * A viewfinder that just stays black tells a person their camera is
		 * broken; the truth is usually a refusal they can undo.
		 */
		notice?: string;
		onclose?: () => void;
		ontool?: (id: 'gallery' | 'torch' | 'flip') => void;
	}

	let { model, variant = 'screen', feed, notice, onclose, ontool }: Props = $props();

	const GLYPHS: Record<'gallery' | 'torch' | 'flip', UtilityIconId> = {
		gallery: 'image',
		torch: 'zap',
		flip: 'rotate-ccw'
	};

	/**
	 * The sample code in the fixture frame: the galleries' own placeholder
	 * pattern (`qr-pattern.ts`) — three finder squares and seeded noise, the
	 * same on every run, and plainly not a code that encodes anything.
	 */
	const SAMPLE = qrPattern(PLACEHOLDER_MODULES, PLACEHOLDER_SEED);
</script>

<div class="scan {variant}">
	{#if variant === 'modal'}
		<header>
			<h2>{model.title}</h2>
			<button type="button" class="close" aria-label={model.closeLabel} onclick={onclose}>
				<Icon icon={UTILITY_ICONS.x} size="lg" />
			</button>
		</header>
	{:else}
		<div class="bar">
			<button type="button" class="close round" aria-label={model.closeLabel} onclick={onclose}>
				<Icon icon={UTILITY_ICONS.x} size="lg" />
			</button>
		</div>
	{/if}

	<div class="frame">
		{#if feed}
			{@render feed()}
		{:else}
			<!-- The fixture frame: what a board shows where a camera would look. -->
			<span class="feed" aria-hidden="true" data-testid="scan-fixture-frame">
				<svg
					class="sample"
					viewBox="0 0 {PLACEHOLDER_MODULES} {PLACEHOLDER_MODULES}"
					shape-rendering="crispEdges"
				>
					{#each SAMPLE as row, r (r)}
						{#each row as on, c (c)}
							{#if on}<rect x={c} y={r} width="1" height="1" />{/if}
						{/each}
					{/each}
				</svg>
			</span>
		{/if}
		<span class="corner tl" aria-hidden="true"></span>
		<span class="corner tr" aria-hidden="true"></span>
		<span class="corner bl" aria-hidden="true"></span>
		<span class="corner br" aria-hidden="true"></span>
	</div>

	<p class="hint">{notice ?? model.hint}</p>

	<div class="tools">
		{#each model.tools as tool (tool.id)}
			<button type="button" class="tool" onclick={() => ontool?.(tool.id)}>
				<span class="disc"><Icon icon={UTILITY_ICONS[GLYPHS[tool.id]]} size="md" /></span>
				<span class="tool-label">{tool.label}</span>
			</button>
		{/each}
	</div>
</div>

<style>
	.scan {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--color-bg-base);
	}

	.modal {
		height: auto;
		border-radius: var(--radius-2xl);
		padding: 0 var(--space-2xl) var(--space-2xl);
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: var(--space-xl);
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.bar {
		display: flex;
		justify-content: flex-end;
		padding: var(--space-xl) var(--layout-screenPaddingX);
	}

	.close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		border: none;
		background: none;
		border-radius: var(--radius-full);
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.round {
		background: var(--color-bg-raised);
	}

	.frame {
		position: relative;
		align-self: center;
		width: min(68%, var(--size-qrCard));
		aspect-ratio: 1;
		/* The phone lays the frame out optically rather than dead-centre: the
		   tool row below it is heavier than the close button above. */
		margin-block: auto;
	}

	/* The desktop viewfinder is landscape, not square — measured 3:2 in DS1L,
	   which is roughly what a webcam actually hands you. The phone keeps its
	   square, which is what a rear camera aimed at a code wants. */
	.modal .frame {
		width: 100%;
		aspect-ratio: 3 / 2;
		margin-block: 0;
	}

	.feed {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		border-radius: var(--radius-md);
		background: var(--color-bg-sunken);
	}

	/* The sample code, sitting inside the brackets with room around it — a
	   code held up to a viewfinder, drawn quietly: it is a picture of a scan,
	   not a code to scan. */
	.sample {
		width: 56%;
		aspect-ratio: 1;
		fill: var(--color-fg-muted);
		opacity: var(--opacity-dim);
	}

	/* The desktop's viewfinder is landscape: size the code by its height. */
	.modal .sample {
		width: auto;
		height: 64%;
	}

	/* Brackets, not a border: the frame aims the camera, and a closed
	   rectangle around a QR code competes with the code's own quiet zone. */
	.corner {
		position: absolute;
		width: var(--icon-2xl);
		height: var(--icon-2xl);
		border: var(--border-emphasis) solid var(--color-fg-base);
	}

	.tl {
		top: 0;
		inset-inline-start: 0;
		border-inline-end: none;
		border-block-end: none;
		border-start-start-radius: var(--radius-md);
	}

	.tr {
		top: 0;
		inset-inline-end: 0;
		border-inline-start: none;
		border-block-end: none;
		border-start-end-radius: var(--radius-md);
	}

	.bl {
		bottom: 0;
		inset-inline-start: 0;
		border-inline-end: none;
		border-block-start: none;
		border-end-start-radius: var(--radius-md);
	}

	.br {
		bottom: 0;
		inset-inline-end: 0;
		border-inline-start: none;
		border-block-start: none;
		border-end-end-radius: var(--radius-md);
	}

	.hint {
		margin: 0;
		padding-block: var(--space-xl);
		text-align: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.modal .hint {
		text-align: start;
		padding-block: var(--space-lg);
	}

	.tools {
		display: flex;
		justify-content: center;
		gap: var(--space-3xl);
		padding-block: var(--space-3xl) var(--space-5xl);
	}

	.modal .tools {
		gap: var(--space-md);
		padding-block: 0;
	}

	.tool {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-sm);
		border: none;
		background: none;
		font-family: var(--font-ui);
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.modal .tool {
		flex: 1;
		flex-direction: row;
		justify-content: center;
		padding-block: var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		color: var(--color-fg-base);
	}

	.disc {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-md);
		height: var(--size-control-md);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
	}

	.modal .disc {
		display: none;
	}

	.tool-label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}
</style>
