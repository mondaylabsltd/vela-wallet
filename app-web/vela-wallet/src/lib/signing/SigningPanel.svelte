<script lang="ts">
	import SigningHeader from './ui/SigningHeader.svelte';
	import SigningBody from './ui/SigningBody.svelte';
	import type { SigningModel } from './model';

	/**
	 * The desktop third column's signing request (spec 022, DCS1–8 / DE4).
	 *
	 * Same atoms, same order, same words as the phone sheet — the only
	 * difference is that the page stays fully visible beside it, which is the
	 * desktop's own anti-phishing advantage: you can compare the request
	 * against the page that raised it without dismissing either.
	 */
	interface Props {
		model: SigningModel;
		onconfirm?: () => void;
		/** `leg`: the batch leg whose card was tapped; absent = the single approval. */
		onchip?: (id: string, leg?: number) => void;
		onfee?: () => void;
		onfeepick?: (id: string) => void;
		onspeed?: () => void;
		onspeedpick?: (id: string) => void;
	}

	let { model, onconfirm, onchip, onfee, onfeepick, onspeed, onspeedpick }: Props = $props();
</script>

<div class="panel">
	<!-- The column's own title and ✕ are the panel's (`ThirdPanel`): the wallet's
	     own request draws its headline here and nothing else. -->
	<SigningHeader dapp={model.dapp} network={model.network} headline={model.headline} />
	<!-- A hand-off (spec 102) is the body's too: the fee and account rows, then the card. -->
	<SigningBody {model} {onconfirm} {onchip} {onfee} {onfeepick} {onspeed} {onspeedpick} />
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
	}
</style>
