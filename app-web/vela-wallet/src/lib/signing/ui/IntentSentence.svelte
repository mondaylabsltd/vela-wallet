<script lang="ts">
	import type { Tone } from '../model';

	interface Props {
		text: string;
		tone: Tone;
	}

	let { text, tone }: Props = $props();
</script>

<p class="sentence" data-tone={tone}>{text}</p>

<style>
	.sentence {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
	}

	/* The plain-language line is the one thing a hurried person reads, so it
	   carries the tone of the transaction rather than staying neutral. */
	[data-tone='accent'] {
		color: var(--color-fg-base);
	}

	[data-tone='danger'] {
		color: var(--color-error-base);
	}

	[data-tone='caution'] {
		color: var(--color-warning-base);
	}
</style>
