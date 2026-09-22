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
	import { buildSigningModel, clearSignerModel, signWithModel } from '$lib/signing/live';
	import { signingSheet } from '$lib/signing/core/sheet.svelte';
	import { signRequest } from '$lib/signing/core/sign-resident.svelte';
	import { session } from '$lib/session/core/session.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { FeeQuote } from '$lib/flows/core/fee-quote.svelte';
	import { currency } from '$lib/settings/core/currency.svelte';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { FeeCall } from '$lib/core/generated/FeeCall';
	import type { FeeTier } from '$lib/core/generated/FeeTier';
	import { SpeedControl } from '$lib/flows/core/speed-control.svelte';
	import { onMount } from 'svelte';
	import { setSignMethod } from '$lib/onboarding/core/passkey';
	import { signPreference } from '$lib/settings/core/sign-pref.svelte';
	import { clearSignerSession } from '$lib/signing/core/clear-signer.svelte';
	import ClearSignerSheet from '$lib/signing/ui/ClearSignerSheet.svelte';

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
					identiconSvg: identiconSvgForClient(view.address)
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

	/**
	 * The REAL calls of a transaction request, as the fee machine prices them.
	 * `null` for anything that is not an on-chain operation (a message has no
	 * fee) or whose params this cannot read — then no fee is drawn, and the
	 * slide does not wait for one.
	 */
	function callsOf(kind: string, paramsJson: string): FeeCall[] | null {
		try {
			const first = (JSON.parse(paramsJson) as unknown[])[0] as
				{ to?: string; value?: string; data?: string; calls?: unknown[] } | undefined;
			const raw = kind === 'batch' ? (first?.calls ?? []) : kind === 'transaction' ? [first] : [];
			const calls = (raw as { to?: string; value?: string; data?: string }[]).map((call) => ({
				to: call?.to ?? '',
				value: BigInt(call?.value ?? '0x0').toString(),
				data: call?.data ?? '0x'
			}));
			return calls.length > 0 && calls.every((call) => call.to !== '') ? calls : null;
		} catch {
			return null;
		}
	}

	// The speed control (spec 069): the send form's own, over this sheet's fee
	// session. A dApp transaction starts at the person's stored default, can be
	// bumped for this one request, and — like a send — goes at the fastest speed
	// where that costs no more. Every rule is the `fee_speed` core's.
	const speedControl = new SpeedControl(
		() => fee,
		() => signView.request !== null && signView.surface !== 'hidden' && quotedFor !== ''
	);
	// Still choosing: the sheet is up and nothing has been signed yet. A free
	// upgrade is never decided under somebody who already slid.
	speedControl.attach(
		() =>
			signView.request !== null &&
			signView.surface !== 'hidden' &&
			!signView.is_signing &&
			!signView.is_submitting
	);
	onMount(() => {
		void speedControl.boot();
		// Where every request's "Sign with" starts, and the page the Clear
		// Signer opens (spec 071).
		void signPreference.boot();
		return () => speedControl.dispose();
	});

	// A transaction is shown WITH what it costs (the founder's standing rule:
	// signing mirrors Send). Until this, no surface asked for the quote when a
	// request arrived, so the web sheet drew no fee for anything — and the
	// backup to Ethereum, which costs real dollars, said nothing about it.
	let quotedFor = '';
	$effect(() => {
		const request = signView.request;
		if (!request || signView.surface === 'hidden' || !identity) {
			// The request went: its one-shot speed goes with it.
			if (quotedFor !== '') speedControl.reset();
			quotedFor = '';
			return;
		}
		if (quotedFor === request.id) return;
		const calls = callsOf(request.kind, request.params_json);
		if (calls === null) return;
		// A new request starts at the stored default, never at the last one's pick.
		speedControl.reset();
		quotedFor = request.id;
		const account = view.accounts.find(
			(row) => row.account.address.toLowerCase() === identity.address.toLowerCase()
		)?.account;
		void fee.requestQuote({
			chainId: request.chain_id,
			account: identity.address,
			calls,
			feeToken: null,
			// HOW FAST is the speed control's to say: the stored default, a
			// one-shot pick, or a free upgrade.
			tier: speedControl.tier,
			publicKeyHex: account
				? (account.keys[0]?.public_key_hex ?? account.public_key_hex)
				: undefined
		});
	});

	// WHERE the signing passkey is — or whether the Clear Signer signs (spec
	// 071) — this request's, and only this request's. Every request starts at
	// Settings' default (`sign_pref`); a pick here lies over it for this request
	// alone and never reaches the preference. The passkey module reads it at the
	// ceremony, and hears `null` the moment the sheet is gone, so a choice made
	// for one request never signs another.
	let picked = $state<{ id: string; method: string } | null>(null);
	let signWithOpen = $state(false);
	/** The fee-coin list is open. Like Send's: every coin the relay takes, the core's verdict on each. */
	let feeOpen = $state(false);
	const signWith = $derived(
		signWithModel({
			offered: signPreference.view.offered,
			defaultMethod: signPreference.view.method,
			picked: picked !== null && picked.id === signView.request?.id ? picked.method : null,
			open: signWithOpen,
			m: messages
		})
	);
	$effect(() => {
		setSignMethod(signView.request && signView.surface !== 'hidden' ? signWith.method : null);
	});
	$effect(() => {
		if (signView.request && signView.surface !== 'hidden') return;
		picked = null;
		signWithOpen = false;
		feeOpen = false;
	});

	function onSignWith(id: string | null): void {
		if (id === null) {
			signWithOpen = !signWithOpen;
			return;
		}
		const request = signView.request;
		if (request) picked = { id: request.id, method: id };
		signWithOpen = false;
	}

	/** The Clear Signer's sheet: waiting on its page, or the sentence it ended with. */
	const clearSigner = $derived(clearSignerModel(clearSignerSession.view, messages));

	const model = $derived.by(() => {
		if (!identity) return null;
		const built = buildSigningModel({
			sign: signView,
			clear: signingSheet.clear,
			guard: signingSheet.guard,
			fee: speedControl.feeInForce,
			feeOpen,
			speed: {
				view: speedControl.view,
				feeOptions: (tier: FeeTier) => speedControl.feeOptions(tier)
			},
			currency: currency.view,
			m: messages,
			identity,
			identicon: identiconSvgForClient
		});
		if (!built) return built;
		return {
			...built,
			signWith: signWith.row
		};
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
			// The fee this sheet DISPLAYED, signed verbatim — amount, recipient
			// and the speed it was priced at (spec 069), as the phones and the
			// desktop always have. Until now the web sent none, so its submit
			// re-priced on its own and a speed picked here could not reach the
			// relay. No recipient, no in-band quote: the core's own rule.
			quoted_fee:
				quote && quote.fee_recipient
					? {
							amount: quote.fee_asset.type === 'erc20' ? quote.fee_asset.amount : quote.total_wei,
							recipient: quote.fee_recipient,
							tier: quote.tier
						}
					: null,
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
		onfee={() => {
			// Failed → ask again. More than one coin → open the list, here in the
			// sheet. Otherwise the host's own surface, if it has one.
			if (fee.view?.failed) fee.requote();
			else if ((fee.view?.options.length ?? 0) > 1) feeOpen = !feeOpen;
			else onfee();
		}}
		onfeepick={(id) => {
			// The pick is a quote PARAMETER, and part of the operation every
			// speed's preview replays (spec 069): the whole question is asked
			// again in that coin, so a speed picked next is still paid in it.
			// Telling the session in force alone would leave the previews in
			// the old coin, and promoting one would switch the coin back. The
			// approve carries `fee_token` from the same view.
			const token = id === 'native' ? null : id;
			const last = fee.lastRequest;
			if (last && last.feeToken !== token) {
				void fee.requestQuote({ ...last, feeToken: token, tier: speedControl.tier });
			}
			feeOpen = false;
		}}
		onsignwith={onSignWith}
		onspeed={() => speedControl.toggle()}
		onspeedpick={(id) => {
			if (id === 'fast' || id === 'standard' || id === 'slow') speedControl.pick(id);
		}}
	/>
{/if}

<!--
	Whichever surface the signature started from — this sheet, the send screen,
	the key backup — the Clear Signer's waiting and its ending are drawn here,
	over it, and that surface stays as it was underneath.
-->
{#if clearSigner}
	<ClearSignerSheet
		model={clearSigner}
		onreopen={() => clearSignerSession.reopen()}
		ondismiss={() =>
			clearSigner?.waiting ? clearSignerSession.cancel() : clearSignerSession.dismiss()}
	/>
{/if}
