<script lang="ts">
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { ConfirmModel, MenuModel } from '../model';

	/**
	 * The mobile menu modality (C5 / C6) and the destructive confirmation (c2s),
	 * both hosted by the reused spec-015 BottomSheet — one component, three
	 * contents (FR-007).
	 */
	interface Props {
		menu?: MenuModel;
		confirm?: ConfirmModel;
		onselect?: (label: string) => void;
		onclose?: () => void;
	}

	let { menu, confirm, onselect, onclose }: Props = $props();
</script>

{#if menu !== undefined}
	<BottomSheet title={menu.label} hideTitle {onclose}>
		<div class="rows" role="menu" aria-label={menu.label}>
			{#each menu.items as item (item.label)}
				<button
					type="button"
					role="menuitem"
					class:destructive={item.destructive}
					class:divider={item.dividerAfter}
					onclick={() => {
						onselect?.(item.label);
						onclose?.();
					}}
				>
					<Icon icon={UTILITY_ICONS[item.icon]} size="lg" />
					<span>{item.label}</span>
				</button>
			{/each}
		</div>
		{#if menu.cancel !== undefined}
			<button type="button" class="cancel" onclick={onclose}>{menu.cancel}</button>
		{/if}
	</BottomSheet>
{:else if confirm !== undefined}
	<BottomSheet title={confirm.title} {onclose}>
		<p class="body">{confirm.body}</p>
		<button
			type="button"
			class="cancel destructive-cta"
			onclick={() => {
				onselect?.(confirm.confirm);
				onclose?.();
			}}
		>
			{confirm.confirm}
		</button>
		<button type="button" class="cancel" onclick={onclose}>{confirm.cancel}</button>
	</BottomSheet>
{/if}

<style>
	.rows {
		display: flex;
		flex-direction: column;
		padding-block: var(--space-md);
	}

	.rows button {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		height: var(--size-control-lg);
		padding-inline: var(--space-md);
		border: none;
		background: none;
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		text-align: start;
		cursor: pointer;
	}

	.rows button:active {
		transform: scale(var(--motion-press-row));
	}

	.rows button.destructive {
		color: var(--color-error-base);
	}

	.rows button.divider {
		border-bottom: var(--border-hairline) solid var(--color-border-base);
		margin-bottom: var(--space-md);
	}

	.body {
		margin: 0 0 var(--space-xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.cancel {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: var(--size-control-lg);
		margin-top: var(--space-md);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		cursor: pointer;
	}

	.cancel:active {
		transform: scale(var(--motion-press-button));
	}

	.cancel.destructive-cta {
		border-color: transparent;
		background: var(--color-error-fill);
		color: var(--color-onAccent);
	}
</style>
