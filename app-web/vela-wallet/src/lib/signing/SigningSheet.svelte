<script lang="ts">
	import SigningHeader from './ui/SigningHeader.svelte';
	import SigningBody from './ui/SigningBody.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import type { SigningModel } from './model';

	/**
	 * The phone signing sheet (spec 022) — a bottom sheet over the page that
	 * asked for the signature, so the site you are dealing with never leaves
	 * the screen. It is the app's one sheet (`BottomSheet`, `signing` skin):
	 * the same grabber, drag, scrim and Escape as every other, and past the
	 * desktop breakpoint the same centred card it always was.
	 *
	 * Dismissal is rejection. The scrim, the drag and Escape all do the same
	 * thing, and none of them is labelled "Reject", because a wallet with a
	 * reject button teaches people to reach for it without reading. While the
	 * signature is in flight (`dismissible: false`) none of them does anything
	 * — the drag resists and comes back.
	 */
	interface Props {
		model: SigningModel;
		/** false while the signature is in flight. */
		dismissible?: boolean;
		onclose?: () => void;
		onconfirm?: () => void;
		/** `leg`: the batch leg whose card was tapped; absent = the single approval. */
		onchip?: (id: string, leg?: number) => void;
		oncustom?: (text: string, leg?: number) => void;
		onfee?: () => void;
		onfeepick?: (id: string) => void;
		onspeed?: () => void;
		onspeedpick?: (id: string) => void;
	}

	let {
		model,
		dismissible = true,
		onclose,
		onconfirm,
		onchip,
		oncustom,
		onfee,
		onfeepick,
		onspeed,
		onspeedpick
	}: Props = $props();
</script>

<BottomSheet title={model.panelTitle} hideTitle variant="signing" {dismissible} {onclose}>
	<SigningHeader dapp={model.dapp} network={model.network} />
	<SigningBody
		{model}
		{onconfirm}
		{onclose}
		{onchip}
		{oncustom}
		{onfee}
		{onfeepick}
		{onspeed}
		{onspeedpick}
	/>
</BottomSheet>
