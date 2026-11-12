<script lang="ts">
	import { accountName } from '$lib/multisig/core/account-name';
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
	import { approveOptsOf, buildSigningModel, signingCloseEvent } from '$lib/signing/live';
	import { signingSheet } from '$lib/signing/core/sheet.svelte';
	import { signRequest } from '$lib/signing/core/sign-resident.svelte';
	import { session } from '$lib/session/core/session.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { FeeQuote } from '$lib/flows/core/fee-quote.svelte';
	import { currency } from '$lib/settings/core/currency.svelte';
	import { networkSpace } from '$lib/network-space/network-space.svelte';
	import type { SigningMessages } from '$lib/signing/messages';
	import { feeCallsOf } from '$lib/signing/fee-calls';
	import { simulateRequest } from '$lib/signing/fee-balance-changes';
	import type { FeeCall } from '$lib/core/generated/FeeCall';
	import { poolRpcCall } from '$lib/services/rpc-pool';
	import type { SimEnded } from '$lib/signing/verdict';
	import DappReceipt from '$lib/signing/ui/DappReceipt.svelte';
	import {
		dappReceiptModel,
		endsOnSignedTick,
		landingCloseAction,
		closedLandingHoldsSheet,
		landingFor,
		withSheetRefusal,
		handoffLands,
		landingToRaise,
		autoCloseAfterMs,
		trackEntryFor,
		type DappReceiptCopy,
		type DappReceiptState
	} from '$lib/signing/dapp-receipt';
	import type { SignMethodKind } from '$lib/core/generated/SignMethodKind';
	import { explorerBaseURL } from '$lib/services/networks';
	import { landingPace, typicalInclusionSeconds } from '$lib/core/kernels';
	import { FeeFailureLog } from '$lib/signing/fee-failure-log';
	import { feeRowTap } from '$lib/flows/fee-failure';
	import { subscribeTxTracker, txTrackerView } from '$lib/wallet/core/tracker-resident';
	import type { FeeTier } from '$lib/core/generated/FeeTier';
	import type { FeeView } from '$lib/core/generated/FeeView';
	import type { HoldPhase } from '$lib/ui/hold-confirm';
	import { SpeedControl } from '$lib/flows/core/speed-control.svelte';
	import { panelSurface } from '$lib/dapp/panel-surface.svelte';
	import { onMount, untrack } from 'svelte';
	import { adviceStanding, openReview, sheetBeside } from '$lib/agent/advice.svelte';
	import { adviceCard } from '$lib/agent/advisor';
	import PageHandoffSheet from '$lib/signing/ui/PageHandoffSheet.svelte';
	import { pageHandoff } from '$lib/signing/page-handoff.svelte';
	import { refuseBlind } from '$lib/settings/refuse-blind.svelte';
	import { ownNodeSettingsHref } from '$lib/settings/own-node-href';
	import { ownNodes } from '$lib/services/own-node.svelte';

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
	/** When the relay put it on the network (spec 099 R6): the ring counts from here. */
	let relaySentAtMs = $state<number | null>(null);
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
	function watchLanding(opHash: string, chain: number, maybeSent: boolean): void {
		// The request whose operation this is, while its answer may still be
		// out (a lost relay reply raises the landing before the answer).
		landingRequest = untrack(() => signRequest.view.request?.id ?? null);
		landingChain = chain;
		relaySentAtMs = null;
		// How long this chain usually takes, from the CORE's table — the same
		// number the send receipt draws its ring with. `0` (a chain Vela ships
		// no estimate for) makes the ring circle instead of filling.
		landingTypicalS = typicalInclusionSeconds(chain);
		landing = { kind: 'submitting' };
		readTracker(opHash, maybeSent);
		unsubscribeTracker?.();
		unsubscribeTracker = subscribeTxTracker(() => readTracker(opHash, maybeSent));
		onlanding?.();
	}

	/**
	 * What the tracker says, as the core's ending (`landingFor` →
	 * `signEndingState`, spec 082 RA8): confirmed, reverted, not sent, or
	 * still followed with its outcome — "may have been sent" while the relay
	 * never acknowledged it (RA10). Nothing drawn before the tracker has an
	 * entry, unless the handoff itself said the reply was lost.
	 */
	function readTracker(opHash: string, maybeSent: boolean): void {
		const entry = trackEntryFor(txTrackerView().entries, opHash);
		if (!entry && !maybeSent) return;
		landing = landingFor(entry, opHash, maybeSent);
		relaySentAtMs = entry?.relay_sent_at_ms ?? null;
	}

	/**
	 * Spec 079: a message signature ends on the tick. The core clears the
	 * sheet the moment it answers, and the sheet used to simply vanish — the
	 * person never saw that it had signed. The request the sheet last showed
	 * is matched against the answer the resident recorded; a match that
	 * carried a signature raises "已签名！" for a beat, then it goes by itself.
	 *
	 * Neither is `$state`: the effect below writes them and must not re-run
	 * because it did.
	 */
	let shownRequest: { id: string; kind: SignMethodKind; origin: string } | null = null;
	let answerSeen: unknown = null;
	let signedTick = $state(false);
	/** The request the tick is for — so a DIFFERENT one owed is told apart from it. */
	let tickFor = $state<{ id: string; origin: string } | null>(null);
	$effect(() => {
		const request = signRequest.view.request;
		if (request && signRequest.view.surface !== 'hidden') {
			shownRequest = { id: request.id, kind: request.kind, origin: request.origin };
		}
	});
	$effect(() => {
		const answer = signRequest.answered;
		if (!answer || answer === answerSeen) return;
		answerSeen = answer;
		// The request the person closed after approving has its answer: what
		// it handed the tracker (if anything) is marked seen, so no landing
		// rises for it, and the next approval's landing is not swallowed.
		if (closedInFlight !== null && answer.id === closedInFlight) {
			const handoff = signRequest.view.tracker_handoff;
			if (handoff) {
				landedOp = handoff.user_op_hash;
				landingClosed(handoff.user_op_hash);
			}
			closedInFlight = null;
		}
		if (!receipt) return;
		const shown = shownRequest;
		// One answer per shown request: an id a site reuses later is not the
		// request this sheet drew.
		if (shown?.id === answer.id) shownRequest = null;
		if (endsOnSignedTick(answer, shown)) {
			tickFor = shown ? { id: shown.id, origin: shown.origin } : null;
			signedTick = true;
		}
	});
	$effect(() => {
		if (!signedTick) return;
		const timer = setTimeout(() => (signedTick = false), autoCloseAfterMs({ kind: 'signed' }) ?? 0);
		return () => clearTimeout(timer);
	});

	/**
	 * Spec 082 G65 (RJ20): the full-panel tick hid the NEXT request's card for
	 * ≥ 1.4 s — A signed, B's Connect already queued behind it, and B looked
	 * like it arrived late. When the panel already owes another request, the
	 * tick is skipped and that card shows at once; the page has its signature
	 * either way. `panelSurface.current` is what the worker says this window
	 * owes next (null outside the side panel, where the tick is unchanged).
	 */
	const nextOwed = $derived.by(() => {
		const owed = panelSurface.current;
		if (!owed || !signedTick) return false;
		return !(tickFor !== null && owed.id === tickFor.id && owed.origin === tickFor.origin);
	});
	$effect(() => {
		if (nextOwed) signedTick = false;
	});

	/**
	 * Spec 079: the request the person closed with the ✕ AFTER approving
	 * (the Android `closedAfterApproval`, iOS the same). The operation goes
	 * on and the page still gets its answer, but nothing of it comes back
	 * over whatever they went to — no landing, no tick. Cleared by that
	 * request's answer. Not `$state`: the effects write it.
	 *
	 * A second approval cannot begin while this one's pipeline runs (the
	 * core's confirm gate waits for it), so the next hand-off after a close is
	 * always this request's own.
	 */
	let closedInFlight: string | null = null;
	interface CloseIntent {
		event: ReturnType<typeof signingCloseEvent>;
		requestId: string | null;
		/** Closed with the operation still under way (not a failure already answered). */
		inFlight: boolean;
	}
	/** What the ✕ meant at the moment it was pressed (the exit plays first). */
	let closeIntent: CloseIntent | undefined;

	function closeIntentNow(): CloseIntent {
		const status = model?.status;
		return {
			event: signingCloseEvent(status),
			requestId: signView.request?.id ?? null,
			// A failure, or "not sent yet" (PR 2 polish): either way the page is
			// answered on close and nothing is on its way.
			inFlight: status !== undefined && status.stage !== 'failed' && status.stage !== 'not_sent'
		};
	}

	function closeSheet(): void {
		const intent = closeIntent ?? closeIntentNow();
		closeIntent = undefined;
		if (intent.event === null) return;
		if (intent.event === 'dismiss_tapped') {
			// In flight: the ending must not come back over the page.
			if (intent.inFlight && intent.requestId !== null) closedInFlight = intent.requestId;
			shownRequest = null;
		}
		signRequest.dispatch({ type: intent.event });
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
	 * Round 3 (the owner's side panel): the close is the CORE's to remember.
	 * `landedOp` lives as long as this component, and every screen that hosts
	 * the sheet mounts its own — Wallet → Settings → Wallet made a new host,
	 * which found the hand-off still in the view and raised "Confirmed" again
	 * for a transaction closed long before. Told here, the view says
	 * `landing_closed` for that operation to every host mounted after it.
	 */
	function landingClosed(op: string | null): void {
		if (op !== null) signRequest.dispatch({ type: 'landing_closed', user_op_hash: op });
	}
	/** The operation the landing on screen is for. */
	let landingOp: string | null = null;

	/**
	 * Spec 082 G37 (D3): the request whose landing was raised and then closed
	 * while its answer was still out. A lost relay reply raises the landing
	 * before the answer; the chain check found the op, 已确认 closed itself —
	 * and the sheet underneath came back as 提交至网络… for 49 s, until the page
	 * was answered. That request's sheet now waits hidden for its answer; the
	 * page still gets it, and the next request's sheet is not affected.
	 */
	let landingRequest: string | null = null;
	let settledRequest = $state<string | null>(null);
	const sheetHidden = $derived(
		(settledRequest !== null && signRequest.view.request?.id === settledRequest) ||
			// The same, as the core keeps it — so a host mounted after the close
			// does not bring that request's sheet back either.
			closedLandingHoldsSheet(signRequest.view)
	);
	$effect(() => {
		const id = signRequest.view.request?.id ?? null;
		// The request was answered (or another took its place): nothing to hide.
		if (id !== untrack(() => settledRequest)) settledRequest = null;
	});

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
		// Spec 082 RJ1: not the write-ahead's hand-off while its POST is out.
		const offered = signRequest.view.tracker_handoff;
		const handoff =
			offered && handoffLands(offered, signRequest.view.pending_op_hash) ? offered : null;
		const op = landingToRaise(handoff?.user_op_hash, landedOp);
		if (!op || !handoff) return;
		landedOp = op;
		// Closed already — here, or on a screen this one replaced: the
		// transaction is over for the person, and nothing of it is drawn again.
		if (signRequest.view.landing_closed) return;
		// Closed after approving: tracked, answered — and not raised again.
		if (closedInFlight !== null) {
			landingClosed(op);
			return;
		}
		landingOp = op;
		watchLanding(op, handoff.chain_id, handoff.maybe_sent);
	});

	/**
	 * The landing as drawn: a refusal says why by the sheet's own field while
	 * its request is still the sheet's (PR 2 note 9, `withSheetRefusal`).
	 */
	function landingShown(state: DappReceiptState): DappReceiptState {
		const mine = landingRequest !== null && signView.request?.id === landingRequest;
		return withSheetRefusal(state, mine ? signView.failure_refusal_key : null);
	}

	/** The landing goes: Done, or a landed transaction's own beat. */
	function closeLanding(): void {
		const action = landingCloseAction(signRequest.view, landingRequest);
		// Spec 097 N4: a refusal after "Submitted" is held by the core while
		// it shows; this Done is the close that answers the page, once.
		if (action === 'dismiss_tapped') signRequest.dispatch({ type: 'dismiss_tapped' });
		// Its request still unanswered: its sheet does not come back (G37).
		else if (action === 'hide') settledRequest = landingRequest;
		landingClosed(landingOp);
		landingOp = null;
		landingRequest = null;
		landing = null;
		unsubscribeTracker?.();
		unsubscribeTracker = undefined;
		onreceiptdone?.();
	}

	/**
	 * Spec 079: a landed transaction closes by itself after a beat (~2.6 s, as
	 * on Android and iOS) — nobody should have to close a success. Done still
	 * closes it sooner. "Still confirming" and "unknown" never close on their
	 * own: the person is being told something they need to read.
	 */
	$effect(() => {
		const state = landing;
		if (!state) return;
		const after = autoCloseAfterMs(state);
		if (after === null) return;
		const timer = setTimeout(closeLanding, after);
		return () => clearTimeout(timer);
	});

	const view = $derived(session.view);
	const signView = $derived(signRequest.view);

	const identity = $derived(
		view.has_wallet
			? {
					name: accountName(view.address, view.accounts[view.active_index]?.account.name ?? ''),
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
	// upgrade is never decided under somebody who already confirmed.
	speedControl.attach(
		() =>
			signView.request !== null &&
			signView.surface !== 'hidden' &&
			!signView.is_signing &&
			!signView.is_submitting
	);
	onMount(() => {
		void speedControl.boot();
		// The sheet prices what it shows in the display currency, and no money
		// figure is drawn until that currency is the person's
		// (`CurrencyView.committed`). The wallet and Settings routes boot the
		// store themselves; the extension's request window has only this host,
		// and without this its figures would wait for ever. Idempotent.
		void currency.boot();
		return () => {
			stopNodeWatch();
			speedControl.dispose();
		};
	});

	// A transaction is shown WITH what it costs (the founder's standing rule:
	// signing mirrors Send). Until this, no surface asked for the quote when a
	// request arrived, so the web sheet drew no fee for anything — and the
	// backup to Ethereum, which costs real dollars, said nothing about it.
	let quotedFor = '';
	/**
	 * The sheet's own simulation of a request, once it has ended: the core's
	 * reading of the node's reply and, for a check, the trust machine's
	 * judgment of what it moves (`simulateRequest`). The one `eth_simulateV1`
	 * read that tells the fee machine what the calls move is also the sheet's
	 * verdict — what moves, that nothing does, that the call fails, or that
	 * nothing could be checked. It belongs to the request it was run for —
	 * the sheet is handed it for that request only, and a new request starts
	 * with none (its place says "Checking…").
	 */
	let checked = $state<{ requestId: string; sim: SimEnded } | null>(null);
	/** A simulation that ended with no reply to read: "could not check". */
	const UNREAD: SimEnded = { verdict: null, judged: null };
	/**
	 * dapp-15: the request whose simulation is being asked AGAIN — its node
	 * answers again, or the person pressed "Check again". The last verdict
	 * stays on the sheet, marked stale, until the new one lands.
	 */
	let recheckingFor = $state<string | null>(null);
	/** How often a node that could not be reached is asked whether it is back. */
	const NODE_BACK_POLL_MS = 5_000;
	let nodeWatch: ReturnType<typeof setInterval> | null = null;
	/** "Check again", for the verdict on the sheet — or nothing to check again. */
	let simAgain = $state<(() => void) | null>(null);
	function stopNodeWatch(): void {
		if (nodeWatch !== null) clearInterval(nodeWatch);
		nodeWatch = null;
		simAgain = null;
	}

	/**
	 * Run request `requestId`'s simulation and put its verdict on the sheet.
	 * A verdict the core marks as about the network (`recheck` — no node
	 * answered) is asked again, by itself, once the chain answers a plain read
	 * again, and on the person's "Check again" (dapp-15).
	 *
	 * The core is told of every run (`sim_started` / `sim_settled`): until a
	 * verdict is on the sheet the confirm is shut, "Checking…". A re-check of
	 * a verdict already on the sheet keeps that verdict's door — the core's
	 * rule (§39, r22-6: `on_sim_started` ignores it): an unconfirmed verdict
	 * is asked again every few seconds, and a door that shut on each re-check
	 * flipped between "Sign anyway" and a shut "Confirm send", cancelling a
	 * hold or a "Press again to confirm" begun late in a cycle. The landed
	 * verdict — drawn stale meanwhile — keeps its door until the re-check
	 * lands; only a verdict that changes changes it.
	 */
	function simulateOnSheet(
		requestId: string,
		from: string,
		calls: FeeCall[],
		chainId: number,
		again: boolean
	): void {
		stopNodeWatch();
		if (again) recheckingFor = requestId;
		signRequest.dispatch({ type: 'sim_started', id: requestId });
		// However the read ends — a verdict, no reply to read, or a throw — the
		// verdict's place is no longer "Checking…": what ended with nothing to
		// report is "could not check" (`landedVerdict`), never a place left
		// waiting. Said and told to the core in ONE step, so no frame draws a
		// verdict over a confirm that still waits for it. For a request that
		// has gone nothing is said: its wait went with it.
		const settled = (sim: SimEnded) => {
			if (quotedFor !== requestId) return;
			checked = { requestId, sim };
			if (recheckingFor === requestId) recheckingFor = null;
			signRequest.dispatch({ type: 'sim_settled', id: requestId });
			if (sim.verdict?.recheck !== true) return;
			const askAgain = () => {
				if (quotedFor === requestId && recheckingFor !== requestId) {
					simulateOnSheet(requestId, from, calls, chainId, true);
				}
			};
			simAgain = askAgain;
			let asking = false;
			nodeWatch = setInterval(() => {
				if (quotedFor !== requestId) return stopNodeWatch();
				if (asking) return;
				asking = true;
				void poolRpcCall('eth_blockNumber', [], chainId)
					.then((answer) => typeof answer.result === 'string')
					.catch(() => false)
					.then((back) => {
						asking = false;
						if (back) askAgain();
					});
			}, NODE_BACK_POLL_MS);
		};
		void simulateRequest(
			speedControl,
			{ from, calls, chainId },
			() => quotedFor === requestId
		).then(
			(sim) => {
				// `null`: the request went while the node was answering.
				if (sim !== null) settled(sim);
			},
			() => settled(UNREAD)
		);
	}
	$effect(() => {
		const request = signView.request;
		if (!request || signView.surface === 'hidden' || !identity) {
			// The request went: its one-shot speed goes with it — and so does
			// its fee session (PR 2 note 1). The core asks a failed fee again by
			// itself, on the timer this session answers; a sheet that has gone
			// must not go on asking, so the session ends here and the next
			// request starts a new one.
			if (quotedFor !== '') {
				speedControl.reset();
				untrack(() => fee.dispose());
			}
			quotedFor = '';
			checked = null;
			recheckingFor = null;
			stopNodeWatch();
			return;
		}
		if (quotedFor === request.id) return;
		const calls = feeCallsOf(request.kind, request.params_json);
		if (calls === null) {
			// Nothing on chain to ask about. A message is simulated by nobody and
			// has no verdict at all. A transaction whose calls do not read is not
			// simulated either, and the core is told of no simulation — so its
			// verdict's place says from the first frame that nothing was checked,
			// never "Checking…" for a check nobody is making.
			const onChain = request.kind === 'transaction' || request.kind === 'batch';
			if (onChain && untrack(() => checked?.requestId) !== request.id) {
				checked = { requestId: request.id, sim: UNREAD };
			}
			return;
		}
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
		// Issue 411: what the calls move decides which coin can pay — a swap
		// whose path names the stablecoins it trades leaves the machine nothing
		// to count on until a simulation says what the swap leaves of them.
		// Told to the fee in force and every speed pricing these calls, for as
		// long as this request is the one on the sheet; a revert or a node that
		// could not check tells it nothing.
		// The same read is the sheet's verdict: what it means is the core's
		// (`simOutcome`, then `token_trust`'s judgment of a check's moves), kept
		// for this request alone.
		const requestId = request.id;
		// PR 3: the confirm waits for this read's verdict — the one part of
		// the sheet the site being signed for cannot write. The core is told
		// the simulation is out in the step that sends it, and that its
		// verdict is on the sheet in the step that puts it there — for a check
		// that is when its moves are judged, not when the node replied;
		// between the two it holds the confirm (`sim_checking`), for four
		// seconds at most on a timer of its own. Nothing here holds the
		// confirm or keeps a clock. A message never reaches this line: it has
		// no calls.
		untrack(() => simulateOnSheet(requestId, identity.address, calls, request.chain_id, false));
	});

	/**
	 * The fee in force, as the core says it (issue 483): an account the chain
	 * could not be read for is the fee's own failure in its view —
	 * `chain_read`, or `internal` when the read never left the app — so the
	 * row, the footer and the retry all name the same cause, with nothing
	 * laid over the view here.
	 *
	 * PR 2 note 1: the core asks a failed fee again by itself (`start_ttl`,
	 * 3 s, 6 s, then every 8 s — answered by the fee session's own timer), and
	 * its view says the failure once for the row and the footer
	 * (`FeeView.failure`), kept through the re-ask. Nothing here schedules a
	 * re-quote or holds a failure of its own.
	 */
	const feeShown = $derived(speedControl.feeInForce);

	/**
	 * §39 (r22-3 / A1): the fee the sheet DREW when the confirm began — the
	 * start of the hold, or the assistive path's first press — for the request
	 * it began on. While the hold runs or the confirm is armed the fee row is
	 * held still on it (a newer quote is not drawn under a press in progress),
	 * and it is what the approve signs (`quoted_fee`); the fee machine's quote
	 * at the approve goes beside it (`current_fee`). Above it, the core signs
	 * nothing (`fee_rose`): the row then draws the new figure, and the next
	 * confirm pays it. Let go once the confirm is idle again.
	 */
	let drawnFee = $state<{ requestId: string; view: FeeView } | null>(null);
	let confirmPhase: HoldPhase = 'idle';
	function onConfirmPhase(next: HoldPhase): void {
		confirmPhase = next;
		const id = signView.request?.id ?? null;
		if (next !== 'idle') {
			if (id !== null && drawnFee?.requestId !== id) {
				drawnFee = { requestId: id, view: $state.snapshot(feeShown) as FeeView };
			}
			return;
		}
		// A completed hold (or the second press) goes idle and confirms in the
		// same step: the approve reads the fee drawn first, then it is let go.
		queueMicrotask(() => {
			if (confirmPhase === 'idle') drawnFee = null;
		});
	}
	/** The fee the sheet draws: the one held under a press in progress, else the fee in force. */
	const feeDrawn = $derived(
		drawnFee !== null && drawnFee.requestId === signView.request?.id ? drawnFee.view : feeShown
	);
	/** Every failed run and every recovery is a `fee:` line (spec 082 G61). */
	const feeFailures = new FeeFailureLog();
	$effect(() => {
		const view = feeShown;
		const request = signView.request ?? null;
		// This request's own quote — never the last request's failure left in
		// the session (a message has no fee to ask about).
		const mine = request !== null && signView.surface !== 'hidden' && quotedFor === request.id;
		untrack(() => feeFailures.observe(mine ? view : null, request?.chain_id ?? null));
	});

	/** The fee-coin list is open. Like Send's: every coin the relay takes, the core's verdict on each. */
	let feeOpen = $state(false);
	$effect(() => {
		if (signView.request && signView.surface !== 'hidden') return;
		feeOpen = false;
	});

	/**
	 * cs-10: the request whose message runs past the sheet and has not been
	 * read to its end — the sheet's observation, handed to the core's gate.
	 * Kept by request id, so it never holds another request's confirm.
	 */
	let unreadFor = $state<string | null>(null);

	/**
	 * Spec 103: what this sheet holds beside the request — its simulation and
	 * its fee — for the agent's `vela_review`, and for the digest an opinion
	 * is bound to. Published for the request on the sheet, and for no other.
	 */
	$effect(() => {
		const request = signView.request;
		const id = request && signView.surface !== 'hidden' ? request.id : null;
		const sim = id !== null && checked?.requestId === id ? checked.sim : null;
		const onChain = request?.kind === 'transaction' || request?.kind === 'batch';
		// A figure for another chain is a previous request's, not this one's.
		const fee =
			id !== null && (!feeShown.fee || feeShown.fee.chain_id === request?.chain_id)
				? feeShown
				: null;
		untrack(() =>
			sheetBeside.publish({
				requestId: id,
				sim,
				simPending: id !== null && onChain && sim === null,
				fee
			})
		);
	});

	/**
	 * Round 7 (S4): the request whose signature went to the signing page. The
	 * hand-off ends the moment the page answers, a beat before the core leaves
	 * `awaiting_signature`: kept for that request, so the sheet never says
	 * "Waiting for biometric…" while the page (not this window) signs.
	 */
	let signedOnPage = $state<string | null>(null);
	$effect(() => {
		if (pageHandoff.current?.kind !== 'signature') return;
		const id = untrack(() => signView.request?.id ?? null);
		if (id !== null) signedOnPage = id;
	});
	const onPage = $derived(
		pageHandoff.current?.kind === 'signature' ||
			(signedOnPage !== null && signedOnPage === (signView.request?.id ?? null))
	);

	const built = $derived.by(() => {
		if (!identity) return null;
		return buildSigningModel({
			sign: signView,
			clear: signingSheet.clear,
			guard: signingSheet.guard,
			// §39 (A1): held still while a hold runs or the confirm is armed.
			fee: feeDrawn,
			// Round 7 (S4): the page has it — "Waiting for the signing page…".
			progress: { ...signRequest.progress, onPage },
			// This request's own simulation, and no other's.
			sim: checked !== null && checked.requestId === signView.request?.id ? checked.sim : null,
			simRechecking: recheckingFor !== null && recheckingFor === signView.request?.id,
			messageUnread: unreadFor !== null && unreadFor === signView.request?.id,
			// R22: "Refuse blind signing" — the core's gate refuses an "… anyway".
			refuseBlind: refuseBlind.current(),
			// R22: where "Only use my node" is, for an unconfirmed verdict's hint.
			ownNodeHref: (chainId: number) =>
				ownNodeSettingsHref(
					typeof document === 'undefined' ? 'en' : document.documentElement.lang || 'en',
					chainId
				),
			// R22: a network read through the person's own node alone.
			ownNodeOnly: (chainId: number) => ownNodes.ownOnly.includes(chainId),
			feeOpen,
			speed: {
				view: speedControl.view,
				feeOptions: (tier: FeeTier) => speedControl.feeOptions(tier)
			},
			currency: currency.view,
			// r18-9: the testnet space shows no fiat on the sheet.
			space: networkSpace.current,
			m: messages,
			identity,
			identicon: identiconSvgForClient
		});
	});

	/**
	 * The sheet, with the AI agent's opinion on this request beside it
	 * (spec 103, `vela_advise`) — its own card, set here and never by the
	 * blocks. Drawn as current only while the review it was given for is the
	 * one on the sheet (its digest) and the fee is still the one it was given
	 * for, within the core's band; otherwise greyed, as given for an earlier
	 * version or another fee. Nothing else of the model reads it.
	 */
	const model = $derived.by(() => {
		const sheet = built;
		const held = sheetBeside.advice;
		if (!sheet || !held || held.requestId !== signView.request?.id) return sheet;
		const standing = adviceStanding(held.advice, openReview());
		return { ...sheet, advice: adviceCard(held.advice, standing, messages.agentAdvice) };
	});

	/**
	 * What the approve carries (`approveOptsOf`): the fee this sheet displayed,
	 * the GUARD's rewritten params — the capped approval, not the requested
	 * one — and, since spec 093, the record's intent and the guard's token,
	 * each copied from its machine's view.
	 */
	function approveOpts() {
		// §39 (A1): the figure drawn when the confirm began, and the quote now.
		return approveOptsOf(feeDrawn, signingSheet.clear, signingSheet.guard, fee.view);
	}

	/** The chip ids the drawn editor emits, in the guard's vocabulary. */
	function guardChip(id: string, leg?: number): void {
		// cs-17: a grant-all's deliberate tap — the core's consent to allow all.
		if (id === 'grant') {
			signingSheet.dispatchGuard(
				leg === undefined
					? { type: 'grant_deliberately_chosen' }
					: { type: 'leg_grant_deliberately_chosen', index: leg }
			);
			return;
		}
		if (id === 'requested' || id === 'balance' || id === 'custom' || id === 'revoke') {
			// A batch leg's card talks to its OWN leg: the core ignores the
			// single approval's `preset_selected` on a batch.
			signingSheet.dispatchGuard(
				leg === undefined
					? { type: 'preset_selected', mode: id }
					: { type: 'leg_preset_selected', index: leg, mode: id }
			);
		}
	}

	/**
	 * Every keystroke in the cap field, back to the machine that validates it.
	 *
	 * The core owns the parse (`custom_amount_changed` is already dot-normalized
	 * by the shell, as payment_request's amount is), so nothing here decides
	 * whether what was typed is a number — it only carries it.
	 */
	function guardCustom(text: string, leg?: number): void {
		signingSheet.dispatchGuard(
			leg === undefined
				? { type: 'custom_amount_changed', text }
				: { type: 'leg_custom_amount_changed', index: leg, text }
		);
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
			model={dappReceiptModel(landingShown(landing), receipt, (txHash) => {
				const base = explorerBaseURL(landingChain);
				return base ? `${base}/tx/${txHash}` : null;
			})}
			progress={landing.kind === 'submitted'
				? (landingPace(relaySentAtMs, landingTypicalS, nowMs).progress ?? undefined)
				: undefined}
			ondone={closeLanding}
		/>
	</div>
{:else if signedTick && receipt && !model && !nextOwed}
	<!--
		Spec 079: the message is signed — the tick, the same landing layer and
		disc, gone by itself (`autoCloseAfterMs`). A new request's sheet wins
		over it (`!model`); the page already has its signature.
	-->
	<div class="landing-over">
		<DappReceipt
			model={dappReceiptModel({ kind: 'signed' }, receipt, () => null)}
			ondone={() => (signedTick = false)}
		/>
	</div>
{:else if model && !sheetHidden}
	<!--
		The ✕ IS the rejection (the 022 interaction contract draws no reject
		button), so closing answers the transport with 4001 through the core —
		and since spec 079 the ✕ is the ONLY way it closes: no scrim, no drag,
		no Escape (`SigningSheet` → `BottomSheet dismissible="explicit"`).
		After the approval the sheet is a status, and the ✕ a plain close
		(`dismiss_tapped`) once the signature exists: the operation carries on
		and the page still gets its answer (`signingCloseEvent`).
	-->
	<SigningSheetView
		{model}
		dismissible={model.status ? model.status.closable : true}
		onclosestart={() => (closeIntent = closeIntentNow())}
		onclose={closeSheet}
		onretry={() => signRequest.dispatch({ type: 'retry_tapped' })}
		onconfirm={() => {
			// R22 (D4): a hand-off card this confirm raises carries its posture.
			const confirm = model.confirm;
			pageHandoff.posture =
				(confirm.holdMs ?? 0) > 0 || confirm.danger
					? {
							danger: confirm.danger === true,
							holdMs: confirm.holdMs ?? 0,
							holdLabel: confirm.holdLabel ?? '',
							againLabel: confirm.againLabel ?? ''
						}
					: null;
			signRequest.dispatch({ type: 'approve_tapped', opts: approveOpts() });
		}}
		onmessageunread={(unread) => (unreadFor = unread ? (signView.request?.id ?? null) : null)}
		onrecheck={() => simAgain?.()}
		onconfirmphase={onConfirmPhase}
		onchip={guardChip}
		oncustom={guardCustom}
		onfee={() => {
			// Exactly what the row says (`feeRowTap`, PR 2 polish). Failed, by the
			// core's `tap`: ask again, at once (the core drops its own pending
			// re-ask with the attempt this moves on) — and while a re-ask is out
			// it is the answer awaited, a second tap asks nothing; or, after the
			// relay answered that it would fail, open the coins; or nothing.
			// Otherwise more than one coin opens the list, here in the sheet, and
			// one coin is the host's own surface, if any.
			switch (feeRowTap(feeShown, fee.view?.options.length ?? 0)) {
				case 'requote':
					fee.requote();
					break;
				case 'toggle_coins':
					feeOpen = !feeOpen;
					break;
				case 'host':
					onfee();
					break;
				case 'none':
					break;
			}
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
				if ((last.tier ?? speedControl.tier) === speedControl.tier) {
					// Correctness batch item 4: the core switches the figure to that
					// coin at once — provisional, drawn with the measuring sign —
					// and measures it again with that coin's fee leg; the confirm
					// holds until it lands (`select_fee_asset`). Nothing blanks to
					// "estimating". `selectAsset` re-points the request on record
					// too, so every speed's preview prices in the coin picked.
					fee.selectAsset(token);
				} else {
					void fee.requestQuote({
						...last,
						feeToken: token,
						autoFeeToken: false,
						tier: speedControl.tier
					});
				}
			}
			feeOpen = false;
		}}
		onfeerefresh={() => speedControl.refresh()}
		onspeed={() => speedControl.toggle()}
		onspeedpick={(id) => {
			if (id === 'fast' || id === 'standard' || id === 'slow') speedControl.pick(id);
		}}
	/>
{/if}

<!--
	The page, everywhere (owner, 2026-10-30): an account that reviews and signs
	on its trusted page is handed there from this sheet — the hand-off card,
	over it (a bottom sheet, so it stacks above the signing sheet on every
	width).
-->
{#if messages.handoff !== undefined}
	<PageHandoffSheet
		m={messages.handoff}
		locale={typeof document === 'undefined' ? 'en' : document.documentElement.lang || 'en'}
		wide={false}
		closeLabel={messages.close}
		kind="signature"
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
