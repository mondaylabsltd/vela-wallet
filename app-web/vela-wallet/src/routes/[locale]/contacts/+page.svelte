<script lang="ts">
	/**
	 * The address book, live (spec 024; completed in 028 US5).
	 *
	 * The same three responsibilities as /wallet and /settings — the guard,
	 * the identity of the machinery (a route-scoped ContactsCore session,
	 * research D8), the way out (the tab bar) — plus the one thing this route
	 * pioneers: a full interaction surface over components that were pure
	 * pictures. Which SCREEN is showing (list/detail/group) is render state
	 * here; WHAT the book contains is only ever the core's view.
	 *
	 * 028 US5 finished the surface. Everything 018 drew now does something,
	 * and every rule behind it is the core's: the file format and import
	 * policy (`import_file` / `export_requested`), group membership
	 * (`set_group_members` / `set_contact_groups`), the recent-recipient
	 * suggestions (`load_send_history`, now real). This route translates taps
	 * into events, hands files in and out, and carries a person to /wallet
	 * when they want to move money (`contact-handoff.ts`).
	 */
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { MediaQuery } from 'svelte/reactivity';
	import ContactsDesktop from '$lib/contacts/ContactsDesktop.svelte';
	import ContactsHome from '$lib/contacts/ContactsHome.svelte';
	import ContactEditSheet from '$lib/contacts/ui/ContactEditSheet.svelte';
	import ContactQr from '$lib/contacts/ui/ContactQr.svelte';
	import GroupEditSheet from '$lib/contacts/ui/GroupEditSheet.svelte';
	import GroupForm from '$lib/contacts/ui/GroupForm.svelte';
	import PickList from '$lib/contacts/ui/PickList.svelte';
	import SheetOrDialog from '$lib/contacts/ui/SheetOrDialog.svelte';
	import type { ContactFormCopy, GroupFormCopy } from '$lib/contacts/forms';
	import { createContactsSession, type ContactsSession } from '$lib/contacts/core/contacts';
	import { addMenu, groupMenuMobile } from '$lib/contacts/fixtures';
	import {
		buildContactsDesktopLive,
		buildContactsLive,
		displayName,
		groupPickModel,
		importReport,
		memberPickModel,
		type ContactsUiState
	} from '$lib/contacts/live';
	import type { ContactsUiEvent } from '$lib/contacts/ui-events';
	import type { ContactExportScope } from '$lib/core/generated/ContactExportScope';
	import type { ContactFileFormat } from '$lib/core/generated/ContactFileFormat';
	import type { ContactsView } from '$lib/core/generated/ContactsView';
	import { loadCore } from '$lib/core/client';
	import { flowHandoffQuery, type FlowHandoff } from '$lib/flows/contact-handoff';
	import { pickTextFile, saveTextFile } from '$lib/services/file-io';
	import { preferences } from '$lib/services/preferences.svelte';
	import { session } from '$lib/session/core/session.svelte';
	import AccountSwitcher from '$lib/session/ui/AccountSwitcher.svelte';
	import IdenticonViewerHost from '$lib/wallet/ui/IdenticonViewerHost.svelte';
	import Dialog from '$lib/settings/ui/Dialog.svelte';
	import { BREAKPOINT_DESKTOP } from '$lib/tokens/tokens';
	import Button from '$lib/ui/Button.svelte';
	import { balance } from '$lib/wallet/core/balance.svelte';
	import { feed } from '$lib/wallet/core/feed.svelte';
	import { WEB_DESTINATIONS } from '$lib/wallet/destinations';
	import { avatarSvgForClient } from '$lib/wallet/identicon';
	import { shortenAddress } from '$lib/wallet/identity';
	import { fill } from '$lib/wallet/messages';
	import { encodeQr } from '$lib/wallet/qr';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const m = $derived(data.contactsMessages);

	const welcome = $derived(resolve('/[locale]', { locale: data.locale }));
	const walletHref = $derived(resolve('/[locale]/wallet', { locale: data.locale }));
	const createHref = $derived(resolve('/[locale]/create', { locale: data.locale }));

	/** The account switcher behind the sidebar header's name (founder call, 2026-09-05). */
	let switching = $state(false);
	const settingsHref = $derived(resolve('/[locale]/settings', { locale: data.locale }));

	const sessionView = $derived(session.view);
	const signedIn = $derived(sessionView.allowed_route === 'wallet');
	/** Past the breakpoint the book is three columns (DC1), as on /wallet and /settings. */
	const wide = new MediaQuery(`(min-width: ${BREAKPOINT_DESKTOP}px)`, false);

	// --- The machine -------------------------------------------------------

	let view = $state<ContactsView | null>(null);
	let contacts: ContactsSession | null = null;
	let disposed = false;
	/** The account the core was last told about — a switch re-reads the book. */
	let toldAddress: string | null = null;

	onMount(() => {
		void session.boot();
		void balance.boot();
		// The feed is what 最近往来 reads (spec 028 US5) — the same resident the
		// wallet home draws, pointed at the same account below.
		void feed.boot();
		preferences.boot();
		void (async () => {
			await loadCore();
			if (disposed) return;
			contacts = createContactsSession({
				onView: (next) => (view = next),
				onError: (error) => console.error('[contacts] core fault:', error)
			});
			// Hydrate: reads the three stores and the local send history. The
			// address is the session's, when it already has one; the effect
			// below tells the core the moment it does.
			toldAddress = signedIn ? sessionView.address : null;
			contacts.start({ type: 'account_switched', my_address: toldAddress });
		})();
		return () => {
			disposed = true;
			contacts?.dispose();
			contacts = null;
		};
	});

	$effect(() => {
		if (sessionView.allowed_route === 'onboarding') void goto(welcome, { replaceState: true });
	});

	$effect(() => {
		if (!signedIn) return;
		void balance.setAccount(sessionView.address);
		void feed.setAccount(sessionView.address);
		// The core's own account boundary: history-derived suggestions must
		// never cross books (contacts.rs `AccountSwitched`).
		if (contacts !== null && sessionView.address !== toldAddress) {
			toldAddress = sessionView.address;
			contacts.dispatch({ type: 'account_switched', my_address: toldAddress });
		}
	});

	// --- Render state ------------------------------------------------------

	let ui = $state<ContactsUiState>({ screen: 'list', query: '' });

	/**
	 * What sits over the screen. The forms and the two drawn menus (C5 / C6)
	 * are the phone's sheets and the desktop's column or dialog; the rest —
	 * the pickers, the QR, the export format, a rename, the confirms — are
	 * `SheetOrDialog`, one component for both layouts.
	 */
	type SheetState =
		| { kind: 'none' }
		| { kind: 'add-menu' }
		| { kind: 'add' }
		| { kind: 'edit'; name: string; address: string }
		| { kind: 'group-new' }
		| { kind: 'group-rename'; id: string; name: string }
		| { kind: 'group-menu'; id: string }
		| { kind: 'confirm-delete'; address: string; name: string }
		| { kind: 'confirm-group-delete'; id: string; name: string }
		| { kind: 'member-pick'; id: string }
		| { kind: 'group-pick'; address: string }
		| { kind: 'qr'; address: string }
		| { kind: 'export'; scope: ContactExportScope };
	let sheet = $state<SheetState>({ kind: 'none' });

	/**
	 * Whose menu is open. The desktop anchors its own context menus and only
	 * tells the route which row was right-clicked; the pick that follows
	 * arrives as `sheet-select` and is resolved against this.
	 */
	let menuTarget = $state<
		{ kind: 'group'; id: string } | { kind: 'contact'; address: string } | null
	>(null);

	/**
	 * A sheet row that OPENS another sheet is followed, synchronously, by the
	 * menu sheet's own `sheet-close` — which must not shut what was just
	 * opened. Set when a selection chained, consumed by the next close.
	 */
	let swallowNextClose = false;

	/** The address copy's one-word acknowledgement, briefly. */
	let copied = $state<string | null>(null);
	let copyTimer: ReturnType<typeof setTimeout> | undefined;

	const extras = $derived({ feed: feed.view });

	const model = $derived.by(() => {
		if (view === null) return null;
		const built = buildContactsLive(view, m, avatarSvgForClient, ui, extras);
		switch (sheet.kind) {
			case 'confirm-delete':
				return {
					...built,
					confirm: {
						title: m.deleteTitle,
						body: fill(m.deleteBody, { name: sheet.name }),
						confirm: m.delete,
						cancel: m.cancel
					}
				};
			case 'confirm-group-delete':
				return {
					...built,
					confirm: {
						title: m.groupDelete,
						body: fill(m.groupDeleteBody, { name: sheet.name }),
						confirm: m.groupDelete,
						cancel: m.cancel
					}
				};
			case 'add-menu':
				return { ...built, sheet: addMenu(m) };
			case 'group-menu':
				return { ...built, sheet: groupMenuMobile(m) };
			default:
				return built;
		}
	});

	/**
	 * The wide layout's app sidebar: the session's identity over the empty
	 * header. No network list — that is the wallet's filter (spec 028 Phase 9,
	 * RULING 2), and a filter with nothing to filter is noise wearing a
	 * checkmark.
	 */
	const sidebar = $derived.by(() => {
		if (!signedIn) return data.sidebar;
		const name = sessionView.accounts[sessionView.active_index]?.account.name ?? '';
		return {
			...data.sidebar,
			header: {
				name,
				addressDisplay: shortenAddress(sessionView.address),
				addressFull: sessionView.address,
				identiconSvg: avatarSvgForClient(sessionView.address, name)
			}
		};
	});

	/** The same `ui`, rendered beside the list rather than pushed over it. */
	const desktopModel = $derived(
		view === null
			? null
			: buildContactsDesktopLive(view, m, avatarSvgForClient, ui, sidebar, extras)
	);

	/** The import's outcome, in the corpus's words, until acknowledged. */
	const report = $derived(view === null ? undefined : importReport(view, m));

	/** The QR sheet's subject, resolved from the book so a rename shows through. */
	const qrSubject = $derived.by(() => {
		if (sheet.kind !== 'qr' || view === null) return undefined;
		const address = sheet.address;
		const contact = view.contacts.find((c) => c.address === address);
		if (contact === undefined) return undefined;
		const name = displayName(contact);
		return {
			name,
			address: contact.address,
			identiconSvg: avatarSvgForClient(contact.address, name),
			// The address, encoded — what the receive card does for our own.
			code: encodeQr(contact.address)
		};
	});

	// --- Files in and out --------------------------------------------------

	/** The export the core wrote is handed over the moment it appears, then taken. */
	$effect(() => {
		const file = view?.export;
		if (file === undefined || file === null) return;
		void saveTextFile(file.filename, file.content, file.mime).then(() =>
			contacts?.dispatch({ type: 'export_taken' })
		);
	});

	async function importBook(intoGroup: string | undefined): Promise<void> {
		const file = await pickTextFile('.json,.csv,.txt');
		if (file === null) return;
		// The core sniffs the format, parses, and rules (existing-wins; a bad
		// file is refused before any write). The report lands in the view.
		contacts?.dispatch({
			type: 'import_file',
			content: file.text,
			filename: file.name,
			into_group: intoGroup ?? null,
			now_ms: Date.now()
		});
	}

	function exportBook(format: ContactFileFormat): void {
		if (sheet.kind !== 'export') return;
		contacts?.dispatch({
			type: 'export_requested',
			scope: sheet.scope,
			format,
			exported_at_iso: new Date().toISOString()
		});
		sheet = { kind: 'none' };
	}

	// --- Money: hand the person to /wallet ----------------------------------

	/** The wallet route reads the query once and opens the flow (`contact-handoff.ts`). */
	function handOff(handoff: FlowHandoff): void {
		void goto(resolve(`/[locale]/wallet${flowHandoffQuery(handoff)}`, { locale: data.locale }));
	}

	function sendTo(address: string): void {
		handOff({ kind: 'send', recipient: address });
	}

	function receiveFrom(): void {
		handOff({ kind: 'receive' });
	}

	/**
	 * 群发转账. A group of one is a send to that one — the recipient arrives
	 * prefilled and the form opens on them at once; two or more seed split
	 * mode once a token is chosen. An empty group's button is disabled and its
	 * caption says why, so this is never reached with nobody.
	 */
	function batchSend(groupId: string): void {
		const group = view?.groups.find((g) => g.id === groupId);
		if (group === undefined || group.members.length === 0) return;
		const only = group.members.length === 1 ? group.members[0] : undefined;
		if (only !== undefined) sendTo(only.address);
		else handOff({ kind: 'group-send', groupId });
	}

	async function copyAddress(address: string): Promise<void> {
		try {
			await navigator.clipboard.writeText(address);
			copied = address;
			clearTimeout(copyTimer);
			copyTimer = setTimeout(() => (copied = null), 1500);
		} catch {
			// The address is on screen in full; a refused clipboard is not a
			// wallet fault and gets no alarm.
		}
	}

	// --- The translation table: what happened → what the core is told ------

	function openEdit(address: string | undefined): void {
		const contact = view?.contacts.find((c) => c.address === address);
		if (contact !== undefined)
			sheet = { kind: 'edit', name: contact.name ?? '', address: contact.address };
	}

	function askDelete(address: string): void {
		const contact = view?.contacts.find((c) => c.address === address);
		sheet = {
			kind: 'confirm-delete',
			address,
			name: contact !== undefined ? displayName(contact) : address
		};
	}

	function deleteContact(address: string): void {
		contacts?.dispatch({ type: 'delete', address, now_ms: Date.now() });
		sheet = { kind: 'none' };
		if (ui.selectedAddress === address) {
			ui = { ...ui, screen: 'list', selectedAddress: undefined };
		}
	}

	function deleteGroup(id: string): void {
		contacts?.dispatch({ type: 'group_delete', id });
		sheet = { kind: 'none' };
		if (ui.selectedGroupId === id) {
			ui = { ...ui, screen: 'list', selectedGroupId: undefined };
		}
	}

	/**
	 * A row of an open menu or confirm, by label. Confirms resolve first —
	 * their labels are also menu rows — then the menus, against whichever
	 * group or contact the menu was opened for.
	 */
	function onSheetSelect(label: string): void {
		const from = sheet;
		if (from.kind === 'confirm-delete') {
			if (label === m.delete) deleteContact(from.address);
			return;
		}
		if (from.kind === 'confirm-group-delete') {
			if (label === m.groupDelete) deleteGroup(from.id);
			return;
		}
		const groupId =
			from.kind === 'group-menu'
				? from.id
				: menuTarget?.kind === 'group'
					? menuTarget.id
					: ui.selectedGroupId;
		const address = menuTarget?.kind === 'contact' ? menuTarget.address : ui.selectedAddress;
		const group = view?.groups.find((g) => g.id === groupId);
		sheet = { kind: 'none' };
		menuTarget = null;

		let next: SheetState = { kind: 'none' };
		switch (label) {
			case m.addTitle:
				next = { kind: 'add' };
				break;
			case m.importFile:
			case m.importAll:
				void importBook(undefined);
				break;
			case m.exportTitle:
			case m.exportAll:
				next = { kind: 'export', scope: { type: 'all' } };
				break;
			case m.importGroup:
				if (groupId !== undefined) void importBook(groupId);
				break;
			case m.exportGroup:
				if (groupId !== undefined) next = { kind: 'export', scope: { type: 'group', id: groupId } };
				break;
			case m.groupRename:
			case m.groupEdit:
				if (group !== undefined) next = { kind: 'group-rename', id: group.id, name: group.name };
				break;
			case m.groupDelete:
				if (group !== undefined)
					next = { kind: 'confirm-group-delete', id: group.id, name: group.name };
				break;
			case m.send:
				if (address !== undefined) sendTo(address);
				break;
			case m.receive:
				receiveFrom();
				break;
			case m.copyAddress:
				if (address !== undefined) void copyAddress(address);
				break;
			case m.edit:
				openEdit(address);
				return;
			case m.moveGroup:
				if (address !== undefined) next = { kind: 'group-pick', address };
				break;
			case m.delete:
				if (address !== undefined) askDelete(address);
				return;
		}
		if (next.kind !== 'none') {
			sheet = next;
			swallowNextClose = from.kind === 'add-menu' || from.kind === 'group-menu';
		}
	}

	function onUiEvent(event: ContactsUiEvent): void {
		switch (event.kind) {
			case 'tab':
				if (event.id === 'wallet') void goto(walletHref);
				else if (event.id === 'settings') void goto(settingsHref);
				// explore has no web route by decision (spec 022).
				return;
			case 'query':
				ui = { ...ui, query: event.value };
				return;
			case 'add':
				// The phone's "+" opens the drawn C5 sheet (new / import / export);
				// the desktop header has its own import/export menu, so its
				// "添加联系人" is the form itself.
				sheet = wide.current ? { kind: 'add' } : { kind: 'add-menu' };
				return;
			case 'empty-primary':
				sheet = { kind: 'add' };
				return;
			case 'empty-secondary':
				void importBook(undefined);
				return;
			case 'open':
				ui = { ...ui, screen: 'detail', selectedAddress: event.address, allActivity: false };
				// Let the core inspect the address: resolve its identity (written
				// back onto a saved-but-unnamed contact as `resolved_name`) and
				// classify it. Deduped and cached per address by the core. The
				// classification chain is mainnet until a send flow (026) names one.
				contacts?.dispatch({ type: 'inspect_recipient', chain_id: 1, address: event.address });
				return;
			case 'back':
				ui = { ...ui, screen: 'list', selectedAddress: undefined, selectedGroupId: undefined };
				return;
			case 'edit':
				openEdit(ui.selectedAddress);
				return;
			case 'delete':
				askDelete(event.address);
				return;
			case 'sheet-select':
				onSheetSelect(event.label);
				return;
			case 'sheet-close':
				if (swallowNextClose) {
					swallowNextClose = false;
					return;
				}
				sheet = { kind: 'none' };
				menuTarget = null;
				return;
			case 'group-open':
				ui = { ...ui, screen: 'group', selectedGroupId: event.id };
				return;
			case 'group-new':
				sheet = { kind: 'group-new' };
				return;
			case 'add-member':
				if (ui.selectedGroupId !== undefined)
					sheet = { kind: 'member-pick', id: ui.selectedGroupId };
				return;
			case 'action':
				switch (event.id) {
					case 'send':
						sendTo(event.address);
						return;
					case 'receive':
						receiveFrom();
						return;
					case 'qr':
						sheet = { kind: 'qr', address: event.address };
						return;
					case 'copy':
						void copyAddress(event.address);
						return;
					case 'move-group':
						sheet = { kind: 'group-pick', address: event.address };
						return;
				}
				return;
			case 'activity-all':
				ui = { ...ui, allActivity: true };
				return;
			case 'batch-send':
				batchSend(event.id);
				return;
			case 'group-menu':
				menuTarget = { kind: 'group', id: event.id };
				if (!wide.current) sheet = { kind: 'group-menu', id: event.id };
				return;
			case 'contact-menu':
				menuTarget = { kind: 'contact', address: event.address };
				return;
		}
	}

	function saveContact(draft: { name: string; address: string }): void {
		contacts?.dispatch({
			type: 'save',
			input: {
				address: draft.address,
				name: draft.name,
				note: null,
				favorite: null,
				kind: null,
				resolved_name: null,
				resolved_source: null
			},
			now_ms: Date.now()
		});
		sheet = { kind: 'none' };
		// On the desktop the column that held the form shows what it made —
		// the phone's list already shows the new row. The core keys contacts
		// by the lowercased address.
		if (wide.current) {
			ui = {
				...ui,
				screen: 'detail',
				selectedAddress: draft.address.toLowerCase(),
				selectedGroupId: undefined
			};
		}
	}

	function saveGroup(name: string): void {
		contacts?.dispatch({
			type: 'group_save',
			input: { id: null, name, color: null, members: null }
		});
		sheet = { kind: 'none' };
	}

	function renameGroup(name: string): void {
		if (sheet.kind !== 'group-rename') return;
		contacts?.dispatch({
			type: 'group_save',
			input: { id: sheet.id, name, color: null, members: null }
		});
		sheet = { kind: 'none' };
	}

	/** 添加成员's answer: the whole membership, ticked. The core normalises. */
	function saveMembers(addresses: string[]): void {
		if (sheet.kind !== 'member-pick') return;
		contacts?.dispatch({ type: 'set_group_members', id: sheet.id, members: addresses });
		sheet = { kind: 'none' };
	}

	/** 移入分组's answer: which groups hold this contact. */
	function saveGroups(groupIds: string[]): void {
		if (sheet.kind !== 'group-pick') return;
		contacts?.dispatch({
			type: 'set_contact_groups',
			address: sheet.address,
			group_ids: groupIds
		});
		sheet = { kind: 'none' };
	}

	function acknowledgeReport(): void {
		contacts?.dispatch({ type: 'import_acknowledged' });
	}

	function closeSheet(): void {
		sheet = { kind: 'none' };
	}

	/** The forms' copy — the same strings whichever container draws them. */
	function contactCopy(kind: 'add' | 'edit'): ContactFormCopy {
		return {
			title: kind === 'add' ? m.addTitle : m.editTitle,
			nameLabel: m.nameLabel,
			namePlaceholder: m.namePlaceholder,
			addressLabel: m.addressLabel,
			addressPlaceholder: m.addressPlaceholder,
			save: m.save,
			cancel: m.cancel,
			invalidAddress: m.invalidAddress
		};
	}

	const groupCopy = $derived<GroupFormCopy>({
		title: m.groupNew,
		nameLabel: m.groupNameLabel,
		namePlaceholder: m.groupNamePlaceholder,
		save: m.save,
		cancel: m.cancel
	});
</script>

<svelte:head>
	<title>{m.title}</title>
	<meta name="robots" content="noindex" />
</svelte:head>

{#if signedIn && model !== null && desktopModel !== null && view !== null}
	{#if wide.current}
		<div class="desktop-shell">
			<ContactsDesktop
				model={desktopModel}
				onuievent={onUiEvent}
				onaccounts={() => (switching = true)}
				contactForm={sheet.kind === 'add' || sheet.kind === 'edit'
					? {
							copy: contactCopy(sheet.kind),
							initial:
								sheet.kind === 'edit' ? { name: sheet.name, address: sheet.address } : undefined,
							onsave: saveContact,
							onclose: closeSheet
						}
					: undefined}
			/>

			<!-- The phone's other sheets become centred dialogs here — one field,
			     or a yes/no — as settings' add-network and sign-out are. The
			     contact form is the exception: it is the third column, above. -->
			{#if sheet.kind === 'group-new'}
				<Dialog title={m.groupNew} closeLabel={m.cancel} onclose={closeSheet}>
					<GroupForm copy={groupCopy} onsave={saveGroup} />
				</Dialog>
			{:else if sheet.kind === 'confirm-delete' || sheet.kind === 'confirm-group-delete'}
				{@const confirm = model.confirm}
				{#if confirm !== undefined}
					<Dialog title={confirm.title} closeLabel={confirm.cancel} onclose={closeSheet}>
						<p class="dialog-body">{confirm.body}</p>
						<div class="dialog-actions">
							<Button
								variant="danger"
								shape="rounded"
								onclick={() => onUiEvent({ kind: 'sheet-select', label: confirm.confirm })}
							>
								{confirm.confirm}
							</Button>
						</div>
					</Dialog>
				{/if}
			{/if}
		</div>
	{:else}
		<main class="page">
			<ContactsHome {model} destinations={WEB_DESTINATIONS} onuievent={onUiEvent} />
		</main>

		{#if sheet.kind === 'add' || sheet.kind === 'edit'}
			<ContactEditSheet
				copy={contactCopy(sheet.kind)}
				initial={sheet.kind === 'edit' ? { name: sheet.name, address: sheet.address } : undefined}
				onsave={saveContact}
				onclose={closeSheet}
			/>
		{:else if sheet.kind === 'group-new'}
			<GroupEditSheet copy={groupCopy} onsave={saveGroup} onclose={closeSheet} />
		{/if}
	{/if}

	<!-- Surfaces both layouts share: a sheet on the phone, a dialog on the desktop. -->
	{#if sheet.kind === 'group-rename'}
		<SheetOrDialog
			wide={wide.current}
			title={m.groupRename}
			closeLabel={m.cancel}
			onclose={closeSheet}
		>
			{#key sheet.id}
				<GroupForm
					copy={{ ...groupCopy, title: m.groupRename }}
					initialName={sheet.name}
					onsave={renameGroup}
				/>
			{/key}
		</SheetOrDialog>
	{:else if sheet.kind === 'member-pick'}
		<SheetOrDialog
			wide={wide.current}
			title={m.addMember}
			closeLabel={m.cancel}
			height="tall"
			onclose={closeSheet}
		>
			{#key sheet.id}
				<PickList
					model={memberPickModel(view, sheet.id, m, avatarSvgForClient)}
					onsave={saveMembers}
				/>
			{/key}
		</SheetOrDialog>
	{:else if sheet.kind === 'group-pick'}
		<SheetOrDialog
			wide={wide.current}
			title={m.moveGroup}
			closeLabel={m.cancel}
			onclose={closeSheet}
		>
			{#key sheet.address}
				<PickList model={groupPickModel(view, sheet.address, m)} onsave={saveGroups} />
			{/key}
		</SheetOrDialog>
	{:else if qrSubject !== undefined}
		{@const subject = qrSubject}
		<SheetOrDialog
			wide={wide.current}
			title={m.actionQr}
			closeLabel={m.cancel}
			height="tall"
			onclose={closeSheet}
		>
			<ContactQr
				name={subject.name}
				address={subject.address}
				identiconSvg={subject.identiconSvg}
				code={subject.code}
				copyLabel={m.copyAddress}
				copiedLabel={m.copied}
				copied={copied === subject.address}
				oncopy={() => copyAddress(subject.address)}
			/>
		</SheetOrDialog>
	{:else if sheet.kind === 'export'}
		<SheetOrDialog
			wide={wide.current}
			title={m.exportTitle}
			closeLabel={m.cancel}
			onclose={closeSheet}
		>
			<p class="dialog-body">{m.exportBody}</p>
			<div class="dialog-actions">
				<Button variant="secondary" shape="rounded" onclick={() => exportBook('csv')}>CSV</Button>
				<Button variant="primary" shape="rounded" onclick={() => exportBook('json')}>JSON</Button>
			</div>
		</SheetOrDialog>
	{/if}

	{#if report !== undefined}
		<SheetOrDialog
			wide={wide.current}
			title={report.title}
			closeLabel={m.cancel}
			onclose={acknowledgeReport}
		>
			<p class="dialog-body" data-testid="import-report">{report.body}</p>
			<div class="dialog-actions">
				<Button variant="primary" shape="rounded" onclick={acknowledgeReport}>{m.done}</Button>
			</div>
		</SheetOrDialog>
	{/if}

	{#if copied !== null && sheet.kind !== 'qr'}
		<div class="toast" role="status">{m.copied}</div>
	{/if}
{:else}
	<!-- The cores have not ruled yet. An empty surface, not a fixture book. -->
	<div class="waiting" aria-busy="true"></div>
{/if}

<IdenticonViewerHost copy={data.identiconViewer} />

{#if switching && signedIn}
	<AccountSwitcher
		copy={{ accounts: data.accountsMessages, close: data.identiconViewer.close }}
		wide={wide.current}
		oncreate={() => void goto(createHref)}
		onsignin={() => void goto(welcome)}
		onclose={() => (switching = false)}
	/>
{/if}

<style>
	.page {
		height: 100dvh;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-base);
	}

	/* The three columns are `height: 100%` of this frame (the settings and
	   wallet routes frame theirs the same way). */
	.desktop-shell {
		position: relative;
		height: 100dvh;
		overflow: hidden;
	}

	.dialog-body {
		margin: 0 0 var(--space-xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.dialog-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-md);
	}

	.toast {
		position: fixed;
		inset-inline: 0;
		bottom: var(--space-4xl);
		z-index: 3;
		width: fit-content;
		margin-inline: auto;
		padding: var(--space-md) var(--space-xl);
		border-radius: var(--radius-full);
		background: var(--color-fg-base);
		color: var(--color-bg-base);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		box-shadow: var(--shadow-lg);
		pointer-events: none;
	}

	.waiting {
		min-height: 100dvh;
		background: var(--color-bg-base);
	}
</style>
