// Ported from src/services/wallet-state-core/sign-executor.ts @ f9bcb278 — RN
// seams rewritten to the web modules; every ordering and every wording verbatim.
/**
 * The only place the `sign_request` core touches the outside world.
 *
 * Seven operations, each one existing service call — the vocabulary the core
 * declares (`SendResponse` / `CheckBundlerFunding` / `AttemptSponsorship` /
 * `SignAndSubmit` / `PersistRecord` / `UpdateRecord` / `SwitchActiveAccount`).
 * No branching on business meaning: single-flight, BUG-2's "a rejected pipeline
 * may not submit", the funding rid pin, the record-then-respond order and the
 * §12.1.6 sequencing all live in Rust.
 *
 * What DOES live here, because the core's doc comments put it here:
 *
 * - **The 15 s pre-check race.** `checkBundlerFunding` raced with a timeout that
 *   answers `null`, exactly as `dapp-connection.tsx:666-698` fell through to
 *   submit on a slow RPC.
 * - **The submit outcome, typed.** Whether an op was accepted, may have been
 *   sent or was provably not sent is the CORE's (`submit_step`, spec 082 RA1);
 *   this side maps the typed verdicts it throws — a passkey cancel, a page that
 *   is gone (`asker_gone`), the relay's underfunded refusal with the live
 *   `fetchBundlerAccountInfo` composition — into `SignSubmitOutcome`.
 * - **The record codec.** The core owns the id scheme, the status and the FINAL
 *   (capped) params; `buildSigningRecord` still owns `capRequest` clipping,
 *   `signedContent`, the recipient/value projection and the asset-sim blob.
 * - **Persist/Update serialisation per `record_id`** — the core states the shell
 *   must do it, and it is load-bearing: `updateTransaction` on a row that has
 *   not been written yet is a silent no-op, which would strand a confirmed op
 *   as forever-'pending' (the TS `await pendingSave` before the patch).
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with.
 */

import { PasskeyError } from '$lib/onboarding/core/passkey';
import {
	attemptSilentSponsorship,
	checkBundlerFunding,
	clearBundlerCache,
	fetchBundlerAccountInfo,
	parseBundlerUnderfunded,
	recommendedFundingWei,
	underfundedRequiredWei,
	type FundingNeeded
} from '$lib/services/bundler-service';
import { buildSigningRecord } from '$lib/services/dapp-history';
import { nativeSymbol } from '$lib/services/networks';
import { deleteTransaction, saveTransaction, updateTransaction } from '$lib/services/records';
import { serializeAssetSim } from '$lib/services/sim/tx-simulation';
import {
	DAppReceiptPendingError,
	handleDAppRequest,
	type DAppSubmitHooks,
	type SigningAccount
} from '$lib/services/dapp-submit';
import { UserOpNotSentError } from '$lib/services/safe-transaction';
import { dappReceiptWaitMs, userOpNotSentDetail, userOpWriteAheadWaitMs } from '$lib/core/kernels';

import type { SignFundingNeeded } from '$lib/core/generated/SignFundingNeeded';
import type { SignShellResult } from '$lib/core/generated/SignShellResult';
import type { SignEffect, SignShellPorts } from './sign-types';
import { AskerGoneError, signErrorMessage, WriteAheadTimeoutError } from './sign-types';
import { wireTier } from '$lib/flows/core/wire-tier';

export { signErrorMessage } from './sign-types';

/** The proactive pre-check's ceiling (`dapp-connection.tsx:670`). */
const PRECHECK_TIMEOUT_MS = 15_000;

/**
 * The reactive fallback threshold when the bundler's message named no
 * `required:` amount (`dapp-connection.tsx:833`), kept verbatim.
 */
const REACTIVE_THRESHOLD_MARGIN_WEI = 100_000_000_000_000n;

// ---------------------------------------------------------------------------
// Wire codecs
// ---------------------------------------------------------------------------

/** `FundingNeeded` (bigints) → the core's decimal-string wire shape. */
export function toWireFunding(funding: FundingNeeded): SignFundingNeeded {
	return {
		deposit_address: funding.depositAddress,
		safe_address: funding.safeAddress,
		chain_id: funding.chainId,
		native_symbol: funding.nativeSym,
		threshold_wei: funding.thresholdWei.toString(),
		recommended_wei: funding.recommendedWei.toString(),
		current_balance_wei: funding.currentBalance.toString()
	};
}

/**
 * A decimal wei string back to a bigint. The core only ever emits values it was
 * given, but a malformed one must not throw inside the effect loop — `0n` is
 * the same nothing an absent balance meant.
 */
export function fromWireWei(value: string): bigint {
	try {
		return BigInt(value);
	} catch {
		return 0n;
	}
}

/**
 * The raw JSON-RPC params array the core carried through, verbatim. A payload
 * this shell cannot parse is `[]`, never a throw: the core already refused
 * anything it could not parse itself (fail-closed at `approve_with`), so this
 * is only ever the defensive tail.
 */
function parseParams(json: string): unknown[] {
	try {
		const parsed: unknown = JSON.parse(json);
		return Array.isArray(parsed) ? parsed : [];
	} catch {
		return [];
	}
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/**
 * The relay refused the op (spec 082 RJ3): a submit-time `NotSent` with a
 * rejection other than "relayer unavailable" — the relay being away is not a
 * refusal, and a person may try again after it.
 */
export function isRelayRefusal(error: unknown): boolean {
	return (
		error instanceof UserOpNotSentError &&
		error.rejection !== null &&
		error.rejection !== 'relayer_unavailable'
	);
}

export function createSignExecutor(ports: SignShellPorts) {
	/**
	 * op hash → chain, for the ops this executor answered BY their op hash, so
	 * the answer can carry `opHash: {chainId}` (spec 082 RF3). Bounded like the
	 * other per-op maps; consumed by the answer it belongs to.
	 */
	const answeredOps = new Map<string, number>();

	/**
	 * Writes in flight per `record_id`. `saveTransaction` and `updateTransaction`
	 * are separate round trips over the same key, and the core issues the patch
	 * without waiting for the insert (it only waits for the ack it needs for §4).
	 * Chaining them here is the `await pendingSave` of the TS path.
	 */
	const writes = new Map<string, Promise<void>>();

	/**
	 * The write-ahead waiting for its clearance (spec 082 RJ1), by request id:
	 * the op's hash and the release. The core answers `ClearToPost` once the
	 * record of the op is on disk; nothing is POSTed before it.
	 */
	const clearances = new Map<string, { userOpHash: string; clear: () => void }>();

	/**
	 * The receipt wait of each request in flight, by request id — aborted when
	 * the core answers that request itself (`OpTracked`, spec 082 RJ4), so the
	 * wait stops rather than run on to a result the core will drop.
	 */
	const receiptWaits = new Map<string, AbortController>();

	/**
	 * Write the op down before any byte of it leaves (spec 082 RJ1): announce it
	 * (`OpSigned`), then wait for the core's `ClearToPost` — at most
	 * `userOpWriteAheadWaitMs()`. No clearance in time → no POST; the caller's
	 * submit fails as "not sent", which it provably is.
	 */
	function writeAhead(id: string, userOpHash: string, submitBlock: number | null): Promise<void> {
		return new Promise<void>((resolve, reject) => {
			const timer = setTimeout(() => {
				if (clearances.get(id)?.userOpHash !== userOpHash) return;
				clearances.delete(id);
				console.warn(
					`[sign_request] write-ahead: no clearance for ${userOpHash.slice(0, 10)}… in ` +
						`${userOpWriteAheadWaitMs()} ms — not posting`
				);
				reject(new WriteAheadTimeoutError(userOpHash));
			}, userOpWriteAheadWaitMs());
			// Registered BEFORE the announcement: the clearance can come back
			// within the same turn of the effect loop.
			clearances.set(id, {
				userOpHash,
				clear: () => {
					clearTimeout(timer);
					resolve();
				}
			});
			ports.opSigned(id, userOpHash, submitBlock);
		});
	}

	function serialised(recordId: string, task: () => Promise<void>): Promise<void> {
		const previous = writes.get(recordId) ?? Promise.resolve();
		const next = previous.then(task, task);
		writes.set(recordId, next);
		void next.finally(() => {
			if (writes.get(recordId) === next) writes.delete(recordId);
		});
		return next;
	}

	async function execute(effect: SignEffect): Promise<SignShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'send_response': {
				// The core answered this request: a receipt wait still running for it
				// (the core answered through the tracker, RJ4) stops here.
				receiptWaits.get(operation.id)?.abort();
				receiptWaits.delete(operation.id);
				// F2: the transport that OWNS the request, resolved from the id the
				// core carried — never a shared ref.
				const transport = ports.transportFor(operation.transport_id);
				if (operation.payload.type === 'ok') {
					// An on-chain request answered by its op hash — the receipt is
					// late, or the op may only have been sent — carries the chain, so
					// the extension can translate the page's receipt reads for that
					// hash (spec 082 RF3).
					const result = operation.payload.result;
					const opChain = result ? answeredOps.get(result.toLowerCase()) : undefined;
					if (result) answeredOps.delete(result.toLowerCase());
					transport?.sendResponse(
						operation.id,
						result,
						undefined,
						opChain !== undefined ? { chainId: opChain } : undefined
					);
				} else {
					// The core owns the code and the kind; the words are this side's, and
					// `signErrorMessage` reproduces the exact string the TS provider sent
					// for each one (a `submit_failed` detail IS `err.message`, verbatim).
					transport?.sendResponse(operation.id, undefined, {
						code: operation.payload.code,
						kind: operation.payload.kind,
						message: signErrorMessage({
							kind: operation.payload.kind,
							detail: operation.payload.message
						})
					});
				}
				return { type: 'responded' };
			}

			case 'check_bundler_funding': {
				if (operation.bust_cache) {
					// Drop the cached (stale, underfunded) balance so the retry reads the
					// freshly funded amount instead of re-prompting (`:927-933`).
					clearBundlerCache(operation.chain_id, operation.account);
				}
				const cost =
					operation.bundler_cost_wei != null ? fromWireWei(operation.bundler_cost_wei) : undefined;
				let timer: ReturnType<typeof setTimeout> | undefined;
				try {
					const funding = await Promise.race([
						checkBundlerFunding(operation.chain_id, operation.account, cost),
						new Promise<FundingNeeded | null>((resolve) => {
							timer = setTimeout(() => resolve(null), PRECHECK_TIMEOUT_MS);
						})
					]);
					return { type: 'pre_check', funding: funding ? toWireFunding(funding) : null };
				} finally {
					if (timer) clearTimeout(timer);
				}
			}

			case 'attempt_sponsorship': {
				const funding: FundingNeeded = {
					depositAddress: operation.funding.deposit_address,
					safeAddress: operation.funding.safe_address,
					chainId: operation.funding.chain_id,
					nativeSym: operation.funding.native_symbol,
					thresholdWei: fromWireWei(operation.funding.threshold_wei),
					recommendedWei: fromWireWei(operation.funding.recommended_wei),
					currentBalance: fromWireWei(operation.funding.current_balance_wei),
					recommendedFormatted: '',
					currentFormatted: ''
				};
				const silent = await attemptSilentSponsorship(funding, { force: operation.force });
				if (silent.outcome === 'funded')
					return { type: 'sponsorship', outcome: { type: 'funded' } };
				if (silent.outcome === 'confirming') {
					return { type: 'sponsorship', outcome: { type: 'confirming' } };
				}
				return {
					type: 'sponsorship',
					outcome: { type: 'denied', reason: silent.denialReason ?? null }
				};
			}

			case 'sign_and_submit': {
				const account: SigningAccount = { id: operation.credential_id };
				const answered = new AbortController();
				receiptWaits.get(operation.id)?.abort();
				receiptWaits.set(operation.id, answered);
				const hooks: DAppSubmitHooks = {
					claim: (phase, submit) => ports.askerLive(operation.id, phase, submit),
					ceremony: (stage) => ports.ceremony(operation.id, stage),
					// What is LEFT of the page's answer window since the approval
					// (spec 082 RA12) — never a fresh full wait after a slow submit.
					receiptWaitMs: () => {
						const approvedAt = ports.approvedAtMs(operation.id);
						return dappReceiptWaitMs(approvedAt === null ? 0 : Date.now() - approvedAt);
					},
					// RJ1: the record before the bytes.
					writeAhead: (userOpHash, submitBlock) =>
						writeAhead(operation.id, userOpHash, submitBlock),
					answered: answered.signal
				};
				try {
					const result = await handleDAppRequest(
						{
							id: operation.id,
							method: operation.method,
							params: parseParams(operation.params_json),
							origin: ports.requestOrigin(operation.id)
						},
						account,
						operation.address,
						operation.chain_id,
						operation.max_fee_per_gas != null ? fromWireWei(operation.max_fee_per_gas) : undefined,
						// The op on its way — accepted, or may have been sent — is a fact
						// the core needs BEFORE this promise settles: §4's durable record
						// and the tracker hand-off are written from it.
						(hash, maybeSent, submitBlock) =>
							ports.opSubmitted(operation.id, hash, maybeSent, submitBlock),
						operation.gas_fee_token,
						operation.quoted_fee
							? {
									amount: fromWireWei(operation.quoted_fee.amount),
									recipient: operation.quoted_fee.recipient,
									// The speed the displayed fee was priced at (spec 069), when
									// the approve carried one.
									tier: wireTier(operation.quoted_fee.tier)
								}
							: undefined,
						// The never-unlimited gate is the CORE's on this path: `proceed_submit`
						// (`sign_request.rs`) ran `enforce_no_unlimited` over this request and
						// over every `wallet_sendCalls` leg before it emitted this effect, and
						// refuses by failing the inflight request. Letting the TS copy decide
						// it again would put one safety mandate in two implementations that
						// nothing keeps in step. Native never reaches this `.web.ts` module and
						// so keeps its own TS guard (Hermes has no wasm) — see SubmitGuardOwner.
						'core',
						hooks
					);
					// EIP-5792's answer IS the op hash (the batch id): remembered like
					// a late receipt's, for the extension's receipt translation.
					if (operation.method === 'wallet_sendCalls' && typeof result === 'string') {
						answeredOps.set(result.toLowerCase(), operation.chain_id);
					}
					return {
						type: 'submit',
						outcome: {
							type: 'succeeded',
							result: typeof result === 'string' ? result : String(result ?? '')
						},
						now_ms: Date.now()
					};
				} catch (error) {
					// Accepted (or may have been sent), receipt late (issue 262, spec 082
					// RA2): the page gets the op hash, and the tracker alone closes it.
					if (error instanceof DAppReceiptPendingError) {
						answeredOps.set(error.userOpHash.toLowerCase(), operation.chain_id);
						return {
							type: 'submit',
							outcome: { type: 'receipt_pending', user_op_hash: error.userOpHash },
							now_ms: Date.now()
						};
					}
					return {
						type: 'submit',
						outcome: await submitOutcomeOf(operation, error),
						now_ms: Date.now()
					};
				} finally {
					clearances.delete(operation.id);
					if (receiptWaits.get(operation.id) === answered) receiptWaits.delete(operation.id);
				}
			}

			case 'clear_to_post': {
				// The write-ahead record is on disk (spec 082 RJ1): the POST may go.
				// Only for the op it names — a clearance for another is dropped.
				const waiting = clearances.get(operation.id);
				if (waiting && waiting.userOpHash.toLowerCase() === operation.user_op_hash.toLowerCase()) {
					clearances.delete(operation.id);
					waiting.clear();
				}
				return { type: 'responded' };
			}

			case 'delete_record': {
				// A write-ahead record whose op was proven never sent (spec 082 RJ1):
				// it goes, the Activity row with it. Serialised with its own write.
				await serialised(operation.record_id, () =>
					deleteTransaction(operation.record_id).catch((e) => {
						console.warn('[sign_request] Failed to delete record:', e);
					})
				);
				ports.recordsWritten();
				return { type: 'record_updated' };
			}

			case 'persist_record': {
				const record = operation.record;
				const sim = ports.assetSim();
				const row = buildSigningRecord({
					method: record.method,
					params: parseParams(record.params_json),
					result: record.result,
					from: record.from,
					chainId: record.chain_id,
					dappOrigin: record.dapp_origin,
					nowMs: record.now_ms,
					status: record.status,
					userOpHash: record.user_op_hash,
					assetChanges: sim ? serializeAssetSim(sim) : undefined,
					intent: record.intent ?? undefined,
					// Spec 082 T182: a may-have-been-sent op is one after a reload
					// too — its record says so, and where its landing check starts.
					maybeSent: record.maybe_sent,
					submitBlock: record.submit_block
				});
				// The core owns the id (`dapp-<ms>-tx|typed|msg`); the builder derives
				// the same one from the same clock, but the core's is authoritative
				// because the patch below is keyed on it.
				await serialised(record.record_id, () =>
					saveTransaction({ ...row, id: record.record_id }).catch((e) => {
						console.warn('[sign_request] Failed to save record:', e);
					})
				);
				// The row shows within one poke, not the next 10–30 s tick (RG3).
				ports.recordsWritten();
				return { type: 'record_persisted' };
			}

			case 'update_record': {
				const close = operation.close;
				// `admitted`: the relay took the write-ahead op (spec 082 RJ1) — it is
				// no longer "may have been sent", and stays pending for the tracker.
				const patch =
					close.type === 'confirmed'
						? ({ status: 'confirmed', txHash: close.tx_hash } as const)
						: close.type === 'admitted'
							? ({ maybeSent: false } as const)
							: ({ status: 'failed' } as const);
				await serialised(operation.record_id, () =>
					updateTransaction(operation.record_id, patch).catch((e) => {
						console.warn('[sign_request] Failed to patch record:', e);
					})
				);
				ports.recordsWritten();
				// Listing what this tx silently delivered is NOT done here any more:
				// `tx_tracker` polls the receipt it hands off at `OpSubmitted` and
				// routes the AUTHENTIC logs to `token_trust`'s `ReceiptLogsConfirmed`,
				// which is the single legal auto-add entry point. Doing it here as well
				// would make web's custom-token list have two writers.
				return { type: 'record_updated' };
			}

			case 'switch_active_account': {
				await ports.switchActiveAccount(operation.index);
				return { type: 'account_switched' };
			}
		}
	}

	/**
	 * A submit that ended without an op on its way, as the core's typed
	 * outcome. Nothing here reads the relay's words to decide anything: the
	 * relay's refusal was read by the core (`submit_step` → `NotSent`, spec 082
	 * RA1). What is left is the one composition the core cannot do — an
	 * underfunded refusal needs the gas account's live facts for the funding
	 * view (`dapp-connection.tsx:807-874`).
	 */
	async function submitOutcomeOf(
		operation: Extract<SignEffect['operation'], { type: 'sign_and_submit' }>,
		error: unknown
	): Promise<Extract<SignShellResult, { type: 'submit' }>['outcome']> {
		// Nothing was signed or sent, and nobody is there to answer (RB5).
		if (error instanceof AskerGoneError) return { type: 'asker_gone' };
		// The record was not written in time, so nothing was POSTed (RJ1): not
		// sent, in the core's fixed words — never a refusal.
		if (error instanceof WriteAheadTimeoutError) {
			return { type: 'failed', message: userOpNotSentDetail(), refused: false };
		}
		if (error instanceof PasskeyError && error.kind === 'cancelled') {
			// Never an error, never a response, never a durable 'rejected' (⑧).
			return { type: 'passkey_cancelled' };
		}
		const message = (error as { message?: string } | null)?.message ?? 'Signing failed';
		// The relay's underfunded refusal (the core's reading), or a pre-submit
		// estimate that met the same refusal in the relay's own words.
		const underfundedRefusal =
			error instanceof UserOpNotSentError
				? error.rejection === 'bundler_underfunded'
				: parseBundlerUnderfunded(message) !== null;
		const underfunded = underfundedRefusal ? parseBundlerUnderfunded(message) : null;
		if (underfundedRefusal) {
			try {
				clearBundlerCache(operation.chain_id, operation.address);
				const info = await fetchBundlerAccountInfo(operation.chain_id, operation.address);
				// Prefer live account info; fall back to the values parsed from the error.
				const depositAddress = info?.depositAddress || underfunded?.depositAddress;
				if (depositAddress) {
					const currentBalance = info?.spendableBalance ?? underfunded?.spendableWei ?? 0n;
					const thresholdWei =
						underfunded?.requiredWei ?? currentBalance + REACTIVE_THRESHOLD_MARGIN_WEI;
					const nativeSym =
						info?.nativeSym ??
						(underfunded?.asset === 'pathUSD' ? 'pathUSD' : nativeSymbol(operation.chain_id));
					return {
						type: 'underfunded',
						message,
						funding: {
							deposit_address: depositAddress,
							safe_address: operation.address,
							chain_id: operation.chain_id,
							native_symbol: nativeSym,
							threshold_wei: (
								(underfunded && underfundedRequiredWei(underfunded)) ??
								thresholdWei
							).toString(),
							recommended_wei: recommendedFundingWei(thresholdWei, currentBalance).toString(),
							current_balance_wei: currentBalance.toString()
						}
					};
				}
			} catch {
				/* fall through to the generic failure */
			}
		}
		// A NotSent op's words are the relay's refusal, or the core's fixed
		// "relay unreachable; nothing was sent" — never the pool's raw text. A
		// refusal by the relay (anything but "relayer unavailable") is said as
		// one (spec 082 RJ3): the page is told the network refused it, and the
		// sheet offers no Retry — trying again sends the same refused op.
		return { type: 'failed', message, refused: isRelayRefusal(error) };
	}

	function toFailure(effect: SignEffect, error: unknown): SignShellResult {
		const operation = effect.operation;
		switch (operation.type) {
			case 'send_response':
				// `sendResponse` is fire-and-forget on every transport; a throw inside
				// one is that transport's problem, never the pipeline's.
				return { type: 'responded' };
			case 'check_bundler_funding':
				// The TS `try { … } catch { /* proceed to submit */ }`: an unreachable
				// bundler must not block the approve — the post-submit classification
				// above is the safety net.
				return { type: 'pre_check', funding: null };
			case 'attempt_sponsorship':
				// Proactive: the same catch, so a failed grant proceeds to submit.
				// Reactive (force): the TS fell to its generic error; opening the
				// top-up sheet instead keeps the request answerable by the user and is
				// the only outcome this operation can express.
				return operation.force
					? { type: 'sponsorship', outcome: { type: 'denied', reason: null } }
					: { type: 'sponsorship', outcome: { type: 'funded' } };
			case 'sign_and_submit':
				// `execute` classifies its own failures; this is the defensive tail.
				return {
					type: 'submit',
					outcome: {
						type: 'failed',
						message: (error as { message?: string } | null)?.message ?? 'Signing failed',
						refused: isRelayRefusal(error)
					},
					now_ms: Date.now()
				};
			case 'clear_to_post':
				return { type: 'responded' };
			case 'delete_record':
				return { type: 'record_updated' };
			case 'persist_record':
				// Best effort, exactly like the TS inner `.catch(console.warn)` — and
				// the core must still be told, or §4's respond step never fires.
				return { type: 'record_persisted' };
			case 'update_record':
				return { type: 'record_updated' };
			case 'switch_active_account':
				// A failed switch still has to ack, or the approval surface stays shut
				// forever (§12.1.6 gates on this).
				return { type: 'account_switched' };
		}
	}

	return { execute, toFailure };
}
