<script lang="ts">
	/**
	 * The desktop rule, as one component: every phone bottom sheet becomes a
	 * centred dialog past the breakpoint (spec 023/024). The contacts route has
	 * seven such surfaces (pickers, the QR, the export format, a rename, two
	 * confirms, the import report); branching each of them twice would be
	 * fourteen copies of the same `{#if wide}`.
	 */
	import type { Snippet } from 'svelte';
	import Dialog from '$lib/settings/ui/Dialog.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';

	interface Props {
		wide: boolean;
		title: string;
		closeLabel: string;
		height?: 'half' | 'tall';
		onclose: () => void;
		children: Snippet;
	}

	let { wide, title, closeLabel, height = 'half', onclose, children }: Props = $props();
</script>

{#if wide}
	<Dialog {title} {closeLabel} {onclose}>
		{@render children()}
	</Dialog>
{:else}
	<BottomSheet {title} {closeLabel} {height} {onclose}>
		{@render children()}
	</BottomSheet>
{/if}
