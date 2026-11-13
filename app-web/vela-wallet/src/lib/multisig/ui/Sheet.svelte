<script lang="ts">
	/**
	 * Every multisig sheet's frame: a bottom sheet on the phone, a centred
	 * dialog past the desktop breakpoint — the desktop rule for every phone
	 * sheet, as the account switcher draws it. `busy` holds it open while a
	 * signature is in flight.
	 *
	 * `footer` (r15-3): the sheet's action row, OUTSIDE the scroll — on the
	 * phone the bottom sheet's foot, past the breakpoint the dialog's. A step
	 * taller than what the window shows (Safari 26's wallet tab shows 714 of
	 * an 874 pt phone; an iPhone SE ~548) scrolls its body; its action stays
	 * in sight, never under the sheet's edge or the browser's toolbar.
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
		footer?: Snippet;
	}

	let { title, closeLabel, wide, busy = false, onclose, children, footer }: Props = $props();
</script>

{#if wide}
	<Dialog {title} {closeLabel} {footer} onclose={busy ? undefined : onclose}>
		{@render children()}
	</Dialog>
{:else}
	<BottomSheet {title} {closeLabel} {footer} height="tall" dismissible={!busy} {onclose}>
		{@render children()}
	</BottomSheet>
{/if}
