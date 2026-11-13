<script lang="ts">
	import AddressText from '$lib/ui/AddressText.svelte';
	import SourceMark from './SourceMark.svelte';
	import type { KeyValueRow } from '../model';

	interface Props {
		rows: KeyValueRow[];
	}

	let { rows }: Props = $props();
</script>

<dl>
	{#each rows as row, i (i)}
		<div class="row">
			<dt>
				{row.label}{#if row.source}<SourceMark text={row.source} />{/if}
			</dt>
			<dd class:mono={row.mono} data-tone={row.valueTone ?? 'neutral'}>
				<AddressText text={row.value} />
			</dd>
		</div>
	{/each}
</dl>

<style>
	dl {
		margin: 0;
		display: flex;
		flex-direction: column;
	}

	.row {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-xl);
		padding-block: var(--space-lg);
	}

	dt {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		flex-shrink: 0;
	}

	dd {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		text-align: end;
		overflow-wrap: anywhere;
	}

	.mono {
		font-family: var(--font-mono);
		font-weight: var(--weight-regular);
	}

	[data-tone='danger'] {
		color: var(--color-error-base);
	}

	[data-tone='caution'] {
		color: var(--color-warning-base);
	}

	[data-tone='accent'] {
		color: var(--color-fg-base);
	}

	[data-tone='success'] {
		color: var(--color-success-base);
	}
</style>
