<script lang="ts">
	import Button from '$lib/ui/Button.svelte';
	import type { SigningModel } from '../model';

	/**
	 * The sheet's one action: the confirm and the line the core says under a
	 * shut one — or, for a refused request, its way out.
	 *
	 * Its own component because of where it is drawn (PR 3 device round): the
	 * phone sheet and the centred card keep it in a foot OUTSIDE their scroll
	 * (`SigningSheet` → `BottomSheet footer`), so nothing the body holds — a
	 * verdict that lands late, a tall one — can move it or push it under a
	 * fold; the desktop column draws it after its rows (`SigningBody`).
	 */
	interface Props {
		model: SigningModel;
		onconfirm?: () => void;
		/** Spec 081: the way out of a refused request. */
		onclose?: () => void;
	}

	let { model, onconfirm, onclose }: Props = $props();

	/** Spec 099 R7: why the confirm is shut, in the core's words. */
	const note = $derived(model.confirm.enabled ? undefined : model.confirm.note);
	/**
	 * The note comes and goes with the gate — "Working out the network fee…"
	 * on every re-quote, refresh and new speed — and the phone sheet is
	 * bottom-anchored: each time it came, the whole sheet above rose by a line
	 * (the Android device: ~33 px a re-quote; the web measured 29). Once a note
	 * has been said, its line stays, holding the last words invisibly while
	 * the gate is open, so a measurement moves nothing.
	 */
	let lastNote: string | undefined;
	const noteLine = $derived.by(() => {
		if (note !== undefined) lastNote = note;
		return lastNote;
	});
</script>

<div class="action">
	<!--
		Spec 081: a refused request shows no fee and no confirm. Leaving a dead
		"Enable module" button under the refusal reads as an option the person
		merely failed to use.
	-->
	{#if model.dismissOnly}
		<!-- RB12 (G16): the shared Button — bordered, full width, the control height. -->
		<div class="dismiss">
			<Button variant="secondary" shape="rounded" onclick={() => onclose?.()}>
				{model.dismissOnly}
			</Button>
		</div>
	{:else}
		<!--
			Issue 461: a tap confirms, as it does on the Send screen — the shared
			primary button, full width, labelled with the action alone; it was a
			slide. The second, deliberate step is the passkey prompt the tap
			raises. Shut while the core's gate is (`confirm_state`); once
			approved the status replaces the form, so the button is never seen
			dimmed by its own press.
		-->
		<div class="confirm">
			<Button
				variant="primary"
				shape="rounded"
				testid="signing-confirm"
				disabled={!model.confirm.enabled}
				onclick={() => onconfirm?.()}
			>
				{model.confirm.action}
			</Button>
		</div>
		{#if noteLine}
			<!-- Spec 099 R7: a shut confirm says why, in the core's words. One
			     element for both states — read on every frame, so the line it
			     holds is always the last one said. -->
			<p
				class="confirm-note"
				class:reserved={note === undefined}
				aria-hidden={note === undefined ? 'true' : undefined}
			>
				{noteLine}
			</p>
		{/if}
	{/if}
</div>

<style>
	/* The rows' own column and gap (`SigningBody`'s footer), so the action
	   sits under them the same way wherever it is drawn. */
	.action {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}
	.confirm-note {
		margin: var(--space-sm) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		text-align: center;
	}
	.confirm-note.reserved {
		visibility: hidden;
	}
	.dismiss,
	.confirm {
		display: flex;
	}
	.dismiss > :global(*),
	.confirm > :global(*) {
		flex: 1;
	}
</style>
