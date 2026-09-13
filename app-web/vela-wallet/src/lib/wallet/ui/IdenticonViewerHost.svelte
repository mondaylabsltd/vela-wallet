<script lang="ts">
	/**
	 * The one answer to every artwork's question (`identicon-viewer.svelte.ts`).
	 *
	 * Mounted once per signed-in route. It hands the store the corpus name for
	 * the artwork buttons and draws the viewer for whatever they open — so a
	 * contact row, the header and the signer line of a signing sheet all open
	 * the same viewer, and none of them needs to know where it lives.
	 */
	import { identiconViewer } from '../identicon-viewer.svelte';
	import type { WalletMessages } from '../messages';
	import IdenticonViewer from './IdenticonViewer.svelte';

	interface Props {
		copy: WalletMessages['identiconViewer'];
	}

	let { copy }: Props = $props();

	$effect(() => {
		identiconViewer.openLabel = copy.a11yOpen;
		return () => {
			identiconViewer.openLabel = '';
			identiconViewer.close();
		};
	});
</script>

{#if identiconViewer.current !== null}
	{@const subject = identiconViewer.current}
	<IdenticonViewer
		{copy}
		address={subject.address}
		identiconSvg={subject.identiconSvg}
		onClose={() => identiconViewer.close()}
	/>
{/if}
