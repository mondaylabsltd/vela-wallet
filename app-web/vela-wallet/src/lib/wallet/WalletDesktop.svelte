<script lang="ts">
	import type { ActivityRowModel, AssetRowModel, PanelId, WalletDesktopModel } from './model';
	import ActionButtonRow from './ui/ActionButtonRow.svelte';
	import ActivityRow from './ui/ActivityRow.svelte';
	import AssetDetailPanel from './ui/AssetDetailPanel.svelte';
	import AssetRow from './ui/AssetRow.svelte';
	import BalanceDisplay from './ui/BalanceDisplay.svelte';
	import ReceivePanel from './ui/ReceivePanel.svelte';
	import SectionHeader from './ui/SectionHeader.svelte';
	import Sidebar from './ui/Sidebar.svelte';
	import ThirdPanel from './ui/ThirdPanel.svelte';

	interface Props {
		model: WalletDesktopModel;
		/** Sidebar navigation. Absent in the gallery, where the rail is a picture. */
		onnav?: (id: 'wallet' | 'contacts' | 'explore' | 'settings') => void;
		/** The sidebar header's name button: the account switcher. Absent in the gallery. */
		onaccounts?: () => void;
		/** Spec 025: tap-to-hide. The core owns `hidden`; this only reports the tap. */
		onbalancetoggle?: () => void;
		/** The balance status line was tapped (spec 028 Phase 8): the rescue for what it says. */
		onstatus?: () => void;
		/**
		 * Spec 021: the dock and the two section actions open a flow in the
		 * third column. When it is wired the flow host owns that column, so
		 * this component stops drawing its own spec-015 panels — two things
		 * cannot occupy one column.
		 */
		onflow?: (
			entry: 'receive' | 'receive-token' | 'send' | 'scan' | 'activity' | 'add-token' | 'tx-detail',
			/** The asset column's two doors name the token they are about (RULING 3). */
			detail?: { assetId?: string }
		) => void;
		/** The sidebar's network filter was used. Absent in the gallery. */
		onchainselect?: (row: WalletDesktopModel['sidebar']['networks'][number]) => void;
		/**
		 * A held token's row was tapped (live). The page answers by putting
		 * the token in `model.panels.assetDetail` and opening the column
		 * through `model.initialPanel` — the model drives the column here,
		 * because the flow host shares it and only the page sees both.
		 */
		onasset?: (row: AssetRowModel) => void;
		/** The asset column was closed (live). */
		onassetclose?: () => void;
		/** An activity row was tapped (live): which one, before the flow opens. */
		onactivity?: (row: ActivityRowModel) => void;
	}

	let {
		model,
		onnav,
		onaccounts,
		onflow,
		onbalancetoggle,
		onchainselect,
		onasset,
		onassetclose,
		onactivity,
		onstatus
	}: Props = $props();

	// The third column replaces the mobile bottom sheet (research.md D5).
	// Pure UI state: which content it hosts, seeded by the fixture state — and
	// on a live page driven by it, since a live model names what it shows.
	let panel: PanelId = $derived(model.initialPanel);

	function closeAsset() {
		panel = 'none';
		onassetclose?.();
	}
</script>

<div class="desktop">
	<Sidebar sidebar={model.sidebar} {onnav} {onaccounts} {onchainselect} />

	<main>
		<div class="content">
			<BalanceDisplay balance={model.balance} ontoggle={onbalancetoggle} {onstatus} />

			<div class="actions">
				<ActionButtonRow
					layout="pills"
					receive={model.actions.receive}
					send={model.actions.send}
					scan={model.actions.scan}
					onreceive={() => (onflow === undefined ? (panel = 'receive') : onflow('receive'))}
					onsend={() => onflow?.('send')}
					onscan={() => onflow?.('scan')}
				/>
			</div>

			<SectionHeader
				title={model.activitySection.title}
				action={model.activitySection.action}
				onaction={() => onflow?.('activity')}
			/>
			{#each model.activityGroups as group (group.label)}
				<p class="day">{group.label}</p>
				<ul>
					{#each group.rows as row, i (i)}
						<li>
							<ActivityRow
								{row}
								onclick={() => {
									onactivity?.(row);
									onflow?.('tx-detail');
								}}
							/>
						</li>
					{/each}
				</ul>
			{/each}

			<!-- The desktop's assets action reads 添加, so it opens the add-token
			     panel stacked on the assets one — which is what makes the back
			     chevron in the DT3L mock lead somewhere. -->
			<SectionHeader
				title={model.assetsSection.title}
				action={model.assetsSection.action}
				onaction={() => onflow?.('add-token')}
			/>
			<ul>
				{#each model.assetRows as row, i (i)}
					<li>
						<AssetRow
							{row}
							onclick={() => (onasset === undefined ? (panel = 'asset-detail') : onasset(row))}
						/>
					</li>
				{/each}
			</ul>
		</div>
	</main>

	<!-- The asset detail is this component's in both worlds: spec 015's D3
	     panel, which the flows deliberately did not fork (nav.svelte.ts). On a
	     live page it opens through the model, and the page keeps the flow host
	     closed while it is showing — two things cannot occupy one column. The
	     receive panel is gallery-only; live, that column is the receive flow's. -->
	{#if panel === 'asset-detail'}
		<ThirdPanel
			title={model.panels.assetDetail.title}
			closeLabel={model.closeLabel}
			onclose={closeAsset}
		>
			<AssetDetailPanel
				panel={model.panels.assetDetail}
				onsend={() => onflow?.('send', { assetId: model.panels.assetDetail.id })}
				onreceive={() => onflow?.('receive-token', { assetId: model.panels.assetDetail.id })}
				onselect={(row) => onactivity?.(row)}
			/>
		</ThirdPanel>
	{:else if onflow !== undefined}
		<!-- the flow host draws the column -->
	{:else if panel === 'receive'}
		<ThirdPanel
			title={model.panels.receive.title}
			closeLabel={model.closeLabel}
			onclose={() => (panel = 'none')}
		>
			<ReceivePanel panel={model.panels.receive} />
		</ThirdPanel>
	{/if}
</div>

<style>
	.desktop {
		display: flex;
		height: 100%;
		/* Capped and centred past the widest the mocks were drawn for (spec
		   038 T078): on a 4 K display the frame no longer hugs the left edge. */
		width: 100%;
		max-width: var(--layout-frameMax);
		margin-inline: auto;
		background: var(--color-bg-base);
		overflow: hidden;
	}

	main {
		flex: 1;
		min-width: 0;
		overflow-y: auto;
	}

	.content {
		max-width: var(--layout-maxContentWidth);
		padding: var(--space-4xl) var(--space-4xl) var(--space-4xl) var(--space-3xl);
		display: flex;
		flex-direction: column;
	}

	.actions {
		max-width: calc(var(--layout-maxContentWidth) * 0.75);
		padding-block: var(--space-2xl) var(--space-3xl);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	/* The day a group files under — the phone prints it; the desktop layout
	   had dropped it (spec 038 #E3), which left a chosen date format with
	   nowhere to show outside the detail column. */
	.day {
		margin: 0;
		padding-block: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
