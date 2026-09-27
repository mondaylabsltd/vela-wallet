<script lang="ts">
	/**
	 * ST15's "将要发送的内容" — a collapsible block. It defaults OPEN,
	 * because the point of the disclosure is that somebody can see what is
	 * about to leave their device before they press send, and a closed box
	 * would be a promise instead of a showing.
	 *
	 * Feedback v2 (A4) took the box away: a quiet muted header with its
	 * chevron, and the caller's rows beneath in the UI face — no sunken
	 * panel, no monospace. The rows own their colours.
	 */
	import { untrack, type Snippet } from 'svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		label: string;
		/** Seed only — the person's later toggling owns the state from then on. */
		initialOpen?: boolean;
		children: Snippet;
	}

	let { label, initialOpen = true, children }: Props = $props();
	// `untrack`: this is a SEED, not a binding. A caller re-rendering with the
	// same prop must not slam the box shut under somebody who just opened it.
	let expanded = $state(untrack(() => initialOpen));
</script>

<section class="disclosure">
	<button type="button" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
		<span>{label}</span>
		<span class="chevron" class:up={expanded}>
			<Icon icon={UTILITY_ICONS['chevron-down']} size="sm" />
		</span>
	</button>
	{#if expanded}
		<div class="body">{@render children()}</div>
	{/if}
</section>

<style>
	button {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		width: 100%;
		min-height: var(--size-control-sm);
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		text-align: start;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	/* The chevron follows the words at their size. */
	.chevron {
		display: flex;
		transition: transform var(--motion-duration-fast) ease-out;
	}

	.chevron :global(svg) {
		width: 1.25em;
		height: 1.25em;
	}

	.chevron.up {
		transform: rotate(180deg);
	}

	.body {
		padding-top: var(--space-sm);
		font-family: var(--font-ui);
	}

	@media (prefers-reduced-motion: reduce) {
		.chevron {
			transition: none;
		}
	}
</style>
