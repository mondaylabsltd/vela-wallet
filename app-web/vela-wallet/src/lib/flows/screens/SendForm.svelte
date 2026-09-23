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
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import AssetRow from '$lib/wallet/ui/AssetRow.svelte';
	import AmountInput from '../ui/AmountInput.svelte';
	import FeeRow from '../ui/FeeRow.svelte';
	import FeeSpeedRow from '../ui/FeeSpeedRow.svelte';
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
		/** Ask the chain for the fee again (spec 068). */
		onfeerefresh?: () => void;
		/** Fold / unfold the speed control, and pick a tier for THIS send only. */
		onspeed?: () => void;
		onspeedpick?: (id: string) => void;
		/** The ⇄ swap — the core's `toggle_fiat_input` (issue 197). */
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
		/**
		 * The core's `estimating_gas`: Continue was pressed and the pre-check is
		 * out, for as long as fifteen seconds. The gate closes for it too, and a
		 * button that only went dark read as "you cannot" when it meant "wait".
		 */
		ctaBusy?: boolean;
		/** One amount into every row that has none (`model.fillEmpty`). */
		onfillEmpty?: (amount: string) => void;
	}

	let {
		model,
		onpickRecipient,
		onscan,
		onrecipientAction,
		onremoveRecipient,
		onfee,
		onfeerefresh,
		onspeed,
		onspeedpick,
		ondenom,
		onmax,
		onaddRecipient,
		onrecipientRow,
		onpickRecipientRow,
		oncontinue,
		onamount,
		onrecipient,
		ctaDisabled = false,
		ctaBusy = false,
		onfillEmpty
	}: Props = $props();

	/** The three doors into a split, each with the glyph of what it opens. */
	const ACTION_ICONS = { add: 'plus', contacts: 'users-round', import: 'upload' } as const;
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
							{#if row.max !== undefined}
								<button type="button" class="max" onclick={() => onmax?.(i)}>{row.max}</button>
							{/if}
						{/snippet}
					</AssetRow>
				</li>
			{/each}
		</ul>
	{/if}

	{#if model.amount !== undefined}
		<AmountInput
			value={model.amount.value}
			placeholder={model.amount.placeholder}
			adornment={model.amount.adornment}
			fiat={model.amount.fiat}
			denomLabel={model.amount.denomLabel}
			denomToggle={model.amount.denomToggle}
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

	{#if model.fillEmpty !== undefined && onfillEmpty}
		{@const fill = model.fillEmpty}
		<button type="button" class="fill" onclick={() => onfillEmpty(fill.amount)}>
			<Icon icon={UTILITY_ICONS.copy} size="sm" />
			{fill.label}
		</button>
	{/if}

	{#if model.recipientActions !== undefined}
		<GhostPillRow
			items={model.recipientActions.map((action) => ({
				...action,
				icon: ACTION_ICONS[action.id]
			}))}
			onselect={(id) => onrecipientAction?.(id as 'add' | 'contacts' | 'import')}
		/>
	{/if}

	{#if model.mode !== 'split' && model.summary !== undefined}
		<SummaryLine
			label={model.summary.label}
			value={model.summary.value}
			detail={model.summary.detail}
			over={model.summary.over}
		/>
	{/if}

	<div class="fee-block">
		<FeeRow fee={model.fee} onopen={onfee} onrefresh={onfeerefresh} />
		{#if model.speed !== undefined}
			<FeeSpeedRow speed={model.speed} ontoggle={onspeed} onselect={onspeedpick} />
		{/if}
	</div>

	<!--
		A split can be sixty people long, and what it adds up to is the one
		figure that matters while the rows are being typed. So in a split the
		total, the core's refusal and Continue travel together at the bottom of
		the screen, instead of waiting under the last card.
	-->
	<div class="foot" class:pinned={model.mode === 'split'}>
		{#if model.mode === 'split' && model.summary !== undefined}
			<SummaryLine
				label={model.summary.label}
				value={model.summary.value}
				detail={model.summary.detail}
				over={model.summary.over}
				remaining={model.summary.remaining}
			/>
		{/if}

		{#if model.alert !== undefined}
			<p class="alert" role="alert">
				<Icon icon={UTILITY_ICONS['circle-alert']} size="sm" />
				<span>{model.alert}</span>
			</p>
		{:else if model.hint !== undefined}
			<p class="alert hint">
				<Icon icon={UTILITY_ICONS.info} size="sm" />
				<span>{model.hint}</span>
			</p>
		{/if}

		<div class="cta">
			<Button
				variant="primary"
				shape="rounded"
				onclick={oncontinue}
				disabled={ctaDisabled && !ctaBusy}
				loading={ctaBusy}
			>
				{model.cta}
			</Button>
		</div>
	</div>
</div>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	/* The fee and the speed are one thought — what this transfer costs and
	   how fast it lands — so they sit closer to each other than to the rows
	   above and below (spec 068). */
	.fee-block {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
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

	/* Offered only while it would do something, under the rows it would fill.
	   Quiet: it edits the form, it does not move the money. */
	.fill {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		align-self: flex-start;
		padding: var(--space-sm) 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
		text-decoration: underline;
		text-underline-offset: var(--space-xs);
		cursor: pointer;
	}

	.fill:active {
		transform: scale(var(--motion-press-button));
	}

	.foot {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.pinned {
		position: sticky;
		inset-block-end: 0;
		/* The form's own gap sits above it; the rule is where the list ends. */
		padding-block-start: var(--space-sm);
		background: var(--color-bg-base);
		border-block-start: var(--border-hairline) solid var(--color-border-base);
	}

	.cta {
		padding-block: var(--space-md) var(--space-xl);
	}

	/* The core's refusal, where the eye is when the button did nothing. */
	.alert {
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		/* `--color-danger-base` was never a token, so this sentence — the core's
		   refusal — had been printing in the body colour. */
		color: var(--color-error-base);
	}

	/* Unfinished, not refused: the same place, in the quiet colour. */
	.hint {
		color: var(--color-fg-muted);
	}

	.alert :global(svg) {
		flex-shrink: 0;
		margin-block-start: var(--space-xs);
	}
</style>
