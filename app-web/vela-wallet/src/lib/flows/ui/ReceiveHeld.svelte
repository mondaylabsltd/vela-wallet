<script lang="ts">
	/**
	 * §37 (review 1-7): Receive with no address. The core's
	 * `receive_standing` held it back — an account whose keys only the key
	 * index vouched for ("Not yet confirmed on-chain", with Check again), or
	 * one the chain proved another wallet's ("This isn't your registered
	 * wallet"). In place of every row, code, copy, share and request: the
	 * core's two sentences, "Checking on-chain…" while the chain is asked,
	 * and the way to ask it again.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { ReceiveHeldModel } from '../model';

	interface Props {
		held: ReceiveHeldModel;
		/** Check again: the chain asked once more. Absent in the gallery. */
		oncheckagain?: () => void;
	}

	let { held, oncheckagain }: Props = $props();
</script>

<section class="held" data-testid="receive-held" aria-live="polite">
	<span class="icon" aria-hidden="true"><Icon icon={UTILITY_ICONS.info} size="md" /></span>
	{#if held.title}<h3>{held.title}</h3>{/if}
	{#if held.body}<p class="body">{held.body}</p>{/if}
	{#if held.checking !== undefined}
		<p class="checking" role="status" data-testid="receive-held-checking">{held.checking}</p>
	{:else if held.checkAgain !== undefined}
		<Button variant="secondary" testid="receive-held-check-again" onclick={() => oncheckagain?.()}
			>{held.checkAgain}</Button
		>
	{/if}
</section>

<style>
	.held {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-lg) var(--space-md);
		text-align: center;
	}

	.icon {
		display: flex;
		color: var(--color-warning-base);
	}

	h3 {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.body {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.checking {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
