<script lang="ts">
	/**
	 * §12: what stands in the review's place while it is not drawn — the
	 * core's door (`review.door`). Being read: a calm "Reading the
	 * transaction…", the sign button shut. Not read: why, that nothing can be
	 * signed until it is, and Try again (the machine's `review_again`). A
	 * review that is read draws nothing here: the review itself is drawn.
	 */
	import Button from '$lib/ui/Button.svelte';
	import type { ReviewStateMessages } from '../messages';
	import { reasonWords, type MultisigSignDoor } from '../core/review-door';

	let {
		m,
		door,
		onRetry
	}: {
		m: ReviewStateMessages;
		door: MultisigSignDoor;
		/** Sends `review_again`. */
		onRetry: () => void;
	} = $props();
</script>

{#if door.shows === 'loading'}
	<p class="reading" role="status" aria-busy="true" data-testid="review-loading">
		{m.loading}
	</p>
{:else if door.shows === 'failed'}
	<div class="failed" role="alert" data-testid="review-failed">
		<p class="why">{reasonWords(m, door.reason_key)}</p>
		<p class="blocked">{m.blocked}</p>
		{#if door.retry}
			<Button variant="secondary" shape="rounded" onclick={onRetry} testid="review-retry"
				>{m.tryAgain}</Button
			>
		{/if}
	</div>
{/if}

<style>
	.reading {
		margin: var(--space-lg) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.failed {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		align-items: flex-start;
		margin: var(--space-lg) 0 0;
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-warning-soft);
		border: var(--border-hairline) solid var(--color-warning-border);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.failed p {
		margin: 0;
		overflow-wrap: anywhere;
	}

	.blocked {
		color: var(--color-fg-muted);
	}
</style>
