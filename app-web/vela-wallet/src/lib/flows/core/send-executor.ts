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

import { fromHex, verifySafeWebAuthn } from '$lib/core/kernels';
import { getAllNetworksSync, networkId, nativeSymbol } from '$lib/services/networks';
import { PasskeyError } from '$lib/onboarding/core/passkey';
import { cancelChallenge, signChallenge } from '$lib/signing/sign-challenge';
import { addCustomNetworkByChainId } from '$lib/services/add-network.svelte';
import { parseBundlerUnderfunded, probeTreasury } from '$lib/services/bundler-service';
import { hapticError, hapticSuccess } from '$lib/services/platform';
import { resolveRecipientIdentity } from '$lib/services/recipient-identity';
import { resolveRecipientRisk } from '$lib/services/recipient-risk';
import {
	keySetOf,
	sendBatchCalls,
	UserOpFeeHoldError,
	UserOpRejectedError,
	type SubmitResult
} from '$lib/services/safe-transaction';
import { findAccountByAddress, findAccountByCredentialId } from '$lib/services/accounts';
import { saveTransactions, updateTransactions } from '$lib/services/records';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { resolveTokenMetadata } from '$lib/services/token-metadata';
import { serializeAssetSim, simulateAssetChanges } from '$lib/services/sim/tx-simulation';
import { clearTokenCache, fetchTokens } from '$lib/services/wallet-api';

import type { SendChainInfo } from '$lib/core/generated/SendChainInfo';
import type { SendShellResult } from '$lib/core/generated/SendShellResult';
import type { SendTxRecord } from '$lib/core/generated/SendTxRecord';
import { prewarmFees } from './fee-prewarm';
import { wireTier } from './wire-tier';
import {
	fromWireAmount,
	toSendToken,
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
	/** The bundler's own receipt promise, or `null` if this process never had it. */
	submitted: SubmitResult | null;
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
		...(record.usd != null ? { usd: record.usd } : {})
	};
}

export function createSendExecutor(ports: SendShellPorts) {
	/**
	 * Accepted ops awaiting their receipt, by hash. Populated by `SubmitUserOp`
	 * and consumed by the `TrackSubmitted` that always follows it.
	 */
	const submitted = new Map<string, SubmitResult>();

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
				// The relay is holding the op until network fees fit what the user
				// signed. Still queued, so the record stays pending — only the wording
				// changes (invariant ⑦).
				if (error instanceof UserOpFeeHoldError) {
					ports.receiptUpdate(handoff.userOpHash, { type: 'fee_held' });
					return;
				}
				// A definitive relay refusal / drop / revert, versus a slow or
				// unreachable poll. Only the former is a real failure.
				const rejected = error instanceof UserOpRejectedError;
				const message = (error as { message?: string } | null)?.message ?? '';
				if (!rejected && !/dropped from the network|reverted|failed/i.test(message)) return;
				ports.receiptUpdate(handoff.userOpHash, { type: 'failed', rejected });
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
				return {
					type: 'fee_estimated',
					outcome: await ports.feeQuote({
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
					})
				};
			}

			case 'prewarm_fees': {
				// Fire-and-forget: the reads run on into the fee caches while the
				// person chooses; the core hears back at once and waits for none.
				prewarmFees(operation.account, operation.chain_ids, ports.feeTier?.() ?? 'fast');
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
								operator_served: false
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
					const signFn = async (challenge: Uint8Array) => {
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
					return { type: 'submitted', user_op_hash: result.userOpHash, now_ms: Date.now() };
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
				await saveTransactions(operation.records.map(toLocalTransaction)).catch(() => {});
				return { type: 'records_persisted' };
			}

			case 'track_submitted': {
				const handoff: SendTrackerHandoff = {
					userOpHash: operation.user_op_hash,
					recordIds: operation.record_ids,
					chainId: operation.chain_id,
					submitted: submitted.get(operation.user_op_hash) ?? null
				};
				submitted.delete(operation.user_op_hash);
				if (trackerSink) trackerSink(handoff);
				else waitForReceipt(handoff);
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
	 * The submit-failure classification the core deliberately does not own: every
	 * regex, in the order `useSendController.ts:1072-1104` applied them.
	 */
	function classifySubmit(
		error: unknown
	): Extract<SendShellResult, { type: 'submit_failed' }>['failure'] {
		const err = error as { code?: unknown; message?: string } | null | undefined;
		if (error instanceof PasskeyError && error.kind === 'cancelled') {
			return { type: 'passkey_cancelled' };
		}
		const message = err?.message ?? String(error ?? '');
		if (/gas relayer is unavailable/i.test(message)) return { type: 'relayer_unavailable' };
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
