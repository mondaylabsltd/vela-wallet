<script lang="ts">
	/**
	 * SD2 / SD2b / SD2d / DSD2L — the send form, in its three modes.
	 *
	 * One component, because the three ARE one form: single is a token, an
	 * amount and a person; split is the same token to several people; sweep is
	 * several tokens to one person. The SPEC sheet makes them mutually
	 * exclusive — entering one greys the door to the other — so they share a
	 * mode rather than living in three screens that would each need their own
	 * fee row, summary line and CTA.
	 */
	import Button from '$lib/ui/Button.svelte';
	import AssetRow from '$lib/wallet/ui/AssetRow.svelte';
	import AmountInput from '../ui/AmountInput.svelte';
	import FeeRow from '../ui/FeeRow.svelte';
	import GhostPillRow from '../ui/GhostPillRow.svelte';
	import RecipientCard from '../ui/RecipientCard.svelte';
	import RecipientField from '../ui/RecipientField.svelte';
	import SummaryLine from '../ui/SummaryLine.svelte';
	import TokenHeaderCard from '../ui/TokenHeaderCard.svelte';
	import type { SendFormModel } from '../model';

	interface Props {
		model: SendFormModel;
		onpickRecipient?: () => void;
		onscan?: () => void;
		onrecipientAction?: (id: 'add' | 'contacts' | 'import') => void;
		onremoveRecipient?: (index: number) => void;
		onfee?: () => void;
		ondenom?: () => void;
		onmax?: (index: number) => void;
		onaddRecipient?: () => void;
		/**
		 * The split rows, typed into (spec 028 Phase 10): a patch to one row,
		 * and the book opened for one row. Absent, the cards are the drawn ones.
		 */
		onrecipientRow?: (index: number, patch: { address?: string; amount?: string }) => void;
		onpickRecipientRow?: (index: number) => void;
		/**
		 * The primary action (spec 026). Absent, the CTA is the drawn button it
		 * has always been — the gallery renders a picture, not a dead promise.
		 */
		oncontinue?: () => void;
		/** Present ⇒ the amount and the address can be typed here. */
		onamount?: (value: string) => void;
		onrecipient?: (value: string) => void;
		/** The core's gate: `can_continue`. Absent leaves the button armed. */
		ctaDisabled?: boolean;
	}

	let {
		model,
		onpickRecipient,
		onscan,
		onrecipientAction,
		onremoveRecipient,
		onfee,
		ondenom,
		onmax,
		onaddRecipient,
		onrecipientRow,
		onpickRecipientRow,
		oncontinue,
		onamount,
		onrecipient,
		ctaDisabled = false
	}: Props = $props();
</script>

<div class="form">
	{#if model.token !== undefined}
		<TokenHeaderCard token={model.token} onmax={() => onmax?.(0)} />
	{/if}

	{#if model.sweepSummary !== undefined}
		<p class="sweep-summary">{model.sweepSummary}</p>
	{/if}

	{#if model.sweepRows !== undefined}
		<ul class="sweep">
			{#each model.sweepRows as row, i (row.symbol)}
				<li>
					<AssetRow
						row={{
							ticker: row.symbol,
							chain: row.balanceLabel,
							badgeColor: row.mark.badgeColor,
							logoUrls: row.mark.logoUrls,
							badgeLogoUrl: row.mark.badgeLogoUrl,
							badgeHidden: row.mark.badgeHidden,
							balance: row.amount,
							fiat: { kind: 'none' },
							masked: false
						}}
					>
						{#snippet trailing()}
							<button type="button" class="max" onclick={() => onmax?.(i)}>{row.max}</button>
						{/snippet}
					</AssetRow>
				</li>
			{/each}
		</ul>
	{/if}

	{#if model.amount !== undefined}
		<AmountInput
			value={model.amount.value}
			fiat={model.amount.fiat}
			denomLabel={model.amount.denomLabel}
			{ondenom}
			oninput={onamount}
		/>
	{/if}

	{#if model.recipient !== undefined}
		<RecipientField
			label={model.recipient.label}
			lines={model.recipient.lines}
			address={model.recipient.address}
			identiconSvg={model.recipient.identiconSvg}
			pickLabel={model.recipient.pickLabel}
			scanLabel={model.recipient.scanLabel}
			note={model.recipient.note}
			onpick={onpickRecipient}
			{onscan}
			oninput={onrecipient}
		/>
	{/if}

	{#if model.addRecipient !== undefined}
		<button type="button" class="add" onclick={onaddRecipient}>
			<span class="plus" aria-hidden="true">+</span>
			{model.addRecipient}
		</button>
	{/if}

	{#if model.recipients !== undefined}
		<ul class="recipients">
			{#each model.recipients as recipient, i (recipient.id ?? recipient.ordinal)}
				<li>
					<RecipientCard
						{recipient}
						symbol={model.token?.symbol}
						onremove={() => onremoveRecipient?.(i)}
						oninput={onrecipientRow ? (patch) => onrecipientRow(i, patch) : undefined}
						onpick={onpickRecipientRow ? () => onpickRecipientRow(i) : undefined}
					/>
				</li>
			{/each}
		</ul>
	{/if}

	{#if model.recipientActions !== undefined}
		<GhostPillRow
			items={model.recipientActions}
			onselect={(id) => onrecipientAction?.(id as 'add' | 'contacts' | 'import')}
		/>
	{/if}

	{#if model.summary !== undefined}
		<SummaryLine label={model.summary.label} value={model.summary.value} />
	{/if}

	<FeeRow fee={model.fee} onopen={onfee} />

	<!-- The core's standing reading of the money, under the fee it is about.
	     `Max` filling 0 because the fee is larger than the balance is the case
	     this exists for: the figure is right, and without the sentence it reads
	     as a broken button (issue 210). -->
	{#if model.warning !== undefined}
		<p class="warning" role="status">{model.warning}</p>
	{/if}

	{#if model.alert !== undefined}
		<p class="alert" role="alert">{model.alert}</p>
	{/if}

	<div class="cta">
		<Button variant="primary" shape="rounded" onclick={oncontinue} disabled={ctaDisabled}>
			{model.cta}
		</Button>
	</div>
</div>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	.sweep-summary {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.sweep {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.sweep li {
		padding-inline: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}

	.recipients {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.max {
		padding: var(--space-xs) var(--space-md);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		font-family: var(--font-ui);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		cursor: pointer;
	}

	/* The door from a single send into a split. Quiet on purpose: most sends
	   have one recipient, and this is the affordance for the ones that don't. */
	.add {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		align-self: flex-start;
		padding: var(--space-sm) 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.plus {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
	}

	.cta {
		padding-block: var(--space-md) var(--space-xl);
	}
	/* The core's refusal, where the eye is when the button did nothing. */
	.alert {
		margin: 0;
		padding-top: var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-danger-base);
	}

	/* Amber, not red: this one stands while the condition does, and the
	   person has not asked for anything to happen yet. */
	.warning {
		margin: 0;
		padding-top: var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-warning-base);
	}
</style>
