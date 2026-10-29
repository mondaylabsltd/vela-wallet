<script lang="ts">
	/**
	 * The pushed sub-page's header: back arrow, title, optional second line
	 * (ST9/ST9b/ST10/ST12/ST13/ST14). The desktop panels reuse the title
	 * pair without the arrow, which is what `back={false}` is for.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		title: string;
		subtitle?: string;
		back?: boolean;
		backLabel?: string;
		onback?: () => void;
	}

	let { title, subtitle, back = true, backLabel, onback }: Props = $props();
</script>

<header class="nav">
	{#if back}
		<button type="button" class="back" aria-label={backLabel} onclick={onback}>
			<Icon icon={UTILITY_ICONS['chevron-left']} size="xl" />
		</button>
	{/if}
	<div class="titles">
		<h1>{title}</h1>
		{#if subtitle !== undefined}
			<p>{subtitle}</p>
		{/if}
	</div>
</header>

<style>
	.nav {
		display: flex;
		align-items: flex-start;
		gap: var(--space-md);
		padding-block: var(--space-xl) var(--space-lg);
	}

	.back {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-hitTarget);
		height: var(--size-hitTarget);
		margin-inline-start: calc(-1 * var(--space-lg));
		border: none;
		background: none;
		color: var(--color-fg-base);
		cursor: pointer;
		flex-shrink: 0;
	}

	.titles {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 0;
		/* Optical: the arrow is centred in a 44 box, the title is not. */
		padding-block-start: var(--space-md);
	}

	h1 {
		margin: 0;
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	p {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
