<script lang="ts">
	/**
	 * One intro illustration (spec 020).
	 *
	 * Two colours, not `currentColor`: every drawing is line work plus exactly
	 * one accent thing, and the whole point of the slide is which thing that is.
	 * `outline` elements fill with the page background first, so they sit OVER
	 * what is behind them — the compass needle's southern half is the reason.
	 */
	import { INTRO_ART, INTRO_ART_VIEWBOX, type IntroArtId } from '$lib/intro/art';

	interface Props {
		art: IntroArtId;
	}

	let { art }: Props = $props();

	const elements = $derived(INTRO_ART[art]);
	const paint = (role: 'line' | 'accent') =>
		role === 'accent' ? 'var(--color-fg-base)' : 'var(--color-fg-subtle)';
</script>

<!-- Decoration. The slide's headline and body say what it means, so announcing
     the picture too would read the same sentence twice. -->
<svg
	class="art"
	viewBox="0 0 {INTRO_ART_VIEWBOX.width} {INTRO_ART_VIEWBOX.height}"
	role="presentation"
	aria-hidden="true"
	stroke-linecap="round"
	stroke-linejoin="round"
>
	{#each elements as el, i (i)}
		<path
			d={el.d}
			opacity={el.opacity}
			fill={el.mode === 'fill'
				? paint(el.role)
				: el.mode === 'outline'
					? 'var(--color-bg-base)'
					: 'none'}
			stroke={el.mode === 'fill' ? 'none' : paint(el.role)}
			stroke-width={el.width}
		/>
	{/each}
</svg>

<style>
	.art {
		display: block;
		flex: none;
		width: var(--size-introArtW);
		height: var(--size-introArtH);
	}
</style>
