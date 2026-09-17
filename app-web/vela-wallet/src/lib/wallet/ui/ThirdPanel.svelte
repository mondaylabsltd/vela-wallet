<script lang="ts">
	import type { Snippet } from 'svelte';
	import { UTILITY_ICONS } from '../icons';
	import Icon from './Icon.svelte';

	interface Props {
		title: string;
		closeLabel: string;
		/**
		 * Spec 021: the panel stacks. Receive opens a network list, a network
		 * opens its QR; Send runs a picker, a form, a confirmation, a receipt.
		 * When there is somewhere to go back TO, the header says so — closing
		 * the whole column is not the same gesture as stepping back one.
		 */
		backLabel?: string;
		onback?: () => void;
		onclose?: () => void;
		/**
		 * Which screen the column is showing (spec 038 #D4). When it changes the
		 * content scrolls back to its top: a confirm opened after a long split
		 * form used to inherit the form's scroll offset and open on its last
		 * rows, hero and count above the fold.
		 */
		scrollKey?: string;
		children: Snippet;
	}

	let { title, closeLabel, backLabel, onback, onclose, scrollKey, children }: Props = $props();

	let content = $state<HTMLDivElement | null>(null);
	$effect(() => {
		void scrollKey;
		if (content) content.scrollTop = 0;
	});

	function onkeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') onclose?.();
	}
</script>

<svelte:window {onkeydown} />

<aside class="panel">
	<header>
		{#if backLabel !== undefined}
			<button type="button" class="back" aria-label={backLabel} onclick={onback}>
				<Icon icon={UTILITY_ICONS['chevron-left']} size="lg" />
			</button>
		{/if}
		<h2>{title}</h2>
		<button type="button" aria-label={closeLabel} onclick={onclose}>
			<Icon icon={UTILITY_ICONS.x} size="lg" />
		</button>
	</header>
	<div class="content" bind:this={content}>
		{@render children()}
	</div>
</aside>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		width: calc(var(--layout-maxContentWidth) / 2);
		flex-shrink: 0;
		height: 100%;
		background: var(--color-bg-base);
		border-inline-start: var(--border-hairline) solid var(--color-border-base);
	}

	header {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-xl) var(--space-3xl);
	}

	/* The title takes the slack, so the close button stays pinned to the
	   trailing edge whether or not a back chevron precedes it. */
	h2 {
		flex: 1;
		min-width: 0;
	}

	.back {
		margin-inline-start: calc(var(--space-lg) * -1);
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	button {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	button:hover {
		background: var(--color-bg-sunken);
		color: var(--color-fg-base);
	}

	.content {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		scrollbar-gutter: stable;
		padding: 0 var(--space-3xl) var(--space-3xl);
	}
</style>
