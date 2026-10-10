// Ported from src/services/wallet-state-core/send-executor.ts @ f9bcb278 — RN
// seams rewritten to the web modules (passkey `signChallenge`, synchronous
// account finders over localStorage, the kernels import made static); every
// rule, every regex and every ordering verbatim.
/**
 * The only place the `send` core touches the outside world.
 *
 * Nineteen operations, each one existing service call — the vocabulary the core
 * declares. No branching on business meaning: the step machine, the 15 s
 * pre-check race, the re-entry lock, the cancel checkpoints, the
 * displayed-is-signed gate and the persist-then-track ordering all live in Rust.
 *
 * What DOES live here, because the core's doc comments put it here:
 *
 * - **Every wording regex.** `PasskeyErrorCode.CANCELLED`,
 *   `parseBundlerUnderfunded` and `/gas relayer is unavailable/i` are classified
 *   HERE into typed `SendSubmitFailure` variants; the core only ever sees the
 *   variant and only ever emits a semantic error key (invariant ⑮). The raw
 *   message is logged exactly where `useSendController.ts:1101` logged it.
 * - **The amount codec.** The core states base units as decimal strings;
 *   `safe-transaction.ts` reads `value` as HEX everywhere. `toShellCall` is that
 *   boundary, and it is the difference between signing the amount that was
 *   displayed and signing a different one.
 * - **The passkey ceremony.** `SubmitUserOp` is one sentence to the core; here it
 *   is the `signFn` closure `sendNative`/`sendERC20`/`sendBatchCalls` invoke,
 *   including the identity-provider compatibility check. When Settings' "Sign
 *   with" is the Trusted Signer (spec 071) the same closure takes the send to its
 *   page instead: no site asked, so the request names none and the send's own
 *   calls are its intent.
 * - **`prefetchForSend` cache warming** is the shell's (the core says so); it
 *   runs from the controller on token selection, not from an operation.
 * - **The `tx_tracker` seam.** `TrackSubmitted` is the hand-off point, and on web
 *   it is now taken: `useSendController.web.ts` installs a sink before it builds
 *   the session, so the receipt wait below is the NO-SINK path only (a session
 *   built without one — the send core's own test harness). See
 *   `setSendTrackerSink`.
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with.
 */

import {
	fromHex,
	userOpNotSentDetail,
	userOpWriteAheadWaitMs,
	verifySafeWebAuthn
} from '$lib/core/kernels';
import { getAllNetworksSync, networkId, nativeSymbol } from '$lib/services/networks';
import { PasskeyError } from '$lib/onboarding/core/passkey';
import { cancelChallenge, signChallenge, VenueBlockedError } from '$lib/signing/sign-challenge';
import { addCustomNetworkByChainId } from '$lib/services/add-network.svelte';
import { parseBundlerUnderfunded, probeTreasury } from '$lib/services/bundler-service';
import { hapticError, hapticSuccess } from '$lib/services/platform';
import { resolveRecipientIdentity } from '$lib/services/recipient-identity';
import { resolveRecipientRisk } from '$lib/services/recipient-risk';
import {
	keySetOf,
	sendBatchCalls,
	type SignFn,
	UserOpNotSentError,
	UserOpRevertedError,
	type SubmitResult
} from '$lib/services/safe-transaction';
import { findAccountByAddress, findAccountByCredentialId } from '$lib/services/accounts';
import { deleteTransactions, saveTransactions, updateTransactions } from '$lib/services/records';
import {
	outcomeOf,
	trackSubmitted,
	txTrackerView,
	withdrawTracked
} from '$lib/wallet/core/tracker-resident';
import { WriteAheadTimeoutError } from '$lib/signing/core/sign-types';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { resolveTokenMetadata } from '$lib/services/token-metadata';
import { serializeAssetSim, simulateAssetChanges } from '$lib/services/sim/tx-simulation';
import { clearTokenCache, fetchTokens } from '$lib/services/wallet-api';

import type { SendChainInfo } from '$lib/core/generated/SendChainInfo';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendShellResult } from '$lib/core/generated/SendShellResult';
import type { SendTxRecord } from '$lib/core/generated/SendTxRecord';
import { prewarmFees } from './fee-prewarm';
import { wireTier } from './wire-tier';
import {
	fromWireAmount,
	toSendToken,
	toSendFeeOutcome,
	toShellCall,
	type SendEffect,
	type SendShellPorts
} from './send-types';

/**
 * The `tx_tracker` seam.
 *
 * `TrackSubmitted` arrives the moment the pending records are durable
 * (invariant ⑥'s ordering half), carrying the op hash, the record ids it belongs
 * to and the chain. Deciding what happens to that op next — the receipt poll,
 * the record patch on a dropped op, the reconcile — is `tx_tracker`'s job, and
 * `useSendController.web.ts` installs the sink that gives it to that machine:
 *
 * ```ts
 * setSendTrackerSink((handoff) => trackSubmitted(
 *   handoff.userOpHash, handoff.recordIds, handoff.chainId,
 *   (outcome) => session.dispatch({ type: 'receipt_update', ... }),
 * ));
 * ```
 *
 * With a sink installed the fallback below does not run at all: the tracker owns
 * the poll (at the shared 3 s throttle every other watcher of that hash uses),
 * the record patch and the 24 h abandon line, and it hands back only the three
 * verdicts `ReceiptUpdate` accepts. `handoff.submitted` is offered for a
 * consumer that would rather await the bundler's own promise; the tracker
 * deliberately does not, because that would be a second unthrottled poller.
 *
 * Without one, the fallback runs — and unlike `sign_request`'s seam it is NOT a
 * stand-in data source: it is the very `result.waitForTxHash()` chain that
 * `useSendController.ts:1045-1070` ran, moved verbatim to the same point in the
 * sequence (the records are already written, so the `await pendingWrites` it
 * used to open with is what the core's ordering now guarantees).
 */
export interface SendTrackerHandoff {
	userOpHash: string;
	recordIds: string[];
	chainId: number;
	/** The submit's reply was lost; the hash is the local one (spec 082 RA4). */
	maybeSent: boolean;
	/** The head read before the first POST — where the find-event starts. */
	submitBlock: number | null;
	/** The bundler's own receipt promise, or `null` if this process never had it. */
	submitted: SubmitResult | null;
	/**
	 * The account that signed it (`TrackSubmitted.sender`): the tracker's way
	 * of knowing this account has an operation in flight on this chain, so its
	 * next confirm waits (correctness batch item 3).
	 */
	sender: string | null;
}

/**
 * The session the executor belongs to, for the one fact it must dispatch
 * mid-submit (`OpSigned`, spec 082 RJ1). Absent: a harness with no session —
 * then there is no write-ahead to wait for, and the submit posts as before.
 */
export interface SendExecutorSelf {
	dispatch(event: SendEvent): void;
}

let trackerSink: ((handoff: SendTrackerHandoff) => void) | null = null;

export function setSendTrackerSink(sink: ((handoff: SendTrackerHandoff) => void) | null): void {
	trackerSink = sink;
}

// The 'fast' tier moved with the quote: `use-fee-quote.web.ts` states it once,
// for both surfaces, where the `QuoteRequested` that carries it is built.

/** The chain registry snapshot the core validates locked requests against. */
function chainInfos(): SendChainInfo[] {
	return getAllNetworksSync().map((network) => ({
		chain_id: network.chainId,
		network: networkId(network.chainId),
		native_symbol: nativeSymbol(network.chainId)
	}));
}

/** One core record onto the `LocalTransaction` row it has always been. */
function toLocalTransaction(record: SendTxRecord): LocalTransaction {
	return {
		id: record.id,
		userOpHash: record.user_op_hash,
		txHash: record.tx_hash,
		from: record.from,
		to: record.to,
		...(record.to_name != null ? { toName: record.to_name } : {}),
		value: record.value,
		symbol: record.symbol,
		decimals: record.decimals,
		logoUrls: record.logo_urls,
		chainId: record.chain_id,
		timestamp: record.timestamp_s,
		status: 'pending',
		type: 'send',
		...(record.usd != null ? { usd: record.usd } : {}),
		// Spec 082 T182: kept with the row, so a reload follows a
		// may-have-been-sent payment as one — never as a plain pending op.
		...(record.maybe_sent ? { maybeSent: true } : {}),
		...(record.submit_block != null ? { submitBlock: record.submit_block } : {})
	};
}

export function createSendExecutor(ports: SendShellPorts, self?: SendExecutorSelf) {
	/**
	 * Accepted ops awaiting their receipt, by hash. Populated by `SubmitUserOp`
	 * and consumed by the `TrackSubmitted` that always follows it.
	 */
	const submitted = new Map<string, SubmitResult>();

	/**
	 * The write-ahead waiting for the core's `ClearToPost` (spec 082 RJ1), by
	 * op hash: `true` lets the POST go, `false` stops it (nothing is sent).
	 */
	const clearances = new Map<string, (onDisk: boolean) => void>();

	/**
	 * Whether the write-ahead's records of each op are really on disk (RJ1), by
	 * op hash: set by its `PersistTxRecords`, used up by its `ClearToPost`. The
	 * core clears the POST on the store's acknowledgement, and a write the
	 * store refused is acknowledged all the same (logged, or the send would
	 * never move on) — so the clearance is only as good as this mark. Set by
	 * each write, so a second attempt of the same op is judged by its own.
	 */
	const aheadOnDisk = new Map<string, boolean>();

	/**
	 * The wallet's own Send writes ahead too (spec 082 RJ1): the op is signed
	 * and hashed, nothing has left. The core writes every recipient's record
	 * ("may have been sent"), hands the op to the tracker and answers
	 * `ClearToPost`; only then is it POSTed. No clearance within
	 * `userOpWriteAheadWaitMs()` → no POST, and the send fails as not sent —
	 * a quit from here on leaves a pending record the tracker resolves.
	 */
	function writeAhead(
		dispatch: SendExecutorSelf['dispatch'],
		userOpHash: string,
		submitBlock: number | null
	): Promise<void> {
		const key = userOpHash.toLowerCase();
		return new Promise<void>((resolve, reject) => {
			const timer = setTimeout(() => {
				if (!clearances.has(key)) return;
				clearances.delete(key);
				console.warn(
					`[send] write-ahead: no clearance for ${userOpHash.slice(0, 10)}… in ` +
						`${userOpWriteAheadWaitMs()} ms — not posting`
				);
				reject(new WriteAheadTimeoutError(userOpHash));
			}, userOpWriteAheadWaitMs());
			clearances.set(key, (onDisk) => {
				clearTimeout(timer);
				if (onDisk) {
					resolve();
					return;
				}
				console.warn(
					`[send] write-ahead: the store refused the records of ${userOpHash.slice(0, 10)}… — not posting`
				);
				reject(new WriteAheadTimeoutError(userOpHash));
			});
			dispatch({
				type: 'op_signed',
				user_op_hash: userOpHash,
				submit_block: submitBlock,
				now_ms: Date.now()
			});
		});
	}

	/**
	 * The receipt hears the tracker's verdict on `userOpHash` (spec 082 RJ1,
	 * RJ4). The write-ahead hands the op to the tracker BEFORE its POST, and
	 * the send core names the op only when the relay's reply (or its loss)
	 * comes back: a `ReceiptUpdate` before that is a stale hash to it, and is
	 * dropped. The chain check can find the landed op while that reply is being
	 * lost (EX-W1); the watcher that carried the verdict is spent by then and a
	 * terminal entry never changes again — so the receipt said "may have been
	 * sent" over money the tracker saw land. Run once the core holds the
	 * `Submitted` result, which the effect loop resolves in this same task.
	 */
	function catchUpReceipt(dispatch: SendExecutorSelf['dispatch'], userOpHash: string): void {
		setTimeout(() => {
			const key = userOpHash.toLowerCase();
			const entry = txTrackerView().entries.find((row) => row.user_op_hash.toLowerCase() === key);
			const outcome = entry ? outcomeOf(entry) : null;
			if (outcome) dispatch({ type: 'receipt_update', user_op_hash: userOpHash, outcome });
		}, 0);
	}

	/**
	 * The next `FetchTokens` must really walk the chains: the send flow asked
	 * for a fresh list (`ClearTokenCache` comes first on a refresh), or a
	 * network was just added and the list in memory has never seen it. The
	 * balance machine's holdings would answer both with what was already there.
	 */
	let walkNextFetch = false;

	/**
	 * Today's receipt convergence, detached exactly as it was: it must never block
	 * the receipt screen, and a slow or unreachable poll leaves the payment
	 * submitted (invariant ⑤) rather than turning it into an error.
	 */
	function waitForReceipt(handoff: SendTrackerHandoff): void {
		const result = handoff.submitted;
		if (!result) return;
		void result
			.waitForTxHash()
			.then(async (hash: string) => {
				ports.receiptUpdate(handoff.userOpHash, { type: 'confirmed', tx_hash: hash });
				await updateTransactions(handoff.recordIds, { txHash: hash, status: 'confirmed' }).catch(
					() => {}
				);
			})
			.catch(async (error: unknown) => {
				// A landed revert, versus a slow or unreachable poll: only the former
				// is a real failure. The relay's refusal and its fee hold are the
				// tracker's to read from the relay's status (spec 082 RJ4) — the
				// receipt wait reads the receipt alone.
				if (!(error instanceof UserOpRevertedError)) return;
				ports.receiptUpdate(handoff.userOpHash, {
					type: 'failed',
					rejected: false,
					not_sent: false
				});
				await updateTransactions(handoff.recordIds, { status: 'failed' }).catch(() => {});
			});
	}

	async function execute(effect: SendEffect, signal?: AbortSignal): Promise<SendShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'fetch_tokens': {
				// The asset list's own holdings, when it has settled a round for
				// this account: one source, so the picker opens on what is already
				// in memory and every balance on it is the balance on the home row.
				const walk = walkNextFetch;
				walkNextFetch = false;
				const held = walk ? undefined : await ports.holdings?.(operation.address);
				if (held?.kind === 'tokens') {
					return { type: 'tokens_loaded', tokens: held.tokens, chains: chainInfos() };
				}
				if (held?.kind === 'unreadable') {
					// Two rounds in a row reached nothing — the core says it could
					// not load the tokens (`alertLoadTokensError`).
					return { type: 'tokens_loaded', tokens: null, chains: chainInfos() };
				}
				try {
					const tokens = await fetchTokens(operation.address, {
						onProgress: (partial) => {
							// Display-only; lock resolution always waits for the full answer.
							ports.tokensFetched(partial);
							ports.tokensPartial(partial.map(toSendToken));
						}
					});
					ports.tokensFetched(tokens);
					// Read AFTER the fetch: an "add this network" retry re-runs the whole
					// boot, and the chain it just added has to be in this snapshot.
					return { type: 'tokens_loaded', tokens: tokens.map(toSendToken), chains: chainInfos() };
				} catch {
					// `catch(() => showAlert(send.alertLoadTokensError))` — the core words it.
					return { type: 'tokens_loaded', tokens: null, chains: chainInfos() };
				}
			}

			case 'clear_token_cache': {
				clearTokenCache(operation.address);
				walkNextFetch = true;
				return { type: 'token_cache_cleared' };
			}

			case 'resolve_token_metadata': {
				const meta = await resolveTokenMetadata(operation.chain_id, [operation.address]);
				const found = meta.get(operation.address);
				return {
					type: 'token_metadata',
					meta: found ? { symbol: found.symbol, decimals: found.decimals } : null
				};
			}

			case 'add_network': {
				const result = await addCustomNetworkByChainId(operation.chain_id);
				if (result.ok) {
					// The core re-runs the boot next; the chain it just added is in no
					// list the asset screen holds yet.
					walkNextFetch = true;
					return { type: 'network_added', outcome: { type: 'added' } };
				}
				if (result.reason === 'not-found') {
					return { type: 'network_added', outcome: { type: 'not_found' } };
				}
				// A check that could not be made is not a refusal: the add did not
				// happen, and nothing is said against the network (invariant ③).
				if (result.reason === 'unverified') {
					return { type: 'network_added', outcome: { type: 'error' } };
				}
				return {
					type: 'network_added',
					outcome: { type: 'not_compatible', detail: result.error ?? null }
				};
			}

			case 'estimate_fee': {
				// Asked of the screen's live `fee_policy` session, NOT of
				// `estimateTransactionFee`. That is the whole point: the number this
				// pre-check gates on and the number the fee card shows are the same
				// object, produced once, by the machine that owns the rules. See
				// `SendShellPorts.feeQuote`.
				//
				// The fee leg is deliberately not built here — `fee_policy` appends its
				// own, to the recipient its own quote named, so the simulated operation
				// is the submitted one.
				const answer = await ports.feeQuote({
					chainId: operation.chain_id,
					account: operation.account,
					// A batch takes precedence only when it HAS legs — `estimateTransactionFee`
					// required `batchCalls.length > 0` for the same reason, and an
					// empty one would otherwise silence the single call beside it.
					calls:
						operation.batch && operation.batch.length > 0
							? operation.batch
							: operation.tx
								? [operation.tx]
								: [],
					feeToken: operation.gas_fee_token,
					// Nobody chose the coin: the fee machine picks one that can pay,
					// and the estimate's `fee_asset` says which (spec 078).
					autoFeeToken: operation.auto_fee_token,
					publicKeyHex: operation.public_key_hex ?? undefined
				});
				return { type: 'fee_estimated', outcome: toSendFeeOutcome(answer) };
			}

			case 'prewarm_fees': {
				// Fire-and-forget: the reads run on into the fee caches while the
				// person chooses; the core hears back at once and waits for none.
				prewarmFees(operation.account, operation.chain_ids, ports.feeTier?.() ?? 'standard');
				return { type: 'fees_prewarmed' };
			}

			case 'probe_treasury': {
				const probe = await probeTreasury(operation.chain_id);
				if (probe.kind === 'low-float') {
					return {
						type: 'treasury_probed',
						probe: {
							type: 'low_float',
							status: {
								chain_id: probe.status.chainId,
								address: probe.status.address,
								asset: probe.status.asset === 'pathUSD' ? 'path_usd' : 'native',
								balance: probe.status.balance.toString(),
								floor: probe.status.floor.toString(),
								bootstrap_needed: probe.status.bootstrapNeeded,
								// The CORE decides whether this is a network Vela ships,
								// and therefore whose relayer the operator owns. The shell
								// reports the probe; it does not judge it (spec 060).
								operator_served: false,
								// …nor words its figures: the coin and the amounts in it
								// are the core's (issue 422).
								coin: null
							}
						}
					};
				}
				if (probe.kind === 'covered')
					return { type: 'treasury_probed', probe: { type: 'covered' } };
				if (probe.kind === 'uncovered') {
					return { type: 'treasury_probed', probe: { type: 'uncovered' } };
				}
				return { type: 'treasury_probed', probe: { type: 'unknown' } };
			}

			case 'load_account_credential': {
				const stored = findAccountByCredentialId(operation.account_id);
				const publicKeyHex = stored?.publicKeyHex ?? null;
				ports.credentialLoaded(publicKeyHex);
				return { type: 'account_credential', public_key_hex: publicKeyHex };
			}

			case 'submit_user_op': {
				try {
					const credentialId = ports.credentialId(operation.account);
					if (!credentialId) {
						// Fail closed: the wallet must never sign with a credential that
						// does not belong to the account the core built this batch for.
						throw new Error('No passkey credential for the active account');
					}
					// A multi-key wallet signs with ANY of its founding keys: allow-list
					// every credential and let the provider pick; the assertion's own
					// credentialId then selects the signer address (r-field) and the
					// full key set builds the (undeployed) initCode.
					const stored = findAccountByAddress(operation.account);
					// Only a genuinely multi-key account changes shape here — a
					// single-key wallet keeps the exact historical wire (and bytes).
					const keySet = stored?.keys && stored.keys.length > 1 ? keySetOf(stored) : null;
					const credentials = keySet
						? keySet.keys.map((key) => ({ id: key.credentialId }))
						: [{ id: credentialId }];
					const signer = {
						account: operation.account,
						keys: stored
							? keySetOf(stored).keys
							: [{ credentialId, publicKeyHex: operation.public_key_hex }],
						credentials,
						// The wallet's own send: no method, no site (contract §1).
						request: { method: '', params: [], origin: '', chainId: operation.chain_id }
					};
					const signFn: SignFn = async (challenge: Uint8Array) => {
						// The passkey sheet is opening — the core moves to 'signing' here,
						// exactly where `setTxStatus('signing')` sat.
						ports.signingStarted();
						const assertion = await signChallenge(challenge, signer);
						const compat = verifySafeWebAuthn(assertion);
						if (!compat.ok) {
							throw new Error(
								"Your device's identity provider is not compatible with Vela Wallet. " +
									'Please switch to Google Password Manager.\n\n' +
									compat.reason
							);
						}
						return {
							signature: fromHex(assertion.signatureHex),
							authenticatorData: fromHex(assertion.authenticatorDataHex),
							clientDataJSON: fromHex(assertion.clientDataJSONHex),
							credentialId: assertion.credentialId
						};
					};
					// RJ1: the records before the bytes. The wallet's own page has no
					// asker to lose, so the write-ahead is the whole gate.
					const dispatch = self?.dispatch;
					if (dispatch) {
						signFn.beforePost = ({ userOpHash, submitBlock }) =>
							writeAhead(dispatch, userOpHash, submitBlock);
					}
					// One call stays a single `executeUserOp` and N stay a MultiSend
					// (`buildNativeCallData`), so this is byte-for-byte the calldata
					// `sendNative`/`sendERC20` produced for a single transfer.
					const result = await sendBatchCalls(
						operation.account,
						operation.calls.map(toShellCall),
						operation.chain_id,
						keySet ?? operation.public_key_hex,
						signFn,
						operation.max_fee_per_gas != null
							? fromWireAmount(operation.max_fee_per_gas)
							: undefined,
						operation.gas_fee_token,
						operation.quoted_fee
							? {
									amount: fromWireAmount(operation.quoted_fee.amount),
									recipient: operation.quoted_fee.recipient,
									// The speed the person was shown, named on the wire beside
									// the reimbursement it was priced with (spec 068). The core
									// takes it from the SAME estimate as the amount (spec 069),
									// so the screen and the chain cannot disagree about which
									// tier this send is — and `rapid` never gets this far.
									tier: wireTier(operation.quoted_fee.tier)
								}
							: undefined
					);
					submitted.set(result.userOpHash, result);
					// What the tracker learned while the reply was out (RJ1, RJ4).
					if (dispatch) catchUpReceipt(dispatch, result.userOpHash);
					// A lost reply is not a failure (spec 082 RA4): the payment may be
					// on its way, so it is recorded and followed under the local hash.
					return {
						type: 'submitted',
						user_op_hash: result.userOpHash,
						now_ms: Date.now(),
						maybe_sent: result.maybeSent,
						submit_block: result.submitBlock
					};
				} catch (error) {
					return { type: 'submit_failed', failure: classifySubmit(error) };
				}
			}

			case 'cancel_passkey_sign': {
				cancelChallenge();
				return { type: 'passkey_cancel_acknowledged' };
			}

			case 'persist_tx_records': {
				// ONE atomic write for every sibling: a per-record `Promise.all` races
				// the read-modify-write and silently drops all but one (invariant ⑥).
				let onDisk = true;
				await saveTransactions(operation.records.map(toLocalTransaction)).catch((e) => {
					onDisk = false;
					console.warn('[send] Failed to save records:', e);
				});
				// The write-ahead's records (may have been sent): whether they are
				// really there is what its clearance turns on (RJ1).
				const ahead = operation.records.find((record) => record.maybe_sent);
				if (ahead) aheadOnDisk.set(ahead.user_op_hash.toLowerCase(), onDisk);
				return { type: 'records_persisted' };
			}

			case 'track_submitted': {
				if (operation.admitted) {
					// The relay took the op the write-ahead announced (RJ1): the
					// tracker hears it straight away — never "may have been sent"
					// again. The sink is the page's and forwards no `admitted`.
					// The receipt watches again: an early verdict (a "not sent" the
					// slow POST outlasted, delivered before the core named the op
					// and dropped as a stale hash) spent the write-ahead's watcher,
					// and the tracker revives the entry on this hand-off.
					if (trackerSink) {
						const dispatch = self?.dispatch;
						const userOpHash = operation.user_op_hash;
						trackSubmitted(
							userOpHash,
							operation.record_ids,
							operation.chain_id,
							dispatch
								? (outcome) =>
										dispatch({ type: 'receipt_update', user_op_hash: userOpHash, outcome })
								: undefined,
							false,
							operation.submit_block,
							true,
							operation.sender
						);
						submitted.delete(operation.user_op_hash);
						return { type: 'track_handed_off' };
					}
				}
				const handoff: SendTrackerHandoff = {
					userOpHash: operation.user_op_hash,
					recordIds: operation.record_ids,
					chainId: operation.chain_id,
					maybeSent: operation.maybe_sent,
					submitBlock: operation.submit_block,
					submitted: submitted.get(operation.user_op_hash) ?? null,
					sender: operation.sender
				};
				submitted.delete(operation.user_op_hash);
				if (trackerSink) trackerSink(handoff);
				else waitForReceipt(handoff);
				return { type: 'track_handed_off' };
			}

			case 'clear_to_post': {
				// Every record of the op is on disk (RJ1): the POST may go — only if
				// the store really took them, whatever it acknowledged.
				const key = operation.user_op_hash.toLowerCase();
				const clear = clearances.get(key);
				clearances.delete(key);
				const onDisk = aheadOnDisk.get(key) === true;
				aheadOnDisk.delete(key);
				clear?.(onDisk);
				return { type: 'post_cleared' };
			}

			case 'mark_admitted': {
				// The relay took it: no longer "may have been sent"; still pending
				// for the tracker. One write for every sibling.
				await updateTransactions(operation.record_ids, { maybeSent: false }).catch(() => {});
				return { type: 'records_persisted' };
			}

			case 'delete_tx_records': {
				// Proven never sent (RJ1): the write-ahead's records go, in one write.
				await deleteTransactions(operation.ids).catch(() => {});
				return { type: 'records_persisted' };
			}

			case 'track_withdrawn': {
				// …and the tracker forgets them, patching nothing.
				submitted.delete(operation.user_op_hash);
				withdrawTracked(operation.user_op_hash, operation.record_ids);
				return { type: 'track_handed_off' };
			}

			case 'resolve_identity': {
				const identity = await resolveRecipientIdentity(operation.address);
				return {
					type: 'identity_resolved',
					identity: identity ? { name: identity.name, source: identity.source } : null
				};
			}

			case 'resolve_risk': {
				const risk = await resolveRecipientRisk(operation.chain_id, operation.address);
				return {
					type: 'risk_resolved',
					risk: { is_contract: risk.isContract, first_time: risk.firstInteraction }
				};
			}

			case 'simulate_calls': {
				const sim = await simulateAssetChanges(
					operation.account,
					operation.calls.map(toShellCall),
					operation.chain_id
				);
				// Opaque to the core: bigint deltas cross as decimal strings, the same
				// codec a signing record is stored through.
				return {
					type: 'sim_resolved',
					sim_json: sim ? JSON.stringify(serializeAssetSim(sim)) : null
				};
			}

			case 'start_timer': {
				// Abort-aware so leaving the screen mid-pre-check cannot hold a 15 s
				// timer alive; the loop drops the answer to an aborted effect anyway.
				await new Promise<void>((resolve) => {
					const timer = setTimeout(resolve, operation.ms);
					signal?.addEventListener(
						'abort',
						() => {
							clearTimeout(timer);
							resolve();
						},
						{ once: true }
					);
				});
				return { type: 'timer_elapsed', tag: operation.tag };
			}

			case 'haptic': {
				if (operation.kind === 'success') hapticSuccess();
				else hapticError();
				return { type: 'haptic_played' };
			}

			case 'show_alert': {
				ports.alert(operation.kind);
				return { type: 'alert_acknowledged' };
			}

			case 'close': {
				ports.close();
				return { type: 'closed' };
			}
		}
	}

	/**
	 * The submit failure in the core's vocabulary. A refusal is the CORE's
	 * reading of the relay's answer (`submit_step` → `NotSent{rejection}`,
	 * spec 082 RA1); the passkey sheet's own cancel is the one fact only this
	 * side has. Anything thrown before the submit (an estimate, a deploy check)
	 * keeps the underfunded reading of its words, which is all it ever had.
	 */
	function classifySubmit(
		error: unknown
	): Extract<SendShellResult, { type: 'submit_failed' }>['failure'] {
		const err = error as { code?: unknown; message?: string } | null | undefined;
		if (error instanceof PasskeyError && error.kind === 'cancelled') {
			return { type: 'passkey_cancelled' };
		}
		// Spec 102 (P2b-W1): this account cannot sign on the web — nothing was
		// signed or sent. The core says why on the confirm screen, in the
		// person's language (`SendView.tx_venue_block`), from the block itself.
		if (error instanceof VenueBlockedError) return { type: 'venue_blocked', block: error.block };
		// The records were not written in time, so nothing was POSTed (RJ1).
		if (error instanceof WriteAheadTimeoutError) {
			return { type: 'other', message: userOpNotSentDetail() };
		}
		const message = err?.message ?? String(error ?? '');
		if (error instanceof UserOpNotSentError) {
			if (error.rejection === 'relayer_unavailable') return { type: 'relayer_unavailable' };
			if (error.rejection === 'bundler_underfunded') return { type: 'bundler_underfunded' };
			// Another operation of this account still holds the nonce (the
			// relay's `nonce_in_flight`, or an older relay's `[existingHash:…]`
			// marker — both read by the core as `NonceHeld`): not a failure of
			// the network. Said as "waiting for your last transaction", and Try
			// again waits for it like any held confirm (correctness batch item 3).
			if (typeof error.rejection === 'object' && error.rejection !== null) {
				if ('nonce_held' in error.rejection) return { type: 'previous_pending' };
			}
			console.warn('[send] submit not sent:', message);
			return { type: 'other', message: message || null };
		}
		// Wording-tolerant: the bundler has reworded this before (legacy
		// "...bundler EOA" → current "...bundler gas account ... Deposit to:").
		if (parseBundlerUnderfunded(message)) return { type: 'bundler_underfunded' };
		// Never surface a raw RPC/library exception on the money-flow confirm
		// screen. Log it for diagnostics; the core shows a calm, localized key.
		console.warn('[send] unhandled tx error:', message || String(error));
		return { type: 'other', message: message || null };
	}

	function toFailure(effect: SendEffect, error: unknown): SendShellResult {
		const operation = effect.operation;
		switch (operation.type) {
			case 'fetch_tokens':
				return { type: 'tokens_loaded', tokens: null, chains: chainInfos() };
			case 'clear_token_cache':
				return { type: 'token_cache_cleared' };
			case 'resolve_token_metadata':
				// `resolveTokenMetadata` threw → the request names a token this wallet
				// cannot describe, which is the unknown-token exception.
				return { type: 'token_metadata', meta: null };
			case 'add_network':
				// The TS `catch { setAddNetworkMsg(t('send.lock.netAddError')) }`.
				return { type: 'network_added', outcome: { type: 'error' } };
			case 'estimate_fee':
				// The estimate is MANDATORY: a failure alerts and never advances to
				// confirm with a fabricated preview (invariant ②).
				return { type: 'fee_estimated', outcome: { type: 'failed', kind: 'estimate_failed' } };
			case 'prewarm_fees':
				// Nothing to report either way: the reads are best effort.
				return { type: 'fees_prewarmed' };
			case 'probe_treasury':
				// `probeTreasury` swallows its own errors; this is the defensive tail,
				// and "unknown" is the only honest answer — never routed as uncovered.
				return { type: 'treasury_probed', probe: { type: 'unknown' } };
			case 'load_account_credential':
				// A throwing read is the same alert a missing public key produces.
				return { type: 'account_credential', public_key_hex: null };
			case 'submit_user_op':
				// `execute` classifies its own failures; this is the defensive tail.
				return { type: 'submit_failed', failure: classifySubmit(error) };
			case 'cancel_passkey_sign':
				return { type: 'passkey_cancel_acknowledged' };
			case 'persist_tx_records':
				// Best effort, exactly like the TS `.catch(() => {})` — and the core
				// must still be told, or `TrackSubmitted` never fires (invariant ⑥).
				return { type: 'records_persisted' };
			case 'track_submitted':
				return { type: 'track_handed_off' };
			case 'clear_to_post':
				return { type: 'post_cleared' };
			case 'mark_admitted':
			case 'delete_tx_records':
				// The core must still be told, or the send never moves on.
				return { type: 'records_persisted' };
			case 'track_withdrawn':
				return { type: 'track_handed_off' };
			case 'resolve_identity':
				return { type: 'identity_resolved', identity: null };
			case 'resolve_risk':
				return { type: 'risk_resolved', risk: null };
			case 'simulate_calls':
				// Best-effort: a failed sim leaves the confirm surface empty.
				return { type: 'sim_resolved', sim_json: null };
			case 'start_timer':
				return { type: 'timer_elapsed', tag: operation.tag };
			case 'haptic':
				return { type: 'haptic_played' };
			case 'show_alert':
				return { type: 'alert_acknowledged' };
			case 'close':
				return { type: 'closed' };
		}
	}

	return { execute, toFailure };
}
