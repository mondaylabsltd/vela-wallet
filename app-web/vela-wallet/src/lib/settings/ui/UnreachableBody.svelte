<script lang="ts">
	/**
	 * Spec 092 — every network the wallet cannot reach, in one place.
	 *
	 * The home's line ("Can't reach 3 networks right now") opens this. Every
	 * network is listed, held or not: while one cannot be read nobody knows
	 * what it holds now. Each row says what was last read there and offers the
	 * network's RPC fix; the list follows the live view, so a network that
	 * comes back leaves it while it is open.
	 *
	 * A row with no `action` (PR 3 note 4: the network's RPC is fine, its
	 * token list could not be loaded) draws no button — there is no endpoint
	 * to repair. Its text takes the row's whole width.
	 */
	import type { UnreachableModel } from '../model';
	import ChainMark from './ChainMark.svelte';

	interface Props {
		panel: UnreachableModel;
		onfix?: (chainId: number) => void;
	}

	let { panel, onfix }: Props = $props();
</script>

{#if panel.summary !== undefined}
	<p class="summary">{panel.summary}</p>
{/if}

<ul data-testid="unreachable-list">
	{#each panel.rows as row (row.id)}
		<li>
			<ChainMark mark={row.mark} />
			<span class="text">
				<span class="name">{row.name}</span>
				<span class="line">{row.line}</span>
			</span>
			{#if row.action !== undefined}
				<button type="button" onclick={() => onfix?.(row.chainId)}>{row.action}</button>
			{/if}
		</li>
	{/each}
</ul>

<style>
	.summary {
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
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

	.line {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		font-variant-numeric: tabular-nums;
	}

	button {
		border: none;
		background: none;
		padding: var(--space-sm) 0 var(--space-sm) var(--space-md);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
		cursor: pointer;
		white-space: nowrap;
		transition: transform var(--motion-duration-fast) ease;
	}

	button:active {
		transform: scale(var(--motion-press-button));
	}

	@media (prefers-reduced-motion: reduce) {
		button:active {
			transform: none;
		}
	}
</style>
