<script lang="ts">
	import type { Tone } from '../model';

	/**
	 * The eyebrow over a hero — "Send", "Approve", "Blind signature". The
	 * wallet's own request, which has no figure to lead with, says its intent
	 * as the header's headline instead (`SigningHeader`), not here.
	 */
	interface Props {
		text: string;
		tone: Tone;
	}

	let { text, tone }: Props = $props();
</script>

<p class="intent" data-tone={tone}>{text}</p>

<style>
	.intent {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		letter-spacing: var(--letterSpacing-sectionLabel);
		color: var(--color-fg-muted);
	}

	[data-tone='danger'] {
		color: var(--color-error-base);
	}

	[data-tone='caution'] {
		color: var(--color-warning-base);
	}
</style>
