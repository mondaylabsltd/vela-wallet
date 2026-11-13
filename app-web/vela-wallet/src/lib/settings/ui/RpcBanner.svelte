<script lang="ts">
	/**
	 * SR1's amber banner: the count of unreachable networks, then one chip per
	 * network with its own 修复 action. Per-chain rather than one global
	 * "fix" button, because the fix IS per chain — a shared button would have
	 * to ask which one first.
	 */
	import type { RpcBannerModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import ChainMark from './ChainMark.svelte';

	interface Props {
		banner: RpcBannerModel;
		onfix?: (id: string) => void;
	}

	let { banner, onfix }: Props = $props();
</script>

<section class="banner">
	<p class="head">
		<Icon icon={UTILITY_ICONS['triangle-alert']} size="md" />
		<span>{banner.text}</span>
	</p>
	<ul>
		{#each banner.chips as chip (chip.id)}
			<li>
				<button type="button" onclick={() => onfix?.(chip.id)}>
					<ChainMark mark={chip.mark} size="sm" />
					<span class="name">{chip.name}</span>
					<span class="action">{chip.action}</span>
				</button>
			</li>
		{/each}
	</ul>
</section>

<style>
	.banner {
		padding: var(--space-xl);
		border: var(--border-hairline) solid var(--color-warning-border);
		border-radius: var(--radius-lg);
		background: var(--color-warning-soft);
	}

	.head {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-warning-base);
	}

	ul {
		list-style: none;
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-md);
		margin: 0;
		padding: 0;
	}

	button {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-inline: var(--space-md);
		padding-block: var(--space-md);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-bg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		cursor: pointer;
	}

	.name {
		color: var(--color-fg-base);
	}

	/* The thing that fixes it: the text's ink, underlined as a link. */
	.action {
		color: var(--color-fg-base);
		text-decoration: underline;
		font-weight: var(--weight-semibold);
	}
</style>
