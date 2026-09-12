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
 * - **The tracker hand-off, deduped by hash.** A dApp transaction that reached
 *   the relay is handed to `tx_tracker` the moment the view publishes it.
 */
import { loadCore } from '$lib/core/client';
import type { SignEvent } from '$lib/core/generated/SignEvent';
import type { SignView } from '$lib/core/generated/SignView';
import { getAllNetworksSync } from '$lib/services/networks';
import type { AssetSimResult } from '$lib/services/sim/tx-simulation';
import { dispatchTxTracker } from '$lib/wallet/core/tracker-resident';
import { session } from '$lib/session/core/session.svelte';
import { createSignRequestSession, type SignRequestSession } from './sign-session';
import type { SignResponder } from './sign-types';

/** The machine's own initial projection — mirrored until the first view lands. */
export const INITIAL_SIGN_VIEW: SignView = {
	surface: 'hidden',
	request: null,
	is_signing: false,
	is_submitting: false,
	pending_op_hash: null,
	error: null,
	funding: null,
	confirm_gate_open: false,
	reconcile_pending: false,
	swipe_action: 'none',
	tracker_handoff: null,
	notice: null,
	global_chain_id: 1
};

const MAX_TRACKED_TRANSPORTS = 32;

class SignRequest {
	view = $state<SignView>(INITIAL_SIGN_VIEW);

	#loop: SignRequestSession | null = null;
	#booting: Promise<void> | null = null;
	#transports = new Map<string, SignResponder>();
	#nextTransportId = 0;
	#assetSim: AssetSimResult | null = null;
	#networksKey: string | null = null;
	#accountsKey: string | null = null;
	#stopAccountsMirror: (() => void) | null = null;
	#lastHandoffHash = '';

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
			this.#loop = createSignRequestSession({
				onView: (view) => {
					this.view = view;
					this.#drainHandoff(view);
				},
				onError: (error) => console.error('[sign_request] core fault:', error),
				ports: {
					transportFor: (id) => this.#transports.get(id) ?? null,
					opSubmitted: (id, userOpHash) =>
						this.dispatch({
							type: 'op_submitted',
							id,
							user_op_hash: userOpHash,
							now_ms: Date.now()
						}),
					assetSim: () => this.#assetSim,
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
		void this.boot().then(() => this.#loop?.dispatch(event));
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
		if (handoff.user_op_hash === this.#lastHandoffHash) return;
		this.#lastHandoffHash = handoff.user_op_hash;
		dispatchTxTracker({
			type: 'submitted',
			user_op_hash: handoff.user_op_hash,
			record_ids: handoff.record_ids,
			chain_id: handoff.chain_id
		});
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
