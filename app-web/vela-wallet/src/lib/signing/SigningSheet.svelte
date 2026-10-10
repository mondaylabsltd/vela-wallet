<script lang="ts">
	import SigningHeader from './ui/SigningHeader.svelte';
	import SigningBody from './ui/SigningBody.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import StatusHero from '$lib/flows/ui/StatusHero.svelte';
	import Button from '$lib/ui/Button.svelte';
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
	 * greyed confirm. The ✕ then closes without refusing, once the signature
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
		/** Spec 096 F8: "Try again" on a failure that sent nothing. */
		onretry?: () => void;
		/** Spec 102 (D4): Open on the hand-off card — the apps' only; absent on the web. */
		onopenpage?: () => void;
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
		onclosestart,
		onretry,
		onopenpage
	}: Props = $props();

	/** The ✕'s own path: the host reads the close's meaning, then the exit plays. */
	function closeNow(): void {
		onclosestart?.();
		sheet?.close();
	}

	let sheet = $state<{
		close: () => void;
		placeOf: (selector: string) => number | null;
		keepAt: (selector: string, top: number, keep?: { show?: string; under?: string }) => void;
		recentre: () => void;
	}>();

	/**
	 * PR 3 final note F2 — the confirm stays where it is when a verdict lands.
	 *
	 * This sheet runs no simulation and keeps no room for one (spec 082 RG6).
	 * The one verdict it can say arrives after it has opened: the relay's own
	 * estimate answering that the operation will revert (RJ19), a danger line
	 * under the intent — at the moment the fee lands and the confirm opens.
	 * Measured before this: on the phone sheet at its full height the confirm
	 * went DOWN 69 px (at 320 × 700, out of the window), and on the centred
	 * card 39 px, as the tap it had just become ready for was on its way.
	 *
	 * The confirm's place is read before the line is drawn (or taken away, or
	 * reworded) and given back after: the sheet grows upward, as the phone
	 * sheet below its full height always did. The line itself is never
	 * scrolled out of sight for it — and neither is the header: who is
	 * asking, and the ✕ that refuses, stay at the top of a sheet that scrolls.
	 */
	const CONFIRM = '[data-testid="signing-confirm"]';
	const VERDICT = '[data-verdict]';
	const HEADER = '[data-signing-top]';
	const verdict = $derived(
		model.blocks.find((block) => block.kind === 'warning' && block.verdict === true)
	);
	const verdictText = $derived(verdict?.kind === 'warning' ? verdict.text : undefined);
	let confirmAt: number | null = null;
	$effect.pre(() => {
		void verdictText;
		confirmAt = sheet?.placeOf(CONFIRM) ?? null;
	});
	$effect(() => {
		void verdictText;
		if (confirmAt !== null) sheet?.keepAt(CONFIRM, confirmAt, { show: VERDICT, under: HEADER });
	});
	// The form gives way to the status (or comes back): another sheet's worth
	// of content, in the middle again.
	const showsStatus = $derived(model.status !== undefined && model.status !== null);
	$effect(() => {
		void showsStatus;
		sheet?.recentre();
	});
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
	<!-- Kept at the top of a sheet that scrolls: the site that is asking and
	     the ✕ never leave the screen, whatever is read under them. -->
	<div class="top" data-signing-top>
		<SigningHeader
			dapp={model.dapp}
			network={model.network}
			headline={model.headline}
			closeLabel={model.closeLabel}
			closeDisabled={!dismissible}
			onclose={onclose && !model.dismissOnly ? closeNow : undefined}
		/>
	</div>
	<!-- Spec 102 (D4): a hand-off is drawn by the body, under the sheet's own
	     fee row and signing account (`SigningBody`). -->
	{#if model.status && model.handoff === undefined}
		<div class="status" data-testid="signing-status" aria-live="polite">
			<StatusHero
				stage={model.status.stage}
				title={model.status.title}
				captions={model.status.captions}
			/>
		</div>
		{#if model.status.actions}
			<!-- Spec 096 F8: a failure says how to leave it — Close answers the
			     page, Try again (only when nothing was sent) goes back to review. -->
			<div class="actions" data-testid="signing-status-actions">
				<Button variant="secondary" shape="rounded" onclick={closeNow}>
					{model.status.actions.close}
				</Button>
				{#if model.status.actions.retry && onretry}
					<Button variant="primary" shape="rounded" onclick={() => onretry?.()}>
						{model.status.actions.retry}
					</Button>
				{/if}
			</div>
		{/if}
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
			{onopenpage}
		/>
	{/if}
</BottomSheet>

<style>
	/* The sheet's own surface, so what scrolls under the header is not seen
	   through it. */
	.top {
		position: sticky;
		top: 0;
		z-index: 1;
		background: var(--color-bg-raised);
		/* A little of the surface under it, so what scrolls beneath does not
		   touch its words — and none of it in the layout at rest. */
		padding-bottom: var(--space-md);
		margin-bottom: calc(-1 * var(--space-md));
	}

	/* The receipt's centrepiece, with the room the form's body had. */
	.status {
		display: flex;
		justify-content: center;
		padding-block: var(--space-3xl);
	}
	.actions {
		display: flex;
		gap: var(--space-lg);
		padding-bottom: var(--space-xl);
	}
	.actions > :global(*) {
		flex: 1;
	}
</style>
