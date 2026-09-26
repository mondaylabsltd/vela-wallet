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
	import { buildSigningModel, signWithModel } from '$lib/signing/live';
	import { signingSheet } from '$lib/signing/core/sheet.svelte';
	import { signRequest } from '$lib/signing/core/sign-resident.svelte';
	import { session } from '$lib/session/core/session.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { FeeQuote } from '$lib/flows/core/fee-quote.svelte';
	import { currency } from '$lib/settings/core/currency.svelte';
	import type { SigningMessages } from '$lib/signing/messages';
	import { feeCallsOf } from '$lib/signing/fee-calls';
	import DappReceipt from '$lib/signing/ui/DappReceipt.svelte';
	import {
		dappReceiptModel,
		landingToRaise,
		receiptProgress,
		type DappReceiptCopy,
		type DappReceiptState
	} from '$lib/signing/dapp-receipt';
	import { explorerBaseURL } from '$lib/services/networks';
	import { typicalInclusionSeconds } from '$lib/core/kernels';
	import { subscribeTxTracker, txTrackerView } from '$lib/wallet/core/tracker-resident';
	import type { FeeTier } from '$lib/core/generated/FeeTier';
	import { SpeedControl } from '$lib/flows/core/speed-control.svelte';
	import { onMount } from 'svelte';
	import { setSignMethod } from '$lib/onboarding/core/passkey';
	import { signPreference } from '$lib/settings/core/sign-pref.svelte';

	interface Props {
		messages: SigningMessages;
		/** The live fee session. One per surface — never a second quote. */
		fee: FeeQuote;
		/** The fee-coin sheet is a shell surface; a host that draws none says so. */
		onfee?: () => void;
		/**
		 * Spec 077: the send receipt's own words, for the landing this host
		 * draws once a transaction is submitted. A host that passes none draws
		 * no landing — which is what every surface did before 077.
		 */
		receipt?: DappReceiptCopy;
		/**
		 * A transaction has been answered and is landing HERE. The request
		 * window uses it to stop closing itself.
		 */
		onlanding?: () => void;
		/** The person pressed Done on the landing. */
		onreceiptdone?: () => void;
	}
	let { messages, fee, onfee = () => {}, receipt, onlanding, onreceiptdone }: Props = $props();

	/**
	 * Spec 077: where a submitted transaction stands.
	 *
	 * It lives HERE rather than in one surface, because every surface that
	 * mounts this sheet submits through the same machine — a dApp request,
	 * Settings' backup to Ethereum, a payment request. The first version put
	 * the landing in the request window alone and the BACKUP still had none:
	 * it posts into the same seam deliberately, so as not to build "a second,
	 * lesser copy of the most dangerous screen there is". One sheet, one
	 * landing.
	 */
	let landing = $state<DappReceiptState | null>(null);
	let landingChain = $state(0);
	let landedAtMs = $state(0);
	let unsubscribeTracker: (() => void) | undefined;
	/** This chain's usual time to land, in seconds; `0` when there is none. */
	let landingTypicalS = $state(0);
	/**
	 * The receipt's clock. One second is the right grain — the ring has to
	 * MOVE while a person watches it, and a `Date.now()` read inside the markup
	 * would be sampled once and then sit still (spec 038 #D3 does the same).
	 */
	let nowMs = $state(Date.now());
	$effect(() => {
		if (landing?.kind !== 'submitted') return;
		const tick = setInterval(() => (nowMs = Date.now()), 1000);
		return () => clearInterval(tick);
	});

	/**
	 * Follow one operation to the chain.
	 *
	 * The tracker is ALREADY following it — `sign-resident` hands every
	 * `tracker_handoff` over the moment it appears — so this subscribes to what
	 * is running rather than starting a second watch of the same hash.
	 */
	function watchLanding(opHash: string, chain: number): void {
		landingChain = chain;
		landedAtMs = Date.now();
		// How long this chain usually takes, from the CORE's table — the same
		// number the send receipt draws its ring with. `0` (a chain Vela ships
		// no estimate for) makes the ring circle instead of filling.
		landingTypicalS = typicalInclusionSeconds(chain);
		landing = { kind: 'submitting' };
		readTracker(opHash);
		unsubscribeTracker?.();
		unsubscribeTracker = subscribeTxTracker(() => readTracker(opHash));
		onlanding?.();
	}

	/**
	 * What the tracker says, as the receipt's own state.
	 *
	 * `dropped` / `rejected` are failures with a hash to look at; `unreachable`
	 * is NOT — the wallet could not ask, which is not the chain saying no, and
	 * a cross drawn for it would be a verdict this wallet does not have.
	 */
	function readTracker(opHash: string): void {
		const wanted = opHash.toLowerCase();
		const entry = txTrackerView().entries.find((row) => row.user_op_hash.toLowerCase() === wanted);
		if (!entry) return;
		if (entry.status === 'confirmed' && entry.tx_hash) {
			landing = { kind: 'confirmed', opHash, txHash: entry.tx_hash };
		} else if (entry.status === 'dropped' || entry.status === 'rejected') {
			landing = { kind: 'failed', opHash };
		} else {
			landing = { kind: 'submitted', opHash };
		}
		if (entry.submitted_at_ms) landedAtMs = entry.submitted_at_ms;
	}

	/**
	 * The operation a landing has already been raised for.
	 *
	 * Deliberately NOT `$state`: the effect below writes it, and the one thing it
	 * must not do is depend on it. Gating on `landing !== null` instead is what
	 * made the receipt undismissable — Done set `landing = null`, the effect
	 * re-ran, the handoff was still in the view, and it raised the same receipt
	 * again. Measured in the packaged extension: neither a trusted click nor a
	 * synthetic one could get past it.
	 */
	let landedOp: string | null = null;

	/**
	 * The handoff lands on a view AFTER the one that answered the requester, so
	 * this WATCHES for it rather than reading it at the answer. Reading it too
	 * early found `null` every time and the request window closed on the person
	 * mid-submit — found by driving the packaged extension, not by a test.
	 *
	 * One landing per operation: a NEW request brings a new hash and raises a new
	 * receipt, and a dismissed one stays dismissed.
	 */
	$effect(() => {
		if (!receipt) return;
		const handoff = signRequest.view.tracker_handoff;
		const op = landingToRaise(handoff?.user_op_hash, landedOp);
		if (!op || !handoff) return;
		landedOp = op;
		watchLanding(op, handoff.chain_id);
	});

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
		const calls = feeCallsOf(request.kind, request.params_json);
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
			// Nobody has chosen the fee coin for this request: the fee machine
			// pays in one that can (spec 078) instead of quoting the native coin
			// a wallet may not hold and sending the person to the picker. The
			// approve carries the view's `fee_token` — the coin it picked — so
			// the coin displayed is the coin signed, exactly as after a tap.
			autoFeeToken: true,
			// HOW FAST is the speed control's to say: the stored default, a
			// one-shot pick, or a free upgrade.
			tier: speedControl.tier,
			publicKeyHex: account
				? (account.keys[0]?.public_key_hex ?? account.public_key_hex)
				: undefined
		});
	});

	// WHERE the signing passkey is — or whether the Trusted Signer signs (spec
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

<!--
	Spec 077: the landing, over whatever surface mounted this sheet. It sits
	BEFORE the sheet so a submitted transaction is what is on screen, and the
	sheet underneath is gone by then anyway (the request was answered).
-->
{#if landing && receipt}
	<div class="landing-over">
		<DappReceipt
			model={dappReceiptModel(landing, receipt, (txHash) => {
				const base = explorerBaseURL(landingChain);
				return base ? `${base}/tx/${txHash}` : null;
			})}
			progress={landing.kind === 'submitted'
				? receiptProgress(landedAtMs, landingTypicalS, nowMs)
				: undefined}
			ondone={() => {
				landing = null;
				unsubscribeTracker?.();
				unsubscribeTracker = undefined;
				onreceiptdone?.();
			}}
		/>
	</div>
{:else if model}
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
			// A tap is the person's choice, never the machine's: priced as
			// picked from here on (`autoFeeToken: false`). While the machine
			// was still choosing, even a tap on the coin it had picked is
			// asked again — `feeToken` there only named the fallback, and the
			// previews must stop choosing for themselves too.
			const token = id === 'native' ? null : id;
			const last = fee.lastRequest;
			if (last && (last.feeToken !== token || last.autoFeeToken)) {
				void fee.requestQuote({
					...last,
					feeToken: token,
					autoFeeToken: false,
					tier: speedControl.tier
				});
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

<style>
	/* Over the surface that raised the request — its own scrim, like the sheet's. */
	.landing-over {
		position: fixed;
		inset: 0;
		/* Above the sheet's own 20/21 (`SigningSheet.svelte`): the landing
		   replaces the sheet rather than sharing the screen with it. */
		z-index: 60;
		background: var(--color-bg-base);
		overflow-y: auto;
	}
</style>
