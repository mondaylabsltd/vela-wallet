<script lang="ts">
	/**
	 * A payment request the wallet cannot take up as it is (`live-send-lock`):
	 * what is wrong, and — for a network it does not have — the one way on.
	 * Drawn in the sheet or dialog the page raises over the send; its title is
	 * the stop's own.
	 */
	import Button from '$lib/ui/Button.svelte';
	import type { SendLockModel } from '../live-send-lock';

	interface Props {
		lock: SendLockModel;
		/** "Add this network" was pressed — the chain the request names. */
		onadd?: (chainId: number) => void;
	}

	let { lock, onadd }: Props = $props();

	const add = $derived(lock.add);
</script>

<div class="lock" data-testid="send-lock">
	<p class="body">{lock.body}</p>
	{#if add !== undefined}
		<div class="add">
			<Button
				variant="primary"
				shape="rounded"
				disabled={add.busy}
				onclick={() => !add.busy && onadd?.(add.chainId)}
			>
				{add.label}
			</Button>
		</div>
		<!-- The line an add that did not happen is said on, kept from the first
		     frame: its arrival moves nothing in the sheet. -->
		<p class="failed" class:said={lock.failed !== undefined} role="status">
			{lock.failed ?? ' '}
		</p>
	{/if}
</div>

<style>
	.lock {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	p {
		margin: 0;
	}

	.body {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.add {
		display: flex;
	}

	.add > :global(*) {
		flex: 1;
	}

	.failed {
		min-height: 1lh;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-error-base);
		text-align: center;
		visibility: hidden;
	}

	.failed.said {
		visibility: visible;
	}
</style>
