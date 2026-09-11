<script lang="ts">
	import type { AssetDetailPanelModel } from '../model';
	import { UTILITY_ICONS } from '../icons';
	import ActivityRow from './ActivityRow.svelte';
	import Icon from './Icon.svelte';
	import TokenIcon from './TokenIcon.svelte';
	import { copyText } from '$lib/services/clipboard';

	interface Props {
		panel: AssetDetailPanelModel;
		/** The two actions, live. Absent in the gallery, where they are drawn. */
		onsend?: () => void;
		onreceive?: () => void;
		/** A transaction row was tapped — the same detail the home's rows open (spec 038 #E2). */
		onselect?: (row: ActivityRowModel) => void;
	}

	let { panel, onsend, onreceive, onselect }: Props = $props();

	let copiedIndex = $state(-1);
	let timer: ReturnType<typeof setTimeout> | undefined;

	function copy(index: number) {
		const fact = panel.facts[index];
		void copyText(fact?.copyValue ?? fact?.value ?? '');
		copiedIndex = index;
		clearTimeout(timer);
		timer = setTimeout(() => (copiedIndex = -1), 150);
	}
</script>

<div class="detail">
	<div class="head">
		<TokenIcon
			ticker={panel.token.ticker}
			badgeColor={panel.token.badgeColor}
			logoUrls={panel.token.logoUrls}
			badgeLogoUrl={panel.token.badgeLogoUrl}
			badgeHidden={panel.token.badgeHidden}
		/>
		<div class="head-text">
			<p class="balance">{panel.token.balance}</p>
			<p class="fiat">{panel.token.fiatLine}</p>
		</div>
	</div>

	<div class="actions">
		<button type="button" onclick={onsend}>
			<Icon icon={UTILITY_ICONS['arrow-up-right']} size="base" />
			<span>{panel.send}</span>
		</button>
		<button type="button" onclick={onreceive}>
			<Icon icon={UTILITY_ICONS['arrow-down-left']} size="base" />
			<span>{panel.receive}</span>
		</button>
	</div>

	<dl class="facts">
		{#each panel.facts as fact, i (fact.label)}
			<div class="fact">
				<dt>{fact.label}</dt>
				<dd>
					<span>{fact.value}</span>
					{#if fact.copy !== undefined}
						<button
							type="button"
							class="copy"
							class:copied={copiedIndex === i}
							aria-label={fact.copy}
							onclick={() => copy(i)}
						>
							<Icon icon={copiedIndex === i ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />
						</button>
					{/if}
				</dd>
			</div>
		{/each}
	</dl>

	{#if panel.explorerUrl !== undefined}
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- an explorer page outside the app -->
		<a class="explorer" href={panel.explorerUrl} target="_blank" rel="noreferrer noopener">
			<span>{panel.viewOnExplorer}</span>
			<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
		</a>
	{:else}
		<button type="button" class="explorer">
			<span>{panel.viewOnExplorer}</span>
			<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
		</button>
	{/if}

	<h3>{panel.transactionsTitle}</h3>
	<ul class="rows">
		{#each panel.rows as row, i (i)}
			<li><ActivityRow {row} onclick={onselect ? () => onselect(row) : undefined} /></li>
		{/each}
	</ul>
</div>

<style>
	.detail {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	p {
		margin: 0;
	}

	.head {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
	}

	.head-text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.balance {
		font-family: var(--font-display);
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.fiat {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.actions {
		display: flex;
		gap: var(--space-lg);
	}

	.actions button {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		height: var(--size-control-md);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.actions button:active {
		transform: scale(var(--motion-press-button));
	}

	.facts {
		margin: 0;
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.fact {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-lg);
		padding-block: var(--space-lg);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	dt {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	dd {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
		text-align: end;
		overflow-wrap: anywhere;
	}

	dd {
		display: inline-flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-sm);
	}

	.copy {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-lg);
		height: var(--icon-lg);
		flex-shrink: 0;
		padding: 0;
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.copy:hover {
		color: var(--color-fg-base);
	}

	.copied {
		color: var(--color-success-base);
	}

	.explorer {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		align-self: flex-start;
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
		text-decoration: none;
	}

	.explorer:hover {
		color: var(--color-fg-base);
	}

	h3 {
		margin: var(--space-md) 0 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.rows {
		list-style: none;
		margin: 0;
		padding: 0;
	}
</style>
