<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Sidebar from '$lib/wallet/ui/Sidebar.svelte';
	import ThirdPanel from '$lib/wallet/ui/ThirdPanel.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import { BREAKPOINT_CONTACTS_OVERLAY } from '$lib/tokens/tokens';
	import type { ChainRowModel } from '$lib/wallet/model';
	import type { ContactDraft, ContactFormCopy } from './forms';
	import type { ContactModel, ContactsDesktopModel, ContactsPanelId } from './model';
	import type { OnContactsUiEvent } from './ui-events';
	import AlphaSectionList from './ui/AlphaSectionList.svelte';
	import ContactDetailPanel from './ui/ContactDetailPanel.svelte';
	import ContactForm from './ui/ContactForm.svelte';
	import ContactRow from './ui/ContactRow.svelte';
	import ContextMenu from './ui/ContextMenu.svelte';
	import DropdownMenu from './ui/DropdownMenu.svelte';
	import EmptyStateCTA from './ui/EmptyStateCTA.svelte';
	import GhostAddRow from './ui/GhostAddRow.svelte';
	import GroupRail from './ui/GroupRail.svelte';
	import SearchHeader from './ui/SearchHeader.svelte';

	interface Props {
		model: ContactsDesktopModel;
		/** Live wiring (spec 024). Absent = the gallery's pure picture. */
		onuievent?: OnContactsUiEvent;
		/** The sidebar's network filter was used. Absent in the gallery. */
		onchainselect?: (row: ChainRowModel) => void;
		/** The sidebar header's name button: the account switcher. Absent in the gallery. */
		onaccounts?: () => void;
		/**
		 * The add/edit form, when one is open. It takes the third column — what
		 * the phone draws as a bottom sheet — because a sheet sliding up the
		 * bottom of a desktop window is a phone control at the wrong size.
		 * `initial` present = editing, and the column offers a way BACK to the
		 * detail it replaced.
		 */
		contactForm?: {
			copy: ContactFormCopy;
			initial?: ContactDraft;
			onsave: (draft: ContactDraft) => void;
			onclose: () => void;
		};
	}

	let { model, onuievent, onchainselect, onaccounts, contactForm }: Props = $props();

	// The third column plays the mobile bottom-sheet role (spec 015 mechanics
	// reused); pure UI state seeded by the fixture.
	let panel: ContactsPanelId = $derived(model.initialPanel);
	let closing = $state(false);
	let selected = $derived(model.selectedContact);

	let openMenu = $derived<'header' | 'group' | 'contact' | undefined>(model.openMenu);
	let menuAt = $state<{ x: number; y: number } | undefined>(undefined);
	let railHost = $state<HTMLDivElement | undefined>();

	/** Live <1120 overlay mode; `forceOverlay` pins it for the dc2n stage (D6). */
	let narrow = $state(false);
	const overlay = $derived(model.forceOverlay || narrow);

	onMount(() => {
		const query = window.matchMedia(`(max-width: ${BREAKPOINT_CONTACTS_OVERLAY}px)`);
		const sync = () => (narrow = query.matches);
		sync();
		query.addEventListener('change', sync);
		return () => query.removeEventListener('change', sync);
	});

	// DC6 renders the context menu with no pointer behind it: anchor it to the
	// selected group row the way a real right-click on that row would.
	onMount(() => {
		if (model.openMenu !== 'group') return;
		const name = model.rail.selectedGroup ?? model.rail.groups[0]?.name;
		if (name === undefined) return;
		const row = railHost?.querySelector(`[data-contacts-group="${name}"]`);
		if (row === null || row === undefined) return;
		const box = row.getBoundingClientRect();
		menuAt = { x: box.left + box.width / 2, y: box.bottom };
	});

	/** A rail row's right-click: the menu opens here, and the route learns whose it is. */
	function openContext(group: string, event: MouseEvent) {
		event.preventDefault();
		menuAt = { x: event.clientX, y: event.clientY };
		openMenu = 'group';
		const id = model.rail.groups.find((row) => row.name === group)?.id;
		if (id !== undefined) onuievent?.({ kind: 'group-menu', id });
	}

	/** The open group's ⋯ — the same menu, anchored under the button. */
	function openGroupMenuAt(event: MouseEvent) {
		const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
		menuAt = { x: box.right, y: box.bottom };
		openMenu = 'group';
		const id = model.group?.group.id;
		if (id !== undefined) onuievent?.({ kind: 'group-menu', id });
	}

	/** A contact row's right-click (the drawn M2 menu). */
	function openContactContext(contact: ContactModel, event: MouseEvent) {
		event.preventDefault();
		menuAt = { x: event.clientX, y: event.clientY };
		openMenu = 'contact';
		onuievent?.({ kind: 'contact-menu', address: contact.addressFull });
	}

	function closeContext() {
		openMenu = undefined;
		menuAt = undefined;
	}

	function closePanel() {
		if (panel !== 'none') closing = true;
	}

	function onPanelAnimationEnd() {
		if (!closing) return;
		closing = false;
		panel = 'none';
		selected = undefined;
		// The route's `ui` still names the contact; `back` is what forgets it.
		onuievent?.({ kind: 'back' });
	}

	/** A rail row names a group; the route speaks in ids. 全部 is `back`. */
	function selectGroup(name: string | undefined) {
		if (name === undefined) {
			onuievent?.({ kind: 'back' });
			return;
		}
		const id = model.rail.groups.find((group) => group.name === name)?.id;
		if (id !== undefined) onuievent?.({ kind: 'group-open', id });
	}
</script>

<div class="desktop" class:overlay-mode={model.forceOverlay}>
	<Sidebar
		sidebar={model.sidebar}
		onnav={(id) => onuievent?.({ kind: 'tab', id })}
		{onaccounts}
		{onchainselect}
	/>

	<main>
		<header class="page-head">
			<h1>{model.title}</h1>
			<span class="spacer"></span>
			<div class="search">
				<SearchHeader
					search={model.search}
					layout="desktop"
					clearLabel={model.closeLabel}
					onquery={(value) => onuievent?.({ kind: 'query', value })}
				/>
			</div>
			<button type="button" class="add" onclick={() => onuievent?.({ kind: 'add' })}>
				<Icon icon={UTILITY_ICONS['user-round-plus']} size="base" />
				<span>{model.addLabel}</span>
			</button>
			<div class="menu-anchor">
				<button
					type="button"
					class="icon-button"
					aria-label={model.menuLabel}
					aria-haspopup="menu"
					onclick={() => (openMenu = openMenu === 'header' ? undefined : 'header')}
				>
					<Icon icon={UTILITY_ICONS.ellipsis} size="base" />
				</button>
				{#if openMenu === 'header'}
					<DropdownMenu
						menu={model.headerMenu}
						onselect={(label) => {
							openMenu = undefined;
							onuievent?.({ kind: 'sheet-select', label });
						}}
						onclose={() => (openMenu = undefined)}
					/>
				{/if}
			</div>
		</header>

		<hr class="head-rule" />

		<div class="body">
			<div class="rail-host" bind:this={railHost}>
				<GroupRail
					rail={model.rail}
					onselect={selectGroup}
					ongroupmenu={openContext}
					onnew={() => onuievent?.({ kind: 'group-new' })}
				/>
			</div>

			<div class="content">
				{#if model.empty !== undefined}
					<div class="center">
						<EmptyStateCTA
							empty={model.empty}
							layout="desktop"
							onprimary={() => onuievent?.({ kind: 'empty-primary' })}
							onsecondary={() => onuievent?.({ kind: 'empty-secondary' })}
						/>
					</div>
				{:else if model.group !== undefined}
					{@const group = model.group}
					<div class="group-head">
						<h2>{group.group.name}</h2>
						<p class="members">{group.group.membersLabel}</p>
						<span class="spacer"></span>
						<button
							type="button"
							class="cta"
							disabled={group.group.members.length === 0}
							onclick={() =>
								group.group.id !== undefined &&
								onuievent?.({ kind: 'batch-send', id: group.group.id })}
						>
							{group.cta}
						</button>
						<button
							type="button"
							class="icon-button"
							aria-label={group.menuLabel}
							aria-haspopup="menu"
							onclick={openGroupMenuAt}
						>
							<Icon icon={UTILITY_ICONS.ellipsis} size="base" />
						</button>
					</div>
					<ul class="members-list">
						{#each group.group.members as member, i (member.addressFull)}
							<li>
								<ContactRow
									contact={member}
									divider={i < group.group.members.length - 1}
									onclick={() => {
										panel = 'contact-detail';
										selected = member.name;
										onuievent?.({ kind: 'open', address: member.addressFull });
									}}
									oncontextmenu={(event) => openContactContext(member, event)}
								/>
							</li>
						{/each}
					</ul>
					<GhostAddRow
						label={group.addMember}
						onclick={() => onuievent?.({ kind: 'add-member' })}
					/>
					<p class="caption">{group.captionTitled}</p>
				{:else}
					<AlphaSectionList
						sections={model.sections}
						{selected}
						onselect={(contact) => {
							panel = 'contact-detail';
							selected = contact.name;
							onuievent?.({ kind: 'open', address: contact.addressFull });
						}}
						oncontactmenu={openContactContext}
					/>
				{/if}
			</div>
		</div>
	</main>

	{#if contactForm !== undefined}
		{@const form = contactForm}
		{#if overlay}
			<div class="scrim" role="presentation" onclick={form.onclose}></div>
		{/if}
		<div class="panel-host">
			<ThirdPanel
				title={form.copy.title}
				closeLabel={form.copy.cancel}
				backLabel={form.initial !== undefined ? form.copy.cancel : undefined}
				onback={form.initial !== undefined ? form.onclose : undefined}
				onclose={form.onclose}
			>
				<!-- Keyed per contact: the form seeds its fields once, on open. -->
				{#key form.initial?.address ?? ''}
					<ContactForm copy={form.copy} initial={form.initial} onsave={form.onsave} />
				{/key}
			</ThirdPanel>
		</div>
	{:else if panel === 'contact-detail' || closing}
		{#if overlay}
			<div class="scrim" role="presentation" onclick={closePanel}></div>
		{/if}
		<div class="panel-host" class:closing onanimationend={onPanelAnimationEnd}>
			<ThirdPanel title={model.panelTitle} closeLabel={model.closeLabel} onclose={closePanel}>
				{#if model.detail !== undefined}
					{@const detail = model.detail}
					<ContactDetailPanel
						{detail}
						onedit={() => onuievent?.({ kind: 'edit' })}
						ondelete={() => onuievent?.({ kind: 'delete', address: detail.contact.addressFull })}
						onaction={(id) =>
							onuievent?.({ kind: 'action', id, address: detail.contact.addressFull })}
						oncopy={() =>
							onuievent?.({ kind: 'action', id: 'copy', address: detail.contact.addressFull })}
						onaddgroup={() =>
							onuievent?.({
								kind: 'action',
								id: 'move-group',
								address: detail.contact.addressFull
							})}
						onactivityall={() => onuievent?.({ kind: 'activity-all' })}
					/>
				{/if}
			</ThirdPanel>
		</div>
	{/if}

	{#if openMenu === 'group'}
		<ContextMenu
			menu={model.groupMenu}
			at={menuAt}
			onselect={(label) => {
				closeContext();
				onuievent?.({ kind: 'sheet-select', label });
			}}
			onclose={closeContext}
		/>
	{:else if openMenu === 'contact'}
		<ContextMenu
			menu={model.contactMenu}
			at={menuAt}
			onselect={(label) => {
				closeContext();
				onuievent?.({ kind: 'sheet-select', label });
			}}
			onclose={closeContext}
		/>
	{/if}
</div>

<style>
	.desktop {
		position: relative;
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
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.page-head {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-xl) var(--space-3xl);
	}

	h1 {
		margin: 0;
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.spacer {
		flex: 1;
	}

	.search {
		width: calc(var(--layout-maxContentWidth) / 3);
		min-width: 0;
	}

	.add {
		display: inline-flex;
		align-items: center;
		gap: var(--space-md);
		height: var(--size-control-sm);
		padding-inline: var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		white-space: nowrap;
		cursor: pointer;
	}

	.add:hover {
		opacity: var(--opacity-hover);
	}

	.menu-anchor {
		position: relative;
	}

	.icon-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.icon-button:hover {
		color: var(--color-fg-base);
	}

	.head-rule {
		width: 100%;
		border: none;
		border-top: var(--border-hairline) solid var(--color-border-base);
		margin: 0;
	}

	.body {
		flex: 1;
		min-height: 0;
		display: flex;
		gap: var(--space-3xl);
		padding: var(--space-3xl);
		overflow-y: auto;
	}

	.rail-host {
		flex-shrink: 0;
	}

	.content {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}

	.center {
		flex: 1;
		display: flex;
		flex-direction: column;
		justify-content: center;
	}

	.group-head {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-bottom: var(--space-2xl);
	}

	.group-head h2 {
		margin: 0;
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.members {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.cta {
		display: inline-flex;
		align-items: center;
		height: var(--size-control-sm);
		padding-inline: var(--space-xl);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-accent-base);
		color: var(--color-onAccent);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		white-space: nowrap;
		cursor: pointer;
	}

	.cta:active {
		transform: scale(var(--motion-press-button));
	}

	.cta:disabled {
		opacity: var(--opacity-disabled, 0.4);
		cursor: default;
	}

	.members-list {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.caption {
		margin: var(--space-xl) 0 0;
		padding-inline: var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	/* Third column: 0 → 400 in 240ms ease-out, close 200ms (SPEC 桌面). */
	.panel-host {
		display: flex;
		flex-shrink: 0;
		overflow: hidden;
		animation: panel-in var(--motion-panel-in) ease-out;
	}

	.panel-host.closing {
		animation: panel-out var(--motion-panel-out) ease-out forwards;
	}

	@keyframes panel-in {
		from {
			transform: translateX(100%);
			opacity: 0;
		}

		to {
			transform: translateX(0);
			opacity: 1;
		}
	}

	@keyframes panel-out {
		from {
			transform: translateX(0);
			opacity: 1;
		}

		to {
			transform: translateX(100%);
			opacity: 0;
		}
	}

	.scrim {
		position: absolute;
		inset: 0;
		z-index: 1;
		background: var(--color-fixed-backdrop);
	}

	/* Narrow window: the column overlays the list instead of squeezing it.
	   The literal below is `--breakpoint-contactsOverlay` spelled out — a media
	   query cannot read a custom property, so the token audit whitelists it
	   (alongside --breakpoint-desktop) and pins it to the token export. */
	@media (max-width: 1120px) {
		.panel-host {
			position: absolute;
			inset-block: 0;
			inset-inline-end: 0;
			z-index: 2;
			box-shadow: var(--shadow-lg);
		}
	}

	.overlay-mode .panel-host {
		position: absolute;
		inset-block: 0;
		inset-inline-end: 0;
		z-index: 2;
		box-shadow: var(--shadow-lg);
	}

	/* Reduced motion: slide is replaced by an opacity crossfade (FR-011). */
	@media (prefers-reduced-motion: reduce) {
		.panel-host {
			animation: fade-in var(--motion-crossfade) ease-out;
		}

		.panel-host.closing {
			animation: fade-out var(--motion-crossfade) ease-out forwards;
		}

		@keyframes fade-in {
			from {
				opacity: 0;
			}

			to {
				opacity: 1;
			}
		}

		@keyframes fade-out {
			from {
				opacity: 1;
			}

			to {
				opacity: 0;
			}
		}
	}
</style>
