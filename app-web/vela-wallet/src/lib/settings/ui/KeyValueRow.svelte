<script lang="ts">
	/**
	 * About's technical-detail and link rows (ST14 / DST8): a label at the
	 * start, a value at the end, mono where the value is an identifier and an
	 * external mark where it is a destination.
	 */
	import type { KeyValueRowModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		row: KeyValueRowModel;
	}

	let { row }: Props = $props();
</script>

{#snippet content()}
	<span class="label">{row.label}</span>
	<span class="value" class:mono={row.mono}>{row.value}</span>
	{#if row.external}
		<span class="glyph"><Icon icon={UTILITY_ICONS['external-link']} size="sm" /></span>
	{/if}
{/snippet}

<!-- A row that draws the external glyph opens its destination (spec 072):
     the three About links were pictures of links. -->
{#if row.href !== undefined}
	<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- About's outbound links, not app routes -->
	<a class="kv" class:link={row.external} href={row.href} target="_blank" rel="noreferrer noopener">
		{@render content()}
	</a>
{:else}
	<div class="kv" class:link={row.external}>{@render content()}</div>
{/if}

<style>
	.kv {
		display: flex;
		text-decoration: none;
		align-items: center;
		gap: var(--space-lg);
		min-height: var(--size-control-md);
		padding-block: var(--space-lg);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.label {
		flex: 1;
		min-width: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.link .label {
		color: var(--color-fg-base);
		font-weight: var(--weight-semibold);
	}

	.value {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		text-align: end;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.value.mono {
		font-family: var(--font-mono);
	}

	.link .value {
		color: var(--color-fg-subtle);
	}

	.glyph {
		display: flex;
		color: var(--color-fg-subtle);
		flex-shrink: 0;
	}
</style>
