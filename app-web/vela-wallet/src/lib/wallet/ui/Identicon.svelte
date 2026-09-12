<script lang="ts">
	/**
	 * The identicon, wherever it is drawn.
	 *
	 * Given `address` — the seed the artwork was drawn from — it is a BUTTON
	 * that opens the app-resident viewer (`identicon-viewer.svelte.ts`): the
	 * artwork big, beside the whole address, which is the only way a person
	 * ever learns to recognise one from the other (founder call, 2026-09-05:
	 * every artwork, not just the header's). Without an address it stays a
	 * picture, which is what a QR centre, a placeholder and the gallery want.
	 *
	 * Rows that are themselves buttons place this BESIDE their button, never
	 * inside it: nested interactive content is invalid HTML and reads as one
	 * control to assistive technology.
	 */
	import { identiconViewer } from '../identicon-viewer.svelte';

	interface Props {
		/** Inline SVG markup produced by vela-core (`identiconSvgCircular`) at
		 *  build time — never composed client-side (research.md D1). */
		svg: string;
		/** Diameter in px steps expressed via the icon token scale.
		 *  `hero`/`detail` are the spec-018 contact-detail avatars (64 mobile / 48 desktop);
		 *  `inline` is a line of text's worth (the signer row). */
		size?: 'inline' | 'row' | 'header' | 'board' | 'hero' | 'detail' | 'viewer';
		label?: string;
		/** The seed. Present ⇒ tapping opens the viewer on it. */
		address?: string;
	}

	let { svg, size = 'header', label, address }: Props = $props();

	const openable = $derived(address !== undefined && address !== '' && svg !== '');
</script>

{#if openable && address !== undefined}
	<button
		type="button"
		class="identicon tap {size}"
		aria-label={identiconViewer.openLabel || label || address}
		onclick={() => identiconViewer.open({ address, identiconSvg: svg })}
	>
		<!-- eslint-disable-next-line svelte/no-at-html-tags -- trusted vela-core output, no user content -->
		{@html svg}
	</button>
{:else}
	<span
		class="identicon {size}"
		role={label === undefined ? 'presentation' : 'img'}
		aria-label={label}
		aria-hidden={label === undefined ? 'true' : undefined}
	>
		<!-- eslint-disable-next-line svelte/no-at-html-tags -- trusted vela-core output, no user content -->
		{@html svg}
	</span>
{/if}

<style>
	.identicon {
		display: block;
		padding: 0;
		border: none;
		border-radius: var(--radius-full);
		overflow: hidden;
		background: var(--color-bg-sunken);
		flex-shrink: 0;
	}

	.identicon :global(svg) {
		display: block;
		width: 100%;
		height: 100%;
	}

	.tap {
		cursor: pointer;
		transition: transform var(--motion-duration-fast) ease-out;
	}

	.tap:active {
		transform: scale(var(--motion-press-row));
	}

	@media (prefers-reduced-motion: reduce) {
		.tap {
			transition: none;
		}
	}

	.inline {
		width: var(--icon-md);
		height: var(--icon-md);
	}

	.row {
		width: var(--icon-2xl);
		height: var(--icon-2xl);
	}

	.header {
		width: calc(var(--space-2xl) * 2);
		height: calc(var(--space-2xl) * 2);
	}

	.board {
		width: var(--size-emptyStateCircle);
		height: var(--size-emptyStateCircle);
	}

	.hero {
		width: var(--size-identiconHero);
		height: var(--size-identiconHero);
	}

	.detail {
		width: var(--size-identiconDetail);
		height: var(--size-identiconDetail);
	}

	.viewer {
		width: var(--size-identiconViewer);
		height: var(--size-identiconViewer);
	}
</style>
