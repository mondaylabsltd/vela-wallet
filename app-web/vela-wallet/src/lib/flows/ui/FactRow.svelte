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
		/**
		 * The row's own action, when the model makes it a control (`fact.tap`,
		 * PR 2 polish: the send confirm's failed fee line). Absent, the row is
		 * drawn as the fact it is, whatever the model says.
		 */
		ontap?: () => void;
	}

	let { fact, copied = false, oncopy, ontap }: Props = $props();

	/** A control only when the model says what a tap does AND something will do it. */
	const pressable = $derived(
		fact.tap !== undefined && ontap !== undefined && fact.copy === undefined
	);
</script>

{#snippet lead()}
	{#if fact.lead?.kind === 'dot'}
		<span class="dot" style:background={fact.lead.color} aria-hidden="true"></span>
	{:else if fact.lead?.kind === 'token'}
		<!-- In the same box as the identicons beside it: a network mark at 26
		     over a 20 face read as two sizes of one thing (the confirm's From,
		     To and Network rows; the detail's). -->
		<span class="mark">
			<TokenIcon
				ticker={fact.lead.mark.ticker}
				badgeColor={fact.lead.mark.badgeColor}
				logoUrls={fact.lead.mark.logoUrls}
				badgeLogoUrl={fact.lead.mark.badgeLogoUrl}
				badgeHidden={fact.lead.mark.badgeHidden}
				size="inline"
			/>
		</span>
	{:else if fact.lead?.kind === 'identicon'}
		<span class="mark"
			><Identicon svg={fact.lead.svg} size="row" address={fact.lead.address} /></span
		>
	{/if}
{/snippet}

{#snippet value()}
	<span class="value" class:mono={fact.mono} data-tone={fact.tone}>{fact.value}</span>
{/snippet}

{#if pressable && fact.tap !== undefined}
	<!-- PR 2 polish: the confirm's failed fee line is the control its words
	     and the line under the confirm promise — "Tap to retry" asks again,
	     "Pay with another coin" opens the coins (its chevron says so). While a
	     re-ask is out it is the answer awaited: the press asks nothing. -->
	<button
		type="button"
		class="fact pressable"
		aria-label={fact.tap.label}
		aria-busy={fact.tap.busy === true}
		data-does={fact.tap.does}
		onclick={() => {
			if (fact.tap?.busy !== true) ontap?.();
		}}
	>
		<span class="label">{fact.label}</span>
		<span class="value-wrap">
			{@render lead()}
			{@render value()}
			{#if fact.tap.does === 'choose_coin'}
				<span class="chevron"><Icon icon={UTILITY_ICONS['chevron-right']} size="sm" /></span>
			{/if}
		</span>
	</button>
{:else}
	<div class="fact">
		<span class="label">{fact.label}</span>
		<span class="value-wrap">
			{#if fact.detail !== undefined}
				<span class="lines">
					<span class="first">
						{@render lead()}
						<span class="value" class:mono={fact.mono} data-tone={fact.tone}>{fact.value}</span>
					</span>
					<span class="detail">{fact.detail}</span>
				</span>
			{:else}
				{@render lead()}
				<span class="value" class:mono={fact.mono} data-tone={fact.tone}>{fact.value}</span>
			{/if}
			{#if fact.copy !== undefined}
				<button type="button" aria-label={fact.copy} class:copied onclick={oncopy}>
					<Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />
				</button>
			{/if}
		</span>
	</div>
{/if}

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

	/* The token mark and the identicon both shrink to this row's scale here
	   — a fact row is a line of text with a hint of art, not a row with an
	   avatar — and to ONE box, so a network beside a face is the face's size.
	   The art's own size class is outranked on purpose (the wrapper's scope
	   plus the row's), and its border is inside the box. */
	.mark {
		display: flex;
		width: var(--icon-lg);
		height: var(--icon-lg);
		flex-shrink: 0;
	}

	.value-wrap .mark :global(> *) {
		box-sizing: border-box;
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

	/* A name over the address it stands for (spec 097 F): the name may be cut,
	   the address under it never is. Issue 423: the art sits beside the NAME,
	   on one line, as the From row's does, and the line under it — whose word
	   the name is and the short address — is in the body face. Beside the
	   two-line column the art floated between the lines, far from a short
	   name, over a line set in mono. */
	.lines {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		min-width: 0;
	}

	.first {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		min-width: 0;
		max-width: 100%;
	}

	.detail {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		white-space: nowrap;
	}

	/* Spec 093: an unlimited allowance reads in red; what came back, in green. */
	.value[data-tone='danger'] {
		color: var(--color-error-base);
	}

	.value[data-tone='success'] {
		color: var(--color-success-base);
	}

	/* The whole row is the target (PR 2 polish): the same row, unstyled as a
	   button, so a pressable fact reads exactly as the facts beside it. */
	.pressable {
		width: 100%;
		margin: 0;
		padding-inline: 0;
		border: none;
		background: none;
		font: inherit;
		text-align: start;
		color: inherit;
		cursor: pointer;
	}

	.pressable[aria-busy='true'] {
		cursor: progress;
	}

	.chevron {
		display: flex;
		flex-shrink: 0;
		color: var(--color-fg-subtle);
	}

	.value-wrap > button {
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

	.value-wrap > button:hover {
		color: var(--color-fg-base);
	}

	.copied {
		color: var(--color-success-base);
	}
</style>
