<script lang="ts">
	import BlockList from './BlockList.svelte';
	import FeeRow from './FeeRow.svelte';
	import HandoffCard from './HandoffCard.svelte';
	import SignerRow from './SignerRow.svelte';
	import SigningAction from './SigningAction.svelte';
	import TechDetails from './TechDetails.svelte';
	import type { SigningModel } from '../model';

	/**
	 * Everything below the dApp header, shared by the phone sheet and the
	 * desktop third column: blocks, then the fixed footer (technical details →
	 * fee → signer → confirm). The two shells differ in chrome, never in what
	 * they say about a transaction — that is the whole point of one renderer.
	 *
	 * PR 3 device round: the confirm is `SigningAction`, and a host with a foot
	 * outside its scroll draws it there itself (`pinned` — the phone sheet and
	 * the centred card), so the body can grow and scroll without moving it.
	 *
	 * Spec 102 (D4): an account that reviews and signs on a trusted page gets
	 * the hand-off card in place of the preview and the confirm. The fee row —
	 * its speed control and fee-coin picker — and the signing account stay
	 * above the card: the fee is chosen here before the page opens (D-18), and
	 * the card does not say it again.
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
		/** Spec 102 (D4): Open on the hand-off card — the apps' only; absent on the web. */
		onopenpage?: () => void;
		/**
		 * The host draws the action (`SigningAction`) in its own pinned foot:
		 * nothing is drawn after the signing account here.
		 */
		pinned?: boolean;
	}

	let {
		model,
		pinned = false,
		onconfirm,
		onclose,
		onchip,
		oncustom,
		onfee,
		onfeepick,
		onspeed,
		onspeedpick,
		onfeerefresh,
		onopenpage
	}: Props = $props();

	// cs29 ships the disclosure open; anything after that is the person's call.
	let techOverride = $state<boolean | undefined>();
	const techOpen = $derived(techOverride ?? model.techOpen);
</script>

{#if model.handoff}
	<!-- Spec 102 (D4): reviewed on the trusted page — a hand-off, not a preview. -->
	<div class="footer handoff">
		<FeeRow
			fee={model.fee}
			ontoggle={onfee}
			onpick={onfeepick}
			{onspeed}
			{onspeedpick}
			onrefresh={onfeerefresh}
		/>
		<SignerRow
			label={model.signer.label}
			name={model.signer.name}
			identiconSvg={model.signer.identiconSvg}
			address={model.signer.address}
		/>
	</div>
	<HandoffCard handoff={model.handoff} onopen={onopenpage} />
{:else}
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
		{#if !pinned}
			<SigningAction {model} {onconfirm} {onclose} />
		{/if}
	</div>
{/if}

<style>
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

	/* No preview above: the rows start where the blocks would have. */
	.footer.handoff {
		padding-top: var(--space-xl);
	}
</style>
