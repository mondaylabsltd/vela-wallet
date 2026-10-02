<script lang="ts">
	/**
	 * A dApp record's "Technical details" (spec 093): folded by default, the
	 * core's lines in its order when opened — the operation, the stored
	 * request, the typed-data type, the hashes.
	 *
	 * The stored request is the one heavy thing a record keeps, so it is not
	 * part of the model: the row carries a reader, and the store is asked the
	 * moment the section opens — never while it is folded.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import FactRow from './FactRow.svelte';
	import type { FactRowModel, TxTechnicalModel } from '../model';

	interface Props {
		model: TxTechnicalModel;
		oncopy: (fact: FactRowModel) => void;
		/** The fact whose copy just landed, for its tick. */
		copied?: FactRowModel | null;
	}

	let { model, oncopy, copied = null }: Props = $props();

	let open = $state(false);
	/** What each content row reads — asked only while the section is open. */
	const contents = $derived(
		open ? model.rows.map((row) => (row.kind === 'content' ? row.read() : null)) : []
	);
</script>

<section class="technical">
	<button type="button" class="toggle" aria-expanded={open} onclick={() => (open = !open)}>
		<Icon icon={UTILITY_ICONS[open ? 'chevron-down' : 'chevron-right']} size="sm" />
		<span>{model.title}</span>
	</button>

	{#if open}
		<ul>
			{#each model.rows as row, i (i)}
				<li>
					{#if row.kind === 'fact'}
						<FactRow fact={row.fact} copied={copied === row.fact} oncopy={() => oncopy(row.fact)} />
					{:else}
						<div class="content">
							<span class="label">{row.label}</span>
							{#if contents[i]}
								<pre>{contents[i]}</pre>
							{:else}
								<p class="missing">{row.missing}</p>
							{/if}
						</div>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.technical {
		display: flex;
		flex-direction: column;
	}

	.toggle {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		min-height: var(--size-control-md);
		padding: var(--space-lg) 0;
		border: none;
		border-top: var(--border-hairline) solid var(--color-border-base);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
		text-align: start;
	}

	.toggle:active {
		transform: scale(var(--motion-press-row));
	}

	.toggle:hover {
		color: var(--color-fg-base);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	li {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.content {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		padding-block: var(--space-lg);
	}

	.label {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	pre,
	.missing {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
		color: var(--color-fg-muted);
	}

	pre {
		max-height: calc(var(--size-control-md) * 6);
		overflow: auto;
		font-family: var(--font-mono);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
