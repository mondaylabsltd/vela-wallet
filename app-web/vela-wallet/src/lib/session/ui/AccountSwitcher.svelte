<script lang="ts">
	/**
	 * The account switcher, from the header (founder call, 2026-09-05).
	 *
	 * The name-and-chevron in the wallet header drew a disclosure it did not
	 * have: the only switcher was three taps away on the settings screen. This
	 * is that switcher, opened where the name is — every account this browser
	 * is signed into, its multisigs among them (a multisig is an account; its
	 * row adds its rule), the active one checked, and ONE way to add one:
	 * "Add account" offers the four (create, sign in, create a multisig, add
	 * one).
	 *
	 * Nothing here is decided by the shell. The rows are the session's own, in
	 * its order; the totals are the balance core's switcher cache, refreshed
	 * while this is open exactly as the settings page's sheet does it; a pick is
	 * `SwitchAccount` in the session's domain, which persists the index. This
	 * component only draws the sheet — a bottom sheet on the phone, a centred
	 * dialog on the desktop, the desktop rule for every phone 弹框.
	 */
	import { onMount } from 'svelte';
	import { storedUnreadable } from '$lib/onboarding/core/storage';
	import AccountsSheetBody from '$lib/settings/ui/AccountsSheetBody.svelte';
	import Dialog from '$lib/settings/ui/Dialog.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import {
		accountAmount,
		accountLoading,
		liveAccountsSheet,
		switcherBalances
	} from '$lib/settings/live';
	import { shownAddress } from '$lib/multisig/core/shown';
	import type { SettingsMessages } from '$lib/settings/messages';
	import { session } from '$lib/session/core/session.svelte';
	import { intents } from '$lib/intents/index.svelte';
	import { balance } from '$lib/wallet/core/balance.svelte';
	import { networkSpace } from '$lib/network-space/network-space.svelte';
	import { currency } from '$lib/settings/core/currency.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '$lib/multisig/messages';
	import { multisig, type MultisigAccount } from '$lib/multisig/core/store.svelte';
	import { multisigUi } from '$lib/multisig/core/ui.svelte';
	import MultisigRows from '$lib/multisig/ui/MultisigRows.svelte';
	import { multisigAsAccount } from '$lib/multisig/core/ext';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS, type UtilityIconId } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		copy: { accounts: SettingsMessages['accounts']; close: string };
		/** Past the desktop breakpoint: a dialog, not a sheet. */
		wide: boolean;
		oncreate: () => void;
		onsignin: () => void;
		onclose: () => void;
		/** The multisig section's copy; absent draws no section (the settings page). */
		multisigCopy?: MultisigMessages;
	}

	let { copy, wide, oncreate, onsignin, onclose, multisigCopy }: Props = $props();

	const view = $derived(session.view);

	/**
	 * The sheet, re-derived as the balance core answers: the switcher cache
	 * fills row by row while this is open, and the live total stands in for
	 * the active account's.
	 */
	const input = $derived.by(() => ({
		rows: view.accounts,
		// A multisig shown: no Vela row is the active account.
		activeIndex: multisig.active ? -1 : view.active_index,
		// The live total is the account shown — a multisig's, when one is.
		balances: switcherBalances(balance.view, shownAddress(view.address)),
		hidden: balance.view.switcher.hidden,
		currency: currency.view,
		identicon: identiconSvgForClient,
		others: multisig.accounts.map((a) => a.address),
		// home-9: the testnet space shows no figure for test coins, here too.
		noValue: networkSpace.testnet,
		// ms-24: a row the core is still reading is drawn as loading.
		loading: balance.view.switcher.loading,
		unsynced: view.unsynced,
		// §18 item 2: a stored record this browser cannot read, said under the list.
		unreadable: storedUnreadable()
	}));

	const sheet = $derived.by(() => {
		// The live total stands in for the active row only while it is a
		// figure — not hidden, not a home that read nothing (`switcherBalances`).
		return liveAccountsSheet(input, copy.accounts);
	});

	// The switcher is on screen: the balance core refreshes every row's total
	// for as long as it is, and stops when it goes.
	onMount(() => {
		multisig.boot();
		void currency.boot();
		balance.openSwitcher([
			...session.view.accounts.map((row) => row.account.address),
			...multisig.accounts.map((a) => a.address)
		]);
		return () => balance.closeSwitcher();
	});

	function select(position: number): void {
		const row = view.accounts[position];
		multisig.activate(null);
		if (row !== undefined)
			void intents.invoke('person', { type: 'switch_account', address: row.account.address });
		onclose();
	}

	function selectMultisig(account: MultisigAccount): void {
		multisig.activate(account.address);
		onclose();
	}

	function openMultisig(kind: 'create' | 'add'): void {
		onclose();
		multisigUi.open({ kind });
	}

	/**
	 * One wallet leaves, the others stay (2026-09-23). The switcher closes
	 * because the list under it just changed: leaving it open would put the
	 * next row where the finger already is.
	 */
	/** "Add account": the list gives way to the four ways to add one. */
	let adding = $state(false);

	const choices = $derived.by(() => {
		const m = multisigCopy;
		type Choice = { icon: UtilityIconId; title: string; note: string; go: () => void };
		const base: Choice[] = [
			{
				icon: 'user-round-plus',
				title: sheet.primary,
				note: m?.switcher.createAccountNote ?? '',
				go: oncreate
			},
			{
				icon: 'user-round',
				title: sheet.secondary,
				note: m?.switcher.signInNote ?? '',
				go: onsignin
			}
		];
		if (!m) return base;
		const more: Choice[] = [
			{
				icon: 'users-round',
				title: m.switcher.create,
				note: m.switcher.createMultisigNote,
				go: () => openMultisig('create')
			},
			{
				icon: 'download',
				title: m.switcher.add,
				note: m.switcher.addMultisigNote,
				go: () => openMultisig('add')
			}
		];
		return [...base, ...more];
	});

	/**
	 * ms-20: the multisigs whose only member on this device is the account at
	 * `position` — removing it leaves them read-only here, said before it goes.
	 */
	function lastMemberNote(position: number): string | null {
		const row = view.accounts[position];
		if (!row || !multisigCopy) return null;
		const address = row.account.address.toLowerCase();
		const local = new Set(view.accounts.map((r) => r.account.address.toLowerCase()));
		const alone = multisig.accounts.filter((a) => {
			const here = a.owners.filter((o) => local.has(o.toLowerCase()));
			return here.length === 1 && here[0].toLowerCase() === address;
		});
		if (alone.length === 0) return null;
		return multisigCopy.switcher.lastMember.replace(
			'{{names}}',
			alone.map((a) => multisigAsAccount(a).name).join(', ')
		);
	}

	function remove(position: number): void {
		const row = view.accounts[position];
		if (row !== undefined)
			void intents.invoke('person', { type: 'remove_wallet', address: row.account.address });
		onclose();
	}
</script>

{#if wide}
	<Dialog
		title={adding ? (multisigCopy?.switcher.addAccount ?? sheet.title) : sheet.title}
		closeLabel={copy.close}
		{onclose}
	>
		{@render body()}
	</Dialog>
{:else}
	<BottomSheet
		title={adding ? (multisigCopy?.switcher.addAccount ?? sheet.title) : sheet.title}
		closeLabel={copy.close}
		height="tall"
		{onclose}
	>
		{@render body()}
	</BottomSheet>
{/if}

{#snippet body()}
	{#if adding}
		<ul class="choices">
			{#each choices as choice (choice.title)}
				<li>
					<button type="button" class="choice" onclick={choice.go}>
						<span class="icon"><Icon icon={UTILITY_ICONS[choice.icon]} size="md" /></span>
						<span class="text">
							<span class="title">{choice.title}</span>
							{#if choice.note}<span class="note">{choice.note}</span>{/if}
						</span>
						<span class="chev"><Icon icon={UTILITY_ICONS['chevron-right']} size="sm" /></span>
					</button>
				</li>
			{/each}
		</ul>
	{:else}
		<AccountsSheetBody
			{sheet}
			onselect={select}
			onremove={remove}
			removeNote={lastMemberNote}
			{oncreate}
			{onsignin}
		>
			{#snippet more()}
				{#if multisigCopy}
					<MultisigRows
						m={multisigCopy}
						removeLabel={sheet.remove ?? ''}
						removeNamed={sheet.removeNamed}
						cancelLabel={sheet.removeCancel ?? ''}
						amount={(address) => accountAmount(input, address)}
						loading={(address) => accountLoading(input, address)}
						onselect={selectMultisig}
					/>
				{/if}
			{/snippet}
			{#snippet actions()}
				<Button variant="secondary" shape="rounded" onclick={() => (adding = true)}>
					<span class="add"
						><Icon icon={UTILITY_ICONS.plus} size="sm" />{multisigCopy?.switcher.addAccount ??
							sheet.primary}</span
					>
				</Button>
			{/snippet}
		</AccountsSheetBody>
	{/if}
{/snippet}

<style>
	.add {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.choices {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.choices li + li {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.choice {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding: var(--space-lg) 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.icon {
		display: flex;
		align-items: center;
		justify-content: center;
		flex: none;
		width: var(--size-control-md);
		height: var(--size-control-md);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.title {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.note {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.chev {
		display: flex;
		color: var(--color-fg-subtle);
	}
</style>
