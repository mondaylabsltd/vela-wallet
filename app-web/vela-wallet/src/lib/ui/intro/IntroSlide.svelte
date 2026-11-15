<script lang="ts">
	/**
	 * One intro slide: headline, illustration, one paragraph (spec 020).
	 *
	 * Pure presentation — it owns no page state, which is what lets the gallery
	 * render all three side by side and the carousel render one at a time from
	 * the same file.
	 */
	import IntroArt from './IntroArt.svelte';
	import type { IntroArtId } from '#lib/intro/art.ts';

	interface Props {
		art: IntroArtId;
		title: string;
		body: string;
	}

	let { art, title, body }: Props = $props();
</script>

<div class="slide">
	<h1 class="title">{title}</h1>
	<IntroArt {art} />
	<p class="body">{body}</p>
</div>

<style>
	/*
	 * The three gaps are measured off the boards rather than picked off the
	 * spacing scale: this screen is a poster, and the air around the drawing is
	 * the composition. `--space-5xl` (48) on both sides reads as a settings row
	 * with a picture in it.
	 */
	.slide {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
	}

	.title {
		margin: 0 0 var(--layout-introGapTitle);
		color: var(--color-fg-base);
		font-size: var(--text-introTitle);
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		letter-spacing: -0.01em;
	}

	.body {
		max-width: var(--layout-introBodyMeasure);
		margin: var(--layout-introGapBody) 0 0;
		color: var(--color-fg-muted);
		font-size: var(--text-introBody);
		line-height: var(--leading-relaxed);
	}
</style>
