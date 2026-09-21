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
		onchip?: (id: string) => void;
		onfee?: () => void;
		onfeepick?: (id: string) => void;
		onspeed?: () => void;
		onspeedpick?: (id: string) => void;
		onsignwith?: (id: string | null) => void;
	}

	let { model, onconfirm, onchip, onfee, onfeepick, onspeed, onspeedpick, onsignwith }: Props =
		$props();
</script>

<div class="panel">
	<SigningHeader dapp={model.dapp} network={model.network} />
	<SigningBody
		{model}
		{onconfirm}
		{onchip}
		{onfee}
		{onfeepick}
		{onspeed}
		{onspeedpick}
		{onsignwith}
	/>
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
	}
</style>
