<script lang="ts">
	/**
	 * The signing sheet, wherever a request can arrive (spec 027 T340).
	 *
	 * 026 wired this into the wallet route, because that is where a request
	 * could reach a person. 027 adds a second place — the extension's request
	 * window — and a second copy of the wiring would be a second implementation
	 * of the most dangerous screen in the product. So the wiring moved here and
	 * both mount it.
	 *
	 * Everything it touches is already a singleton: `sign_request` is
	 * app-resident because a request can arrive while any screen is showing, and
	 * the two per-request machines live beside it. What this component adds is
	 * the four joins between them and the drawn sheet — and one of those joins
	 * is load-bearing:
	 *
	 * **the approve carries the GUARD's rewritten params**, not the ones the
	 * dApp asked for. Passing the original would be the never-unlimited mandate
	 * defeated at the last step.
	 */
	import SigningSheetView from '$lib/signing/SigningSheet.svelte';
	import { buildSigningModel } from '$lib/signing/live';
	import { signingSheet } from '$lib/signing/core/sheet.svelte';
	import { signRequest } from '$lib/signing/core/sign-resident.svelte';
	import { session } from '$lib/session/core/session.svelte';
	import { avatarSvgForClient } from '$lib/wallet/identicon';
	import { IDLE_FEE_VIEW, type FeeQuote } from '$lib/flows/core/fee-quote.svelte';
	import type { SigningMessages } from '$lib/signing/messages';

	interface Props {
		messages: SigningMessages;
		/** The live fee session. One per surface — never a second quote. */
		fee: FeeQuote;
		/** The fee-coin sheet is a shell surface; a host that draws none says so. */
		onfee?: () => void;
	}
	let { messages, fee, onfee = () => {} }: Props = $props();

	const view = $derived(session.view);
	const signView = $derived(signRequest.view);

	const identity = $derived(
		view.has_wallet
			? {
					name: view.accounts[view.active_index]?.account.name ?? '',
					address: view.address,
					identiconSvg: avatarSvgForClient(
						view.address,
						view.accounts[view.active_index]?.account.name ?? ''
					)
				}
			: null
	);

	// A request arrives → both per-request machines are told about it. It goes →
	// they go with it.
	$effect(() => {
		const request = signView.request;
		if (request && signView.surface !== 'hidden') {
			void signingSheet.present(request, identity?.address ?? null);
		} else {
			signingSheet.dismiss();
		}
	});

	const model = $derived.by(() => {
		if (!identity) return null;
		return buildSigningModel({
			sign: signView,
			clear: signingSheet.clear,
			guard: signingSheet.guard,
			fee: fee.view ?? IDLE_FEE_VIEW,
			m: messages,
			identity,
			identicon: avatarSvgForClient
		});
	});

	/**
	 * What the approve carries. The fee is the live session's, and the params
	 * override is the GUARD's rewrite — the capped approval, not the requested
	 * one.
	 */
	function approveOpts() {
		const quote = fee.view?.fee ?? null;
		return {
			max_fee_per_gas: quote ? quote.max_fee_per_gas : null,
			bundler_cost_wei: null,
			gas_fee_token: fee.view?.fee_token ?? null,
			quoted_fee: null,
			fee_collector: null,
			params_override_json: signingSheet.guard.rewritten_params_json,
			intent: null
		};
	}

	/** The chip ids the drawn editor emits, in the guard's vocabulary. */
	function guardChip(id: string): void {
		if (id === 'requested' || id === 'balance' || id === 'custom' || id === 'revoke') {
			signingSheet.dispatchGuard({ type: 'preset_selected', mode: id });
		}
	}

	/**
	 * Every keystroke in the cap field, back to the machine that validates it.
	 *
	 * The core owns the parse (`custom_amount_changed` is already dot-normalized
	 * by the shell, as payment_request's amount is), so nothing here decides
	 * whether what was typed is a number — it only carries it.
	 */
	function guardCustom(text: string): void {
		signingSheet.dispatchGuard({ type: 'custom_amount_changed', text });
	}
</script>

{#if model}
	<!--
		Dismissal IS rejection (the 022 interaction contract draws no reject
		button), so closing answers the transport with 4001 through the core.
	-->
	<SigningSheetView
		{model}
		onclose={() => signRequest.dispatch({ type: 'reject_tapped' })}
		onconfirm={() => signRequest.dispatch({ type: 'approve_tapped', opts: approveOpts() })}
		onchip={guardChip}
		oncustom={guardCustom}
		{onfee}
	/>
{/if}
