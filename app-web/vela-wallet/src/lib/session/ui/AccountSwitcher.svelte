<script lang="ts">
	/**
	 * The account switcher, from the header (founder call, 2026-09-05).
	 *
	 * The name-and-chevron in the wallet header drew a disclosure it did not
	 * have: the only switcher was three taps away on the settings screen. This
	 * is that switcher, opened where the name is — every account this browser
	 * is signed into, the active one checked, and the two ways to add one
	 * (create, or sign in to one you already have).
	 *
	 * Nothing here is decided by the shell. The rows are the session's own, in
	 * its order; the totals are the balance core's switcher cache, refreshed
	 * while this is open exactly as the settings page's sheet does it; a pick is
	 * `SwitchAccount` in the session's domain, which persists the index. This
	 * component only draws the sheet — a bottom sheet on the phone, a centred
	 * dialog on the desktop, the desktop rule for every phone 弹框.
	 */
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import AccountsSheetBody from '$lib/settings/ui/AccountsSheetBody.svelte';
	import Dialog from '$lib/settings/ui/Dialog.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import { liveAccountsSheet } from '$lib/settings/live';
	import type { SettingsMessages } from '$lib/settings/messages';
	import { session } from '$lib/session/core/session.svelte';
	import { balance } from '$lib/wallet/core/balance.svelte';
	import { currency } from '$lib/settings/core/currency.svelte';
	import { avatarSvgForClient } from '$lib/wallet/identicon';

	interface Props {
		copy: { accounts: SettingsMessages['accounts']; close: string };
		/** Past the desktop breakpoint: a dialog, not a sheet. */
		wide: boolean;
		oncreate: () => void;
		onsignin: () => void;
		onclose: () => void;
	}

	let { copy, wide, oncreate, onsignin, onclose }: Props = $props();

	const view = $derived(session.view);

	/**
	 * The sheet, re-derived as the balance core answers: the switcher cache
	 * fills row by row while this is open, and the live total stands in for
	 * the active account's.
	 */
	const sheet = $derived.by(() => {
		const balances = new SvelteMap<string, number>();
		for (const entry of balance.view.switcher.balances) {
			balances.set(entry.address.toLowerCase(), entry.usd);
		}
		if (balance.view.display_total_usd !== null && view.address !== '') {
			balances.set(view.address.toLowerCase(), balance.view.display_total_usd);
		}
		return liveAccountsSheet(
			{
				rows: view.accounts,
				activeIndex: view.active_index,
				balances,
				currency: currency.view,
				identicon: (address, name) => avatarSvgForClient(address, name)
			},
			copy.accounts
		);
	});

	// The switcher is on screen: the balance core refreshes every row's total
	// for as long as it is, and stops when it goes.
	onMount(() => {
		void currency.boot();
		balance.openSwitcher(session.view.accounts.map((row) => row.account.address));
		return () => balance.closeSwitcher();
	});

	function select(position: number): void {
		const row = view.accounts[position];
		if (row !== undefined) session.switchAccount(row.index);
		onclose();
	}
</script>

{#if wide}
	<Dialog title={sheet.title} closeLabel={copy.close} {onclose}>
		<AccountsSheetBody {sheet} layout="inline" onselect={select} {oncreate} {onsignin} />
	</Dialog>
{:else}
	<BottomSheet title={sheet.title} closeLabel={copy.close} height="tall" {onclose}>
		<AccountsSheetBody {sheet} onselect={select} {oncreate} {onsignin} />
	</BottomSheet>
{/if}
