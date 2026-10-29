<script lang="ts">
	/**
	 * The side panel while its window has Vela open in a tab — one wallet per
	 * window (`one-surface.svelte.ts`). Instead of a second live wallet that
	 * could say something else, the panel says where the wallet is, and offers
	 * the two ways on: go to that tab (the panel closes where Chrome lets it),
	 * or use the panel instead (the tab closes).
	 *
	 * The wallet stays mounted beneath, inert: a send half-filled in the panel
	 * is where the person left it if they come back, and a dApp's request
	 * still rises over it (the layout lifts this while one is owed).
	 */
	import Button from '$lib/ui/Button.svelte';
	import type { ExtensionMessages } from './messages';

	interface Props {
		messages: ExtensionMessages;
		ongototab: () => void;
		onusehere: () => void;
	}

	let { messages, ongototab, onusehere }: Props = $props();
</script>

<section class="aside" aria-labelledby="open-in-tab-title" data-testid="panel-open-in-tab">
	<div class="card">
		<h1 id="open-in-tab-title">{messages.openInTabTitle}</h1>
		<p>{messages.openInTabBody}</p>
		<div class="actions">
			<Button variant="primary" shape="rounded" onclick={ongototab}>{messages.goToTab}</Button>
			<Button variant="secondary" shape="rounded" onclick={onusehere}>
				{messages.useHereInstead}
			</Button>
		</div>
	</div>
</section>

<style>
	.aside {
		position: fixed;
		inset: 0;
		z-index: 65;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--layout-screenPaddingX);
		background: var(--color-bg-base);
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		max-inline-size: 100%;
		text-align: center;
	}

	h1 {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	p {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		margin-block-start: var(--space-md);
	}
</style>
