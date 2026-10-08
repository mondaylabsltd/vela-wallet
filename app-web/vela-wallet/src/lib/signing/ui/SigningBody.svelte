<script lang="ts">
	import Button from '$lib/ui/Button.svelte';
	import BlockList from './BlockList.svelte';
	import FeeRow from './FeeRow.svelte';
	import SignerRow from './SignerRow.svelte';
	import TechDetails from './TechDetails.svelte';
	import type { SigningModel } from '../model';

	/**
	 * Everything below the dApp header, shared by the phone sheet and the
	 * desktop third column: blocks, then the fixed footer (technical details →
	 * fee → signer → confirm). The two shells differ in chrome, never in what
	 * they say about a transaction — that is the whole point of one renderer.
	 */
	interface Props {
		model: SigningModel;
		onconfirm?: () => void;
		/** `leg`: the batch leg whose card was tapped; absent = the single approval. */
		onchip?: (id: string, leg?: number) => void;
		oncustom?: (text: string, leg?: number) => void;
		onfee?: () => void;
		onfeepick?: (id: string) => void;
		/** Spec 069: fold / unfold the speed control, and a one-shot pick. */
		onspeed?: () => void;
		onspeedpick?: (id: string) => void;
		/** Spec 079: the fee row's refresh control. */
		onfeerefresh?: () => void;
		/** Spec 081: the way out of a refused request. */
		onclose?: () => void;
	}

	let {
		model,
		onconfirm,
		onclose,
		onchip,
		oncustom,
		onfee,
		onfeepick,
		onspeed,
		onspeedpick,
		onfeerefresh
	}: Props = $props();

	// cs29 ships the disclosure open; anything after that is the person's call.
	let techOverride = $state<boolean | undefined>();
	const techOpen = $derived(techOverride ?? model.techOpen);
</script>

<div class="blocks">
	<BlockList blocks={model.blocks} {onchip} {oncustom} />
</div>

<div class="footer">
	<TechDetails tech={model.tech} open={techOpen} ontoggle={() => (techOverride = !techOpen)} />
	{#if !model.dismissOnly}
		<FeeRow
			fee={model.fee}
			ontoggle={onfee}
			onpick={onfeepick}
			{onspeed}
			{onspeedpick}
			onrefresh={onfeerefresh}
		/>
	{/if}
	<SignerRow
		label={model.signer.label}
		name={model.signer.name}
		identiconSvg={model.signer.identiconSvg}
		address={model.signer.address}
	/>
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
		{#if !model.confirm.enabled && model.confirm.note}
			<!-- Spec 099 R7: a shut confirm says why, in the core's words. -->
			<p class="confirm-note">{model.confirm.note}</p>
		{/if}
	{/if}
</div>

<style>
	.confirm-note {
		margin: var(--space-sm) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		text-align: center;
	}
	.dismiss,
	.confirm {
		display: flex;
	}
	.dismiss > :global(*),
	.confirm > :global(*) {
		flex: 1;
	}
	.blocks {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block: var(--space-xl);
	}

	.footer {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		padding-top: var(--space-md);
		border-top: var(--border-hairline) solid var(--color-border-base);
	}
</style>
