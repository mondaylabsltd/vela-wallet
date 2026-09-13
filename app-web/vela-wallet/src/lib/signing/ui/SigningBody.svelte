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
	}

	let { model, onconfirm, onchip, oncustom, onfee }: Props = $props();

	// cs29 ships the disclosure open; anything after that is the person's call.
	let techOverride = $state<boolean | undefined>();
	const techOpen = $derived(techOverride ?? model.techOpen);
</script>

<div class="blocks">
	<BlockList blocks={model.blocks} {onchip} {oncustom} />
</div>

<div class="footer">
	<TechDetails tech={model.tech} open={techOpen} ontoggle={() => (techOverride = !techOpen)} />
	<FeeRow fee={model.fee} ontoggle={onfee} />
	<SignerRow
		label={model.signer.label}
		name={model.signer.name}
		identiconSvg={model.signer.identiconSvg}
		address={model.signer.address}
	/>
	<SlideToConfirm
		hint={model.confirm.hint}
		action={model.confirm.action}
		enabled={model.confirm.enabled}
		{onconfirm}
	/>
</div>

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
</style>
