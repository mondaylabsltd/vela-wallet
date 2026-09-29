/**
 * The one `sign_request` core the web app has — APP-RESIDENT.
 *
 * Ported from src/services/wallet-state-core/sign-resident.ts @ f9bcb278,
 * carrying its RULES and dropping its projections: Expo's resident also
 * rebuilt `BLEIncomingRequest` / `FundingNeeded` shapes for its React
 * consumers, while on the web `signing/live.ts` renders `SignView` directly.
 * One shape beats two that can drift.
 *
 * What is carried, and why each one is load-bearing:
 *
 * - **The transport registry.** The core speaks a `transport_id` and nothing
 *   else about transports: a response goes to the transport that OWNS the
 *   request, never a shared reference. In 026 the only transport is the
 *   in-page requester; 027 registers a real one against the same table.
 * - **`op_submitted` mid-flight.** The relay accepting an operation is a fact
 *   the core must hear BEFORE `sign_and_submit` resolves, so the durable
 *   record can precede anything the dApp could poll.
 * - **The verified account switch.** A granted address is answered with a
 *   POSITION in the session's own rows, and an out-of-range index is a silent
 *   no-op there — silent is the danger, because the surface would then open on
 *   an account the origin was never granted. So the switch is checked before
 *   AND after, and a failure is fail-closed: never acked, so the confirm gate
 *   stays shut and nothing can be signed.
 * - **The networks snapshot first.** Until it arrives every chain is
 *   unsupported (fail-closed), so a shell that forgot it would refuse
 *   everything with 4902.
 * - **The tracker hand-off, deduped by record and by what it says.** A dApp
 *   transaction is handed to `tx_tracker` the moment the view publishes it —
 *   first by the write-ahead, before its POST (spec 082 RJ1), then again when
 *   the relay takes it (`admitted`). Deduped by the records it closes and by
 *   `maybe_sent` / `admitted`, not by its hash alone: a second submit of an op
 *   that was never sent can carry the very same hash, and the admitted
 *   hand-off names the same hash and records as the write-ahead one — keyed
 *   on those alone it was never fed, and an accepted op could end "not sent".
 *   A withdrawal (`tracker_withdraw`) is fed once per value.
 * - **The answer follows the tracker** (spec 082 RJ4): `track-forward` hands
 *   the machine every change of its in-flight op's tracker entry
 *   (`OpTracked`), so a refusal, a "never sent" or a landing is answered at
 *   once rather than when the receipt wait runs out.
 */
import { loadCore } from '$lib/core/client';
import type { SignEvent } from '$lib/core/generated/SignEvent';
import type { SignView } from '$lib/core/generated/SignView';
import { getAllNetworksSync } from '$lib/services/networks';
import type { AssetSimResult } from '$lib/services/sim/tx-simulation';
import {
	dispatchTxTracker,
	subscribeTxTracker,
	txTrackerView
} from '$lib/wallet/core/tracker-resident';
import { feed } from '$lib/wallet/core/feed.svelte';
import { session } from '$lib/session/core/session.svelte';
import { onSignCeremony } from '$lib/onboarding/core/passkey';
import { approvalProgress, IDLE_APPROVAL, type ApprovalProgress } from '../approval-progress';
import { createSignRequestSession, type SignRequestSession } from './sign-session';
import { claimThrough, type ClaimPhase, type SignResponder, type SubmitClaim } from './sign-types';
import { handoffKey, withdrawKey, createTrackForward, type TrackForward } from './track-forward';
import { countPanelFailure } from '$lib/services/bug-report';

/** The machine's own initial projection — mirrored until the first view lands. */
export const INITIAL_SIGN_VIEW: SignView = {
	surface: 'hidden',
	request: null,
	is_signing: false,
	is_submitting: false,
	phase: 'idle',
	pending_op_hash: null,
	pending_op_maybe_sent: false,
	error: null,
	funding: null,
	confirm_gate_open: false,
	reconcile_pending: false,
	swipe_action: 'none',
	tracker_handoff: null,
	tracker_withdraw: null,
	failure_refused: false,
	notice: null,
	blocked: null,
	global_chain_id: 1
};

const MAX_TRACKED_TRANSPORTS = 32;

/** The last answer a request got — its id, and whether it carried a result. */
export interface SignAnswer {
	id: string;
	/** A result went out (a signature, a hash) — not a refusal, not an empty ack. */
	ok: boolean;
}

class SignRequest {
	view = $state<SignView>(INITIAL_SIGN_VIEW);
	/**
	 * Spec 079: the last answer, as the sheet needs it. The core clears the
	 * sheet the moment it answers, so a message signature used to just vanish;
	 * the sheet matches this against the request it showed to draw the signed
	 * tick first. Written AFTER the answer went out — the answer goes first
	 * and unconditionally, and nothing here can delay or swallow it.
	 */
	answered = $state<SignAnswer | null>(null);
	/**
	 * Spec 079: where the approved request stands — has its signature been
	 * made, is the passkey prompt up. The sheet's status and its ✕ read it
	 * (`approval-progress.ts` says why the core's view alone cannot).
	 */
	progress = $state<ApprovalProgress>(IDLE_APPROVAL);
	#stopCeremony: (() => void) | null = null;

	#loop: SignRequestSession | null = null;
	#booting: Promise<void> | null = null;
	#transports = new Map<string, SignResponder>();
	#nextTransportId = 0;
	#assetSim: AssetSimResult | null = null;
	#networksKey: string | null = null;
	#accountsKey: string | null = null;
	#stopAccountsMirror: (() => void) | null = null;
	#lastHandoffKey = '';
	#lastWithdrawKey = '';
	#forward: TrackForward | null = null;
	/** The last failure the sheet showed, so a view repeated is not counted again (G61). */
	#lastFailure = '';
	/** request id → the transport that delivered it (for the claim port). */
	#transportOfRequest = new Map<string, string>();
	/** request id → when the person approved it (the dApp's answer window). */
	#approvedAt = new Map<string, number>();

	/** Register a transport and get the id the core will name it by. */
	registerTransport(transport: SignResponder): string {
		const id = `t${(this.#nextTransportId += 1)}`;
		// Bounded: a long-lived tab can see many transports, and the oldest
		// entry is the one whose requests are long settled.
		if (this.#transports.size >= MAX_TRACKED_TRANSPORTS) {
			const oldest = this.#transports.keys().next().value;
			if (oldest !== undefined) this.#transports.delete(oldest);
		}
		this.#transports.set(id, transport);
		return id;
	}

	unregisterTransport(id: string): void {
		this.#transports.delete(id);
	}

	/** The sign-time simulation the last approve carried (presentation only). */
	setApproveSim(sim: AssetSimResult | null | undefined): void {
		this.#assetSim = sim ?? null;
	}

	/** Load the core and start the machine with a networks snapshot. */
	boot(): Promise<void> {
		if (this.#booting) return this.#booting;
		this.#booting = (async () => {
			await loadCore();
			this.#stopCeremony?.();
			this.#stopCeremony = onSignCeremony((event) => {
				this.progress = approvalProgress(this.progress, { type: 'ceremony', event });
			});
			this.#loop = createSignRequestSession({
				onView: (view) => {
					this.view = view;
					this.progress = approvalProgress(this.progress, {
						type: 'view',
						requestId: view.request?.id ?? null,
						inFlight: view.is_signing || view.is_submitting
					});
					this.#drainHandoff(view);
					this.#drainWithdraw(view);
					this.#countFailure(view);
				},
				onError: (error) => console.error('[sign_request] core fault:', error),
				ports: {
					transportFor: (id) => {
						const transport = this.#transports.get(id);
						if (!transport) return null;
						return {
							sendResponse: (rid, result, error, opHash) => {
								try {
									transport.sendResponse(rid, result, error, opHash);
								} finally {
									this.answered = {
										id: rid,
										ok: !error && result !== undefined && result !== null && result !== ''
									};
									this.#transportOfRequest.delete(rid);
									this.#approvedAt.delete(rid);
								}
							}
						};
					},
					opSubmitted: (id, userOpHash, maybeSent, submitBlock) => {
						// The panel's own count of a lost reply (spec 082 G61): it
						// reaches the bug report beside the worker's counters.
						if (maybeSent) countPanelFailure('submit.maybe_sent');
						this.dispatch({
							type: 'op_submitted',
							id,
							user_op_hash: userOpHash,
							now_ms: Date.now(),
							maybe_sent: maybeSent,
							submit_block: submitBlock
						});
						// RJ4: what the tracker already knows of this op, now that the
						// core takes it — a verdict reached while the POST was out was
						// dropped, and a terminal entry never changes again.
						this.#forward?.taken(userOpHash);
					},
					opSigned: (id, userOpHash, submitBlock) =>
						this.dispatch({
							type: 'op_signed',
							id,
							user_op_hash: userOpHash,
							submit_block: submitBlock,
							now_ms: Date.now()
						}),
					ceremony: (id, stage) =>
						this.dispatch({
							type: stage === 'started' ? 'ceremony_started' : 'ceremony_done',
							id
						}),
					askerLive: (id, phase, submit) => this.askerLive(id, phase, submit),
					approvedAtMs: (id) => this.#approvedAt.get(id) ?? null,
					recordsWritten: () => feed.reconciled(1),
					assetSim: () => this.#assetSim,
					// The wallet's own request (the key backup) is from this very
					// origin, and names no site: the Trusted Signer draws an empty
					// origin as the wallet itself, as the sheet does (spec 071).
					requestOrigin: (id) => {
						const request = this.view.request;
						if (request?.id !== id || request.origin === window.location.origin) return '';
						return request.origin;
					},
					switchActiveAccount: async (index: number) => {
						const intended = this.view.request?.signer_address ?? null;
						const rows = session.view.accounts;
						const target = rows[index]?.account.address ?? null;
						// Checked BEFORE the dispatch: a bad index must not move
						// the person's active account either.
						if (target === null || (intended !== null && !sameAddress(target, intended))) {
							return refuseSwitch(index, intended, target);
						}
						session.switchAccount(index);
						if (session.view.active_index !== index) {
							return refuseSwitch(index, intended, target);
						}
						// The machine consumes the ack by reading ITS OWN rows at
						// `active_index` (`approve_with`), so the switch must be in
						// them before the ack lands — not a microtask later.
						this.syncAccounts();
					}
				}
			});
			this.#loop.start(this.#networksEvent());
			// RJ4: the in-flight op's tracker entry, forwarded as `OpTracked`.
			this.#forward?.stop();
			this.#forward = createTrackForward({
				subscribe: subscribeTxTracker,
				current: txTrackerView,
				dispatch: (event) => this.#loop?.dispatch(event)
			});
			this.#forward.watch(this.view.tracker_handoff?.user_op_hash ?? null);
			// The session's rows are the machine's signers (§12.1.6): mirrored on
			// boot and on every change — a sign-in, a switch, a sign-out. Expo's
			// resident had `setSignAccounts` called from the wallet provider on
			// each state change; on the web the session is app-resident state, so
			// the mirror is an effect on it. Without this the machine has NO
			// accounts, and `approve_with` finds no signer and returns silently:
			// the slide commits, the gate is open, and nothing is ever signed
			// (027's SC-304 finding).
			this.syncAccounts();
			this.#stopAccountsMirror?.();
			this.#stopAccountsMirror = $effect.root(() => {
				$effect(() => {
					// Read the tracked fields; the key dedupes unchanged views.
					void session.view.accounts;
					void session.view.active_index;
					void session.view.loading;
					this.syncAccounts();
				});
			});
		})();
		return this.#booting;
	}

	dispatch(event: SignEvent): void {
		if (event.type === 'request_arrived') {
			this.#transportOfRequest.set(event.id, event.transport_id);
		} else if (event.type === 'approve_tapped') {
			// The start of the dApp's answer window (spec 082 RA12): the receipt
			// wait after a slow submit is what is LEFT of it, never a fresh 120 s.
			const id = this.view.request?.id;
			if (id) this.#approvedAt.set(id, Date.now());
		}
		void this.boot().then(() => this.#loop?.dispatch(event));
	}

	/**
	 * Is the asker of request `id` still there (spec 082 RB5)? The owning
	 * transport's claim, when it has one; a transport that cannot lose its
	 * asker (the wallet's own page) is always live.
	 */
	askerLive(id: string, phase: ClaimPhase, submit?: SubmitClaim): Promise<boolean> {
		const transportId = this.#transportOfRequest.get(id);
		return claimThrough(
			transportId ? this.#transports.get(transportId) : undefined,
			id,
			phase,
			submit
		);
	}

	/**
	 * Hand the machine the session's own rows — address and founding credential
	 * — and the active position, in the session's domain (`SwitchAccount.index`
	 * is consumed there). Unchanged rows are dropped: one string compare.
	 * Nothing is sent while the session is still restoring; a stale list would
	 * let a request reconcile against accounts that are about to be replaced.
	 */
	syncAccounts(): void {
		const view = session.view;
		if (view.loading) return;
		const rows = view.accounts;
		const key =
			`${view.active_index}|` +
			rows.map((row) => `${row.account.address.toLowerCase()}:${row.account.id}`).join(',');
		if (key === this.#accountsKey) return;
		this.#accountsKey = key;
		const event: SignEvent = {
			type: 'accounts_changed',
			accounts: rows.map((row) => ({
				address: row.account.address,
				credential_id: row.account.id
			})),
			active_index: view.active_index
		};
		// Synchronous when the machine is up: a caller inside a port needs the
		// rows in place before its ack resolves.
		if (this.#loop) this.#loop.dispatch(event);
		else this.dispatch(event);
	}

	/**
	 * Re-assert the supported-network set. A custom network can be added at any
	 * time, so this runs immediately before every arrival; an unchanged set is
	 * dropped, so it costs one string compare.
	 */
	syncNetworks(): void {
		const chainIds = getAllNetworksSync().map((network) => network.chainId);
		const key = chainIds.join(',');
		if (key === this.#networksKey) return;
		this.#networksKey = key;
		this.dispatch({ type: 'networks_changed', chain_ids: chainIds });
	}

	#networksEvent(): SignEvent {
		const chainIds = getAllNetworksSync().map((network) => network.chainId);
		this.#networksKey = chainIds.join(',');
		return { type: 'networks_changed', chain_ids: chainIds };
	}

	#drainHandoff(view: SignView): void {
		const handoff = view.tracker_handoff;
		if (!handoff) return;
		// By the records it closes AND what it says (see the header): the
		// admitted hand-off names the write-ahead's hash and records again.
		const key = handoffKey(handoff);
		if (key === this.#lastHandoffKey) return;
		this.#lastHandoffKey = key;
		// The tracker entry this op gets is the one forwarded back (RJ4).
		this.#forward?.watch(handoff.user_op_hash);
		dispatchTxTracker({
			type: 'submitted',
			user_op_hash: handoff.user_op_hash,
			record_ids: handoff.record_ids,
			chain_id: handoff.chain_id,
			maybe_sent: handoff.maybe_sent,
			submit_block: handoff.submit_block,
			admitted: handoff.admitted
		});
	}

	/**
	 * A write-ahead record proven never sent (spec 082 RJ1): the tracker
	 * forgets it — once per value, like the hand-off.
	 */
	#drainWithdraw(view: SignView): void {
		const withdraw = view.tracker_withdraw;
		if (!withdraw) return;
		const key = withdrawKey(withdraw);
		if (key === this.#lastWithdrawKey) return;
		this.#lastWithdrawKey = key;
		dispatchTxTracker({
			type: 'withdrawn',
			user_op_hash: withdraw.user_op_hash,
			record_ids: withdraw.record_ids
		});
	}

	/**
	 * The panel's own failure counts (spec 082 G61): a submit that ended "not
	 * sent" or refused. Counters and classes only — never a hash or an address.
	 */
	#countFailure(view: SignView): void {
		const error = view.error;
		// Every view is a fresh object: the failure is told apart by value.
		const key = error
			? `${view.request?.id ?? ''}|${error.kind}|${error.detail ?? ''}|${view.failure_refused}`
			: '';
		if (key === this.#lastFailure) return;
		this.#lastFailure = key;
		if (!error || error.kind !== 'submit_failed') return;
		countPanelFailure(view.failure_refused ? 'submit.refused' : 'submit.not_sent');
	}
}

/** Hex addresses, compared the way every other site in this app compares them. */
function sameAddress(a: string | null | undefined, b: string | null | undefined): boolean {
	return !!a && !!b && a.toLowerCase() === b.toLowerCase();
}

/**
 * A switch that did not land. Never acked — the operation stays unanswered, so
 * `confirm_gate_open` stays false and nothing can be signed. The person can
 * still reject or dismiss the sheet.
 */
function refuseSwitch(
	index: number,
	intended: string | null,
	target: string | null
): Promise<never> {
	console.error(
		`[sign_request] refusing to switch to index ${index} (intended ${intended ?? 'nothing'}, found ` +
			`${target ?? 'nothing'}). The approval surface stays shut; nothing is signed.`
	);
	// Deliberately never resolves.
	return new Promise<never>(() => {});
}

export const signRequest = new SignRequest();
