<script lang="ts">
	/**
	 * The desktop settings surface (spec 023, DST1–DST8 + DST4b + DSR1).
	 *
	 * Three columns, per the desktop SPEC: the app sidebar (spec 015's, reused
	 * verbatim), a narrow second-level nav, and the panel. The phone's sheets
	 * become either a section of the panel it belongs to — the account switcher
	 * IS the 账户 panel — or a centred dialog, which is what add-network and
	 * fix-RPC are. Nothing here is a bottom sheet: macOS System Settings is the
	 * reference, and it has none.
	 */
	import { untrack } from 'svelte';
	import { OPENED_EVENT, type OnNetEvent } from './net-events';
	import type { SettingsPrefEvent } from './pref-events';
	import { removeNetworkQuestion, storageClearQuestion } from './questions';
	import type { NetEndpointField } from '$lib/core/generated/NetEndpointField';
	import type { NetProviderId } from '$lib/core/generated/NetProviderId';
	import type {
		ConfirmSheetModel,
		FeedbackResult,
		SettingsDesktopModel,
		SettingsOverlayId,
		SettingsPageId
	} from './model';
	import type { SidebarModel } from '$lib/wallet/model';
	import Button from '$lib/ui/Button.svelte';
	import Sidebar from '$lib/wallet/ui/Sidebar.svelte';
	import AboutPanel from './ui/AboutPanel.svelte';
	import KeysBlock from './ui/KeysBlock.svelte';
	import AccountsSheetBody from './ui/AccountsSheetBody.svelte';
	import AddNetworkPanel from './ui/AddNetworkPanel.svelte';
	import ConfirmSheet from './ui/ConfirmSheet.svelte';
	import Callout from './ui/Callout.svelte';
	import DangerCard from './ui/DangerCard.svelte';
	import Dialog from './ui/Dialog.svelte';
	import FeedbackBody from './ui/FeedbackBody.svelte';
	import Dropdown from './ui/Dropdown.svelte';
	import EndpointsPanel from './ui/EndpointsPanel.svelte';
	import FormRow from './ui/FormRow.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import NetworkDetailPanel from './ui/NetworkDetailPanel.svelte';
	import NetworksPanel from './ui/NetworksPanel.svelte';
	import RpcBanner from './ui/RpcBanner.svelte';
	import RpcFixBody from './ui/RpcFixBody.svelte';
	import RpcProvidersPanel from './ui/RpcProvidersPanel.svelte';
	import SegmentedControl from './ui/SegmentedControl.svelte';
	import SettingsNavList from './ui/SettingsNavList.svelte';
	import StoragePanel from './ui/StoragePanel.svelte';
	import TextScaleSlider from './ui/TextScaleSlider.svelte';

	interface Props {
		model: SettingsDesktopModel;
		/** The app sidebar's own model (spec 015). Absent in component boards. */
		sidebar?: SidebarModel;
		onnav?: (id: 'wallet' | 'contacts' | 'explore' | 'settings') => void;
		onsignout?: () => void;
		/** The network surfaces' live wiring (spec 024). Absent = gallery. */
		onnetevent?: OnNetEvent;
		/** A preference control was used (spec 028 T433). Absent = gallery. */
		onprefevent?: (event: SettingsPrefEvent) => void;
		/** The sidebar's network filter was used. Absent in the gallery. */
		onchainselect?: (row: SidebarModel['networks'][number]) => void;
		/**
		 * The account page (spec 028 Phase 8): a row picked by its POSITION in
		 * the session's order, and the two journeys the buttons leave for.
		 * Absent in the gallery.
		 */
		onaccountselect?: (position: number) => void;
		onaccountcreate?: () => void;
		onaccountsignin?: () => void;
		/** The account page is showing: the balance core refreshes its rows while it is. */
		onaccountsopen?: (open: boolean) => void;
		/** The Ethereum backup (spec 062): the keys block's one button. */
		onethereumbackup?: () => void;
		/** A storage row's action, by its own id (the phone's `onstorageclear`). */
		onstorageclear?: (id: string) => void;
		/** "Clear all caches" was confirmed. Absent in the gallery. */
		onclearcaches?: () => void;
		/** 发送 in the report panel (spec 081 FR-016). Absent in the gallery. */
		onfeedbacksend?: (report: { what: string; steps: string }) => void;
		feedbackSending?: boolean;
		feedbackResult?: FeedbackResult;
	}

	let {
		model,
		sidebar,
		onnav,
		onsignout,
		onnetevent,
		onprefevent,
		onchainselect,
		onaccountselect,
		onaccountcreate,
		onaccountsignin,
		onaccountsopen,
		onethereumbackup,
		onstorageclear,
		onclearcaches,
		onfeedbacksend,
		feedbackSending = false,
		feedbackResult
	}: Props = $props();

	let page = $state<SettingsPageId>(untrack(() => model.page));
	let overlay = $state<SettingsOverlayId>(untrack(() => model.overlay));
	let openDropdown = $state<string | undefined>(untrack(() => model.dropdown?.rowId));
	/** The storage row or network waiting on an answer, and the question it asks. */
	let pending = $state<{ id: string; sheet: ConfirmSheetModel } | null>(null);

	// The account page has no open/close of its own: showing it IS opening the
	// switcher, so the balance core hears both edges from the page choice.
	$effect(() => {
		onaccountsopen?.(page === 'account');
	});

	/** The panel's own heading, by page. */
	const heading = $derived.by(() => {
		switch (page) {
			case 'account':
				return { title: model.account.title, description: undefined };
			case 'appearance':
				return { title: model.appearance.title, description: undefined };
			case 'localization':
				return { title: model.localization.title, description: model.localization.description };
			case 'fee-speed':
				return { title: model.feeSpeed.title, description: model.feeSpeed.description };
			case 'networks':
				return { title: model.networks.title, description: model.networks.subtitle };
			case 'rpc-providers':
				return { title: model.rpcProviders.title, description: undefined };
			case 'endpoints':
				return { title: model.endpoints.title, description: undefined };
			case 'storage':
				return { title: model.storage.title, description: model.storage.subtitle };
			case 'feedback':
				return { title: model.feedback.title, description: model.feedback.subtitle };
			case 'about':
				return { title: model.about.title, description: undefined };
			default:
				return { title: model.title, description: undefined };
		}
	});

	function toggleDropdown(id: string) {
		openDropdown = openDropdown === id ? undefined : id;
	}

	/**
	 * Show a panel. Showing the providers or the endpoints IS the core event
	 * the phone raises when it pushes the same page (`OPENED_EVENT`): without
	 * it the providers' drafts were never seeded, and leaving a key's field
	 * saved an empty draft over the key it showed (spec 072, P0).
	 */
	function openPage(id: SettingsPageId) {
		page = id;
		const opened = OPENED_EVENT[id];
		if (opened !== undefined) onnetevent?.(opened);
	}

	function closeOverlay() {
		overlay = 'none';
		pending = null;
	}

	/** Put a destructive row's question up; the row acts only on its confirm. */
	function ask(next: SettingsOverlayId, id: string, sheet: ConfirmSheetModel | undefined) {
		if (sheet === undefined) return;
		pending = { id, sheet };
		overlay = next;
	}
</script>

<div class="desktop">
	{#if sidebar !== undefined}
		<!-- The header's name button opens the switcher, which on this screen IS
		     the account page. -->
		<Sidebar {sidebar} {onnav} {onchainselect} onaccounts={() => openPage('account')} />
	{/if}

	<SettingsNavList title={model.title} items={model.nav} selected={page} onselect={openPage} />

	<main>
		<div class="panel">
			<header class="panel-head">
				<div class="titles">
					<h1>{heading.title}</h1>
					{#if heading.description !== undefined}
						<p>{heading.description}</p>
					{/if}
				</div>
				{#if page === 'networks'}
					<button
						type="button"
						class="add"
						onclick={() => {
							onnetevent?.({ kind: 'open-add' });
							overlay = 'add-network';
						}}
					>
						<Icon icon={UTILITY_ICONS.plus} size="sm" />
						<span>{model.networks.addLabel}</span>
					</button>
				{/if}
			</header>

			{#if model.rpcBanner !== undefined}
				<div class="banner"><RpcBanner banner={model.rpcBanner} /></div>
			{/if}

			{#if page === 'account'}
				<AccountsSheetBody
					sheet={{
						title: model.account.title,
						summary: model.account.summary,
						rows: model.account.rows,
						primary: model.account.primary,
						secondary: model.account.secondary
					}}
					layout="inline"
					onselect={onaccountselect}
					oncreate={onaccountcreate}
					onsignin={onaccountsignin}
				/>

				{#if model.account.keys !== undefined}
					<!-- Which keys control this wallet, and their Ethereum backup beneath
					     them (spec 062). Signed HERE: the page hosts the sheet. -->
					<hr />
					<KeysBlock model={model.account.keys} onbackup={() => onethereumbackup?.()} />
				{/if}

				<hr />

				<!-- Live, this ASKS THE CORE and the core's own sheet is the
				     confirmation — one dialog, the one carrying the pending-upload
				     warning. The local dialog below it is the gallery's board
				     (DST1's `sign-out` overlay), not a second step in front of the
				     real one: two dialogs saying the same sentence cost three
				     clicks to leave a wallet (founder, 2026-09-16). -->
				<button
					type="button"
					class="sign-out"
					onclick={() => (onsignout ? onsignout() : (overlay = 'sign-out'))}
				>
					<Icon icon={UTILITY_ICONS['log-out']} size="md" />
					<span>{model.account.signOutLabel}</span>
				</button>
				<p class="sign-out-note">{model.account.signOutNote}</p>

				<!-- Spec 081 FR-017 with 072's dialog: the card asks, the dialog
				     confirms, the route erases — and a failure is said in the
				     dialog's own callout. It was drawn with no handler from the
				     first day. -->
				<DangerCard
					title={model.account.erase.title}
					subtitle={model.account.erase.subtitle}
					action={model.account.erase.action}
					onselect={() => (overlay = 'erase-device')}
				/>
			{:else if page === 'appearance'}
				<FormRow label={model.appearance.language.label}>
					<Dropdown
						value={model.appearance.language.value ?? ''}
						label={model.appearance.language.label}
						open={openDropdown === 'language'}
						rows={model.appearance.language.options}
						ontoggle={() => toggleDropdown('language')}
						onselect={(id) => {
							onprefevent?.({ kind: 'language', id });
							openDropdown = undefined;
						}}
					/>
				</FormRow>
				<FormRow label={model.appearance.textScale.label} wide>
					<TextScaleSlider
						model={model.appearance.textScale.scale}
						onchange={(index) => onprefevent?.({ kind: 'text-scale', index })}
					/>
				</FormRow>
				<FormRow label={model.appearance.theme.label}>
					<SegmentedControl
						model={model.appearance.theme.segmented}
						onselect={(id) => onprefevent?.({ kind: 'theme', id })}
					/>
				</FormRow>
			{:else if page === 'localization'}
				{#each model.localization.rows as row (row.id)}
					<FormRow label={row.label}>
						<Dropdown
							value={row.value ?? ''}
							label={row.label}
							open={openDropdown === row.id}
							rows={row.options ??
								(model.dropdown?.rowId === row.id ? model.dropdown.rows : undefined)}
							ontoggle={() => toggleDropdown(row.id)}
							onselect={(id) => {
								onprefevent?.({ kind: row.id as 'number-format', id });
								openDropdown = undefined;
							}}
						/>
					</FormRow>
				{/each}
			{:else if page === 'fee-speed'}
				<!-- Spec 068. The same FormRow + Dropdown the localization page
				     uses: one preference, chosen the way every other desktop
				     preference is chosen. -->
				{#each model.feeSpeed.rows as row (row.id)}
					<FormRow label={row.label}>
						<Dropdown
							value={row.value ?? ''}
							label={row.label}
							open={openDropdown === row.id}
							rows={row.options}
							ontoggle={() => toggleDropdown(row.id)}
							onselect={(id) => {
								onprefevent?.({ kind: 'fee-speed', id });
								openDropdown = undefined;
							}}
						/>
					</FormRow>
				{/each}
			{:else if page === 'networks'}
				<NetworksPanel
					rows={model.networks.rows}
					addLabel={model.networks.addLabel}
					deleteLabel={model.networks.removeLabel}
					expandable
					onselect={(id) => onnetevent?.({ kind: 'select-network', id })}
					ondelete={(id) => ask('remove-network', id, removeNetworkQuestion(model.networks, id))}
				>
					{#snippet detail()}
						<NetworkDetailPanel
							detail={model.networks.detail}
							showIdentity={false}
							onfield={(field, value) => onnetevent?.({ kind: 'detail-field', field, value })}
							onfieldblur={(field) => onnetevent?.({ kind: 'detail-blur', field })}
						/>
					{/snippet}
				</NetworksPanel>
			{:else if page === 'rpc-providers'}
				<RpcProvidersPanel
					panel={model.rpcProviders}
					onfield={(id, value) =>
						onnetevent?.({ kind: 'provider-key', provider: id as NetProviderId, value })}
					onfieldblur={(id) =>
						onnetevent?.({ kind: 'provider-blur', provider: id as NetProviderId })}
					onaction={(id) => onnetevent?.({ kind: 'provider-test', provider: id as NetProviderId })}
				/>
			{:else if page === 'endpoints'}
				<EndpointsPanel
					panel={model.endpoints}
					onfield={(id, value) =>
						onnetevent?.({ kind: 'endpoint', field: id as NetEndpointField, value })}
					onfieldblur={(id) =>
						onnetevent?.({ kind: 'endpoint-blur', field: id as NetEndpointField })}
					onreset={() => ask('reset-endpoints', '', model.endpoints.resetSheet)}
				/>
			{:else if page === 'storage'}
				<!-- A row's Clear asks first, as on the phone: here it cleared the
				     whole address book on one click (spec 072). -->
				<StoragePanel
					panel={model.storage}
					onclear={(id) =>
						ask(
							'clear-storage-item',
							id,
							storageClearQuestion(model.storage, id, model.clearCachesSheet.cancel)
						)}
					onclearcaches={() => (overlay = 'clear-caches')}
				/>
			{:else if page === 'feedback'}
				<!-- Spec 081 FR-016: the phone's sheet body, as a panel. Same
				     component, so the preview lines and the consent note cannot
				     say one thing on a laptop and another on a phone. -->
				<FeedbackBody
					panel={model.feedback}
					onsend={onfeedbacksend}
					sending={feedbackSending}
					result={feedbackResult}
				/>
			{:else if page === 'about'}
				<AboutPanel panel={model.about} layout="inline" />
			{/if}
		</div>
	</main>

	{#if overlay === 'add-network'}
		<Dialog
			title={model.addNetwork.title}
			subtitle={model.addNetwork.subtitle}
			closeLabel={model.closeLabel}
			onclose={closeOverlay}
		>
			<AddNetworkPanel
				panel={model.addNetwork}
				onsearch={(query) => onnetevent?.({ kind: 'search', query })}
				onselect={(id) => onnetevent?.({ kind: 'pick-suggestion', chainId: Number(id) })}
				oncustomrpc={(value) => onnetevent?.({ kind: 'custom-rpc', value })}
				onprimary={() => onnetevent?.({ kind: 'confirm-add' })}
				onrecheck={() => onnetevent?.({ kind: 'recheck' })}
			/>
		</Dialog>
	{:else if overlay === 'rpc-fix'}
		<Dialog title={model.rpcFix.title} closeLabel={model.closeLabel} onclose={closeOverlay}>
			<RpcFixBody panel={model.rpcFix} onprimary={closeOverlay} />
		</Dialog>
	{:else if overlay === 'sign-out'}
		<Dialog title={model.account.signOutLabel} closeLabel={model.closeLabel} onclose={closeOverlay}>
			<p class="dialog-body">{model.account.signOutNote}</p>
			<div class="dialog-actions">
				<Button variant="danger" shape="rounded" onclick={onsignout}>
					{model.account.signOutLabel}
				</Button>
			</div>
		</Dialog>
	{:else if overlay === 'clear-caches'}
		<!-- The phone's confirm sheet, as a dialog (no bottom sheets on the desktop). -->
		<Dialog
			title={model.clearCachesSheet.title}
			closeLabel={model.closeLabel}
			onclose={closeOverlay}
		>
			<p class="dialog-body">{model.clearCachesSheet.body}</p>
			<div class="dialog-actions">
				<Button
					variant="primary"
					shape="rounded"
					onclick={() => {
						onclearcaches?.();
						closeOverlay();
					}}
				>
					{model.clearCachesSheet.confirm}
				</Button>
			</div>
		</Dialog>
	{:else if (overlay === 'clear-storage-item' || overlay === 'remove-network' || overlay === 'reset-endpoints') && pending}
		<!-- The phone's confirm sheet, in a dialog: titled with what goes. -->
		<Dialog title={pending.sheet.title} closeLabel={model.closeLabel} onclose={closeOverlay}>
			<ConfirmSheet
				sheet={pending.sheet}
				onconfirm={() => {
					const id = pending?.id;
					const what = overlay;
					closeOverlay();
					if (id === undefined) return;
					if (what === 'remove-network') onnetevent?.({ kind: 'delete-network', id });
					else if (what === 'reset-endpoints') onnetevent?.({ kind: 'endpoints-reset' });
					else onstorageclear?.(id);
				}}
				oncancel={closeOverlay}
			/>
		</Dialog>
	{:else if overlay === 'erase-device'}
		<!-- Spec 081 FR-017. Everything the phone's sheet says, in the desktop's
		     container: what is lost, and — the note — that the passkey is NOT,
		     because it lives with the person's passkey provider and not here.
		     `ConfirmSheet` is the phone's own component and draws all three (body,
		     note, callout), so the two clients cannot drift into saying different
		     things about the same destructive action.

		     The dialog does NOT close on confirm: a failed erase has to say so
		     where the person is looking — its callout is in this sheet — and a
		     success leaves this page anyway. -->
		<Dialog title={model.eraseSheet.title} closeLabel={model.closeLabel} onclose={closeOverlay}>
			<ConfirmSheet
				sheet={model.eraseSheet}
				onconfirm={() => onprefevent?.({ kind: 'erase' })}
				oncancel={closeOverlay}
			/>
		</Dialog>
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
		/* NOT `overflow: hidden`: the desktop SPEC requires the dropdown overlay
		   to escape its container's clipping, and a scroll container here is
		   what would clip it. The panel scrolls instead. */
		display: flex;
		justify-content: center;
	}

	.panel {
		width: 100%;
		/* The row measure plus this panel's own gutters (issue 195): at the 800
		   content column a label sat up to 480 pixels from the control it names. */
		max-width: calc(var(--layout-rowMeasure) + var(--space-5xl) * 2);
		height: 100%;
		overflow-y: auto;
		padding: var(--space-4xl) var(--space-5xl) var(--space-5xl);
	}

	.panel-head {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-xl);
		margin-bottom: var(--space-3xl);
	}

	h1 {
		margin: 0;
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.titles p {
		margin: var(--space-md) 0 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.add {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		min-height: var(--size-control-sm);
		padding-inline: var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
		flex-shrink: 0;
	}

	.banner {
		margin-bottom: var(--space-3xl);
	}

	hr {
		border: none;
		border-top: var(--border-hairline) solid var(--color-border-base);
		margin-block: var(--space-4xl);
	}

	.sign-out {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.sign-out-note {
		margin: var(--space-md) 0 var(--space-3xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
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
		gap: var(--space-lg);
	}
</style>
