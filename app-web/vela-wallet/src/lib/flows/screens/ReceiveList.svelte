<script lang="ts">
	/**
	 * R1 / DR1L — the receive network list.
	 *
	 * The subtitle is the whole idea of the screen: one address, every network.
	 * The list under it is not eight addresses, it is eight ways of saying the
	 * same one — which is why every row shows the same characters, and why the
	 * copy button is on each row rather than once at the top.
	 */
	import SearchField from '../ui/SearchField.svelte';
	import NetworkRow from '../ui/NetworkRow.svelte';
	import type { ReceiveListModel } from '../model';
	import { fill } from '$lib/wallet/messages';
	import { copyText } from '$lib/services/clipboard';

	interface Props {
		model: ReceiveListModel;
		/** Desktop drops the big title — the panel header already carries it. */
		chrome?: boolean;
		onqr?: (index: number) => void;
	}

	let { model, chrome = true, onqr }: Props = $props();

	let query = $state('');
	let copiedIndex = $state(-1);
	let copyTimer: ReturnType<typeof setTimeout> | undefined;

	const shown = $derived(
		query.trim() === ''
			? model.rows.map((row, index) => ({ row, index }))
			: model.rows
					.map((row, index) => ({ row, index }))
					.filter(({ row }) => row.name.toLowerCase().includes(query.trim().toLowerCase()))
	);

	/**
	 * The tick holds for 150ms and goes back (SPEC 动效 · 收款). Long enough to
	 * register, short enough that a person copying three networks in a row
	 * never wonders which of the ticks is the live one.
	 */
	function copy(index: number) {
		// The tick is this screen's; the write is the clipboard service's. A
		// drawn row (the gallery) has no whole address and copies nothing.
		void copyText(model.rows[index]?.addressFull ?? '');
		copiedIndex = index;
		clearTimeout(copyTimer);
		copyTimer = setTimeout(() => (copiedIndex = -1), 150);
	}
</script>

<div class="list" class:chrome>
	<p class="subtitle">{model.subtitle}</p>
	<SearchField placeholder={model.searchPlaceholder} bind:value={query} />

	{#if shown.length === 0}
		<p class="empty">{fill(model.emptyText, { query: query.trim() })}</p>
	{:else}
		<ul>
			{#each shown as entry (entry.index)}
				<li>
					<NetworkRow
						row={entry.row}
						copied={copiedIndex === entry.index}
						oncopy={() => copy(entry.index)}
						onqr={() => onqr?.(entry.index)}
					/>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.list {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	.subtitle {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	li + li {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.empty {
		margin: 0;
		padding-block: var(--space-3xl);
		text-align: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
