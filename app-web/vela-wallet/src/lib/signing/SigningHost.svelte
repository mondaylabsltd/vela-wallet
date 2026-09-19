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
	import { currency } from '$lib/settings/core/currency.svelte';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { FeeCall } from '$lib/core/generated/FeeCall';
	import { setSignMethod, type SignMethod } from '$lib/onboarding/core/passkey';

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

	// A transaction is shown WITH what it costs (the founder's standing rule:
	// signing mirrors Send). Until this, no surface asked for the quote when a
	// request arrived, so the web sheet drew no fee for anything — and the
	// backup to Ethereum, which costs real dollars, said nothing about it.
	let quotedFor = '';
	$effect(() => {
		const request = signView.request;
		if (!request || signView.surface === 'hidden' || !identity) {
			quotedFor = '';
			return;
		}
		if (quotedFor === request.id) return;
		const calls = callsOf(request.kind, request.params_json);
		if (calls === null) return;
		quotedFor = request.id;
		const account = view.accounts.find(
			(row) => row.account.address.toLowerCase() === identity.address.toLowerCase()
		)?.account;
		void fee.requestQuote({
			chainId: request.chain_id,
			account: identity.address,
			calls,
			feeToken: null,
			publicKeyHex: account
				? (account.keys[0]?.public_key_hex ?? account.public_key_hex)
				: undefined
		});
	});

	// WHERE the signing passkey is — this request's, and only this request's. The
	// passkey module reads it at the ceremony; it goes back to `auto` the moment
	// the sheet is gone, so a choice made for one request never signs another.
	let signMethod = $state<SignMethod>('auto');
	let signWithOpen = $state(false);
	/** The fee-coin list is open. Like Send's: every coin the relay takes, the core's verdict on each. */
	let feeOpen = $state(false);
	$effect(() => {
		setSignMethod(signMethod);
	});
	$effect(() => {
		if (signView.request && signView.surface !== 'hidden') return;
		signMethod = 'auto';
		signWithOpen = false;
		feeOpen = false;
	});
	function onSignWith(id: string | null): void {
		if (id === null) {
			signWithOpen = !signWithOpen;
			return;
		}
		if (id === 'auto' || id === 'platform' || id === 'hybrid' || id === 'security_key') {
			signMethod = id;
		}
		signWithOpen = false;
	}

	const model = $derived.by(() => {
		if (!identity) return null;
		const titles: Record<SignMethod, string> = {
			auto: messages.signWithAuto,
			platform: messages.signWithPlatform,
			hybrid: messages.signWithHybrid,
			security_key: messages.signWithSecurityKey
		};
		const built = buildSigningModel({
			sign: signView,
			clear: signingSheet.clear,
			guard: signingSheet.guard,
			fee: fee.view ?? IDLE_FEE_VIEW,
			feeOpen,
			currency: currency.view,
			m: messages,
			identity,
			identicon: avatarSvgForClient
		});
		if (!built) return built;
		return {
			...built,
			signWith: {
				label: messages.signWithLabel,
				value: titles[signMethod],
				open: signWithOpen,
				options: (Object.keys(titles) as SignMethod[]).map((id) => ({
					id,
					title: titles[id],
					selected: id === signMethod
				}))
			}
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
		onfee={() => {
			// Failed → ask again. More than one coin → open the list, here in the
			// sheet. Otherwise the host's own surface, if it has one.
			if (fee.view?.failed) fee.requote();
			else if ((fee.view?.options.length ?? 0) > 1) feeOpen = !feeOpen;
			else onfee();
		}}
		onfeepick={(id) => {
			// The pick is a quote PARAMETER: the core re-prices the operation in
			// that coin, and the approve carries `fee_token` from the same view.
			fee.selectAsset(id === 'native' ? null : id);
			feeOpen = false;
		}}
		onsignwith={onSignWith}
	/>
{/if}
