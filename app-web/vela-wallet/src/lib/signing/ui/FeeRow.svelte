<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import LetterAvatar from '$lib/ui/LetterAvatar.svelte';
	import PositiveNote from './PositiveNote.svelte';
	import FeeSpeedRow from '$lib/flows/ui/FeeSpeedRow.svelte';
	import type { FeeModel } from '../model';

	interface Props {
		fee: FeeModel;
		/** Opens / closes the fee-token selector; absent in the gallery. */
		ontoggle?: () => void;
		/** A coin was chosen — its option id. Absent in the gallery, where the list is a picture. */
		onpick?: (id: string) => void;
		/**
		 * The speed control under the fee (spec 069) — the send form's own,
		 * so a dApp transaction's speed is chosen exactly as a send's is.
		 */
		onspeed?: () => void;
		onspeedpick?: (id: string) => void;
	}

	let { fee, ontoggle, onpick, onspeed, onspeedpick }: Props = $props();
</script>

{#if fee.kind === 'offchain'}
	<PositiveNote text={fee.note} quiet />
{:else if fee.kind === 'onchain'}
	{#if fee.selector}
		<section class="selector">
			<button type="button" class="head" onclick={ontoggle}>
				<span>{fee.selector.title}</span>
				<Icon icon={UTILITY_ICONS['chevron-down']} size="sm" />
			</button>
			{#each fee.selector.options as option (option.id)}
				<!-- A coin that cannot pay is DRAWN and not pickable: hiding it would be a
				     second filter beside the core's own, and a live-looking row that does
				     nothing is how somebody pays gas in a coin they do not hold (issue 211). -->
				<button
					type="button"
					class="option"
					class:selected={option.selected}
					disabled={option.insufficient === true}
					onclick={() => onpick?.(option.id)}
				>
					<LetterAvatar letter={option.mark.letter} tint={option.mark.tint} size={32} />
					<span class="who">
						<span class="name">{option.name}</span>
						<span class="balance">{option.balance}</span>
					</span>
					<span class="numbers">
						<span class="fee">{option.fee}</span>
					</span>
					{#if option.selected}
						<span class="check"><Icon icon={UTILITY_ICONS.check} size="base" /></span>
					{/if}
				</button>
			{/each}
		</section>
	{:else}
		{#if fee.tappable}
			<button type="button" class="row" onclick={ontoggle}>
				<span class="label">{fee.label}</span>
				<span class="value">{fee.value}</span>
				<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
			</button>
		{:else}
			<!-- One coin, and a quote in hand: there is nothing to choose and
			     nothing to ask again, so this is the fee STATED. No chevron, no
			     pointer, no press — the house rule that a control which cannot act
			     is not drawn as one (spec 081, dead-controls #6). -->
			<div class="row stated">
				<span class="label">{fee.label}</span>
				<span class="value">{fee.value}</span>
			</div>
		{/if}
		{#if fee.speed}
			<FeeSpeedRow speed={fee.speed} ontoggle={onspeed} onselect={onspeedpick} />
		{/if}
	{/if}
	{#if fee.warning}
		<!-- Issue 262: the reason the slide is shut, under the row that fixes it. -->
		<p class="warning" role="alert">{fee.warning}</p>
	{/if}
{/if}

<style>
	.warning {
		margin: var(--space-sm) var(--space-xl) 0;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: 500;
		color: var(--color-error-base);
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		padding: var(--space-lg) var(--space-xl);
		border: none;
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-family: var(--font-ui);
		color: var(--color-fg-muted);
		cursor: pointer;
		text-align: start;
	}

	.row.stated {
		cursor: default;
	}

	.label {
		flex: 1;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.value {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	.selector {
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		padding: var(--space-lg) var(--space-xl);
	}

	.head {
		width: 100%;
		border: none;
		background: none;
		font-family: var(--font-ui);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.option {
		width: 100%;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
	}

	.option:disabled {
		opacity: var(--opacity-disabled, 0.4);
		cursor: default;
	}

	.option.selected {
		background: var(--color-bg-raised);
	}

	.who {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.balance {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.numbers {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
	}

	.fee {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.check {
		color: var(--color-accent-base);
	}
</style>
