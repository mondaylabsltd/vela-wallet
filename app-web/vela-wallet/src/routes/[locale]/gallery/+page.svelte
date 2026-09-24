<script lang="ts">
	import { resolve } from '$app/paths';
	import { toLocale } from '$lib/i18n/locales';
	import { page } from '$app/state';
	import ActionButtonRow from '$lib/wallet/ui/ActionButtonRow.svelte';
	import ActivityRow from '$lib/wallet/ui/ActivityRow.svelte';
	import AssetRow from '$lib/wallet/ui/AssetRow.svelte';
	import BalanceDisplay from '$lib/wallet/ui/BalanceDisplay.svelte';
	import ChainFilterList from '$lib/wallet/ui/ChainFilterList.svelte';
	import EmptyState from '$lib/wallet/ui/EmptyState.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import NetworkFilterPill from '$lib/wallet/ui/NetworkFilterPill.svelte';
	import SectionHeader from '$lib/wallet/ui/SectionHeader.svelte';
	import SkeletonRow from '$lib/wallet/ui/SkeletonRow.svelte';
	import TabBar from '$lib/wallet/ui/TabBar.svelte';
	import WalletHeader from '$lib/wallet/ui/WalletHeader.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import ActionMenuSheet from '$lib/contacts/ui/ActionMenuSheet.svelte';
	import AddressBlock from '$lib/contacts/ui/AddressBlock.svelte';
	import AlphaIndexRail from '$lib/contacts/ui/AlphaIndexRail.svelte';
	import ContactRow from '$lib/contacts/ui/ContactRow.svelte';
	import ContextMenu from '$lib/contacts/ui/ContextMenu.svelte';
	import DropdownMenu from '$lib/contacts/ui/DropdownMenu.svelte';
	import EmptyStateCTA from '$lib/contacts/ui/EmptyStateCTA.svelte';
	import GhostAddRow from '$lib/contacts/ui/GhostAddRow.svelte';
	import GroupChips from '$lib/contacts/ui/GroupChips.svelte';
	import GroupRail from '$lib/contacts/ui/GroupRail.svelte';
	import GroupRow from '$lib/contacts/ui/GroupRow.svelte';
	import PinnedCTABar from '$lib/contacts/ui/PinnedCTABar.svelte';
	import SearchHeader from '$lib/contacts/ui/SearchHeader.svelte';
	import IntroSlide from '$lib/ui/intro/IntroSlide.svelte';
	import PageDots from '$lib/ui/intro/PageDots.svelte';
	import { INTRO_SLIDES } from '$lib/intro/slides';
	import AboutPanel from '$lib/settings/ui/AboutPanel.svelte';
	import AccountRow from '$lib/settings/ui/AccountRow.svelte';
	import Callout from '$lib/settings/ui/Callout.svelte';
	import ChainMark from '$lib/settings/ui/ChainMark.svelte';
	import CheckList from '$lib/settings/ui/CheckList.svelte';
	import ConfirmSheet from '$lib/settings/ui/ConfirmSheet.svelte';
	import DangerCard from '$lib/settings/ui/DangerCard.svelte';
	import Dropdown from '$lib/settings/ui/Dropdown.svelte';
	import EndpointsPanel from '$lib/settings/ui/EndpointsPanel.svelte';
	import FormRow from '$lib/settings/ui/FormRow.svelte';
	import NetworkRow from '$lib/settings/ui/NetworkRow.svelte';
	import RpcBanner from '$lib/settings/ui/RpcBanner.svelte';
	import RpcProvidersPanel from '$lib/settings/ui/RpcProvidersPanel.svelte';
	import SectionLabel from '$lib/settings/ui/SectionLabel.svelte';
	import SegmentedControl from '$lib/settings/ui/SegmentedControl.svelte';
	import SelectRow from '$lib/settings/ui/SelectRow.svelte';
	import SettingsRow from '$lib/settings/ui/SettingsRow.svelte';
	import StatusPill from '$lib/settings/ui/StatusPill.svelte';
	import StorageBar from '$lib/settings/ui/StorageBar.svelte';
	import StorageGroup from '$lib/settings/ui/StorageGroup.svelte';
	import TextScaleSlider from '$lib/settings/ui/TextScaleSlider.svelte';
	import UrlField from '$lib/settings/ui/UrlField.svelte';
	import Controls from './Controls.svelte';

	let { data } = $props();

	const locale = $derived(toLocale(page.params.locale ?? '') ?? 'en');
	const m = $derived(data.messages);
	const cm = $derived(data.contactsMessages);
	const h1s = $derived(data.models.h1s);
	const c = $derived(data.contacts);
	const alice = $derived(c.list?.sections[0].contacts[0]);
	const intro = $derived(data.intro);
	const ahao = $derived(c.list?.sections[0].contacts[1]);
	const st = $derived(data.settings);
</script>

<svelte:head>
	<title>Vela Wallet · Gallery</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<Controls {locale} />

<main class="gallery">
	<h1>Wallet UI Gallery</h1>

	<section id="gallery-section-screens">
		<h2>Screens</h2>
		<div class="links">
			{#each data.mobileStates as state (state)}
				<a href={resolve('/[locale]/gallery/[state]', { locale, state })}>{state.toUpperCase()}</a>
			{/each}
			{#each data.desktopStates as state (state)}
				<a href={resolve('/[locale]/gallery/[state]', { locale, state })}>{state.toUpperCase()}</a>
			{/each}
			{#each data.contactsMobileStates as state (state)}
				<a href={resolve('/[locale]/gallery/[state]', { locale, state })}>{state.toUpperCase()}</a>
			{/each}
			{#each data.contactsDesktopStates as state (state)}
				<a href={resolve('/[locale]/gallery/[state]', { locale, state })}>{state.toUpperCase()}</a>
			{/each}
			{#each data.settingsMobileStates as state (state)}
				<a href={resolve('/[locale]/gallery/[state]', { locale, state })}>{state.toUpperCase()}</a>
			{/each}
			{#each data.settingsDesktopStates as state (state)}
				<a href={resolve('/[locale]/gallery/[state]', { locale, state })}>{state.toUpperCase()}</a>
			{/each}
		</div>
	</section>

	<section id="gallery-section-identicon">
		<h2>Identicon</h2>
		<div class="board">
			{#each data.board as cell (cell.seed)}
				<figure id="gallery-identicon-{cell.seed}">
					<Identicon svg={cell.svg} size="board" label={cell.seed} />
					<figcaption>{cell.seed}</figcaption>
				</figure>
			{/each}
		</div>
	</section>

	{#if h1s !== undefined}
		<section id="gallery-section-header">
			<h2>WalletHeader · NetworkFilterPill</h2>
			<div class="cell" id="gallery-walletheader-default">
				<WalletHeader header={h1s.header} />
			</div>
			{#if data.models.h7 !== undefined}
				<div class="cell" id="gallery-walletheader-long">
					<WalletHeader header={data.models.h7.header} />
				</div>
				<div class="cell row" id="gallery-networkpill-variants">
					<NetworkFilterPill pill={h1s.pill} />
					<NetworkFilterPill pill={data.models.h7.pill} />
				</div>
			{/if}
		</section>

		<section id="gallery-section-balance">
			<h2>BalanceDisplay</h2>
			{#each ['h1s', 'h2', 'h3', 'h4', 'h5', 'h6', 'h7'] as const as state (state)}
				{#if data.models[state] !== undefined}
					<div class="cell" id="gallery-balance-{state}">
						<BalanceDisplay balance={data.models[state].balance} />
					</div>
				{/if}
			{/each}
		</section>

		<section id="gallery-section-actions">
			<h2>ActionButtonRow</h2>
			<div class="cell" id="gallery-actions-cards">
				<ActionButtonRow receive={m.actions.receive} send={m.actions.send} scan={m.actions.scan} />
			</div>
			<div class="cell" id="gallery-actions-pills">
				<ActionButtonRow
					layout="pills"
					receive={m.actions.receive}
					send={m.actions.send}
					scan={m.actions.scan}
				/>
			</div>
		</section>

		<section id="gallery-section-activity">
			<h2>SectionHeader · ActivityRow</h2>
			<div class="cell" id="gallery-sectionheader-default">
				<SectionHeader title={m.sections.activity} action={m.sections.all} />
			</div>
			{#each h1s.activityGroups as group (group.label)}
				{#each group.rows as row, i (i)}
					<div class="cell" id="gallery-activityrow-{row.kind}-{i}">
						<ActivityRow {row} />
					</div>
				{/each}
			{/each}
			{#if data.models.h5 !== undefined}
				<div class="cell" id="gallery-activityrow-masked">
					<ActivityRow row={data.models.h5.activityGroups[0].rows[0]} />
				</div>
			{/if}
		</section>

		<section id="gallery-section-assets">
			<h2>AssetRow</h2>
			{#if data.models.h4 !== undefined}
				{#each data.models.h4.assetRows as row, i (i)}
					<div class="cell" id="gallery-assetrow-{row.fiat.kind}-{i}">
						<AssetRow {row} />
					</div>
				{/each}
			{/if}
			{#if data.models.h5 !== undefined}
				<div class="cell" id="gallery-assetrow-masked">
					<AssetRow row={data.models.h5.assetRows[0]} />
				</div>
			{/if}
			{#if data.models.h7 !== undefined}
				<div class="cell" id="gallery-assetrow-extreme">
					<AssetRow row={data.models.h7.assetRows[1]} />
				</div>
			{/if}
		</section>

		<section id="gallery-section-empty-loading">
			<h2>EmptyState · Skeleton</h2>
			<div class="cell" id="gallery-empty-activity">
				<EmptyState
					icon={UTILITY_ICONS.inbox}
					title={m.activity.emptyTitle}
					caption={m.activity.emptyCaption}
				/>
			</div>
			<div class="cell" id="gallery-empty-assets">
				<EmptyState
					icon={UTILITY_ICONS.wallet}
					title={m.assets.emptyTitle}
					caption={m.assets.emptyCaption}
				/>
			</div>
			<div class="cell" id="gallery-skeleton-row">
				<SkeletonRow />
				<SkeletonRow kind="block" />
			</div>
		</section>

		<section id="gallery-section-navigation">
			<h2>TabBar · ChainFilterList</h2>
			<div class="cell frameless" id="gallery-tabbar-default">
				<TabBar tabs={h1s.tabs} />
			</div>
			<div class="cell" id="gallery-chainfilterlist-default">
				<ChainFilterList rows={data.sidebar.networks} />
			</div>
		</section>
	{/if}

	<!-- Spec 018: the contacts list-management vocabulary. -->
	<section id="gallery-section-contacts-identicon">
		<h2>Contacts · Identicon (canon seeds)</h2>
		<div class="board">
			{#each c.board as cell (cell.seed)}
				<figure id="gallery-contacts-identicon-{cell.seed}">
					<Identicon svg={cell.svg} size="board" label={cell.seed} />
					<figcaption>{cell.seed}</figcaption>
				</figure>
			{/each}
		</div>
	</section>

	{#if c.list !== undefined && alice !== undefined && ahao !== undefined}
		{@const list = c.list}
		<section id="gallery-section-contacts-rows">
			<h2>ContactRow · GroupRow · GhostAddRow</h2>
			<div class="cell" id="gallery-contacts-contactrow-default">
				<ContactRow contact={alice} />
			</div>
			<div class="cell" id="gallery-contacts-contactrow-hover">
				<ContactRow contact={alice} hover />
			</div>
			<div class="cell" id="gallery-contacts-contactrow-selected">
				<ContactRow contact={alice} selected />
			</div>
			<div class="cell" id="gallery-contacts-contactrow-revealed">
				<ContactRow contact={ahao} revealed actions={list.swipeActions} />
			</div>
			<div class="cell" id="gallery-contacts-contactrow-truncated">
				<ContactRow contact={list.sections[1].contacts[0]} />
			</div>
			{#if c.group !== undefined}
				<div class="cell" id="gallery-contacts-contactrow-member">
					<ContactRow contact={c.group.group.members[0]} />
				</div>
			{/if}
			<div class="cell" id="gallery-contacts-grouprow-default">
				<GroupRow group={list.groups[0]} />
			</div>
			{#if c.group !== undefined}
				<div class="cell" id="gallery-contacts-ghostaddrow-default">
					<GhostAddRow label={c.group.addMember} />
				</div>
			{/if}
		</section>

		<section id="gallery-section-contacts-rail">
			<h2>GroupRail · AlphaIndexRail · SearchField</h2>
			<div class="cell row" id="gallery-contacts-grouprail-default">
				<GroupRail rail={c.rail} />
				<GroupRail rail={c.railDrop} />
			</div>
			<div class="cell row tall" id="gallery-contacts-indexrail-idle">
				<AlphaIndexRail
					letters={list.indexLetters}
					available={list.sections.map((s) => s.letter)}
				/>
				<AlphaIndexRail
					letters={list.indexLetters}
					available={list.sections.map((s) => s.letter)}
					bubble="H"
				/>
			</div>
			<div class="cell" id="gallery-contacts-search-idle">
				<SearchHeader search={list.search} clearLabel={cm.cancel} />
			</div>
			{#if c.filtered !== undefined}
				<div class="cell" id="gallery-contacts-search-filtering">
					<SearchHeader search={c.filtered.search} clearLabel={cm.cancel} />
				</div>
			{/if}
			<div class="cell" id="gallery-contacts-search-desktop">
				<SearchHeader
					search={{ placeholder: cm.searchPlaceholder, shortcut: '⌘F' }}
					layout="desktop"
					clearLabel={cm.cancel}
				/>
			</div>
		</section>
	{/if}

	{#if c.detail !== undefined}
		{@const detail = c.detail}
		<section id="gallery-section-contacts-detail">
			<h2>GroupChips · AddressBlock · RecentActivity</h2>
			<div class="cell" id="gallery-contacts-groupchips-default">
				<GroupChips chips={detail.chips} addLabel={detail.addChipLabel} />
			</div>
			<div class="cell" id="gallery-contacts-addressblock-mobile">
				<AddressBlock address={detail.address} />
			</div>
			<div class="cell" id="gallery-contacts-addressblock-desktop">
				<AddressBlock address={detail.address} layout="desktop" />
			</div>
			<div class="cell" id="gallery-contacts-recentactivity-default">
				<SectionHeader title={detail.activityTitle} action={detail.activityAction} />
				{#each detail.rows as row, i (i)}
					<ActivityRow {row} />
				{/each}
			</div>
		</section>
	{/if}

	<section id="gallery-section-contacts-menus">
		<h2>DropdownMenu · ContextMenu · ActionMenuSheet</h2>
		<div class="cell row" id="gallery-contacts-dropdownmenu-header">
			<DropdownMenu menu={c.menus.header} inline />
		</div>
		<div class="cell row" id="gallery-contacts-contextmenu-group">
			<ContextMenu menu={c.menus.groupContext} inline />
			<ContextMenu menu={c.menus.contactContext} inline />
		</div>
		<div class="cell sheet-cell" id="gallery-contacts-actionmenusheet-add">
			<ActionMenuSheet menu={c.menus.add} />
		</div>
		<div class="cell sheet-cell" id="gallery-contacts-actionmenusheet-group">
			<ActionMenuSheet menu={c.menus.group} />
		</div>
		{#if c.confirm !== undefined}
			<div class="cell sheet-cell" id="gallery-contacts-actionmenusheet-confirm">
				<ActionMenuSheet confirm={c.confirm} />
			</div>
		{/if}
	</section>

	<section id="gallery-section-contacts-empty">
		<h2>EmptyStateCTA · PinnedCTABar</h2>
		{#if c.empty !== undefined}
			<div class="cell" id="gallery-contacts-emptystatecta-empty">
				<EmptyStateCTA empty={c.empty} />
			</div>
			<div class="cell" id="gallery-contacts-emptystatecta-desktop">
				<EmptyStateCTA empty={c.empty} layout="desktop" />
			</div>
		{/if}
		<div class="cell" id="gallery-contacts-emptystatecta-search">
			<EmptyStateCTA empty={c.searchEmpty} icon="search" />
		</div>
		{#if c.group !== undefined}
			<div class="cell" id="gallery-contacts-pinnedctabar-default">
				<PinnedCTABar label={c.group.cta} caption={c.group.ctaCaption} />
			</div>
		{/if}
	</section>

	<!-- Spec 020. The three slides side by side is the ONE view the carousel
	     cannot give you: on a phone you see them one at a time, and whether the
	     drawings are the same weight is a judgement about all three at once. -->
	<section id="gallery-section-intro">
		<h2>Intro slides · PageDots</h2>
		<div class="board">
			{#each INTRO_SLIDES as slide, i (slide.art)}
				<figure id="gallery-intro-{slide.art}">
					<IntroSlide art={slide.art} title={intro[slide.titleKey]} body={intro[slide.bodyKey]} />
					<figcaption>{i + 1} · {slide.art}</figcaption>
				</figure>
			{/each}
		</div>
		{#each [...INTRO_SLIDES.keys()] as i (i)}
			<div class="cell" id="gallery-intro-dots-{i}">
				<PageDots total={INTRO_SLIDES.length} current={i} label="{i + 1} / {INTRO_SLIDES.length}" />
			</div>
		{/each}
	</section>

	<!-- Spec 023: the settings vocabulary. Every one of the forty mocks in
	     design/settings/ is assembled from the components below. -->
	<section id="gallery-section-settings-rows">
		<h2>SettingsRow · SectionLabel · AccountRow</h2>
		<div class="cell" id="gallery-settings-account">
			<AccountRow account={st.account} />
		</div>
		{#each st.sections as section, i (i)}
			{#if section.label !== undefined}
				<div class="cell" id="gallery-settings-label-{i}">
					<SectionLabel
						label={section.label}
						collapsible={section.collapsible}
						collapsed={section.collapsed}
					/>
				</div>
			{/if}
			{#each section.rows as row (row.id)}
				<div class="cell" id="gallery-settings-row-{row.id}">
					<SettingsRow {row} />
				</div>
			{/each}
		{/each}
	</section>

	<section id="gallery-section-settings-controls">
		<h2>SegmentedControl · TextScaleSlider</h2>
		<div class="cell" id="gallery-settings-theme">
			<SegmentedControl model={st.appearance.theme} />
		</div>
		<div class="cell" id="gallery-settings-textscale">
			<TextScaleSlider model={st.appearance.textScale} />
		</div>
	</section>

	<section id="gallery-section-settings-pills">
		<h2>StatusPill · Callout</h2>
		<div class="cell row" id="gallery-settings-pills">
			<StatusPill pill={{ tone: 'ok', label: '45ms', dot: true }} />
			<StatusPill pill={{ tone: 'warn', label: '1.2s', dot: true }} />
			<StatusPill pill={{ tone: 'error', label: st.rpcFixFailing.badge.label, dot: true }} />
			<StatusPill pill={{ tone: 'neutral', label: '—', dot: true }} />
			<StatusPill pill={{ tone: 'accent', label: '112ms' }} />
		</div>
		<div class="cell" id="gallery-settings-callout-warning">
			<Callout callout={st.rpcFixFailing.callout} />
		</div>
		<div class="cell" id="gallery-settings-callout-success">
			<Callout callout={st.rpcFixRestored.callout} />
		</div>
		<div class="cell" id="gallery-settings-callout-danger">
			<Callout callout={{ tone: 'danger', text: st.networkDetail.callout?.text ?? '' }} />
		</div>
		<div class="cell" id="gallery-settings-callout-info">
			<Callout callout={{ tone: 'info', text: st.feedback.consent }} />
		</div>
	</section>

	<section id="gallery-section-settings-select">
		<h2>SelectRow</h2>
		{#each st.languageSheet.rows.slice(0, 3) as row (row.id)}
			<div class="cell" id="gallery-settings-select-{row.id}">
				<SelectRow {row} />
			</div>
		{/each}
		{#each st.currencySheet.rows.slice(0, 2) as row (row.id)}
			<div class="cell" id="gallery-settings-currency-{row.id}">
				<SelectRow {row} />
			</div>
		{/each}
		{#each st.numberSheet.rows.slice(0, 2) as row (row.id)}
			<div class="cell" id="gallery-settings-number-{row.id}">
				<SelectRow {row} />
			</div>
		{/each}
	</section>

	<section id="gallery-section-settings-networks">
		<h2>NetworkRow · ChainMark · UrlField</h2>
		<div class="cell row" id="gallery-settings-marks">
			{#each st.networks.rows as row (row.id)}
				<ChainMark mark={row.mark} />
			{/each}
		</div>
		{#each st.networks.rows.slice(0, 3) as row (row.id)}
			<div class="cell" id="gallery-settings-network-{row.id}">
				<NetworkRow {row} deleteLabel={st.networks.addLabel} />
			</div>
		{/each}
		<div class="cell" id="gallery-settings-url-rpc">
			<UrlField field={st.networkDetail.rpc} />
		</div>
		<div class="cell" id="gallery-settings-url-explorer">
			<UrlField field={st.networkDetail.explorer} />
		</div>
	</section>

	<section id="gallery-section-settings-checks">
		<h2>CheckList</h2>
		{#if st.checks.compatible.checks !== undefined && st.checks.compatible.checksTitle !== undefined}
			<div class="cell" id="gallery-settings-checks-ok">
				<CheckList title={st.checks.compatible.checksTitle} items={st.checks.compatible.checks} />
			</div>
		{/if}
		{#if st.checks.incompatible.checks !== undefined && st.checks.incompatible.checksTitle !== undefined}
			<div class="cell" id="gallery-settings-checks-bad">
				<CheckList
					title={st.checks.incompatible.checksTitle}
					items={st.checks.incompatible.checks}
				/>
			</div>
		{/if}
	</section>

	<section id="gallery-section-settings-storage">
		<h2>StorageBar · StorageGroup · DangerCard</h2>
		<div class="cell" id="gallery-settings-storagebar">
			<StorageBar segments={st.storage.segments} />
		</div>
		{#each st.storage.groups as group (group.label)}
			<div class="cell" id="gallery-settings-storagegroup-{group.label}">
				<StorageGroup {group} />
			</div>
		{/each}
		<div class="cell" id="gallery-settings-dangercard">
			<DangerCard title={st.erase.title} subtitle={st.erase.subtitle} />
		</div>
	</section>

	<section id="gallery-section-settings-panels">
		<h2>RpcProvidersPanel · EndpointsPanel · AboutPanel</h2>
		<div class="cell" id="gallery-settings-providers">
			<RpcProvidersPanel panel={st.rpcProviders} />
		</div>
		<div class="cell" id="gallery-settings-endpoints">
			<EndpointsPanel panel={st.endpoints} />
		</div>
		<div class="cell" id="gallery-settings-about">
			<AboutPanel panel={st.about} />
		</div>
	</section>

	<section id="gallery-section-settings-desktop">
		<h2>FormRow · Dropdown</h2>
		<div class="cell" id="gallery-settings-formrow-dropdown">
			<FormRow label={st.numberSheet.title}>
				<Dropdown value={st.numberSheet.rows[0].label} label={st.numberSheet.title} />
			</FormRow>
		</div>
		<div class="cell" id="gallery-settings-formrow-open">
			<FormRow label={st.numberSheet.title}>
				<Dropdown
					value={st.numberSheet.rows[0].label}
					label={st.numberSheet.title}
					open
					rows={st.numberSheet.rows}
				/>
			</FormRow>
		</div>
		<div class="cell" id="gallery-settings-formrow-segmented">
			<FormRow label={st.appearance.theme.label}>
				<SegmentedControl model={st.appearance.theme} />
			</FormRow>
		</div>
	</section>

	<section id="gallery-section-settings-rescue">
		<h2>RpcBanner · ConfirmSheet</h2>
		{#if st.banner !== undefined}
			<div class="cell" id="gallery-settings-banner">
				<RpcBanner banner={st.banner} />
			</div>
		{/if}
		<div class="cell" id="gallery-settings-confirm-danger">
			<ConfirmSheet sheet={st.signOutSheet} />
		</div>
		<div class="cell" id="gallery-settings-confirm-accent">
			<ConfirmSheet sheet={st.clearCachesSheet} />
		</div>
	</section>
</main>

<style>
	.gallery {
		max-width: var(--layout-maxContentWidth);
		margin-inline: auto;
		padding: var(--space-3xl) var(--layout-screenPaddingX) var(--space-5xl);
		display: flex;
		flex-direction: column;
		gap: var(--space-3xl);
	}

	h1 {
		margin: 0;
		font-size: var(--text-3xl);
		font-weight: var(--weight-bold);
	}

	h2 {
		margin: 0 0 var(--space-lg);
		font-size: var(--text-lg);
		font-weight: var(--weight-semibold);
		color: var(--color-fg-subtle);
		letter-spacing: var(--letterSpacing-sectionLabel);
	}

	.links {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-md);
	}

	.links a {
		display: inline-flex;
		align-items: center;
		height: var(--size-control-sm);
		padding-inline: var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-size: var(--text-base);
		text-decoration: none;
	}

	.board {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-xl);
	}

	.board figure {
		margin: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		max-width: calc(var(--space-5xl) * 3);
	}

	.board figcaption {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-fg-subtle);
		overflow-wrap: anywhere;
		text-align: center;
	}

	.cell {
		padding: var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		margin-bottom: var(--space-lg);
	}

	.cell.row {
		display: flex;
		gap: var(--space-lg);
		flex-wrap: wrap;
	}

	.cell.frameless {
		padding: 0;
		overflow: hidden;
	}

	/* Overlay components need a positioned, clipped host to sit inside. */
	.cell.sheet-cell {
		position: relative;
		height: calc(var(--layout-frameH) / 2);
		padding: 0;
		overflow: hidden;
		background: var(--color-bg-base);
	}

	.cell.tall {
		align-items: stretch;
		min-height: calc(var(--layout-frameH) / 2);
	}
</style>
