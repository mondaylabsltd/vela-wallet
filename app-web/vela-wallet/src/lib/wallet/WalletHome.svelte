<script lang="ts">
	import type { ActivityRowModel, AssetRowModel, WalletHomeModel } from './model';
	import { UTILITY_ICONS } from './icons';
	import ActionButtonRow from './ui/ActionButtonRow.svelte';
	import ActivityRow from './ui/ActivityRow.svelte';
	import AssetRow from './ui/AssetRow.svelte';
	import BalanceDisplay from './ui/BalanceDisplay.svelte';
	import BottomSheet from './ui/BottomSheet.svelte';
	import ChainFilterList from './ui/ChainFilterList.svelte';
	import EmptyState from './ui/EmptyState.svelte';
	import SectionHeader from './ui/SectionHeader.svelte';
	import SkeletonRow from './ui/SkeletonRow.svelte';
	import TabBar from './ui/TabBar.svelte';
	import WalletHeader from './ui/WalletHeader.svelte';

	interface Props {
		model: WalletHomeModel;
		/** Spec 022: the web app has no 探索 — see TabBar's `destinations`. */
		destinations?: readonly ('wallet' | 'contacts' | 'explore' | 'settings')[];
		/** Tab selection. Absent in the gallery, where the bar is a picture. */
		onselect?: (id: 'wallet' | 'contacts' | 'explore' | 'settings') => void;
		/** The header's name button: the account switcher. Absent in the gallery. */
		onaccounts?: () => void;
		/**
		 * Spec 021: the dock, the two section actions and the rows are the
		 * entries into Receive / Send / Scan / Activity / Assets. Absent in the
		 * gallery, where this screen is a picture.
		 */
		onflow?: (
			entry: 'receive' | 'send' | 'scan' | 'activity' | 'assets' | 'token-detail' | 'tx-detail'
		) => void;
		/** Spec 025: tap-to-hide. The core owns `hidden`; this only reports the tap. */
		onbalancetoggle?: () => void;
		/** The balance status line was tapped (spec 028 Phase 8): the rescue for what it says. */
		onstatus?: () => void;
		/** An activity row was tapped (live): which one, before the flow opens. */
		onactivity?: (row: ActivityRowModel) => void;
		/** An asset row was tapped (live): which one, before the token screen opens. */
		onasset?: (row: AssetRowModel) => void;
	}

	let {
		model,
		destinations,
		onselect,
		onaccounts,
		onflow,
		onbalancetoggle,
		onactivity,
		onasset,
		onstatus
	}: Props = $props();

	// Pure UI state: the fixture-provided sheet, once dismissed, stays dismissed.
	// Nothing on this screen reopens it any more — the pill that did is gone.
	let sheetClosed = $state(false);
	const showSheet = $derived(model.sheet !== undefined && !sheetClosed);
</script>

<div class="home" style:--text-scale={model.textScale === 1 ? undefined : model.textScale}>
	<div class="scroll">
		<!-- The header owns the whole width. The network filter pill used to sit
		     at its trailing edge and cost the name and address the room they
		     needed — a wallet called "kimik3 · something" showed as "kimik3 ·…"
		     next to a chip nobody was reading (founder call, 2026-08-26). -->
		<header class="top">
			<WalletHeader header={model.header} onclick={onaccounts} />
		</header>

		<div class="balance">
			<BalanceDisplay balance={model.balance} ontoggle={onbalancetoggle} {onstatus} />
		</div>

		<ActionButtonRow
			receive={model.actions.receive}
			send={model.actions.send}
			scan={model.actions.scan}
			onreceive={() => onflow?.('receive')}
			onsend={() => onflow?.('send')}
			onscan={() => onflow?.('scan')}
		/>

		<SectionHeader
			title={model.activitySection.title}
			action={model.activitySection.action}
			onaction={() => onflow?.('activity')}
		/>
		{#if model.activitySection.mode === 'loading'}
			<SkeletonRow />
			<SkeletonRow />
		{:else if model.activitySection.mode === 'empty' && model.activitySection.empty !== undefined}
			<EmptyState
				icon={UTILITY_ICONS.inbox}
				title={model.activitySection.empty.title}
				caption={model.activitySection.empty.caption}
			/>
		{:else}
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
		{/if}

		<SectionHeader
			title={model.assetsSection.title}
			action={model.assetsSection.action}
			onaction={() => onflow?.('assets')}
		/>
		{#if model.assetsSection.mode === 'loading'}
			<SkeletonRow />
			<SkeletonRow />
			<SkeletonRow />
		{:else if model.assetsSection.mode === 'empty' && model.assetsSection.empty !== undefined}
			<EmptyState
				icon={UTILITY_ICONS.wallet}
				title={model.assetsSection.empty.title}
				caption={model.assetsSection.empty.caption}
			/>
		{:else}
			<ul>
				{#each model.assetRows as row, i (i)}
					<li>
						<AssetRow
							{row}
							onclick={() => {
								onasset?.(row);
								onflow?.('token-detail');
							}}
						/>
					</li>
				{/each}
			</ul>
		{/if}
	</div>

	<TabBar tabs={model.tabs} {destinations} {onselect} />

	{#if showSheet && model.sheet !== undefined}
		<BottomSheet
			title={model.sheet.title}
			trailingIcon="search"
			onclose={() => (sheetClosed = true)}
		>
			<ChainFilterList rows={model.sheet.rows} onselect={() => (sheetClosed = true)} />
		</BottomSheet>
	{/if}
</div>

<style>
	.home {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--color-bg-base);
		overflow: hidden;
	}

	.scroll {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding-inline: var(--layout-screenPaddingX);
	}

	.top {
		display: flex;
		align-items: center;
		padding-block: var(--space-xl);
	}

	.balance {
		padding-block: var(--space-md) var(--space-2xl);
	}

	.day {
		margin: 0;
		padding-block: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
</style>
