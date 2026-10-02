<script lang="ts">
	import type { Tone } from '../model';

	interface Props {
		text: string;
		tone: Tone;
		/**
		 * The sheet's headline rather than the eyebrow over a hero (issue 314):
		 * a request with no figure to lead with — the wallet's own — leads with
		 * what it does.
		 */
		lead?: boolean;
	}

	let { text, tone, lead = false }: Props = $props();
</script>

<p class="intent" class:lead data-tone={tone}>{text}</p>

<style>
	.intent {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		letter-spacing: var(--letterSpacing-sectionLabel);
		color: var(--color-fg-muted);
	}

	/* The headline: the base ink unless its tone is a warning (below). */
	.lead {
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		letter-spacing: normal;
		color: var(--color-fg-base);
	}

	[data-tone='danger'] {
		color: var(--color-error-base);
	}

	[data-tone='caution'] {
		color: var(--color-warning-base);
	}
</style>
