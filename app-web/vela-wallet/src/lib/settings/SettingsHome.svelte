<script lang="ts">
	/**
	 * The phone settings surface (spec 023, ST1–ST16 + SR1–SR5).
	 *
	 * One component, not sixteen. The mocks are a page (`home` plus seven
	 * pushed sub-pages) crossed with an overlay (nine sheets), and everything
	 * inside both is assembled from `ui/`. Which page and which overlay a state
	 * shows is DATA — the fixture layer says so — so the gallery pins a state
	 * by handing over a model, and the real app moves between them by tapping.
	 *
	 * Navigation is local `$state`, seeded from the model. Business state is
	 * not wired: the callbacks are how a route hooks the two behaviours that
	 * already exist (signing out, and leaving for another tab).
	 */
	import { untrack } from 'svelte';
	import type { OnNetEvent } from './net-events';
	import type { NetEndpointField } from '$lib/core/generated/NetEndpointField';
	import type { NetProviderId } from '$lib/core/generated/NetProviderId';
	import type { SettingsHomeModel, SettingsOverlayId, SettingsPageId } from './model';
	import type { SettingsPrefEvent } from './pref-events';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import TabBar from '$lib/wallet/ui/TabBar.svelte';
	import AboutPanel from './ui/AboutPanel.svelte';
	import AccountRow from './ui/AccountRow.svelte';
	import AccountsSheetBody from './ui/AccountsSheetBody.svelte';
	import AddNetworkPanel from './ui/AddNetworkPanel.svelte';
	import BalanceDetailBody from './ui/BalanceDetailBody.svelte';
	import ConfirmSheet from './ui/ConfirmSheet.svelte';
	import DangerCard from './ui/DangerCard.svelte';
	import EndpointsPanel from './ui/EndpointsPanel.svelte';
	import FeedbackBody from './ui/FeedbackBody.svelte';
	import IndexDownScreen from './ui/IndexDownScreen.svelte';
	import NavHeader from './ui/NavHeader.svelte';
	import NetworkDetailPanel from './ui/NetworkDetailPanel.svelte';
	import NetworksPanel from './ui/NetworksPanel.svelte';
	import RelayerBody from './ui/RelayerBody.svelte';
	import RpcBanner from './ui/RpcBanner.svelte';
	import RpcFixBody from './ui/RpcFixBody.svelte';
	import RpcProvidersPanel from './ui/RpcProvidersPanel.svelte';
	import SectionLabel from './ui/SectionLabel.svelte';
	import SegmentedControl from './ui/SegmentedControl.svelte';
	import SelectSheetBody from './ui/SelectSheetBody.svelte';
	import SettingsRow from './ui/SettingsRow.svelte';
	import StoragePanel from './ui/StoragePanel.svelte';
	import TextScaleSlider from './ui/TextScaleSlider.svelte';

	interface Props {
		model: SettingsHomeModel;
		/** Spec 022: the web app has no 探索 — see TabBar's `destinations`. */
		destinations?: readonly ('wallet' | 'contacts' | 'explore' | 'settings')[];
		/** Leaving settings for another tab. Absent in the gallery. */
		onselecttab?: (id: 'wallet' | 'contacts' | 'explore' | 'settings') => void;
		/** The one real behaviour behind this screen today. */
		onsignout?: () => void;
		/** 通讯录 row — the contacts screens are their own route. */
		onopencontacts?: () => void;
		/** The network surfaces' live wiring (spec 024). Absent = gallery. */
		onnetevent?: OnNetEvent;
		/** The person chose a display currency in the sheet (spec 024 phase 5). */
		oncurrencyselect?: (code: string) => void;
		/**
		 * A storage row's action was tapped, by its own id (spec 027 phase 6).
		 * Absent in the gallery, where every row is canon data.
		 */
		onstorageclear?: (id: string) => void;
		/**
		 * A preference row was used (spec 028 T433). Absent in the gallery,
		 * where these controls are pictures of themselves.
		 */
		onprefevent?: (event: SettingsPrefEvent) => void;
		/**
		 * The account switcher (spec 028 Phase 8). A row is picked by its
		 * POSITION in the sheet — the session's own order — and the two buttons
		 * leave for the create and sign-in journeys. Absent in the gallery.
		 */
		onaccountselect?: (position: number) => void;
		onaccountcreate?: () => void;
		onaccountsignin?: () => void;
		/** The switcher opened or closed: the balance core refreshes its rows while it is up. */
		onaccountsopen?: (open: boolean) => void;
		/** The storage page's "clear all caches" was confirmed. Absent in the gallery. */
		onclearcaches?: () => void;
	}

	let {
		model,
		destinations,
		onselecttab,
		onsignout,
		onopencontacts,
		onnetevent,
		oncurrencyselect,
		onstorageclear,
		onprefevent,
		onaccountselect,
		onaccountcreate,
		onaccountsignin,
		onaccountsopen,
		onclearcaches
	}: Props = $props();

	// Seeds, not bindings: a gallery state pins where this opens, and a person
	// tapping owns it from then on.
	let page = $state<SettingsPageId>(untrack(() => model.page));
	let overlay = $state<SettingsOverlayId>(untrack(() => model.overlay));
	let advancedOpen = $state(untrack(() => model.state === 'st1b'));

	/** ST1's 高级 disclosure, applied over the fixture sections. */
	const sections = $derived(
		model.sections.map((section) =>
			section.collapsible === true ? { ...section, collapsed: !advancedOpen } : section
		)
	);

	/** Rows a tap navigates from; everything else opens an overlay. */
	const PAGE_OF: Partial<Record<string, SettingsPageId>> = {
		networks: 'networks',
		'rpc-providers': 'rpc-providers',
		'add-network': 'add-network',
		endpoints: 'endpoints',
		storage: 'storage',
		about: 'about'
	};

	const OVERLAY_OF: Partial<Record<string, SettingsOverlayId>> = {
		language: 'language',
		currency: 'currency',
		'number-format': 'number-format',
		'date-format': 'date-format',
		'time-format': 'time-format',
		feedback: 'feedback'
	};

	function selectRow(id: string) {
		if (id === 'contacts') {
			onopencontacts?.();
			return;
		}
		const next = PAGE_OF[id];
		if (next !== undefined) {
			page = next;
			// Opening these surfaces is itself a core event: the machine probes
			// what the person is about to look at.
			if (next === 'endpoints') onnetevent?.({ kind: 'endpoints-open' });
			if (next === 'rpc-providers') onnetevent?.({ kind: 'providers-open' });
			if (next === 'add-network') onnetevent?.({ kind: 'open-add' });
			return;
		}
		const sheet = OVERLAY_OF[id];
		if (sheet !== undefined) overlay = sheet;
	}

	function close() {
		if (overlay === 'accounts') onaccountsopen?.(false);
		overlay = 'none';
	}

	function openAccounts() {
		overlay = 'accounts';
		onaccountsopen?.(true);
	}

	/** The sub-page's own header copy. `home` has no back affordance. */
	const header = $derived.by(() => {
		switch (page) {
			case 'networks':
				return { title: model.networks.title, subtitle: model.networks.subtitle };
			case 'network-detail':
				return { title: model.networkDetail.title, subtitle: model.networkDetail.subtitle };
			case 'add-network':
				return { title: model.addNetwork.title, subtitle: model.addNetwork.subtitle };
			case 'rpc-providers':
				return { title: model.rpcProviders.title, subtitle: model.rpcProviders.subtitle };
			case 'endpoints':
				return { title: model.endpoints.title, subtitle: undefined };
			case 'storage':
				return { title: model.storage.title, subtitle: model.storage.subtitle };
			case 'about':
				return { title: model.about.title, subtitle: undefined };
			default:
				return undefined;
		}
	});

	/** The sheet's accessible name, by overlay. */
	const sheetTitle = $derived.by(() => {
		switch (overlay) {
			case 'accounts':
				return model.accountsSheet.title;
			case 'sign-out':
				return model.signOutSheet.title;
			case 'language':
				return model.languageSheet.title;
			case 'currency':
				return model.currencySheet.title;
			case 'number-format':
				return model.numberSheet.title;
			case 'date-format':
				return model.dateSheet.title;
			case 'time-format':
				return model.timeSheet.title;
			case 'clear-caches':
				return model.clearCachesSheet.title;
			case 'erase-device':
				return model.eraseSheet.title;
			case 'feedback':
				return model.feedback.title;
			case 'rpc-fix':
				return model.rpcFix.title;
			case 'balance-detail':
				return model.balanceDetail.title;
			case 'relayer':
				return model.relayer.title;
			default:
				return '';
		}
	});

	const sheetSubtitle = $derived.by(() => {
		switch (overlay) {
			case 'language':
				return model.languageSheet.subtitle;
			case 'number-format':
				return model.numberSheet.subtitle;
			case 'date-format':
				return model.dateSheet.subtitle;
			case 'time-format':
				return model.timeSheet.subtitle;
			case 'feedback':
				return model.feedback.subtitle;
			default:
				return undefined;
		}
	});

	/**
	 * SR2–SR4 are sheets over ANOTHER screen (the wallet, the send flow), so
	 * the body behind them is a dimmed title rather than the settings list.
	 * Drawing the settings list behind a "fix Polygon's RPC" sheet would put a
	 * screen there that the person was never on.
	 */
	const rescue = $derived(model.tab === 'wallet');
</script>

{#if model.state === 'sr5'}
	<IndexDownScreen panel={model.indexDown} />
{:else}
	<div class="settings">
		<div class="scroll">
			{#if rescue}
				<h1 class="backdrop">{model.backdropTitle}</h1>
				{#if model.rpcBanner !== undefined}
					<div class="banner"><RpcBanner banner={model.rpcBanner} /></div>
				{/if}
			{:else if header !== undefined}
				<NavHeader
					title={header.title}
					subtitle={header.subtitle}
					backLabel={model.closeLabel}
					onback={() => (page = 'home')}
				/>
			{:else}
				<h1 class="title">{model.title}</h1>
			{/if}

			{#if !rescue}
				{#if page === 'home'}
					<AccountRow account={model.account} onselect={openAccounts} />

					{#each sections as section, index (index)}
						{#if section.label !== undefined}
							<SectionLabel
								label={section.label}
								collapsible={section.collapsible}
								collapsed={section.collapsed}
								ontoggle={() => (advancedOpen = !advancedOpen)}
							/>
						{/if}

						{#if section.collapsed !== true}
							{#each section.rows as row, rowIndex (row.id)}
								<SettingsRow
									{row}
									onselect={selectRow}
									divider={rowIndex < section.rows.length - 1}
								/>
							{/each}
						{/if}

						<!-- The three appearance controls are not rows: they are the
						     control itself, shown inline under 语言 (ST1). -->
						{#if section.appearanceControls === true}
							<TextScaleSlider
								model={model.appearance.textScale}
								onchange={(index) => onprefevent?.({ kind: 'text-scale', index })}
							/>
							<div class="control">
								<SegmentedControl
									model={model.appearance.theme}
									onselect={(id) => onprefevent?.({ kind: 'theme', id })}
								/>
							</div>
							<div class="control">
								<SegmentedControl
									model={model.appearance.avatar}
									onselect={(id) => onprefevent?.({ kind: 'avatar', id })}
								/>
							</div>
						{/if}
					{/each}

					<button type="button" class="sign-out" onclick={() => (overlay = 'sign-out')}>
						{model.signOut.label}
					</button>

					<div class="danger">
						<DangerCard
							title={model.erase.title}
							subtitle={model.erase.subtitle}
							onselect={() => (overlay = 'erase-device')}
						/>
					</div>
				{:else if page === 'networks'}
					<NetworksPanel
						rows={model.networks.rows}
						addLabel={model.networks.addLabel}
						deleteLabel={model.networks.removeLabel}
						onselect={(id) => {
							onnetevent?.({ kind: 'select-network', id });
							page = 'network-detail';
						}}
						ondelete={(id) => onnetevent?.({ kind: 'delete-network', id })}
						onadd={() => {
							onnetevent?.({ kind: 'open-add' });
							page = 'add-network';
						}}
					/>
				{:else if page === 'network-detail'}
					<NetworkDetailPanel
						detail={model.networkDetail}
						onfield={(field, value) => onnetevent?.({ kind: 'detail-field', field, value })}
						onfieldblur={(field) => onnetevent?.({ kind: 'detail-blur', field })}
					/>
				{:else if page === 'add-network'}
					<AddNetworkPanel
						panel={model.addNetwork}
						onsearch={(query) => onnetevent?.({ kind: 'search', query })}
						onselect={(id) => onnetevent?.({ kind: 'pick-suggestion', chainId: Number(id) })}
						oncustomrpc={(value) => onnetevent?.({ kind: 'custom-rpc', value })}
						onprimary={() => onnetevent?.({ kind: 'confirm-add' })}
						onrecheck={() => onnetevent?.({ kind: 'recheck' })}
					/>
				{:else if page === 'rpc-providers'}
					<RpcProvidersPanel
						panel={model.rpcProviders}
						onfield={(id, value) =>
							onnetevent?.({ kind: 'provider-key', provider: id as NetProviderId, value })}
						onfieldblur={(id) =>
							onnetevent?.({ kind: 'provider-blur', provider: id as NetProviderId })}
						onaction={(id) =>
							onnetevent?.({ kind: 'provider-test', provider: id as NetProviderId })}
					/>
				{:else if page === 'endpoints'}
					<EndpointsPanel
						panel={model.endpoints}
						onfield={(id, value) =>
							onnetevent?.({ kind: 'endpoint', field: id as NetEndpointField, value })}
						onfieldblur={(id) =>
							onnetevent?.({ kind: 'endpoint-blur', field: id as NetEndpointField })}
						onreset={() => onnetevent?.({ kind: 'endpoints-reset' })}
					/>
				{:else if page === 'storage'}
					<StoragePanel
						panel={model.storage}
						onclear={onstorageclear}
						onclearcaches={() => (overlay = 'clear-caches')}
					/>
				{:else if page === 'about'}
					<AboutPanel panel={model.about} />
				{/if}
			{/if}
		</div>

		<TabBar tabs={model.tabs} selected={model.tab} {destinations} onselect={onselecttab} />

		{#if overlay !== 'none'}
			<BottomSheet title={sheetTitle} closeLabel={model.closeLabel} height="tall" onclose={close}>
				{#if sheetSubtitle !== undefined}
					<p class="sheet-subtitle">{sheetSubtitle}</p>
				{/if}

				{#if overlay === 'accounts'}
					<AccountsSheetBody
						sheet={model.accountsSheet}
						onselect={(position) => {
							onaccountselect?.(position);
							close();
						}}
						oncreate={onaccountcreate}
						onsignin={onaccountsignin}
					/>
				{:else if overlay === 'sign-out'}
					<ConfirmSheet sheet={model.signOutSheet} onconfirm={onsignout} oncancel={close} />
				{:else if overlay === 'language'}
					<SelectSheetBody
						sheet={model.languageSheet}
						onselect={(id) => {
							onprefevent?.({ kind: 'language', id });
							close();
						}}
					/>
				{:else if overlay === 'currency'}
					<SelectSheetBody
						sheet={model.currencySheet}
						onselect={(id) => {
							oncurrencyselect?.(id);
							close();
						}}
					/>
				{:else if overlay === 'number-format'}
					<SelectSheetBody
						sheet={model.numberSheet}
						onselect={(id) => {
							onprefevent?.({ kind: 'number-format', id });
							close();
						}}
					/>
				{:else if overlay === 'date-format'}
					<SelectSheetBody
						sheet={model.dateSheet}
						onselect={(id) => {
							onprefevent?.({ kind: 'date-format', id });
							close();
						}}
					/>
				{:else if overlay === 'time-format'}
					<SelectSheetBody
						sheet={model.timeSheet}
						onselect={(id) => {
							onprefevent?.({ kind: 'time-format', id });
							close();
						}}
					/>
				{:else if overlay === 'clear-caches'}
					<ConfirmSheet
						sheet={model.clearCachesSheet}
						onconfirm={() => {
							onclearcaches?.();
							close();
						}}
						oncancel={close}
					/>
				{:else if overlay === 'erase-device'}
					<!-- The sheet does NOT close on confirm: an erase that fails must
					     say so where the person is looking, and the route reports it
					     through this sheet's own callout. A success leaves this page
					     entirely. -->
					<ConfirmSheet
						sheet={model.eraseSheet}
						onconfirm={() => onprefevent?.({ kind: 'erase' })}
						oncancel={close}
					/>
				{:else if overlay === 'feedback'}
					<FeedbackBody panel={model.feedback} />
				{:else if overlay === 'rpc-fix'}
					<RpcFixBody panel={model.rpcFix} onprimary={close} />
				{:else if overlay === 'balance-detail'}
					<BalanceDetailBody panel={model.balanceDetail} />
				{:else if overlay === 'relayer'}
					<RelayerBody panel={model.relayer} onprimary={close} />
				{/if}
			</BottomSheet>
		{/if}
	</div>
{/if}

<style>
	.settings {
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
		padding-bottom: var(--space-4xl);
	}

	.title {
		margin: 0;
		padding-block: var(--space-4xl) var(--space-xl);
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	/* The screen a rescue sheet is covering: named, dimmed, inert. */
	.backdrop {
		margin: 0;
		padding-block: var(--space-4xl) var(--space-xl);
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-subtle);
		opacity: var(--opacity-dim);
	}

	.banner {
		padding-block: var(--space-lg);
	}

	.control {
		padding-block: var(--space-md);
	}

	.sign-out {
		display: block;
		width: 100%;
		margin-block: var(--space-4xl) var(--space-3xl);
		padding: var(--space-lg);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.danger {
		padding-bottom: var(--space-3xl);
	}

	.sheet-subtitle {
		/* The sheet header already pads below itself; a negative pull here rode
		   up into the title's line box and overlapped it. */
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
