<script lang="ts">
	/**
	 * The settings list's one row (spec 023).
	 *
	 * Every entry on ST1/ST1b is this: an optional leading glyph, a title, an
	 * optional second line, an optional right-aligned current value, and a
	 * trailing chevron or external-link mark. Nine of them make the phone's
	 * settings home; the danger tone makes the 退出登录 row; there is no second
	 * row component anywhere in this feature.
	 *
	 * A row with an `href` leaves the app (Settings → Community): it is drawn
	 * as a link — a new tab, no opener, no referrer — with the same look and
	 * the same press, so the group reads as rows, not as a list of links.
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

	/**
	 * Is the text block taller than one title line — a subtitle, a value that
	 * dropped under the title, a title that wrapped? Then the glyph belongs to
	 * the title's line, not the block's middle (078 review; Android's
	 * `VelaSettingsRow` measures the same thing). A one-line row keeps the
	 * glyph centred, which is where the title is.
	 */
	let bodyHeight = $state(0);
	let titleEl = $state<HTMLElement>();
	const tall = $derived.by(() => {
		if (titleEl === undefined || bodyHeight === 0) return false;
		const size = parseFloat(getComputedStyle(titleEl).fontSize) || 0;
		return size > 0 && bodyHeight > size * 2;
	});
</script>

{#snippet content()}
	{#if row.icon !== undefined}
		<span class="glyph" class:brand={row.icon.startsWith('brand-')} class:tall
			><Icon icon={UTILITY_ICONS[row.icon]} size="lg" /></span
		>
	{/if}

	<!-- The title block and the value: side by side while both fit at their own
	     widths; otherwise the value takes a line of its own under the title,
	     with the row's whole width (iOS's `TitleAndValue`, the same rule). A
	     title may wrap; it is never "…" — "Idi…" beside "Español (México) ·
	     Sistema" at the largest text size was the founder's report. -->
	<span class="body" bind:clientHeight={bodyHeight}>
		<span class="text">
			<span class="title" bind:this={titleEl}>{row.title}</span>
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
{/snippet}

{#if row.href !== undefined}
	<!-- eslint-disable svelte/no-navigation-without-resolve -- an external page, never an app route -->
	<a
		class="row {tone}"
		class:divider
		class:iconless={row.icon === undefined}
		href={row.href}
		target="_blank"
		rel="noopener noreferrer">{@render content()}</a
	>
	<!-- eslint-enable svelte/no-navigation-without-resolve -->
{:else}
	<button
		type="button"
		class="row {tone}"
		class:divider
		class:iconless={row.icon === undefined}
		onclick={() => onselect?.(row.id)}
	>
		{@render content()}
	</button>
{/if}

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
		text-decoration: none;
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

	/* On a row of more than one line the glyph belongs to the TITLE's line
	   (078 review, as Android draws it): one title line tall — at the title's
	   size, so `1lh` is the title's line — pinned to the top of the block. */
	.glyph.tall {
		align-self: flex-start;
		height: 1lh;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
	}

	/* A brand's mark is FILLED edge to edge. Inset to the middle 20 of its 24
	   box — exactly as iOS (LucideIcons .brand*) and Android (VelaIcons
	   brandIcon) inset theirs — it sits in the icon column at the weight of
	   the stroked glyphs, and the three shells draw the same mark. */
	.glyph.brand :global(svg) {
		transform: scale(0.8333);
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

	/* fg-muted, app-wide (078 review, DECIDED): fg-subtle measured 3.38:1 on
	   the light page and failed AA; fg-muted is 5.1:1 there and 6.3:1 dark. */
	.subtitle {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
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
