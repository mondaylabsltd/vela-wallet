<script lang="ts">
	/**
	 * The label-value row (spec 021 component 15).
	 *
	 * One component for A2's transaction facts, SD3's confirmation summary,
	 * T2's token facts and T3b's chain facts. They differ only in what art the
	 * value carries — a chain dot, a token mark, an identicon, or nothing —
	 * and in whether the value is copyable, so those are props rather than
	 * four near-identical rows.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import type { FactRowModel } from '../model';

	interface Props {
		fact: FactRowModel;
		copied?: boolean;
		oncopy?: () => void;
	}

	let { fact, copied = false, oncopy }: Props = $props();
</script>

<div class="fact">
	<span class="label">{fact.label}</span>
	<span class="value-wrap">
		{#if fact.lead?.kind === 'dot'}
			<span class="dot" style:background={fact.lead.color} aria-hidden="true"></span>
		{:else if fact.lead?.kind === 'token'}
			<TokenIcon
				ticker={fact.lead.mark.ticker}
				badgeColor={fact.lead.mark.badgeColor}
				logoUrls={fact.lead.mark.logoUrls}
				badgeLogoUrl={fact.lead.mark.badgeLogoUrl}
				badgeHidden={fact.lead.mark.badgeHidden}
				size="inline"
			/>
		{:else if fact.lead?.kind === 'identicon'}
			<span class="mark"
				><Identicon svg={fact.lead.svg} size="row" address={fact.lead.address} /></span
			>
		{/if}
		<span class="value" class:mono={fact.mono}>{fact.value}</span>
		{#if fact.copy !== undefined}
			<button type="button" aria-label={fact.copy} class:copied onclick={oncopy}>
				<Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />
			</button>
		{/if}
	</span>
</div>

<style>
	.fact {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-lg);
		padding-block: var(--space-lg);
	}

	.label {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		flex-shrink: 0;
	}

	.value-wrap {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		min-width: 0;
	}

	.dot {
		width: var(--icon-base);
		height: var(--icon-base);
		border-radius: var(--radius-full);
		flex-shrink: 0;
	}

	/* The row-size token mark and identicon both shrink to this row's
	   scale here — a fact row is a line of text with a hint of art, not a
	   row with an avatar. */
	.mark {
		display: flex;
		width: var(--icon-lg);
		height: var(--icon-lg);
		flex-shrink: 0;
	}

	.mark :global(> *) {
		width: 100%;
		height: 100%;
	}

	.value {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.mono {
		font-family: var(--font-mono);
	}

	button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-lg);
		height: var(--icon-lg);
		flex-shrink: 0;
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	button:hover {
		color: var(--color-fg-base);
	}

	.copied {
		color: var(--color-success-base);
	}
</style>
