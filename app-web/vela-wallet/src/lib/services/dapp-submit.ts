/**
 * The dApp SUBMIT path — how a signing request becomes a signature or a UserOp.
 *
 * Ported from src/hooks/use-dapp-signing.ts @ f9bcb278 (a hook in name only:
 * it never used React). Web deltas: the kernels import replaces the vela-core
 * facade (static — the Expo dynamic imports were a bundle-splitting hedge),
 * the passkey call goes through `signChallenge` (the key the account signed in
 * with, or every founding credential in the allow-list for a record from before
 * that), the stored account comes from onboarding storage, and the
 * public-key-index fallback for an UNKNOWN account is gone: a web session is
 * always a stored account, so a missing one is an error, not a lookup.
 * `guardOwner` semantics are unchanged: on the core-driven path the core has
 * already run `enforce_no_unlimited`; the default stays the guarded value.
 */
import {
	dappRequestCalls,
	derSignatureToRaw,
	fromHex,
	hashTypedData,
	typedDataDocument,
	keccak256,
	stripHexPrefix,
	toHex,
	verifySafeWebAuthn
} from '$lib/core/kernels';
import type { TypedData } from '$lib/core/kernels';
/** The account a request is signed for — only its founding credential id is read. */
export interface SigningAccount {
	id: string;
}

import type { Assertion } from '$lib/onboarding/core/passkey';
import { signChallenge, type ChallengeSigner } from '$lib/signing/sign-challenge';
import { AskerGoneError, type ClaimPhase, type SubmitClaim } from '$lib/signing/core/sign-types';
import {
	sendBatchCalls,
	sendContractCall,
	sendNative,
	buildEip1271Signature,
	extractClientDataFields,
	computeSafeMessageHash,
	keySetOf,
	signerAddressFor,
	type BeforePost,
	type QuotedInBandFee,
	type SignFn,
	type SubmitResult,
	type WalletKeySet,
	type WalletSigner,
	UserOpRevertedError
} from './safe-transaction';
import { enforceNoUnlimited } from './approval-guard';
import { toShellCall } from './amount-codec';
import { assertChallengeSigned, attestedSafeMessageHash } from './sign-attest';
import { findAccountByAddress, findAccountByCredentialId, type SignerAccount } from './accounts';
import { getAllNetworksSync } from './networks';
import { resolveChainId } from './chain-id';

/**
 * The stored record for the wallet a request is FOR — by ADDRESS, never by
 * credential.
 *
 * One passkey founds any number of wallets: a single-key one and a multi-key
 * one share the same credential id (on-chain: units 10 and 12 of the golden
 * key; in the parallel space: Parallel One and Parallel Multi). Looking the
 * record up by credential picked whichever came first, and its key set then
 * built the OTHER wallet's initCode — the bundler refused it with
 * `AA14 initCode must return sender` the first time a multi-key Safe was
 * deployed through this path (spec 062, Base, 2026-09-18). The send path had
 * resolved by address since 026; this makes the dApp path do the same. The
 * credential is only the fallback for an address no record carries.
 */
export function storedWalletFor(
	account: SigningAccount,
	safeAddress: string
): SignerAccount | undefined {
	return findAccountByAddress(safeAddress) ?? findAccountByCredentialId(account.id);
}

export interface DAppRequest {
	id: string;
	method: string;
	params: unknown[];
	origin?: string;
}

/**
 * What the core-driven path hands a submit (spec 082), so the wallet never
 * signs or sends for a page that is gone, the sheet says what is really
 * happening, and the dApp is answered inside its window.
 */
export interface DAppSubmitHooks {
	/**
	 * Is the asker still there (RB5)? `sign` is asked before the passkey,
	 * `submit` after the write-ahead's clearance and right before the relay
	 * POST, carrying the op's hash and chain (RJ2). `false` → nothing is sent,
	 * and the core hears `asker_gone`.
	 */
	claim(phase: ClaimPhase, submit?: SubmitClaim): Promise<boolean>;
	/** The passkey (or Trusted Signer) prompt opened / returned (RA9). */
	ceremony(stage: 'started' | 'done'): void;
	/** How long the page may still wait for a receipt (`dappReceiptWaitMs`, RA12). */
	receiptWaitMs(): number;
	/**
	 * The write-ahead (spec 082 RJ1): the op is signed and hashed, nothing has
	 * been POSTed. Resolves once its record is on disk (the core's
	 * `ClearToPost`); throws when that does not come in time — then nothing is
	 * sent. Absent: no record to wait for (a caller outside the core).
	 */
	writeAhead?(userOpHash: string, submitBlock: number | null): Promise<void>;
	/**
	 * Aborted once the core has answered the page by what the tracker knows
	 * (`OpTracked`, spec 082 RJ4): the receipt wait stops there.
	 */
	answered?: AbortSignal;
}

/**
 * `sign`, with the claims and the ceremony around it: claimed before the
 * prompt opens. For an operation that goes to the relay next, the signer also
 * carries the gate its submit runs before the first POST
 * ({@link writeAheadGate}): the write-ahead, then the second claim — with the
 * op's hash, once the signature exists and before a byte of it is sent (RB5,
 * RJ1, RJ2).
 */
export function guardedSign<A extends unknown[], T>(
	sign: (...args: A) => Promise<T>,
	hooks: DAppSubmitHooks | undefined,
	submits: boolean
): ((...args: A) => Promise<T>) & { beforePost?: BeforePost } {
	if (!hooks) return sign;
	const guarded = async (...args: A) => {
		if (!(await hooks.claim('sign'))) throw new AskerGoneError('sign');
		hooks.ceremony('started');
		const signed = await sign(...args);
		hooks.ceremony('done');
		return signed;
	};
	return submits ? Object.assign(guarded, { beforePost: writeAheadGate(hooks) }) : guarded;
}

/**
 * What a dApp op does after its local hash and head read and before its first
 * POST (spec 082 RJ1, RJ2): the record is written and the core clears the
 * POST, then the asker is asked once more — with the hash, so a surface that
 * goes from here on has its page answered by it, never 4900. Either refusal
 * throws, and nothing is sent.
 */
export function writeAheadGate(hooks: DAppSubmitHooks): BeforePost {
	return async ({ userOpHash, submitBlock, chainId }) => {
		if (hooks.writeAhead) await hooks.writeAhead(userOpHash, submitBlock);
		if (!(await hooks.claim('submit', { opHash: userOpHash, chainId }))) {
			throw new AskerGoneError('submit');
		}
	};
}

/** Who reported the op to the core: its hash, whether it may only have been sent, its head. */
export type OnSubmitted = (
	userOpHash: string,
	maybeSent: boolean,
	submitBlock: number | null
) => void;

/**
 * Who signs for `safeAddress` and what was asked — the question
 * {@link signChallenge} answers. The request's own method, (final) params and
 * origin are what the Trusted Signer's page is shown (spec 071); the passkey
 * ceremony reads only `credentials`, and only for a record that names no
 * sign-in key.
 */
function challengeSigner(
	request: DAppRequest,
	stored: SignerAccount | undefined,
	credentials: { id: string }[],
	safeAddress: string,
	chainId: number
): ChallengeSigner {
	return {
		account: safeAddress,
		keys: stored ? keySetOf(stored).keys : [],
		credentials,
		request: {
			method: request.method,
			params: request.params,
			origin: request.origin ?? '',
			chainId
		}
	};
}

// ── Chain ID resolution & validation ──────────────────────────────────────

const UNSUPPORTED_CHAIN_ERROR_CODE = 4902; // EIP-3085: unrecognized chain ID
const UNSUPPORTED_CAPABILITY_ERROR_CODE = 5700; // EIP-5792: unsupported non-optional capability

/**
 * EIP-5792: request capabilities are assumed REQUIRED unless explicitly marked
 * `{ optional: true }`. This wallet supports no request-level capabilities, so a
 * required capability must be rejected with code 5700 ("unsupported non-optional
 * capability") rather than silently ignored. Optional capabilities are dropped.
 */
function assertNoRequiredCapabilities(payload: {
	capabilities?: Record<string, { optional?: boolean }>;
	calls?: Array<{ capabilities?: Record<string, { optional?: boolean }> }>;
}): void {
	const buckets = [payload.capabilities, ...(payload.calls ?? []).map((c) => c.capabilities)];
	const required = new Set<string>();
	for (const caps of buckets) {
		if (!caps) continue;
		for (const [name, value] of Object.entries(caps)) {
			if (value?.optional !== true) required.add(name);
		}
	}
	if (required.size > 0) {
		throw Object.assign(
			new Error(`Unsupported non-optional capabilities: ${[...required].join(', ')}`),
			{ code: UNSUPPORTED_CAPABILITY_ERROR_CODE }
		);
	}
}

// The chain a request is submitted on lives in its own module so the signing
// sheet can read it the same way (083 H3) without importing this one.
export { resolveChainId };

/**
 * Assert the wallet supports the given chain ID.
 * Throws with EIP-3085 error code 4902 if unsupported.
 */
export function assertChainSupported(chainId: number): void {
	const supported = getAllNetworksSync().some((n) => n.chainId === chainId);
	if (!supported) {
		throw Object.assign(
			new Error(`Unsupported chain: ${chainId}. Add this network in wallet settings.`),
			{ code: UNSUPPORTED_CHAIN_ERROR_CODE }
		);
	}
}

/**
 * Extract an embedded chain ID from a dApp request's params, if present.
 * Returns undefined when the request carries no chain hint.
 */
/**
 * The request's ONE typed-data document (JSON), read by the core — unsuffixed /
 * `_v1` = [typedData, address]; `_v3` / `_v4` = [address, typedData]; exactly
 * two params, a real account, one EIP-712 document — or `undefined` when the
 * request is not one. No fallback to another slot: `params[1] ?? params[0]`
 * signed the second of two documents while the sheet showed the first (audit
 * 2026-10-01).
 */
export function pickTypedDataParam(method: string, params: unknown[]): unknown {
	return typedDataDocument(method, JSON.stringify(params)) ?? undefined;
}

export function extractRequestChainId(method: string, params: unknown[]): number | undefined {
	try {
		if (method.includes('signTypedData')) {
			const raw = pickTypedDataParam(method, params);
			const typed = typeof raw === 'string' ? JSON.parse(raw) : raw;
			const cid = typed?.domain?.chainId;
			if (cid != null) {
				const n =
					typeof cid === 'string'
						? cid.startsWith('0x')
							? parseInt(cid, 16)
							: parseInt(cid, 10)
						: Number(cid);
				if (!isNaN(n) && n > 0) return n;
			}
		} else if (method === 'eth_sendTransaction') {
			// Same coercion as the submit-side resolveChainId (string hex/dec OR number) —
			// if this pre-check misses a numeric chainId the modal estimates/displays on
			// the wallet's current chain while the submit goes to the tx's chain.
			const tx = params[0] as { chainId?: string | number } | undefined;
			const n = resolveChainId(0, tx?.chainId);
			if (n > 0) return n;
		} else if (method === 'wallet_sendCalls') {
			const payload = params[0] as { chainId?: string | number } | undefined;
			const n = resolveChainId(0, payload?.chainId);
			if (n > 0) return n;
		}
	} catch {
		/* malformed params — ignore */
	}
	return undefined;
}

/**
 * Build a full Safe-compatible EIP-1271 contract signature from a WebAuthn assertion.
 *
 * This encodes the signature in the format Safe's isValidSignature expects:
 * validAfter(6) + validUntil(6) + signerAddr(32) + offset(32) + v=0x00(1) + dataLen(32) + dynamicData
 * where dynamicData = abi.encode(authenticatorData, clientDataFields, r, s)
 */
function buildContractSignature(assertion: Assertion, signerAddress?: string): string {
	const rawSig = derSignatureToRaw(fromHex(assertion.signatureHex));
	if (!rawSig) throw new Error('Failed to convert signature');

	const authenticatorData = fromHex(assertion.authenticatorDataHex);
	const clientDataJSON = fromHex(assertion.clientDataJSONHex);
	const clientDataFields = extractClientDataFields(clientDataJSON);
	const sigR = rawSig.slice(0, 32);
	const sigS = rawSig.slice(32);

	const sig = buildEip1271Signature(authenticatorData, clientDataFields, sigR, sigS, signerAddress);
	return '0x' + toHex(sig);
}

/**
 * A multi-key wallet's message signing: allow-list every founding credential,
 * let the provider pick, and name the picked key's verifier in the signature.
 * Legacy single-key accounts keep the exact historical path.
 */
async function signSafeMessage(
	request: DAppRequest,
	account: SigningAccount,
	safeAddress: string,
	chainId: number,
	safeHash: Uint8Array,
	hooks?: DAppSubmitHooks
): Promise<{ assertion: Assertion; signerAddress?: string }> {
	const stored = storedWalletFor(account, safeAddress);
	const keySet = stored?.keys && stored.keys.length > 1 ? keySetOf(stored) : null;
	const credentials = keySet
		? keySet.keys.map((key) => ({ id: key.credentialId }))
		: [{ id: account.id }];
	const assertion = await guardedSign(
		signChallenge,
		hooks,
		false
	)(safeHash, challengeSigner(request, stored, credentials, safeAddress, chainId));
	// The authenticator signed the hash that was asked for, and nothing else
	// (spec 028 Phase 8).
	assertChallengeSigned(fromHex(stripHexPrefix(assertion.clientDataJSONHex)), safeHash);
	const signerAddress = keySet ? signerAddressFor(keySet, assertion.credentialId) : undefined;
	return { assertion, signerAddress };
}

/**
 * The EIP-1271 hash the passkey signs, computed by the shell AND the core and
 * refused if they differ (spec 028 Phase 8) — the message the sheet decoded
 * and the bytes the passkey sees come from one reading.
 */
function attestedMessageHash(
	originalHash: Uint8Array,
	chainId: number,
	safeAddress: string
): Uint8Array {
	return attestedSafeMessageHash(
		originalHash,
		chainId,
		safeAddress,
		computeSafeMessageHash(originalHash, chainId, safeAddress)
	);
}

/**
 * Bytes to sign for a personal_sign payload.
 *
 * Not every dApp hex-encodes the message — plain UTF-8 text is common enough
 * that the history decoder already special-cases it (decodeSignMessage in
 * dapp-history.ts). Treat a non-hex payload as UTF-8, which is what MetaMask
 * does and what the signing sheet already displays. The old hex decoder turned
 * such payloads into zero/garbage bytes and signed those, producing a
 * signature the dApp could never verify against its own text.
 */
function isHexPayload(payload: string): boolean {
	if (!payload.startsWith('0x')) return false;
	const body = payload.slice(2);
	return body.length % 2 === 0 && /^[0-9a-fA-F]*$/.test(body);
}

function personalSignBytes(payload: string): Uint8Array {
	return isHexPayload(payload)
		? fromHex(stripHexPrefix(payload))
		: new TextEncoder().encode(payload);
}

/**
 * Handle a personal_sign request.
 * Returns a full Safe contract signature (EIP-1271 compatible).
 */
export async function handlePersonalSign(
	request: DAppRequest,
	account: SigningAccount,
	safeAddress: string,
	chainId: number,
	hooks?: DAppSubmitHooks
): Promise<string> {
	// personal_sign has no embedded chainId — use the fallback
	assertChainSupported(chainId);

	const hexMsg = request.params[0] as string;
	const msgBytes = personalSignBytes(hexMsg);

	const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${msgBytes.length}`);
	const combined = new Uint8Array(prefix.length + msgBytes.length);
	combined.set(prefix);
	combined.set(msgBytes, prefix.length);
	const originalHash = keccak256(combined);

	const safeHash = attestedMessageHash(originalHash, chainId, safeAddress);
	const { assertion, signerAddress } = await signSafeMessage(
		request,
		account,
		safeAddress,
		chainId,
		safeHash,
		hooks
	);
	return buildContractSignature(assertion, signerAddress);
}

/**
 * Handle an eth_signTypedData_v4 request.
 * Returns a full Safe contract signature (EIP-1271 compatible).
 */
export async function handleSignTypedData(
	request: DAppRequest,
	account: SigningAccount,
	safeAddress: string,
	chainId: number,
	hooks?: DAppSubmitHooks
): Promise<string> {
	// The core's reading — the very document the sheet decoded (audit
	// 2026-10-01). A request that is not one is never signed.
	const document = typedDataDocument(request.method, JSON.stringify(request.params));
	if (document === null) throw new Error('Invalid typed-data request');
	const typedData: TypedData = JSON.parse(document);

	const effectiveChainId = resolveChainId(
		chainId,
		typedData.domain?.chainId as string | number | undefined
	);
	assertChainSupported(effectiveChainId);

	const originalHash = hashTypedData(typedData);
	const safeHash = attestedMessageHash(originalHash, effectiveChainId, safeAddress);
	const { assertion, signerAddress } = await signSafeMessage(
		request,
		account,
		safeAddress,
		effectiveChainId,
		safeHash,
		hooks
	);
	return buildContractSignature(assertion, signerAddress);
}

/**
 * Handle an eth_sendTransaction request (full ERC-4337 UserOp).
 */
export async function handleSendTransaction(
	request: DAppRequest,
	account: SigningAccount,
	safeAddress: string,
	chainId: number,
	maxFeeOverride?: bigint,
	onSubmitted?: OnSubmitted,
	// In-band chains only: settle gas in this whitelisted stablecoin (null/omitted
	// = native). Ignored on legacy chains and on Tempo (always pathUSD there).
	gasFeeToken?: string | null,
	// In-band: the displayed fee (amount + recipient) — signed verbatim.
	quotedFee?: QuotedInBandFee,
	hooks?: DAppSubmitHooks
): Promise<string> {
	const txDict = request.params[0] as Record<string, string>;
	const effectiveChainId = resolveChainId(chainId, txDict.chainId);
	assertChainSupported(effectiveChainId);

	const [call] = requestCalls(request);

	// Resolve the wallet's FULL key set (multi-key wallets sign with any
	// founding key; the set also builds an undeployed Safe's initCode). Falls
	// back to the legacy single-key index lookup for unknown accounts.
	let walletSigner: WalletSigner | undefined;
	const stored = storedWalletFor(account, safeAddress);
	if (stored) {
		// Only a genuinely multi-key account changes shape here — a single-key
		// wallet keeps the exact historical string form (and bytes).
		walletSigner = stored.keys && stored.keys.length > 1 ? keySetOf(stored) : stored.publicKeyHex;
	}

	if (!walletSigner) throw new Error('Public key not found');
	const publicKeyHex = walletSigner;
	const keySet: WalletKeySet | null = typeof walletSigner === 'string' ? null : walletSigner;
	const credentials = keySet
		? keySet.keys.map((key) => ({ id: key.credentialId }))
		: [{ id: account.id }];

	const signer = challengeSigner(request, stored, credentials, safeAddress, effectiveChainId);
	const signFn = guardedSign(txSigner(signer), hooks, true);

	const txResult = await sendOneCall(
		safeAddress,
		call,
		effectiveChainId,
		publicKeyHex,
		signFn,
		maxFeeOverride,
		gasFeeToken,
		quotedFee
	);

	return answerFor(txResult, effectiveChainId, onSubmitted, hooks);
}

/**
 * The calls a request sends, as the CORE reads them (`tx_request::calls_of`,
 * spec 096 F1) and in `safe-transaction.ts`'s form: `value` as `0x`-hex,
 * through the one codec Send's calls take (`toShellCall`). Every reader of
 * `value` used to be its own — this one stripped the `0x`, and the gas floor
 * then threw on PancakeSwap's `0xaa87bee538000` before anything was signed.
 * Throws when the core cannot read them: nothing is guessed or signed.
 */
function requestCalls(request: DAppRequest): { to: string; value: string; data: string }[] {
	const calls = dappRequestCalls(request.method, JSON.stringify(request.params));
	if (!calls) throw new Error('Invalid transaction params');
	return calls.map(toShellCall);
}

/** One call: a plain transfer when it carries no calldata, else a contract call. */
function sendOneCall(
	safeAddress: string,
	call: { to: string; value: string; data: string },
	chainId: number,
	publicKeyHex: WalletSigner,
	signFn: SignFn,
	maxFeeOverride: bigint | undefined,
	gasFeeToken: string | null | undefined,
	quotedFee: QuotedInBandFee | undefined
): Promise<SubmitResult> {
	if (call.data === '0x' || call.data === '') {
		return sendNative(
			safeAddress,
			call.to,
			call.value,
			chainId,
			publicKeyHex,
			signFn,
			maxFeeOverride,
			gasFeeToken,
			quotedFee
		);
	}
	return sendContractCall(
		safeAddress,
		call.to,
		call.value,
		fromHex(stripHexPrefix(call.data)),
		chainId,
		publicKeyHex,
		signFn,
		maxFeeOverride,
		gasFeeToken,
		quotedFee
	);
}

/**
 * The one answer an on-chain request gets once its op is on its way (spec 082
 * RA2, RA8): the tx hash when a receipt arrives inside the page's window —
 * a revert included (ruling 9: gas was spent, a transaction exists) — else the
 * op hash (`DAppReceiptPendingError`). An op the relay may only have received
 * (`maybeSent`) is answered the same way, under its local hash: never 4900 or
 * -32603, which a dApp reads as "not sent" and sends again.
 */
async function answerFor(
	txResult: SubmitResult,
	chainId: number,
	onSubmitted: OnSubmitted | undefined,
	hooks: DAppSubmitHooks | undefined
): Promise<string> {
	// Report the hash so the core records and tracks it before anything is
	// answered — accepted, or may have been sent.
	onSubmitted?.(txResult.userOpHash, txResult.maybeSent, txResult.submitBlock);
	try {
		// Inside what is left of the page's window, each poll included (RJ4,
		// G39), and no longer than the core's own answer (`answered`).
		return await txResult.waitForTxHash(hooks?.receiptWaitMs(), hooks?.answered);
	} catch (error) {
		// A landed revert is the page's error, naming its transaction — never
		// the hash a site reads as done (083, owner ruling 2026-10-01). Whatever
		// else the wait ended on — its window, the core's own answer, a relay
		// out of reach — leaves the op in flight: the core tells the page it is
		// not confirmed yet, and only the tracker says the relay refused it
		// (spec 082 RJ4, `OpTracked`).
		if (error instanceof UserOpRevertedError) {
			throw new DAppRevertedError(txResult.userOpHash, error.txHash);
		}
		throw new DAppReceiptPendingError(txResult.userOpHash);
	}
}

/**
 * The passkey half of a transaction signer: the assertion over the SafeOp
 * challenge, with the identity-provider compatibility check.
 */
function txSigner(signer: ChallengeSigner) {
	return async (challenge: Uint8Array) => {
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
}

/**
 * The op is on its way (accepted, or may have been sent) but no receipt
 * arrived inside the wait — its window ran out, the relay was out of reach,
 * or the core answered the page first. The op may still land, so this is NOT
 * a failure and NOT a confirmation (issue 262): the core answers the page
 * "not confirmed yet" (083 — the op hash only as a batch id) and the pending
 * record is left for the tracker.
 */
export class DAppReceiptPendingError extends Error {
	constructor(readonly userOpHash: string) {
		super(`Transaction ${userOpHash.slice(0, 10)}… submitted; its receipt has not arrived yet.`);
		this.name = 'DAppReceiptPendingError';
	}
}

/**
 * The op was included and its execution REVERTED (083): the receipt said
 * `success: false`. The core answers the page one error naming the
 * transaction ([`reverted_detail`]) and closes the record failed.
 */
export class DAppRevertedError extends Error {
	constructor(
		readonly userOpHash: string,
		readonly txHash: string
	) {
		super(`Transaction ${txHash.slice(0, 10)}… was included but reverted.`);
		this.name = 'DAppRevertedError';
	}
}

/**
 * Handle a generic sign request.
 * Returns a full Safe contract signature (EIP-1271 compatible).
 */
export async function handleGenericSign(
	request: DAppRequest,
	account: SigningAccount,
	safeAddress: string,
	chainId: number,
	hooks?: DAppSubmitHooks
): Promise<string> {
	assertChainSupported(chainId);

	const jsonStr = JSON.stringify(request.params);
	const jsonBytes = new TextEncoder().encode(jsonStr);
	const originalHash = keccak256(jsonBytes);

	const safeHash = attestedMessageHash(originalHash, chainId, safeAddress);
	const { assertion, signerAddress } = await signSafeMessage(
		request,
		account,
		safeAddress,
		chainId,
		safeHash,
		hooks
	);
	return buildContractSignature(assertion, signerAddress);
}

/**
 * Which layer owns the never-unlimited submit guard for one call into
 * {@link handleDAppRequest} / {@link handleSendCalls}.
 *
 * `'ts'` — the DEFAULT, and the only value native ever gets: `enforceNoUnlimited`
 * runs right here, at the submit chokepoint, exactly as it always has. Hermes has
 * no WebAssembly, so on iOS/Android this TypeScript copy IS the guard; it can
 * never be deleted.
 *
 * `'core'` — the Rust core already ran its own `enforce_no_unlimited` over this
 * exact request (the single request AND every batch leg) inside `proceed_submit`
 * (`rust/crates/vela-core/src/app/sign_request.rs`) *before* it emitted the
 * `SignAndSubmit` effect that led here, and it refuses by failing the inflight
 * request rather than by throwing. Re-deciding it here would make the same
 * safety call twice out of two separately-maintained implementations — the exact
 * drift hazard this seam exists to remove. On web the core owns the gate.
 *
 * Only `services/wallet-state-core/sign-executor.web.ts` may pass `'core'`: it is
 * the sole handler of that effect and it is a `.web.ts` module, so no native
 * bundle can reach it. The default is the *guarded* value on purpose — a caller
 * that says nothing stays guarded, so forgetting this argument can never open a
 * hole on either platform.
 */
export type SubmitGuardOwner = 'ts' | 'core';

/**
 * Route a request to the appropriate handler.
 * Returns the result to send back to the dApp.
 */
export async function handleDAppRequest(
	request: DAppRequest,
	account: SigningAccount,
	safeAddress: string,
	chainId: number,
	maxFeeOverride?: bigint,
	onSubmitted?: OnSubmitted,
	// In-band chains only: settle gas in this whitelisted stablecoin (null/omitted
	// = native). Only meaningful for the tx/batch methods.
	gasFeeToken?: string | null,
	// In-band: the displayed fee (amount + recipient) — signed verbatim.
	quotedFee?: QuotedInBandFee,
	// Who already enforced the never-unlimited mandate for this request. See
	// {@link SubmitGuardOwner}; omitting it keeps the guard here.
	guardOwner: SubmitGuardOwner = 'ts',
	// Spec 082: the claims, the ceremony and the answer window. Absent → the
	// request is signed as before (a caller with no asker to lose).
	hooks?: DAppSubmitHooks
): Promise<unknown> {
	const { method } = request;

	// Final, descriptor-independent safety net: never sign or submit a request that
	// would grant an unbounded allowance. The UI caps approvals up-front, but this
	// guard catches anything that bypassed it (incl. shapes no descriptor decodes).
	// On the core-driven path the core has already made this exact call.
	if (guardOwner === 'ts') enforceNoUnlimited(method, request.params);

	if (method === 'eth_sendTransaction') {
		return handleSendTransaction(
			request,
			account,
			safeAddress,
			chainId,
			maxFeeOverride,
			onSubmitted,
			gasFeeToken,
			quotedFee,
			hooks
		);
	} else if (method === 'wallet_sendCalls') {
		return handleSendCalls(
			request,
			account,
			safeAddress,
			chainId,
			gasFeeToken,
			quotedFee,
			guardOwner,
			onSubmitted,
			hooks
		);
	} else if (method === 'personal_sign') {
		return handlePersonalSign(request, account, safeAddress, chainId, hooks);
	} else if (method.includes('signTypedData')) {
		return handleSignTypedData(request, account, safeAddress, chainId, hooks);
	} else {
		return handleGenericSign(request, account, safeAddress, chainId, hooks);
	}
}

/**
 * Handle a wallet_sendCalls request (EIP-5792 batched atomic calls).
 * Executes multiple calls as a single UserOp via the Safe account.
 *
 * Resolves with the batch id — the userOpHash — as soon as the relay accepts
 * the op, without a receipt wait: that id is what `wallet_getCallsStatus`
 * reads back. It is not a tx hash, so the core-driven path reports it as
 * `receipt_pending` (`sign-executor.ts`) and the tracker settles the record.
 */
export async function handleSendCalls(
	request: DAppRequest,
	account: SigningAccount,
	safeAddress: string,
	chainId: number,
	// In-band chains only: settle gas in this whitelisted stablecoin (null/omitted
	// = native). Ignored on legacy chains and on Tempo (always pathUSD there).
	gasFeeToken?: string | null,
	// In-band: the displayed fee (amount + recipient) — signed verbatim.
	quotedFee?: QuotedInBandFee,
	// Who already enforced the never-unlimited mandate for these legs. See
	// {@link SubmitGuardOwner}; omitting it keeps the guard here.
	guardOwner: SubmitGuardOwner = 'ts',
	// Spec 082: the op is reported to the core like any other on-chain request,
	// so the batch is recorded, tracked and ended by the tracker.
	onSubmitted?: OnSubmitted,
	hooks?: DAppSubmitHooks
): Promise<string> {
	const payload = request.params[0] as {
		calls: Array<{
			to: string;
			value?: string;
			data?: string;
			capabilities?: Record<string, { optional?: boolean }>;
		}>;
		chainId?: string; // hex chain ID from dApp
		from?: string;
		capabilities?: Record<string, { optional?: boolean }>;
	};

	// EIP-5792: reject any required capability we don't support before touching the
	// wallet (so the dApp gets a clean 5700 rather than a silently-dropped feature).
	assertNoRequiredCapabilities(payload);

	const effectiveChainId = resolveChainId(chainId, payload.chainId);
	assertChainSupported(effectiveChainId);

	const calls = payload.calls ?? [];
	if (calls.length === 0) throw new Error('No calls provided');
	const legs = requestCalls(request);

	// A batch must not smuggle an unbounded approval past the per-tx guard — check
	// every leg as if it were a standalone transaction. On the core-driven path the
	// core has already walked these same legs (`sign_request.rs` `proceed_submit`).
	if (guardOwner === 'ts') {
		for (const c of calls) {
			enforceNoUnlimited('eth_sendTransaction', [{ to: c.to, data: c.data, value: c.value }]);
		}
	}

	// Resolve the wallet's FULL key set (multi-key wallets sign with any
	// founding key; the set also builds an undeployed Safe's initCode). Falls
	// back to the legacy single-key index lookup for unknown accounts.
	let walletSigner: WalletSigner | undefined;
	const stored = storedWalletFor(account, safeAddress);
	if (stored) {
		// Only a genuinely multi-key account changes shape here — a single-key
		// wallet keeps the exact historical string form (and bytes).
		walletSigner = stored.keys && stored.keys.length > 1 ? keySetOf(stored) : stored.publicKeyHex;
	}

	if (!walletSigner) throw new Error('Public key not found');
	const publicKeyHex = walletSigner;
	const keySet: WalletKeySet | null = typeof walletSigner === 'string' ? null : walletSigner;
	const credentials = keySet
		? keySet.keys.map((key) => ({ id: key.credentialId }))
		: [{ id: account.id }];

	const signer = challengeSigner(request, stored, credentials, safeAddress, effectiveChainId);
	const signFn = guardedSign(txSigner(signer), hooks, true);

	// Single call → use existing send logic
	if (legs.length === 1) {
		const txResult = await sendOneCall(
			safeAddress,
			legs[0],
			effectiveChainId,
			publicKeyHex,
			signFn,
			undefined,
			gasFeeToken,
			quotedFee
		);
		return batchIdFor(txResult, onSubmitted);
	}

	// Multiple calls → batch via Safe multiSend
	const txResult = await sendBatchCalls(
		safeAddress,
		legs,
		effectiveChainId,
		publicKeyHex,
		signFn,
		undefined,
		gasFeeToken,
		quotedFee
	);
	return batchIdFor(txResult, onSubmitted);
}

/**
 * EIP-5792's answer: the batch id, which is the op hash — at once, with no
 * receipt wait. Reported to the core first, so the batch is recorded and
 * followed; an op that may only have been sent answers its local hash.
 */
function batchIdFor(txResult: SubmitResult, onSubmitted: OnSubmitted | undefined): string {
	onSubmitted?.(txResult.userOpHash, txResult.maybeSent, txResult.submitBlock);
	return txResult.userOpHash;
}

/**
 * Check if a method is a signing method that needs user approval.
 */
export function isSigningMethod(method: string): boolean {
	return (
		method === 'eth_sendTransaction' ||
		method === 'wallet_sendCalls' ||
		method === 'personal_sign' ||
		method === 'eth_sign' ||
		method.includes('signTypedData')
	);
}

/**
 * Read-only methods answered instantly from local wallet state (no network). The
 * dispatch layer skips the concurrency gate for these so a flood of cheap local
 * queries never queues behind network-bound reads.
 */
export const INSTANT_READONLY_METHODS = new Set([
	'eth_accounts',
	'eth_requestAccounts',
	'eth_chainId',
	'net_version',
	'wallet_getPermissions',
	'wallet_requestPermissions',
	'wallet_addEthereumChain',
	'wallet_getCapabilities'
]);
