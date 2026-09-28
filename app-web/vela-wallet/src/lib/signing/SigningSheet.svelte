<script lang="ts">
	import SigningHeader from './ui/SigningHeader.svelte';
	import SigningBody from './ui/SigningBody.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import StatusHero from '$lib/flows/ui/StatusHero.svelte';
	import type { SigningModel } from './model';

	/**
	 * The phone signing sheet (spec 022) — a bottom sheet over the page that
	 * asked for the signature, so the site you are dealing with never leaves
	 * the screen. It is the app's one sheet (`BottomSheet`, `signing` skin),
	 * and past the desktop breakpoint the same centred card it always was.
	 *
	 * It closes only on its ✕ (spec 079, owner ruling: "除非用户明确关掉，不应该
	 * 很容易误操作，比如下滑就关掉了" — a stray touch on the scrim threw the dApp's
	 * request away and the site had to ask again). No drag, no scrim tap, no
	 * Escape. The ✕ is quiet and is not labelled "Reject", because a wallet with
	 * a big reject button teaches people to reach for it without reading; before
	 * the approval it is the refusal (the host answers 4001).
	 *
	 * After the approval the sheet is a STATUS (spec 079, F11): the header and
	 * the send receipt's `StatusHero` — "waiting for biometric", "submitting",
	 * or why it failed — and no form under it: no fee controls, and never a
	 * greyed slide. The ✕ then closes without refusing, once the signature
	 * exists (`dismissible: false` shuts it while it does not).
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
		/** Spec 079: the fee row's refresh control. */
		onfeerefresh?: () => void;
		/**
		 * Spec 079: the ✕ was pressed — BEFORE the exit plays, so the host
		 * decides what this close means at the moment of the tap (a refusal
		 * before the approval, a plain close after), not 400 ms later.
		 */
		onclosestart?: () => void;
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
		onspeedpick,
		onfeerefresh,
		onclosestart
	}: Props = $props();

	let sheet = $state<{ close: () => void }>();
</script>

<BottomSheet
	bind:this={sheet}
	title={model.panelTitle}
	hideTitle
	variant="signing"
	dismissible={dismissible ? 'explicit' : false}
	{onclose}
>
	<!-- A refused request (spec 081) already offers its one way out, the
	     labelled Close under the refusal; a second ✕ would be two doors. -->
	<SigningHeader
		dapp={model.dapp}
		network={model.network}
		closeLabel={model.closeLabel}
		closeDisabled={!dismissible}
		onclose={onclose && !model.dismissOnly
			? () => {
					onclosestart?.();
					sheet?.close();
				}
			: undefined}
	/>
	{#if model.status}
		<div class="status" data-testid="signing-status" aria-live="polite">
			<StatusHero
				stage={model.status.stage}
				title={model.status.title}
				captions={model.status.captions}
			/>
		</div>
	{:else}
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
			{onfeerefresh}
		/>
	{/if}
</BottomSheet>

<style>
	/* The receipt's centrepiece, with the room the form's body had. */
	.status {
		display: flex;
		justify-content: center;
		padding-block: var(--space-3xl);
	}
</style>
