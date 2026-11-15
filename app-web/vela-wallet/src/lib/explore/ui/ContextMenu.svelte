<script lang="ts">
	import Icon from '#lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS, type UtilityIconId } from '#lib/wallet/icons.ts';
	import type { MenuItemModel } from '../model';

	/** The desktop right-click menu (DE2), anchored where the pointer was. */
	interface Props {
		items: MenuItemModel[];
		x: number;
		y: number;
		onpick?: (id: string) => void;
		onclose?: () => void;
	}

	let { items, x, y, onpick, onclose }: Props = $props();
</script>

<svelte:window
	onkeydown={(event: KeyboardEvent) => {
		if (event.key === 'Escape') onclose?.();
	}}
/>

<div class="catcher" role="presentation" onclick={() => onclose?.()}></div>
<menu class="menu" style:left="{x}px" style:top="{y}px">
	{#each items as item (item.id)}
		<li>
			<button type="button" class:danger={item.danger} onclick={() => onpick?.(item.id)}>
				<Icon icon={UTILITY_ICONS[item.icon as UtilityIconId]} size="sm" />
				<span>{item.label}</span>
			</button>
		</li>
	{/each}
</menu>

<style>
	.catcher {
		position: fixed;
		inset: 0;
	}

	.menu {
		position: fixed;
		z-index: 1;
		/* The contacts menus' width (spec 018 D9); these menus are siblings. */
		min-width: var(--layout-contactsMenuW);
		margin: 0;
		padding: var(--space-md);
		list-style: none;
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		box-shadow: var(--shadow-md);
	}

	button {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding: var(--space-md) var(--space-lg);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
		text-align: start;
	}

	button:hover {
		background: var(--color-bg-sunken);
	}

	.danger {
		color: var(--color-error-base);
	}

	li:last-child button {
		border-top: none;
	}
</style>
