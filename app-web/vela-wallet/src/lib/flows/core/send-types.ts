// Ported from src/services/wallet-state-core/send-types.ts @ f9bcb278 — RN seams
// rewritten to the web modules; the fee codecs moved to `send-estimates.ts` and
// the amount codec to `services/amount-codec.ts`; logic verbatim.
/**
 * Platform-neutral types and pure wire codecs for the `send` core (spec 017,
 * group G12 — `rust/crates/vela-core/src/app/send.rs`).
 *
 * Standalone for the reason `sign-types.ts` states: the native stub
 * (`send-session.ts`) needs these declarations, and importing them from a
 * `.web` module would drag the web-only service graph into the native bundle.
 *
 * The codecs live here rather than in the executor because BOTH sides need
 * them: the executor puts an estimate on the wire, and the controller takes one
 * off it (and puts one back when `GasFeeCard` re-quotes). Every one of them is a
 * fund-safety codec — a truncated `total_wei` prices the confirm screen wrong,
 * and a decimal amount handed to `buildExecuteCallData` (which reads HEX) would
 * move a completely different number.
 */

import { chainName } from '$lib/services/networks';
import { tokenChainId, tokenId, tokenLogoURLs, type APIToken } from '$lib/services/tokens-model';

import type { FeeCall } from '$lib/core/generated/FeeCall';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { SendAlertKind } from '$lib/core/generated/SendAlertKind';
import type { SendFeeOutcome } from '$lib/core/generated/SendFeeOutcome';
import type { SendOperation } from '$lib/core/generated/SendOperation';
import type { SendReceiptOutcome } from '$lib/core/generated/SendReceiptOutcome';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import type { SessionOptions } from '$lib/core/types';

/** One request from the core, carrying the id it will be answered by. */
export type SendEffect = { id: number; operation: SendOperation };

/**
 * The re-entry points and shell-owned surfaces the core deliberately never
 * holds. Everything here is either a mid-flight dispatch (a fact the core must
 * hear BEFORE the operation it belongs to resolves) or a surface that needs the
 * React tree — `t` for the alert wording, the router for `Close`.
 */
export interface SendShellPorts {
	/** `fetchTokens`'s `onProgress` — a progressive chain chunk, display only. */
	tokensPartial(tokens: SendToken[]): void;
	/**
	 * The FULL `APIToken` rows behind a chunk/answer. The core carries the slice
	 * it needs; the shell keeps the originals so `TokenSelector` still renders the
	 * name/logo the API gave, with a stable object identity per row.
	 */
	tokensFetched(tokens: APIToken[]): void;
	/**
	 * The passkey credential that may sign for this Safe address, or `null`.
	 *
	 * `SubmitUserOp` carries the account ADDRESS and its public key — never the
	 * credential id, which is a shell fact (`activeAccount.id`). Resolving it here
	 * lets the port fail closed when the active account has moved on: the wallet
	 * must never sign with a credential that does not belong to the account the
	 * core built the batch for.
	 */
	credentialId(address: string): string | null;
	/**
	 * The stored public key `LoadAccountCredential` just read.
	 *
	 * The core keeps its own copy (it is the estimate's `public_key_hex`), but the
	 * confirm screen's embedded fee card re-quotes on its own and needs it to
	 * build a real initCode for an undeployed Safe — which is exactly what
	 * `prefetchedAccount.current?.publicKeyHex` was for. A mirror, never a second
	 * writer: nothing decides on it here.
	 */
	credentialLoaded(publicKeyHex: string | null): void;
	/** The passkey sheet opened inside `SubmitUserOp` → `Event::SigningStarted`. */
	signingStarted(): void;
	/** Receipt convergence from the tracker seam → `Event::ReceiptUpdate`. */
	receiptUpdate(userOpHash: string, outcome: SendReceiptOutcome): void;
	/** `showAlert` — the core owns the site, the shell owns the words. */
	alert(kind: SendAlertKind): void;
	/** `router.back()` out of the Send flow. */
	close(): void;
	/**
	 * Price an operation, and answer with the settled quote.
	 *
	 * `EstimateFee` used to be one call to `estimateTransactionFee` here, while
	 * the confirm screen's fee card ran its own quote and its own per-asset math
	 * on top. Two writers of one number — the shape four attempts at this
	 * integration foundered on. The operation is now asked of the SAME live
	 * `fee_policy` session the card renders, so the quote the core pre-checks
	 * against, the quote on screen and the quote that is signed are one object
	 * with one owner.
	 *
	 * A port rather than a service import because that session belongs to the
	 * React tree: it is created by the screen, disposed with it, and must not be
	 * reachable from a module.
	 */
	feeQuote(request: SendFeeQuoteRequest): Promise<SendFeeOutcome>;
}

/** What `EstimateFee` asks for, in the shell's own vocabulary. */
export interface SendFeeQuoteRequest {
	chainId: number;
	account: string;
	/** The real calls, WITHOUT the fee leg — `fee_policy` appends its own. */
	calls: FeeCall[];
	/** `null` = native. A quote parameter: it changes the operation being priced. */
	feeToken: string | null;
	/**
	 * The passkey public key `LoadAccountCredential` read for THIS account, so the
	 * initCode the simulation builds is the one the submitted op will carry.
	 * Carried on the request rather than mirrored on the session: one fact, one
	 * copy, and it cannot go stale across an account switch.
	 */
	publicKeyHex: string | undefined;
	/**
	 * How fast this operation should be (spec 068). The shell's to decide —
	 * the core asks what an operation costs, not how urgent it is — so this is
	 * filled in by the surface that owns the speed control, and omitting it
	 * keeps the historical `fast`.
	 */
	tier?: FeeTier;
}

export type SendSessionOptions = SessionOptions<SendView> & {
	ports: SendShellPorts;
};

// ---------------------------------------------------------------------------
// Token codec
// ---------------------------------------------------------------------------

/** The slice of `APIToken` the core needs, `chain_id` pre-resolved. */
export function toSendToken(token: APIToken): SendToken {
	return {
		network: token.network,
		chain_id: tokenChainId(token),
		symbol: token.symbol,
		balance: token.balance,
		decimals: token.decimals,
		token_address: token.tokenAddress,
		price_usd: token.priceUsd,
		logo_urls: tokenLogoURLs(token),
		spam: token.spam
	};
}

/** `tokenId()` for a wire token — the key the shell's originals are held under. */
export function sendTokenId(wire: SendToken): string {
	return `${wire.network}_${wire.token_address ?? 'native'}_${wire.symbol}`;
}

/**
 * A wire token with no original behind it: the locked-request placeholders
 * (`synthNativeToken` / `synthErc20Token`), which the core mints itself. Named
 * after the symbol exactly as those helpers did.
 */
export function synthApiToken(wire: SendToken): APIToken {
	return {
		network: wire.network,
		chainName: chainName(wire.chain_id),
		symbol: wire.symbol,
		balance: wire.balance,
		decimals: wire.decimals,
		logo: wire.logo_urls[0] ?? null,
		name: wire.symbol,
		tokenAddress: wire.token_address,
		priceUsd: wire.price_usd,
		spam: wire.spam
	};
}

/** Index a token list by `tokenId()` so a view's wire rows resolve to originals. */
export function indexTokens(tokens: APIToken[]): Map<string, APIToken> {
	const index = new Map<string, APIToken>();
	for (const token of tokens) index.set(tokenId(token), token);
	return index;
}

// ---------------------------------------------------------------------------
// Call codec
// ---------------------------------------------------------------------------

// The fund-safety codec lives on its own, with its own vectors (spec 026 D25):
// the core states base units as DECIMAL strings and every `safe-transaction.ts`
// consumer reads HEX. Re-exported here so the ported call sites are unchanged.
export { decimalToHex, fromWireAmount, toShellCall } from '$lib/services/amount-codec';
