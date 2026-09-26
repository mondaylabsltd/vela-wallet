<script lang="ts">
	import BlockList from './BlockList.svelte';
	import FeeRow from './FeeRow.svelte';
	import SignerRow from './SignerRow.svelte';
	import SlideToConfirm from './SlideToConfirm.svelte';
	import TechDetails from './TechDetails.svelte';
	import type { SigningModel } from '../model';

	/**
	 * Everything below the dApp header, shared by the phone sheet and the
	 * desktop third column: blocks, then the fixed footer (technical details →
	 * fee → signer → slide). The two shells differ in chrome, never in what
	 * they say about a transaction — that is the whole point of one renderer.
	 */
	interface Props {
		model: SigningModel;
		onconfirm?: () => void;
		onchip?: (id: string) => void;
		oncustom?: (text: string) => void;
		onfee?: () => void;
		onfeepick?: (id: string) => void;
		/** Spec 069: fold / unfold the speed control, and a one-shot pick. */
		onspeed?: () => void;
		onspeedpick?: (id: string) => void;
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
		onspeedpick
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
		<FeeRow fee={model.fee} ontoggle={onfee} onpick={onfeepick} {onspeed} {onspeedpick} />
	{/if}
	<SignerRow
		label={model.signer.label}
		name={model.signer.name}
		identiconSvg={model.signer.identiconSvg}
		address={model.signer.address}
	/>
	<!--
		Spec 081: a refused request shows no fee and no slider. Leaving a dead
		"Slide to confirm · Enable module" under the refusal reads as an option
		the person merely failed to use.
	-->
	{#if model.dismissOnly}
		<button type="button" class="dismiss" onclick={() => onclose?.()}>{model.dismissOnly}</button>
	{:else}
		<SlideToConfirm
			hint={model.confirm.hint}
			action={model.confirm.action}
			enabled={model.confirm.enabled}
			{onconfirm}
		/>
	{/if}
</div>

<style>
	.dismiss {
		width: 100%;
		padding: var(--space-md) var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: transparent;
		color: var(--color-text-primary);
		font-size: var(--font-size-body);
		font-weight: var(--font-weight-semibold);
		cursor: pointer;
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
