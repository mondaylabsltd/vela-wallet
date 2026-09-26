<script lang="ts">
	/**
	 * The settings list's one row (spec 023).
	 *
	 * Every entry on ST1/ST1b is this: an optional leading glyph, a title, an
	 * optional second line, an optional right-aligned current value, and a
	 * trailing chevron or external-link mark. Nine of them make the phone's
	 * settings home; the danger tone makes the 退出登录 row; there is no second
	 * row component anywhere in this feature.
	 */
	import type { SettingsRowModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import StatusPill from './StatusPill.svelte';

	interface Props {
		row: SettingsRowModel;
		/** Absent in the gallery, where the list is a picture. */
		onselect?: (id: string) => void;
		/** Hairline under the row; the last row of a group drops it. */
		divider?: boolean;
	}

	let { row, onselect, divider = true }: Props = $props();

	const tone = $derived(row.tone ?? 'default');
	const trailing = $derived(row.trailing ?? 'chevron');
</script>

<button
	type="button"
	class="row {tone}"
	class:divider
	class:iconless={row.icon === undefined}
	onclick={() => onselect?.(row.id)}
>
	{#if row.icon !== undefined}
		<span class="glyph"><Icon icon={UTILITY_ICONS[row.icon]} size="lg" /></span>
	{/if}

	<!-- The title block and the value: side by side while both fit at their own
	     widths; otherwise the value takes a line of its own under the title,
	     with the row's whole width (iOS's `TitleAndValue`, the same rule). A
	     title may wrap; it is never "…" — "Idi…" beside "Español (México) ·
	     Sistema" at the largest text size was the founder's report. -->
	<span class="body">
		<span class="text">
			<span class="title">{row.title}</span>
			{#if row.subtitle !== undefined}
				<span class="subtitle">{row.subtitle}</span>
			{/if}
		</span>
		{#if row.badge !== undefined || row.value !== undefined}
			<span class="aside">
				{#if row.badge !== undefined}
					<StatusPill pill={row.badge} />
				{/if}
				{#if row.value !== undefined}
					<span class="value">{row.value}</span>
				{/if}
			</span>
		{/if}
	</span>

	{#if trailing === 'chevron'}
		<span class="trailing"><Icon icon={UTILITY_ICONS['chevron-right']} size="sm" /></span>
	{:else if trailing === 'external'}
		<span class="trailing"><Icon icon={UTILITY_ICONS['external-link']} size="sm" /></span>
	{/if}
</button>

<style>
	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		min-height: var(--size-control-lg);
		padding-block: var(--space-lg);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		color: var(--color-fg-base);
		text-align: start;
		cursor: pointer;
	}

	/* Hairline, not a card: the design language de-containers these lists, and
	   the rule starts after the glyph column so the icons read as a column. */
	.divider {
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.glyph {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-2xl);
		flex-shrink: 0;
		color: var(--color-fg-muted);
	}

	/* Two pieces that wrap as wholes: at their natural widths they share the
	   line, the value at the far end (`space-between`); when they do not fit,
	   the value drops to a line of its own — and alone on a line,
	   `space-between` puts it at the START, under the title. */
	.body {
		display: flex;
		flex: 1;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		column-gap: var(--space-lg);
		row-gap: var(--space-xs);
		min-width: 0;
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 0 1 auto;
		min-width: 0;
	}

	/* Wraps — never "…". A word too long for the row breaks rather than spills. */
	.title {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		overflow-wrap: break-word;
	}

	.subtitle {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		overflow-wrap: break-word;
	}

	.aside {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		min-width: 0;
	}

	.value {
		min-width: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		font-variant-numeric: tabular-nums;
		overflow-wrap: break-word;
	}

	.trailing {
		display: flex;
		color: var(--color-fg-subtle);
		flex-shrink: 0;
	}

	.accent .title,
	.accent .glyph {
		color: var(--color-accent-base);
	}

	.danger .title,
	.danger .glyph {
		color: var(--color-error-base);
	}

	@media (hover: hover) {
		.row:hover .title {
			color: var(--color-fg-base);
		}

		.danger:hover .title {
			color: var(--color-error-base);
		}
	}

	.row:active {
		transform: scale(var(--motion-press-row));
	}

	@media (prefers-reduced-motion: reduce) {
		.row:active {
			transform: none;
		}
	}
</style>
