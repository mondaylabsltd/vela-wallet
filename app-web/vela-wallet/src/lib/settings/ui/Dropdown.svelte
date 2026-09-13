<script lang="ts">
	/**
	 * The desktop's select control (DST2/DST3) and the menu it opens.
	 *
	 * The menu is `position: absolute` inside a relatively-positioned trigger
	 * wrapper, and the panel that hosts it must not clip it — the SPEC calls
	 * that out explicitly (下拉菜单浮层需逃出容器裁剪).
	 */
	import type { SelectRowModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import SelectRow from './SelectRow.svelte';

	interface Props {
		value: string;
		label: string;
		open?: boolean;
		rows?: SelectRowModel[];
		ontoggle?: () => void;
		onselect?: (id: string) => void;
	}

	let { value, label, open = false, rows, ontoggle, onselect }: Props = $props();

	let host = $state<HTMLDivElement | undefined>();

	/**
	 * A way out that is not a choice. The menu opens OVER its trigger (the
	 * macOS popup shape the SPEC asks for), so the trigger cannot be clicked
	 * again to close it — Escape and a click anywhere else have to.
	 */
	function onkeydown(event: KeyboardEvent) {
		if (open && event.key === 'Escape') ontoggle?.();
	}

	function onpointerdown(event: PointerEvent) {
		if (open && host !== undefined && !host.contains(event.target as Node)) ontoggle?.();
	}
</script>

<svelte:window {onkeydown} {onpointerdown} />

<div class="dropdown" bind:this={host}>
	<button
		type="button"
		class="trigger"
		data-field
		aria-haspopup="listbox"
		aria-expanded={open}
		aria-label={label}
		onclick={ontoggle}
	>
		<span class="value">{value}</span>
		<Icon icon={UTILITY_ICONS['chevron-down']} size="sm" />
	</button>

	{#if open && rows !== undefined}
		<div class="menu" role="listbox" aria-label={label}>
			{#each rows as row (row.id)}
				<SelectRow {row} {onselect} />
			{/each}
		</div>
	{/if}
</div>

<style>
	.dropdown {
		position: relative;
		width: 100%;
	}

	.trigger {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md);
		width: 100%;
		min-height: var(--size-control-md);
		padding-inline: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}

	/* Focus is app.css's `data-field` edge: a form control, quiet like the
	   fields beside it, never the accent. */

	.value {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.menu {
		position: absolute;
		inset-inline: 0;
		top: 0;
		z-index: 2;
		padding-inline: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		box-shadow: var(--shadow-lg);
		transform-origin: top;
		animation: open var(--motion-duration-fast) ease-out;
	}

	.menu :global(.select-row:last-child) {
		border-bottom: none;
	}

	@keyframes open {
		from {
			transform: scale(0.98);
			opacity: 0;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.menu {
			animation: none;
		}
	}
</style>
