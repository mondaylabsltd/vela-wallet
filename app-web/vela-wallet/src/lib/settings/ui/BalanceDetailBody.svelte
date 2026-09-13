<script lang="ts">
	/**
	 * SR3 — the quiet breakdown.
	 *
	 * Rate-limiting gets a grey line and no button, because it resolves itself;
	 * a dead RPC gets a red line and 立即重试, because it does not. That
	 * distinction is the whole screen (project memory: 429 is transient, and a
	 * banner for it would train people to ignore banners).
	 */
	import type { BalanceDetailModel } from '../model';
	import ChainMark from './ChainMark.svelte';

	interface Props {
		panel: BalanceDetailModel;
		onretry?: (id: string) => void;
	}

	let { panel, onretry }: Props = $props();
</script>

<p class="summary">{panel.summary}</p>

<p class="section">{panel.sectionPending}</p>
<p class="note">{panel.pendingNote}</p>
<ul>
	{#each panel.pending as row (row.id)}
		<li>
			<ChainMark mark={row.mark} />
			<span class="text">
				<span class="name">{row.name}</span>
				<span class="status {row.tone}">{row.status}</span>
			</span>
			{#if row.action !== undefined}
				<button type="button" onclick={() => onretry?.(row.id)}>{row.action}</button>
			{/if}
		</li>
	{/each}
</ul>

<p class="section">{panel.sectionDone}</p>
<ul>
	{#each panel.done as row (row.id)}
		<li>
			<ChainMark mark={row.mark} />
			<span class="text"><span class="name">{row.name}</span></span>
			<span class="amount">{row.amount}</span>
		</li>
	{/each}
</ul>

{#if panel.unpriced.length > 0}
	<p class="section">{panel.sectionUnpriced}</p>
	<ul>
		{#each panel.unpriced as row (row.id)}
			<li>
				<ChainMark mark={row.mark} />
				<span class="text">
					<span class="name">{row.name}</span>
					<span class="status neutral">{row.detail}</span>
				</span>
			</li>
		{/each}
	</ul>
{/if}

<style>
	.summary {
		margin: 0 0 var(--space-xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.section {
		margin: var(--space-xl) 0 var(--space-sm);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.note {
		margin: 0 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	li {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding-block: var(--space-lg);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.status {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.status.neutral {
		color: var(--color-fg-subtle);
	}

	.status.error {
		color: var(--color-error-base);
	}

	.status.ok {
		color: var(--color-success-base);
	}

	.status.warn {
		color: var(--color-warning-base);
	}

	.status.accent {
		color: var(--color-accent-base);
	}

	button {
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
		cursor: pointer;
		white-space: nowrap;
	}

	.amount {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		font-variant-numeric: tabular-nums;
	}
</style>
