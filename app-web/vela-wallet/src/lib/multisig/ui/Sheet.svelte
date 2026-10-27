<script lang="ts">
	/**
	 * Every multisig sheet's frame: a bottom sheet on the phone, a centred
	 * dialog past the desktop breakpoint — the desktop rule for every phone
	 * sheet, as the account switcher draws it. `busy` holds it open while a
	 * signature is in flight.
	 */
	import type { Snippet } from 'svelte';
	import Dialog from '$lib/settings/ui/Dialog.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';

	interface Props {
		title: string;
		closeLabel: string;
		wide: boolean;
		busy?: boolean;
		onclose: () => void;
		children: Snippet;
	}

	let { title, closeLabel, wide, busy = false, onclose, children }: Props = $props();
</script>

{#if wide}
	<Dialog {title} {closeLabel} onclose={busy ? undefined : onclose}>
		{@render children()}
	</Dialog>
{:else}
	<BottomSheet {title} {closeLabel} height="tall" dismissible={!busy} {onclose}>
		{@render children()}
	</BottomSheet>
{/if}
