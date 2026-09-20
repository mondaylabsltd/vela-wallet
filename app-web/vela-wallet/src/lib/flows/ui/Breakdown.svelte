<script lang="ts">
	/**
	 * The parts of a batch — every recipient of a split by name and avatar,
	 * every asset of a sweep by its mark (spec 038 #D2). One list, drawn the
	 * same on the confirm, the receipt and the transaction detail, so what was
	 * signed, what is landing and what landed read as one thing.
	 *
	 * A row may carry a `note` under its label — the split's confirm repeats the
	 * form's duplicate-payee warning there (issue 203), so the last screen
	 * before a signature says it too.
	 */
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import type { BreakdownRowModel } from '../model';

	interface Props {
		rows: BreakdownRowModel[];
		/** "3 recipients" — above the list when the screen has not said it already. */
		title?: string;
	}

	let { rows, title }: Props = $props();
</script>

<div class="block">
	{#if title !== undefined}
		<p class="title">{title}</p>
	{/if}
	<ul class="breakdown">
		{#each rows as item, i (`${item.address ?? item.label}-${i}`)}
			<li>
				{#if item.lead !== undefined}
					<TokenIcon
						ticker={item.lead.ticker}
						badgeColor={item.lead.badgeColor}
						logoUrls={item.lead.logoUrls}
						badgeLogoUrl={item.lead.badgeLogoUrl}
						badgeHidden={item.lead.badgeHidden}
						size="inline"
					/>
				{:else if item.identiconSvg !== undefined}
					<span class="mark"
						><Identicon svg={item.identiconSvg} size="row" address={item.address} /></span
					>
				{/if}
				<span class="text">
					<span class="label" class:mono={item.mono === true}>
						{item.label}
					</span>
					{#if item.detail !== undefined}<span class="detail">{item.detail}</span>{/if}
					{#if item.note}<span class="note">{item.note}</span>{/if}
				</span>
				<span class="value">{item.value}</span>
			</li>
		{/each}
	</ul>
</div>

<style>
	.block {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.title {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.breakdown {
		list-style: none;
		margin: 0;
		padding: var(--space-xs) var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}

	.breakdown li {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-block: var(--space-md);
	}

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

	/* Label and, when there is one, the sentence under it — one column, so
	   the note is not swallowed by the label's ellipsis. */
	.text {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.note {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-warning-base);
	}

	/* An address is set in mono wherever it is the thing being read. */
	.mono,
	.detail {
		font-family: var(--font-mono);
	}

	.detail {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.value {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}
</style>
