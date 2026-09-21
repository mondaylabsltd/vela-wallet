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
 * - **Every wording regex.** `PasskeyErrorCode.CANCELLED`,
 *   `parseBundlerUnderfunded` and the `fetchBundlerAccountInfo` composition are
 *   classified HERE into typed `SignSubmitOutcome` variants; the core only ever
 *   sees the variant.
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
import { saveTransaction, updateTransaction } from '$lib/services/records';
import { serializeAssetSim } from '$lib/services/sim/tx-simulation';
import { DAppReceiptPendingError, handleDAppRequest } from '$lib/services/dapp-submit';
import type { SigningAccount } from '$lib/services/dapp-submit';

import type { SignFundingNeeded } from '$lib/core/generated/SignFundingNeeded';
import type { SignShellResult } from '$lib/core/generated/SignShellResult';
import type { SignEffect, SignShellPorts } from './sign-types';
import { signErrorMessage } from './sign-types';
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

export function createSignExecutor(ports: SignShellPorts) {
	/**
	 * Writes in flight per `record_id`. `saveTransaction` and `updateTransaction`
	 * are separate round trips over the same key, and the core issues the patch
	 * without waiting for the insert (it only waits for the ack it needs for §4).
	 * Chaining them here is the `await pendingSave` of the TS path.
	 */
	const writes = new Map<string, Promise<void>>();

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
				// F2: the transport that OWNS the request, resolved from the id the
				// core carried — never a shared ref.
				const transport = ports.transportFor(operation.transport_id);
				if (operation.payload.type === 'ok') {
					transport?.sendResponse(operation.id, operation.payload.result);
				} else {
					// The core owns the code and the kind; the words are this side's, and
					// `signErrorMessage` reproduces the exact string the TS provider sent
					// for each one (a `submit_failed` detail IS `err.message`, verbatim).
					transport?.sendResponse(operation.id, undefined, {
						code: operation.payload.code,
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
				try {
					const result = await handleDAppRequest(
						{
							id: operation.id,
							method: operation.method,
							params: parseParams(operation.params_json)
						},
						account,
						operation.address,
						operation.chain_id,
						operation.max_fee_per_gas != null ? fromWireWei(operation.max_fee_per_gas) : undefined,
						// The bundler accepting the op is a fact the core needs BEFORE this
						// promise settles — §4's durable record is written from it.
						(hash: string) => ports.opSubmitted(operation.id, hash),
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
						'core'
					);
					return {
						type: 'submit',
						outcome: {
							type: 'succeeded',
							result: typeof result === 'string' ? result : String(result ?? '')
						},
						now_ms: Date.now()
					};
				} catch (error) {
					// Accepted, receipt late (issue 262): the page still gets the op hash,
					// but the core must not confirm the record — the tracker settles it.
					if (error instanceof DAppReceiptPendingError) {
						return {
							type: 'submit',
							outcome: { type: 'receipt_pending', user_op_hash: error.userOpHash },
							now_ms: Date.now()
						};
					}
					return {
						type: 'submit',
						outcome: await classifySubmit(operation, error),
						now_ms: Date.now()
					};
				}
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
					intent: record.intent ?? undefined
				});
				// The core owns the id (`dapp-<ms>-tx|typed|msg`); the builder derives
				// the same one from the same clock, but the core's is authoritative
				// because the patch below is keyed on it.
				await serialised(record.record_id, () =>
					saveTransaction({ ...row, id: record.record_id }).catch((e) => {
						console.warn('[sign_request] Failed to save record:', e);
					})
				);
				return { type: 'record_persisted' };
			}

			case 'update_record': {
				const close = operation.close;
				const patch =
					close.type === 'confirmed'
						? ({ status: 'confirmed', txHash: close.tx_hash } as const)
						: ({ status: 'failed' } as const);
				await serialised(operation.record_id, () =>
					updateTransaction(operation.record_id, patch).catch((e) => {
						console.warn('[sign_request] Failed to patch record:', e);
					})
				);
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
	 * The submit-failure classification the core deliberately does not own: every
	 * regex, and the live-account-info composition that turns "underfunded" into
	 * the facts the funding view needs (`dapp-connection.tsx:807-874`).
	 */
	async function classifySubmit(
		operation: Extract<SignEffect['operation'], { type: 'sign_and_submit' }>,
		error: unknown
	): Promise<Extract<SignShellResult, { type: 'submit' }>['outcome']> {
		const err = error as { code?: unknown; message?: string } | null | undefined;
		if (error instanceof PasskeyError && error.kind === 'cancelled') {
			// Never an error, never a response, never a durable 'rejected' (⑧).
			return { type: 'passkey_cancelled' };
		}
		const message = err?.message ?? 'Signing failed';
		const underfunded = parseBundlerUnderfunded(message);
		if (underfunded) {
			try {
				clearBundlerCache(operation.chain_id, operation.address);
				const info = await fetchBundlerAccountInfo(operation.chain_id, operation.address);
				// Prefer live account info; fall back to the values parsed from the error.
				const depositAddress = info?.depositAddress || underfunded.depositAddress;
				if (depositAddress) {
					const currentBalance = info?.spendableBalance ?? underfunded.spendableWei ?? 0n;
					const thresholdWei =
						underfunded.requiredWei ?? currentBalance + REACTIVE_THRESHOLD_MARGIN_WEI;
					const nativeSym =
						info?.nativeSym ??
						(underfunded.asset === 'pathUSD' ? 'pathUSD' : nativeSymbol(operation.chain_id));
					return {
						type: 'underfunded',
						message,
						funding: {
							deposit_address: depositAddress,
							safe_address: operation.address,
							chain_id: operation.chain_id,
							native_symbol: nativeSym,
							threshold_wei: (underfundedRequiredWei(underfunded) ?? thresholdWei).toString(),
							recommended_wei: recommendedFundingWei(thresholdWei, currentBalance).toString(),
							current_balance_wei: currentBalance.toString()
						}
					};
				}
			} catch {
				/* fall through to the generic failure */
			}
		}
		return { type: 'failed', message };
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
						message: (error as { message?: string } | null)?.message ?? 'Signing failed'
					},
					now_ms: Date.now()
				};
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
