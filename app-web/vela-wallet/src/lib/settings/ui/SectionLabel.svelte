<script lang="ts">
	/**
	 * The small caps label above a group of rows (spec 023) — 外观 / 区域格式 /
	 * 高级. 高级 is the one that collapses (ST1b), so the chevron is optional
	 * and the whole label becomes a button only when it is present.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		label: string;
		collapsible?: boolean;
		collapsed?: boolean;
		ontoggle?: () => void;
	}

	let { label, collapsible = false, collapsed = false, ontoggle }: Props = $props();
</script>

{#if collapsible}
	<button type="button" class="label toggle" aria-expanded={!collapsed} onclick={ontoggle}>
		<span>{label}</span>
		<span class="chevron" class:up={!collapsed}>
			<Icon icon={UTILITY_ICONS['chevron-down']} size="sm" />
		</span>
	</button>
{:else}
	<p class="label">{label}</p>
{/if}

<style>
	.label {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		margin: 0;
		padding: 0;
		padding-block: var(--space-2xl) var(--space-md);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		text-align: start;
	}

	.toggle {
		cursor: pointer;
	}

	.chevron {
		display: flex;
		transition: transform var(--motion-duration-fast) ease-out;
	}

	.chevron.up {
		transform: rotate(180deg);
	}

	@media (prefers-reduced-motion: reduce) {
		.chevron {
			transition: none;
		}
	}
</style>
