<script lang="ts">
	/**
	 * The body of every picker sheet (ST4–ST8) and of the desktop dropdown —
	 * an optional search field, the rows, and the language sheet's footer note.
	 * `role="listbox"` wraps the rows so a screen reader reads a choice, not a
	 * pile of buttons.
	 */
	import type { SelectSheetModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import SelectRow from './SelectRow.svelte';

	interface Props {
		sheet: SelectSheetModel;
		onselect?: (id: string) => void;
	}

	let { sheet, onselect }: Props = $props();
</script>

{#if sheet.searchPlaceholder !== undefined}
	<label class="search" data-field>
		<Icon icon={UTILITY_ICONS.search} size="md" />
		<input
			type="search"
			placeholder={sheet.searchPlaceholder}
			aria-label={sheet.searchPlaceholder}
		/>
	</label>
{/if}

<div class="rows" role="listbox" aria-label={sheet.title}>
	{#each sheet.rows as row (row.id)}
		<SelectRow {row} {onselect} />
	{/each}
</div>

{#if sheet.footerNote !== undefined}
	<p class="footer">{sheet.footerNote}</p>
{/if}
{#if sheet.footerLink !== undefined}
	<a
		class="footer-link"
		href="https://github.com/mondaylabsltd/vela-wallet/issues"
		target="_blank"
		rel="noreferrer noopener"
	>
		{sheet.footerLink}
	</a>
{/if}

<style>
	.search {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		min-height: var(--size-control-md);
		margin-block: var(--space-md) var(--space-lg);
		padding-inline: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		color: var(--color-fg-subtle);
	}

	input {
		flex: 1;
		min-width: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		outline: none;
	}

	.footer {
		margin: var(--space-xl) 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.footer-link {
		display: inline-block;
		padding-block: var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
	}
</style>
