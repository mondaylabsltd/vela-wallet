<script lang="ts">
	/**
	 * The body of every picker sheet (ST4–ST8) and of the desktop dropdown —
	 * an optional search field, the rows, and the language sheet's footer note.
	 * `role="listbox"` wraps the rows so a screen reader reads a choice, not a
	 * pile of buttons.
	 *
	 * The search field filters what it is drawn over (spec 072): the currency
	 * sheet drew one that did nothing, over a list the rate sources make long.
	 * A row matches on its code, its label or its caption ("yen" finds JPY).
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

	let query = $state('');

	const rows = $derived.by(() => {
		const needle = query.trim().toLocaleLowerCase();
		if (needle === '') return sheet.rows;
		return sheet.rows.filter((row) =>
			[row.id, row.label, row.caption ?? ''].some((text) =>
				text.toLocaleLowerCase().includes(needle)
			)
		);
	});
</script>

{#if sheet.searchPlaceholder !== undefined}
	<label class="search" data-field>
		<Icon icon={UTILITY_ICONS.search} size="md" />
		<input
			type="search"
			placeholder={sheet.searchPlaceholder}
			aria-label={sheet.searchPlaceholder}
			bind:value={query}
		/>
	</label>
{/if}

<div class="rows" role="listbox" aria-label={sheet.title}>
	{#each rows as row (row.id)}
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
