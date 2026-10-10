<script lang="ts">
	import { tick } from 'svelte';
	import SigningHeader from './ui/SigningHeader.svelte';
	import SigningAction from './ui/SigningAction.svelte';
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
	 * Its confirm, and the line the core says under a shut one, stand in the
	 * sheet's foot, outside its scroll (PR 3 device round): whatever the body
	 * holds — a verdict that lands late, a tall one — the confirm is whole, on
	 * screen, and where it was. The body scrolls under the header and over
	 * the foot once the sheet is as tall as it may be.
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
		keepAt: (selector: string, top: number) => void;
		reveal: (selector: string, at?: { nth?: number; under?: string }) => void;
		recentre: () => void;
	}>();

	/**
	 * A verdict lands after the sheet has opened — and the confirm is where
	 * it was, and the verdict is in sight (PR 3 final note F2, then the device
	 * round's rule: no part of a verdict hidden, the confirm never moved).
	 *
	 * This sheet keeps no room for a verdict (spec 082 RG6). Two can land
	 * late: the relay's own estimate answering that the operation will revert
	 * (RJ19), a danger line under the intent, and the sheet's own simulation
	 * saying nothing of the person's moves, the "No asset changes" card under
	 * the request. Each is drawn whole, at its own height: nothing in the
	 * sheet clips or scrolls a block by itself.
	 *
	 * The confirm stands in the sheet's foot, outside the scroll, so on the
	 * phone sheet nothing the body gains can move it: the sheet grows upward,
	 * and then its body scrolls. The centred card grows from its middle, so
	 * its foot is held where it rested across the landing (`keepAt`). Then the
	 * body is scrolled, if it has to be, so that what landed can be read
	 * (`reveal`) — under the header, which stays at the top of the scroll:
	 * who is asking, and the ✕ that refuses. Two that land together are both
	 * brought into sight.
	 */
	const CONFIRM = '[data-testid="signing-confirm"]';
	const VERDICT = '[data-verdict]';
	const HEADER = '[data-signing-top]';
	/** Each verdict on the sheet, as what it says: one that changes has landed again. */
	const verdicts = $derived(
		model.blocks
			.filter(
				(block) => (block.kind === 'warning' || block.kind === 'balances') && block.verdict === true
			)
			.map((block) => JSON.stringify(block))
	);
	const verdictKey = $derived(verdicts.join('\n'));
	let confirmAt: number | null = null;
	let seen: string[] = [];
	$effect.pre(() => {
		void verdictKey;
		confirmAt = sheet?.placeOf(CONFIRM) ?? null;
	});
	$effect(() => {
		void verdictKey;
		if (confirmAt !== null) sheet?.keepAt(CONFIRM, confirmAt);
		const landed = verdicts.flatMap((said, nth) => (seen.includes(said) ? [] : [nth]));
		seen = verdicts;
		if (landed.length === 0) return;
		// Once the card has been laid out with its foot held. Each one that
		// landed, the last first: they all end in sight when the body's window
		// holds them together, and when it cannot, the first — the relay's
		// danger line — is the one it ends on.
		void tick().then(() => {
			for (const nth of landed.reverse()) sheet?.reveal(VERDICT, { nth, under: HEADER });
		});
	});
	// The form gives way to the status (or comes back): another sheet's worth
	// of content, in the middle again.
	const showsStatus = $derived(model.status !== undefined && model.status !== null);
	/**
	 * The form, with its one action in the foot. Not the status, and not a
	 * hand-off (spec 102): its card carries its own Open.
	 */
	const showsForm = $derived(model.handoff === undefined && !showsStatus);
	$effect(() => {
		void showsStatus;
		sheet?.recentre();
	});
</script>

<!-- Outside the scroll: the confirm (or a refused request's way out) is
     never under a fold and never moved by what is read above it. -->
{#snippet foot()}
	<div class="foot" data-signing-foot>
		<SigningAction {model} {onconfirm} {onclose} />
	</div>
{/snippet}

<BottomSheet
	bind:this={sheet}
	title={model.panelTitle}
	hideTitle
	variant="signing"
	dismissible={dismissible ? 'explicit' : false}
	{onclose}
	footer={showsForm ? foot : undefined}
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
			pinned={showsForm}
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

	/* Under the signing account by the rows' own gap (`SigningBody`'s
	   footer), as when it was the last of them. */
	.foot {
		padding-top: var(--space-lg);
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
