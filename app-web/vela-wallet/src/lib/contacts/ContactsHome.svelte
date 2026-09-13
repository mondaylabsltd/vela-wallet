<script lang="ts">
	import ActionButtonRow from '$lib/wallet/ui/ActionButtonRow.svelte';
	import ActivityRow from '$lib/wallet/ui/ActivityRow.svelte';
	import EmptyState from '$lib/wallet/ui/EmptyState.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import SectionHeader from '$lib/wallet/ui/SectionHeader.svelte';
	import TabBar from '$lib/wallet/ui/TabBar.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { ContactsHomeModel } from './model';
	import type { OnContactsUiEvent } from './ui-events';
	import ActionMenuSheet from './ui/ActionMenuSheet.svelte';
	import AddressBlock from './ui/AddressBlock.svelte';
	import AlphaIndexRail from './ui/AlphaIndexRail.svelte';
	import AlphaSectionList from './ui/AlphaSectionList.svelte';
	import ContactRow from './ui/ContactRow.svelte';
	import EmptyStateCTA from './ui/EmptyStateCTA.svelte';
	import GhostAddRow from './ui/GhostAddRow.svelte';
	import GroupChips from './ui/GroupChips.svelte';
	import GroupRow from './ui/GroupRow.svelte';
	import PageHeader from './ui/PageHeader.svelte';
	import PinnedCTABar from './ui/PinnedCTABar.svelte';
	import SearchHeader from './ui/SearchHeader.svelte';

	interface Props {
		model: ContactsHomeModel;
		/** Spec 022: the web app has no 探索 — see TabBar's `destinations`. */
		destinations?: readonly ('wallet' | 'contacts' | 'explore' | 'settings')[];
		/** Live wiring (spec 024). Absent = the gallery's pure picture. */
		onuievent?: OnContactsUiEvent;
	}

	let { model, destinations, onuievent }: Props = $props();

	const live = $derived(onuievent !== undefined);

	// Pure UI state: the fixture opens the sheet; closing/reopening is local.
	// Live models own the sheet's presence instead — the route puts a confirm
	// in and takes it out, so the local latch must not swallow the second one.
	let sheetClosed = $state(false);
	const showSheet = $derived(
		(model.sheet !== undefined || model.confirm !== undefined) && (live || !sheetClosed)
	);

	let scroller = $state<HTMLDivElement | undefined>();

	/** Index-rail jump: direct positioning, no smooth scroll (SPEC 手机). */
	function jump(letter: string) {
		const target = scroller?.querySelector(`#contacts-section-${letter}`);
		if (scroller === undefined || target === null || target === undefined) return;
		scroller.scrollTop += target.getBoundingClientRect().top - scroller.getBoundingClientRect().top;
	}

	const availableLetters = $derived((model.list?.sections ?? []).map((s) => s.letter));
</script>

<div class="home">
	<div class="scroll" bind:this={scroller}>
		{#if model.screen === 'list' || model.screen === 'empty'}
			{@const list = model.list}
			<PageHeader
				title={model.title}
				trailing={[
					{
						icon: 'user-round-plus',
						label: model.addLabel,
						onclick: () => onuievent?.({ kind: 'add' })
					}
				]}
			/>
			{#if list !== undefined}
				<SearchHeader
					search={list.search}
					clearLabel={model.backLabel}
					onquery={(value) => onuievent?.({ kind: 'query', value })}
				/>
			{/if}
		{/if}

		{#if model.screen === 'list' && model.list !== undefined}
			{@const list = model.list}
			{#if list.groups.length > 0 || live}
				<!-- Live books show the section head even before the first group
				     exists — 新建分组 has to live somewhere. Fixtures keep the
				     0-group states exactly as drawn. -->
				<SectionHeader
					title={list.groupsTitle}
					action={list.groupsAction}
					onaction={() => onuievent?.({ kind: 'group-new' })}
				/>
				<div class="groups">
					{#each list.groups as group, i (group.name)}
						<GroupRow
							{group}
							divider={i < list.groups.length - 1}
							onclick={() =>
								group.id !== undefined && onuievent?.({ kind: 'group-open', id: group.id })}
						/>
					{/each}
				</div>
			{/if}

			<SectionHeader title={list.contactsTitle} note={list.contactsCount} />
			<div class="list-region">
				<div class="list">
					{#if list.sections.length === 0 && list.noResults !== undefined}
						<EmptyState
							icon={UTILITY_ICONS.search}
							title={list.noResults}
							caption={list.search.placeholder}
						/>
					{:else}
						<AlphaSectionList
							sections={list.sections}
							revealed={list.revealed}
							swipeActions={list.swipeActions}
							onselect={(contact) => onuievent?.({ kind: 'open', address: contact.addressFull })}
							ondelete={(contact) => {
								sheetClosed = false;
								onuievent?.({ kind: 'delete', address: contact.addressFull });
							}}
						/>
					{/if}
				</div>
				<AlphaIndexRail letters={list.indexLetters} available={availableLetters} onjump={jump} />
			</div>
		{/if}

		{#if model.screen === 'empty' && model.empty !== undefined}
			<div class="center">
				<EmptyStateCTA
					empty={model.empty}
					onprimary={() => onuievent?.({ kind: 'empty-primary' })}
					onsecondary={() => onuievent?.({ kind: 'empty-secondary' })}
				/>
			</div>
		{/if}

		{#if model.screen === 'detail' && model.detail !== undefined}
			{@const detail = model.detail}
			<PageHeader
				back={{ label: model.backLabel, onclick: () => onuievent?.({ kind: 'back' }) }}
				trailing={[
					{ icon: 'pencil', label: model.editLabel, onclick: () => onuievent?.({ kind: 'edit' }) }
				]}
			/>
			<div class="hero">
				<Identicon
					svg={detail.contact.identiconSvg}
					size="hero"
					label={detail.contact.name}
					address={detail.contact.addressFull}
				/>
				<p class="hero-name">{detail.contact.name}</p>
				<p class="hero-address">{detail.contact.addressDisplay}</p>
				<GroupChips
					chips={detail.chips}
					addLabel={detail.addChipLabel}
					onadd={() =>
						onuievent?.({ kind: 'action', id: 'move-group', address: detail.contact.addressFull })}
				/>
			</div>

			<div class="detail-actions">
				<ActionButtonRow
					items={[
						{
							label: detail.actions.send,
							icon: UTILITY_ICONS['arrow-up-right'],
							onclick: () =>
								onuievent?.({ kind: 'action', id: 'send', address: detail.contact.addressFull })
						},
						{
							label: detail.actions.receive,
							icon: UTILITY_ICONS['arrow-down-left'],
							onclick: () =>
								onuievent?.({ kind: 'action', id: 'receive', address: detail.contact.addressFull })
						},
						{
							label: detail.actions.qr,
							icon: UTILITY_ICONS['qr-code'],
							onclick: () =>
								onuievent?.({ kind: 'action', id: 'qr', address: detail.contact.addressFull })
						}
					]}
				/>
			</div>

			<hr />

			<AddressBlock
				address={detail.address}
				oncopy={() =>
					onuievent?.({ kind: 'action', id: 'copy', address: detail.contact.addressFull })}
			/>

			<div class="activity">
				<SectionHeader
					title={detail.activityTitle}
					action={detail.rows.length > 0 ? detail.activityAction : undefined}
					onaction={() => onuievent?.({ kind: 'activity-all' })}
				/>
				{#if detail.rows.length === 0 && detail.emptyActivity !== undefined}
					<p class="activity-empty">{detail.emptyActivity}</p>
				{:else}
					<ul>
						{#each detail.rows as row, i (i)}
							<li><ActivityRow {row} /></li>
						{/each}
					</ul>
				{/if}
			</div>

			<button
				type="button"
				class="destructive-text"
				onclick={() => {
					sheetClosed = false;
					onuievent?.({ kind: 'delete', address: detail.contact.addressFull });
				}}
			>
				{detail.deleteLabel}
			</button>
		{/if}

		{#if model.screen === 'group' && model.group !== undefined}
			{@const group = model.group}
			<PageHeader
				back={{ label: model.backLabel, onclick: () => onuievent?.({ kind: 'back' }) }}
				trailing={[
					{
						icon: 'ellipsis',
						label: group.menuLabel,
						onclick: () => {
							if (group.group.id === undefined) return;
							sheetClosed = false;
							onuievent?.({ kind: 'group-menu', id: group.group.id });
						}
					}
				]}
			/>
			<h1 class="group-title">{group.group.name}</h1>
			<p class="group-count">{group.group.membersLabel}</p>
			<div class="members">
				{#each group.group.members as member, i (member.addressFull)}
					<ContactRow
						contact={member}
						divider={i < group.group.members.length - 1}
						onclick={() => onuievent?.({ kind: 'open', address: member.addressFull })}
					/>
				{/each}
				<GhostAddRow label={group.addMember} onclick={() => onuievent?.({ kind: 'add-member' })} />
			</div>
		{/if}
	</div>

	{#if model.screen === 'group' && model.group !== undefined}
		{@const group = model.group}
		<PinnedCTABar
			label={group.cta}
			caption={group.ctaCaption}
			disabled={group.group.members.length === 0}
			onclick={() =>
				group.group.id !== undefined && onuievent?.({ kind: 'batch-send', id: group.group.id })}
		/>
	{/if}

	{#if model.screen === 'list' || model.screen === 'empty'}
		<TabBar
			tabs={model.tabs}
			selected="contacts"
			{destinations}
			onselect={(id) => onuievent?.({ kind: 'tab', id })}
		/>
	{/if}

	{#if showSheet}
		<ActionMenuSheet
			menu={model.sheet}
			confirm={model.confirm}
			onselect={(label) => onuievent?.({ kind: 'sheet-select', label })}
			onclose={() => {
				sheetClosed = true;
				onuievent?.({ kind: 'sheet-close' });
			}}
		/>
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
		display: flex;
		flex-direction: column;
		padding-inline: var(--layout-screenPaddingX);
	}

	.groups {
		display: flex;
		flex-direction: column;
	}

	.list-region {
		display: flex;
		align-items: flex-start;
		gap: var(--space-md);
		/* rows bleed to the padding edge so the raised hover state reaches it */
		margin-inline: calc(var(--space-lg) * -1);
	}

	.list {
		flex: 1;
		min-width: 0;
	}

	.list-region :global(.rail) {
		position: sticky;
		top: var(--space-md);
		margin-inline-end: calc(var(--space-lg) * -1);
	}

	.center {
		flex: 1;
		display: flex;
		flex-direction: column;
		justify-content: center;
	}

	.hero {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		padding-block: var(--space-2xl) var(--space-3xl);
	}

	.hero-name {
		margin: var(--space-lg) 0 0;
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.hero-address {
		margin: 0 0 var(--space-sm);
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.detail-actions {
		padding-bottom: var(--space-2xl);
	}

	hr {
		width: 100%;
		border: none;
		border-top: var(--border-hairline) solid var(--color-border-base);
		margin-block: 0 var(--space-2xl);
	}

	.activity {
		padding-top: var(--space-2xl);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.activity-empty {
		margin: var(--space-md) 0 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.destructive-text {
		align-self: center;
		margin-block: var(--space-4xl);
		padding: var(--space-md) var(--space-xl);
		border: none;
		background: none;
		color: var(--color-error-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		cursor: pointer;
	}

	.group-title {
		margin: var(--space-2xl) 0 var(--space-sm);
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.group-count {
		margin: 0 0 var(--space-2xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.members {
		display: flex;
		flex-direction: column;
		margin-inline: calc(var(--space-lg) * -1);
	}
</style>
