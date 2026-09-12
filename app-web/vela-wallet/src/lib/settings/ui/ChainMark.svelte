<script lang="ts">
	/**
	 * A chain's circular avatar — one letter over the chain's brand colour,
	 * and, on a live surface, the chain's logo from the data endpoint drawn
	 * over the letter once it has loaded. The letter is never removed: it is
	 * what shows while the bytes are on their way and what stays when the
	 * endpoint has no logo for this chain.
	 *
	 * Fixture data, not a theme token: these colours belong to Ethereum and
	 * BNB, not to Vela's palette, and they must not flip with the theme.
	 */
	import RemoteLogo from '$lib/wallet/ui/RemoteLogo.svelte';
	import type { ChainMarkModel } from '../model';

	interface Props {
		mark: ChainMarkModel;
		size?: 'sm' | 'md';
	}

	let { mark, size = 'md' }: Props = $props();
</script>

<span class="mark {size}" style:background={mark.color} aria-hidden="true">
	{mark.letter}
	<RemoteLogo urls={mark.logoUrl === undefined ? undefined : [mark.logoUrl]} />
</span>

<style>
	.mark {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-full);
		color: var(--color-onAccent);
		font-weight: var(--weight-bold);
		flex-shrink: 0;
	}

	.md {
		width: var(--space-4xl);
		height: var(--space-4xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.sm {
		width: var(--icon-xl);
		height: var(--icon-xl);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
	}
</style>
