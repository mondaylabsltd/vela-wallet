<script lang="ts">
	/**
	 * T3 / T3b / T5 / T5b / DT3L — adding a token, or the network one lives on.
	 *
	 * Two tabs over one shape: a field, a result card, a CTA. The ERC-20 tab
	 * looks a contract up on a chosen network; the native tab looks a network
	 * up by name or chain ID. Every failure state in T5 and T5b is a variant of
	 * the same two elements — the field's error, and what the result card holds
	 * — so they are model states rather than separate screens.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import FactRow from '../ui/FactRow.svelte';
	import MonoField from '../ui/MonoField.svelte';
	import SegmentedToggle from '../ui/SegmentedToggle.svelte';
	import StatusChip from '../ui/StatusChip.svelte';
	import type { AddTokenModel } from '../model';

	interface Props {
		model: AddTokenModel;
		ontab?: (tab: string) => void;
		onnetwork?: () => void;
		oninput?: (value: string) => void;
		onsubmit?: () => void;
		/** T3b live: one of the index's matches was chosen. */
		onpick?: (id: string) => void;
	}

	let { model, ontab, onnetwork, oninput, onsubmit, onpick }: Props = $props();
</script>

<div class="add">
	<SegmentedToggle
		label={model.title}
		selected={model.tab}
		options={[
			{ id: 'erc20', label: model.tabs.erc20 },
			{ id: 'native', label: model.tabs.native }
		]}
		onselect={ontab}
	/>

	{#if model.network !== undefined}
		<button type="button" class="network" onclick={onnetwork}>
			<TokenIcon
				ticker={model.network.mark.ticker}
				badgeColor={model.network.mark.badgeColor}
				logoUrls={model.network.mark.logoUrls}
				badgeLogoUrl={model.network.mark.badgeLogoUrl}
				badgeHidden={model.network.mark.badgeHidden}
			/>
			<span class="network-name">{model.network.name}</span>
			<Icon icon={UTILITY_ICONS['chevron-down']} size="md" label={model.network.pickLabel} />
		</button>
	{/if}

	<MonoField
		label={model.fieldLabel}
		value={model.fieldValue}
		placeholder={model.fieldPlaceholder}
		error={model.fieldError}
		{oninput}
	/>

	{#if model.result.kind === 'searching'}
		<p class="note">{model.result.text}</p>
	{:else if model.result.kind === 'not-found'}
		<p class="note">{model.result.text}</p>
	{:else if model.result.kind === 'token'}
		<div class="card">
			<TokenIcon
				ticker={model.result.mark.ticker}
				badgeColor={model.result.mark.badgeColor}
				logoUrls={model.result.mark.logoUrls}
				badgeLogoUrl={model.result.mark.badgeLogoUrl}
				badgeHidden={model.result.mark.badgeHidden}
			/>
			<span class="text">
				<span class="name">{model.result.name}</span>
				<span class="detail">{model.result.detail}</span>
			</span>
			{#if model.result.chip !== undefined}<StatusChip chip={model.result.chip} />{/if}
		</div>
	{:else if model.result.kind === 'suggestions'}
		<ul class="suggestions">
			{#each model.result.rows as row (row.id)}
				<li>
					<button type="button" class="suggestion" onclick={() => onpick?.(row.id)}>
						<TokenIcon
							ticker={row.mark.ticker}
							badgeColor={row.mark.badgeColor}
							logoUrls={row.mark.logoUrls}
							badgeHidden={row.mark.badgeHidden}
						/>
						<span class="text">
							<span class="name">{row.name}</span>
							<span class="detail">{row.meta}</span>
						</span>
						<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
					</button>
				</li>
			{/each}
		</ul>
	{:else if model.result.kind === 'network'}
		<div class="card column">
			<div class="card-head">
				<TokenIcon
					ticker={model.result.mark.ticker}
					badgeColor={model.result.mark.badgeColor}
					logoUrls={model.result.mark.logoUrls}
					badgeLogoUrl={model.result.mark.badgeLogoUrl}
					badgeHidden={model.result.mark.badgeHidden}
				/>
				<span class="name">{model.result.name}</span>
				{#if model.result.chip !== undefined}<StatusChip chip={model.result.chip} />{/if}
			</div>
			{#if model.result.note !== undefined}
				<p class="reason">{model.result.note}</p>
			{/if}
			{#each model.result.facts as fact (fact.label)}
				<FactRow {fact} />
			{/each}
			{#if model.result.setup !== undefined}
				<!-- eslint-disable svelte/no-navigation-without-resolve -- an external page (Chain Setup), never an app route -->
				<a class="setup" href={model.result.setup.href} target="_blank" rel="noreferrer noopener">
					<span>{model.result.setup.label}</span>
					<Icon icon={UTILITY_ICONS['external-link']} size="sm" />
				</a>
				<!-- eslint-enable svelte/no-navigation-without-resolve -->
			{/if}
		</div>
	{/if}

	<div class="cta">
		<Button variant="primary" shape="rounded" disabled={model.ctaDisabled} onclick={onsubmit}>
			{model.cta}
		</Button>
	</div>
</div>

<style>
	.add {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	.network {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-md) var(--space-lg);
		border: none;
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.network-name {
		flex: 1;
		min-width: 0;
		text-align: start;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
	}

	.card {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
	}

	.column {
		flex-direction: column;
		align-items: stretch;
		gap: 0;
	}

	.card-head {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding-bottom: var(--space-md);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		flex: 1;
		min-width: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.detail {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	/* Why it is refused, or why the wizard stopped: a sentence, read in full. */
	.reason {
		margin: 0;
		padding-bottom: var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	/* A real link out of the app, where the core gave it somewhere to go. */
	.setup {
		display: inline-flex;
		align-items: center;
		align-self: flex-start;
		gap: var(--space-xs);
		min-height: var(--size-control-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-info-base);
		text-decoration: none;
	}

	.setup:hover {
		text-decoration: underline;
	}

	.note {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.suggestions {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.suggestions li + li {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.suggestion {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding-block: var(--space-md);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		color: var(--color-fg-subtle);
		text-align: start;
		cursor: pointer;
	}

	.cta {
		padding-top: var(--space-md);
	}
</style>
