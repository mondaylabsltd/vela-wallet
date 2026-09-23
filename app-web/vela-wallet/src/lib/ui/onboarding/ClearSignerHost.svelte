<script lang="ts">
	/**
	 * The Clear Signer's sheet on the onboarding screens (spec 075).
	 *
	 * Creating a wallet and signing in can run on the signer page, so the
	 * person needs the same sheet there that a signature gets: where is your
	 * signer, the pairing code for another device, the wait, and the sentence
	 * a refusal ends with. `<SigningHost>` draws it over the money screens;
	 * this draws it over the create flow and Welcome, from the same resident
	 * session — so a flow's whole page visit has one sheet, not one per
	 * ceremony.
	 *
	 * The words come through `strings` (the flow's prerendered copy) rather
	 * than a `SigningMessages`: these screens never load the signing surface.
	 */
	import { clearSignerModel } from '$lib/signing/live';
	import type { ClearSignerWords } from '$lib/signing/messages';
	import { clearSignerSession } from '$lib/signing/core/clear-signer.svelte';
	import ClearSignerSheet from '$lib/signing/ui/ClearSignerSheet.svelte';

	interface Props {
		strings: (key: string, params?: Record<string, string | number>) => string;
	}

	let { strings }: Props = $props();

	const words = $derived({
		close: strings('onboarding.common.close'),
		clearSignerWaiting: strings('componentsUi.signing.clearSignerWaiting'),
		clearSignerWaitingHint: strings('componentsUi.signing.clearSignerWaitingHint'),
		clearSignerReopen: strings('componentsUi.signing.clearSignerReopen'),
		clearSignerCancel: strings('common.cancel'),
		clearSignerClosed: strings('componentsUi.signing.clearSignerClosed'),
		clearSignerRefused: strings('componentsUi.signing.clearSignerRefused'),
		clearSignerMismatch: strings('componentsUi.signing.clearSignerMismatch'),
		clearSignerTimeout: strings('componentsUi.signing.clearSignerTimeout'),
		clearSignerWhere: strings('componentsUi.signing.clearSignerWhere'),
		clearSignerThisDevice: strings('componentsUi.signing.clearSignerThisDevice'),
		clearSignerOtherDevice: strings('componentsUi.signing.clearSignerOtherDevice'),
		clearSignerPair: strings('componentsUi.signing.clearSignerPair'),
		clearSignerPairHint: strings('componentsUi.signing.clearSignerPairHint'),
		clearSignerPairWaiting: strings('componentsUi.signing.clearSignerPairWaiting'),
		clearSignerCopyLink: strings('componentsUi.signing.clearSignerCopyLink'),
		clearSignerCode: strings('componentsUi.signing.clearSignerCode'),
		clearSignerCodeConfirm: strings('componentsUi.signing.clearSignerCodeConfirm'),
		clearSignerTunnelDown: strings('componentsUi.signing.clearSignerTunnelDown')
	} satisfies ClearSignerWords);

	const model = $derived(clearSignerModel(clearSignerSession.view, words));
</script>

{#if model}
	<ClearSignerSheet
		{model}
		onreopen={() => clearSignerSession.reopen()}
		onwhere={(where) => clearSignerSession.answerWhere(where)}
		onconfirmcode={() => clearSignerSession.confirmCode()}
		ondismiss={() =>
			model.waiting || model.where !== undefined || model.pair !== undefined
				? clearSignerSession.cancel()
				: clearSignerSession.dismiss()}
	/>
{/if}
