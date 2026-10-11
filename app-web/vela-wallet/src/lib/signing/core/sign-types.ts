// Ported from src/services/wallet-state-core/sign-types.ts @ f9bcb278 — the
// transport becomes a one-method seam (026 has no real transport; 027 does).
/**
 * Platform-neutral types for the `sign_request` core (spec 017, group G11).
 *
 * Standalone for the reason `session-types.ts` states: the native stub
 * (`sign-session.ts`) needs these declarations, and importing them from a
 * `.web` module would drag the web-only service graph into the native bundle.
 * One module per machine also keeps parallel integration waves off each other's
 * files.
 */

import type { SignErrorKind } from '$lib/core/generated/SignErrorKind';
import type { SignErrorNotice } from '$lib/core/generated/SignErrorNotice';
import type { SignOperation } from '$lib/core/generated/SignOperation';
import type { SignView } from '$lib/core/generated/SignView';
import type { SessionOptions } from '$lib/core/types';

import type { AssetSimResult } from '$lib/services/sim/tx-simulation';

/** One request from the core, carrying the id it will be answered by. */
export type SignEffect = { id: number; operation: SignOperation };

/**
 * The live objects and re-entry points the core deliberately never holds.
 *
 * The core speaks `transport_id` and nothing else about transports (F2: a
 * response goes to the transport that OWNS the request, never a shared ref);
 * the shell keeps the id → instance table. `opSubmitted` is the one mid-flight
 * re-entry: the bundler accepting an op is a fact the core must hear BEFORE
 * `SignAndSubmit` resolves, so the durable record can precede anything the dApp
 * can poll (§4).
 */
/**
 * What a transport must be able to do for the core: answer the request it
 * delivered. The web has no real transport yet — 027 brings WalletPair and the
 * remote-inject relay — so the only implementation in 026 is the in-page test
 * requester. Declared here rather than imported from a transport module, so
 * the seam is the contract and not one implementation's shape.
 */
export interface SignResponder {
	/**
	 * `error.kind` is the core's own vocabulary, passed through because a
	 * transport sometimes has to act on WHICH refusal this was — a window
	 * showing a blocked request (spec 081) stays open to explain it, while every
	 * other answer closes it.
	 *
	 * `opHash` rides along with an on-chain request answered by its operation
	 * hash (a receipt still pending, or an op that may have been sent — spec 082
	 * RF3): the extension remembers it so the page's later receipt reads for that
	 * hash can be translated to the real transaction.
	 */
	sendResponse(
		id: string,
		result?: unknown,
		error?: { code: number; message: string; kind?: SignErrorKind },
		opHash?: { chainId: number }
	): void;
	/**
	 * Is the asker still there to be answered (spec 082 RB5)? Asked at the three
	 * points a request becomes harder to take back — approve, before the passkey
	 * (`sign`), and between the write-ahead's clearance and the relay POST
	 * (`submit`). Absent on a transport that cannot lose its asker (the wallet's
	 * own page): then the answer is always yes.
	 *
	 * The `submit` claim carries the operation's hash and chain (spec 082 RJ2):
	 * from then on a surface that goes before answering gets its page told that
	 * hash — it may have been sent — never 4900.
	 */
	claim?(id: string, phase: ClaimPhase, submit?: SubmitClaim): Promise<boolean>;
}

/** The three claim points of a request (spec 082 RB5, contract §14). */
export type ClaimPhase = 'approve' | 'sign' | 'submit';

/**
 * What a `submit` claim carries (spec 082 RJ2): the operation's hash and
 * chain, sent after the write-ahead's clearance and right before the POST, so
 * every request the worker answers by that hash has a durable record behind it.
 */
export type SubmitClaim = { opHash: string; chainId: number };

/**
 * The asker of request `id` was gone when the wallet checked (spec 082 RB5):
 * nothing was signed or sent, and there is nobody to answer. The executor maps
 * it to the core's `asker_gone` outcome.
 */
export class AskerGoneError extends Error {
	constructor(readonly phase: ClaimPhase) {
		super(`The page that asked is gone (${phase})`);
		this.name = 'AskerGoneError';
	}
}

/**
 * The claim through the transport that owns a request (RB5): its own `claim`
 * when it has one; a transport that cannot lose its asker — the wallet's own
 * page — is always live, so nothing changes for it. A claim that throws is
 * not a yes: a false "no" costs one re-approval, a false "yes" a signature
 * nobody is there to receive.
 */
export async function claimThrough(
	transport: SignResponder | undefined,
	id: string,
	phase: ClaimPhase,
	submit?: SubmitClaim
): Promise<boolean> {
	if (!transport?.claim) return true;
	try {
		return await transport.claim(id, phase, submit);
	} catch {
		return false;
	}
}

/**
 * The write-ahead's clearance did not come in time (spec 082 RJ1): the
 * record of the op is not known to be on disk, so nothing was POSTed. The
 * op was provably not sent; the executor reports it as such.
 */
export class WriteAheadTimeoutError extends Error {
	constructor(readonly userOpHash: string) {
		super(`The record of ${userOpHash.slice(0, 10)}… was not written in time; nothing was sent`);
		this.name = 'WriteAheadTimeoutError';
	}
}

export interface SignShellPorts {
	/** The transport that owns a request. `null` when it is already gone. */
	transportFor(transportId: string): SignResponder | null;
	/**
	 * `onSubmitted(hash)` — dispatches `Event::OpSubmitted` mid-`SignAndSubmit`.
	 * `maybeSent`: the relay's reply was lost and `userOpHash` is the local hash
	 * (spec 082 RA2/RA3); `submitBlock`: the head read before the first POST.
	 */
	opSubmitted(id: string, userOpHash: string, maybeSent: boolean, submitBlock: number | null): void;
	/**
	 * The write-ahead (spec 082 RJ1): the op for request `id` is signed and
	 * hashed and nothing has been POSTed — dispatches `Event::OpSigned`. The
	 * core writes the record and answers `ClearToPost` once it is on disk.
	 */
	opSigned(id: string, userOpHash: string, submitBlock: number | null): void;
	/**
	 * `true` while the asker of request `id` is still there (spec 082 RB5):
	 * the owning transport's `claim`, or `true` when that transport has none.
	 * A `submit` claim carries the op's hash and chain (RJ2).
	 */
	askerLive(id: string, phase: ClaimPhase, submit?: SubmitClaim): Promise<boolean>;
	/**
	 * The passkey (or Trusted Signer) prompt opened / returned a signature for
	 * request `id` — `CeremonyStarted` / `CeremonyDone` (spec 082 RA9), so the
	 * sheet's words follow the real stage instead of guessing it.
	 */
	ceremony(id: string, stage: 'started' | 'done'): void;
	/**
	 * When the person approved request `id` (epoch ms) — the start of the dApp's
	 * answer window (`dappReceiptWaitMs`, spec 082 RA12). `null` if unknown.
	 */
	approvedAtMs(id: string): number | null;
	/**
	 * The origin that sent request `id` — what the Trusted Signer's page shows
	 * as the requester (spec 071). `SignAndSubmit` does not carry it, and the
	 * request is still the one on the sheet while it is being signed.
	 */
	requestOrigin(id: string): string;
	/**
	 * The sign-time simulation the last approve carried. Deliberately NOT core
	 * state: `assetChanges` is a presentation blob the record stores for the
	 * Connections-panel replay, and the core would only be forwarding it.
	 */
	assetSim(): AssetSimResult | null | undefined;
	/**
	 * §12.1.6 — switch the active account to the granted one.
	 *
	 * `index` is a position in the list this machine was given, and it is
	 * consumed in the SESSION's domain, where an out-of-range index is a silent
	 * whole no-op. The implementation must therefore feed from the session's own
	 * rows and VERIFY the switch landed before it resolves: resolving is what
	 * opens the approval surface, and resolving on a switch that did not happen
	 * is a signature from an account the origin was never granted.
	 *
	 * It no longer waits for React. The signer the core hands to `SignAndSubmit`
	 * and `CheckBundlerFunding` comes from the core's own
	 * `accounts`/`active_index` (§12.1.6 step 2), so there is nothing left on the
	 * sign path for a React commit to be ahead of.
	 */
	switchActiveAccount(index: number): Promise<void>;
	/**
	 * A record was written or patched — the Activity feed re-reads the store
	 * (spec 082 RG3: `ReconcileCompleted{resolved_count: 1}`), so a dApp row
	 * appears within one poke instead of the next 10–30 s tick.
	 */
	recordsWritten(): void;
}

export type SignRequestSessionOptions = SessionOptions<SignView> & {
	ports: SignShellPorts;
	/**
	 * The clock behind the core's timers (PR 3: the simulation verdict's
	 * deadline) — resolve after `ms`. Absent: `setTimeout`. A test passes one
	 * it can stop.
	 */
	timer?: (ms: number) => Promise<void>;
};

/**
 * The core's `CHAIN_MISMATCH_MESSAGE` (089, `sign_request.rs`): a request that
 * names a chain other than the site's is refused -32602 with this detail,
 * before any sheet. Named in the page's message — the developer's one clue
 * that the site must switch chains first. Pinned to the real core in
 * `chain-mismatch.test.ts`.
 */
export const CHAIN_MISMATCH_MESSAGE = 'chainId does not match the connected chain';

/**
 * The words for the core's semantic error vocabulary — the core owns the code
 * and the kind, the shell owns the copy (that is the stated contract on
 * `SignErrorKind`). Every string here is the one the TypeScript provider
 * produced for the same situation, so neither the dApp's `message` nor the
 * sheet's error card changes wording.
 *
 * Pure and dependency-free so both the executor (which puts it on the wire)
 * and the provider (which renders it) can read it, on either platform.
 *
 * A kind with no provider wording to keep is not restated here: spec 102's
 * `venue_blocked` reads the core's own words (`signErrorWords`, the core's
 * `dapp_rpc::sign_error_words`), which the executor asks for itself.
 */
export function signErrorMessage(notice: SignErrorNotice): string {
	const detail = notice.detail ?? undefined;
	switch (notice.kind) {
		case 'user_rejected':
			return 'User rejected';
		case 'wallet_switched_chains':
			return 'Cancelled: the wallet switched chains';
		case 'unsupported_chain':
			// `assertChainSupported`'s wording minus the id, which the core does not
			// carry on the refusal (it refuses before any UI, so nothing displays it).
			return 'Unsupported chain. Add this network in wallet settings.';
		case 'unauthorized_account':
			return 'The requested account is no longer authorized';
		case 'invalid_params':
			// The chain-switch refusal carries no detail; the approve-path ones do.
			if (detail === undefined) return 'Invalid params: missing chainId';
			if (detail === CHAIN_MISMATCH_MESSAGE) return `Invalid params: ${CHAIN_MISMATCH_MESSAGE}`;
			return detail === 'no calls provided' ? 'No calls provided' : 'Invalid params';
		case 'unsupported_capability':
			return `Unsupported non-optional capabilities: ${detail ?? ''}`;
		case 'unlimited_approval':
			// Since 2026-09-26 an unlimited approval goes out once the approval
			// screen showed it and it was kept; this is the screen not having.
			return `Blocked: the wallet refused an unlimited approval its approval screen did not show (${detail ?? ''}).`;
		case 'self_call_blocked':
			// Spec 081: refused by the wallet, not by the person. The sheet
			// explains it in the reader's language; this is the dApp's copy.
			return `Blocked: this request would change who controls the wallet (${detail ?? ''}).`;
		case 'funding_cancelled':
			return 'Gas account funding cancelled';
		case 'stale_fee_quote':
			return 'The quoted fee expired. Review the request again.';
		case 'submit_failed':
		default:
			return detail ?? 'Signing failed';
	}
}

/** Narrowing helper kept beside the words it belongs to. */
export type SignErrorKindName = SignErrorKind;
