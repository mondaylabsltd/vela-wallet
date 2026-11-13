<script lang="ts">
	/**
	 * A network to pick, as a list on the sheet itself (r4D-4): the likely
	 * ones first and in view without opening or scrolling anything, the rest
	 * one tap away. Picking selects — it never acts: the sheet's own button
	 * does (r4b-8).
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		label: string;
		moreLabel: string;
		/** In the order to offer them: the likely ones first. */
		networks: { chainId: number; displayName: string }[];
		chosen: number | null;
		onchoose: (chainId: number) => void;
		/** How many are shown before "More networks". */
		shown?: number;
		disabled?: boolean;
	}

	let {
		label,
		moreLabel,
		networks,
		chosen,
		onchoose,
		shown = 4,
		disabled = false
	}: Props = $props();

	let all = $state(false);
	const visible = $derived.by(() => {
		if (all || networks.length <= shown + 1) return networks;
		const first = networks.slice(0, shown);
		// The one picked stays in view, wherever it is in the list.
		const pick = networks.find((n) => n.chainId === chosen);
		return pick && !first.includes(pick) ? [...first, pick] : first;
	});
</script>

<div class="choice" role="radiogroup" aria-label={label}>
	{#each visible as n (n.chainId)}
		{@const on = n.chainId === chosen}
		<button
			type="button"
			role="radio"
			aria-checked={on}
			class:on
			{disabled}
			data-chain={n.chainId}
			onclick={() => onchoose(n.chainId)}
		>
			<span class="name">{n.displayName}</span>
			<span class="mark" aria-hidden="true"
				>{#if on}<Icon icon={UTILITY_ICONS.check} size="sm" />{/if}</span
			>
		</button>
	{/each}
	{#if visible.length < networks.length}
		<button type="button" class="more" {disabled} onclick={() => (all = true)}
			>{moreLabel} · {networks.length - visible.length}</button
		>
	{/if}
</div>

<style>
	.choice {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	button {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md);
		min-height: var(--size-control-md);
		padding: var(--space-sm) var(--space-md);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		text-align: start;
		cursor: pointer;
	}

	button.on {
		border-color: var(--color-fg-base);
		background: var(--color-bg-raised);
		font-weight: var(--weight-semibold);
	}

	button:disabled {
		cursor: default;
		opacity: var(--opacity-disabled);
	}

	.mark {
		display: flex;
		flex: none;
		width: var(--icon-sm);
	}

	.more {
		justify-content: center;
		border-style: dashed;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}
</style>
