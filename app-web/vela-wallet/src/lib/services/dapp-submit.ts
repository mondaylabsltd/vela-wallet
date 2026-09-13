/**
 * The dApp SUBMIT path — how a signing request becomes a signature or a UserOp.
 *
 * Ported from src/hooks/use-dapp-signing.ts @ f9bcb278 (a hook in name only:
 * it never used React). Web deltas: the kernels import replaces the vela-core
 * facade (static — the Expo dynamic imports were a bundle-splitting hedge),
 * the passkey call goes through `signWithAny` (every founding credential in
 * the allow-list), the stored account comes from onboarding storage, and the
 * public-key-index fallback for an UNKNOWN account is gone: a web session is
 * always a stored account, so a missing one is an error, not a lookup.
 * `guardOwner` semantics are unchanged: on the core-driven path the core has
 * already run `enforce_no_unlimited`; the default stays the guarded value.
 */
import {
	SAFE_PROXY_RUNTIME_CODE,
	derSignatureToRaw,
	fromHex,
	hashTypedData,
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
import { signWithAny, type Assertion } from '$lib/onboarding/core/passkey';
import { rpcCall } from './rpc-adapter';
import {
	sendBatchCalls,
	sendContractCall,
	sendNative,
	buildEip1271Signature,
	extractClientDataFields,
	computeSafeMessageHash,
	keySetOf,
	signerAddressFor,
	type QuotedInBandFee,
	type WalletKeySet,
	type WalletSigner
} from './safe-transaction';
import { enforceNoUnlimited } from './approval-guard';
import { assertChallengeSigned, attestedSafeMessageHash } from './sign-attest';
import { findAccountByCredentialId } from './accounts';
import { getAllNetworksSync } from './networks';

export interface DAppRequest {
	id: string;
	method: string;
	params: unknown[];
	origin?: string;
}

// ── Chain ID resolution & validation ──────────────────────────────────────

const UNSUPPORTED_CHAIN_ERROR_CODE = 4902; // EIP-3085: unrecognized chain ID
const UNSUPPORTED_CAPABILITY_ERROR_CODE = 5700; // EIP-5792: unsupported non-optional capability

// ── Submitted UserOp → chain tracking ──────────────────────────────────────
//
// Two readers need the chain a userOp was submitted on, even after the wallet
// has since switched networks:
//   • wallet_getCallsStatus — looks up the EIP-5792 batch id (== userOpHash).
//   • eth_getTransactionReceipt / eth_getTransactionByHash — a dApp that polls a
//     receipt by a hash we handed back (a wallet_sendCalls batch id, or an
//     eth_sendTransaction userOpHash) matches nothing on the public RPC and would
//     poll forever; translate it to the real bundle tx via the bundler, on the
//     ORIGINAL chain, not the current one.
// Hashes are opaque, so we remember hash → chainId at submit time.
const userOpChainIds = new Map<string, number>();
const MAX_TRACKED_USEROPS = 200;

function rememberUserOpChain(userOpHash: string, chainId: number): void {
	const key = userOpHash.toLowerCase();
	// Bound memory: drop the oldest entry once we exceed the cap (Map keeps
	// insertion order, so the first key is the oldest).
	if (!userOpChainIds.has(key) && userOpChainIds.size >= MAX_TRACKED_USEROPS) {
		const oldest = userOpChainIds.keys().next().value;
		if (oldest !== undefined) userOpChainIds.delete(oldest);
	}
	userOpChainIds.set(key, chainId);
}

/** The chain a userOp/batch was submitted on, or undefined if we didn't issue it. */
function resolveUserOpChain(userOpHash: string | undefined): number | undefined {
	if (!userOpHash) return undefined;
	return userOpChainIds.get(userOpHash.toLowerCase());
}

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

/**
 * Resolve the effective chain ID from request context.
 * Priority: request-embedded chainId > fallback (component-level chainId).
 */
export function resolveChainId(
	fallback: number,
	...candidates: (string | number | undefined | null)[]
): number {
	for (const c of candidates) {
		if (c == null) continue;
		const n = typeof c === 'string' ? (c.startsWith('0x') ? parseInt(c, 16) : parseInt(c, 10)) : c;
		if (!isNaN(n) && n > 0) return n;
	}
	return fallback;
}

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
 * Pick the typed-data param by method, per the MetaMask ecosystem order the
 * WalletPair spec pins: unsuffixed / `_v1` = [typedData, address]; `_v3` / `_v4`
 * = [address, typedData].
 */
export function pickTypedDataParam(method: string, params: unknown[]): unknown {
	return method === 'eth_signTypedData' || method === 'eth_signTypedData_v1'
		? params[0]
		: (params[1] ?? params[0]);
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
	account: SigningAccount,
	safeHashHex: string
): Promise<{ assertion: Assertion; signerAddress?: string }> {
	const stored = await findAccountByCredentialId(account.id);
	const keySet = stored?.keys && stored.keys.length > 1 ? keySetOf(stored) : null;
	const credentials = keySet
		? keySet.keys.map((key) => ({ id: key.credentialId }))
		: [{ id: account.id }];
	const assertion = await signWithAny(safeHashHex, credentials);
	// The authenticator signed the hash that was asked for, and nothing else
	// (spec 028 Phase 8).
	assertChallengeSigned(
		fromHex(stripHexPrefix(assertion.clientDataJSONHex)),
		fromHex(stripHexPrefix(safeHashHex))
	);
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
	chainId: number
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
	const { assertion, signerAddress } = await signSafeMessage(account, toHex(safeHash));
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
	chainId: number
): Promise<string> {
	const typedDataRaw = pickTypedDataParam(request.method, request.params);
	const typedData: TypedData =
		typeof typedDataRaw === 'string' ? JSON.parse(typedDataRaw) : typedDataRaw;

	const effectiveChainId = resolveChainId(
		chainId,
		typedData.domain?.chainId as string | number | undefined
	);
	assertChainSupported(effectiveChainId);

	const originalHash = hashTypedData(typedData);
	const safeHash = attestedMessageHash(originalHash, effectiveChainId, safeAddress);
	const { assertion, signerAddress } = await signSafeMessage(account, toHex(safeHash));
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
	onSubmitted?: (userOpHash: string) => void,
	// In-band chains only: settle gas in this whitelisted stablecoin (null/omitted
	// = native). Ignored on legacy chains and on Tempo (always pathUSD there).
	gasFeeToken?: string | null,
	// In-band: the displayed fee (amount + recipient) — signed verbatim.
	quotedFee?: QuotedInBandFee
): Promise<string> {
	const txDict = request.params[0] as Record<string, string>;
	const effectiveChainId = resolveChainId(chainId, txDict.chainId);
	assertChainSupported(effectiveChainId);

	const to = txDict.to ?? '';
	const valueHex = txDict.value ?? '0x0';
	const dataHex = txDict.data ?? '0x';

	// Resolve the wallet's FULL key set (multi-key wallets sign with any
	// founding key; the set also builds an undeployed Safe's initCode). Falls
	// back to the legacy single-key index lookup for unknown accounts.
	let walletSigner: WalletSigner | undefined;
	const stored = await findAccountByCredentialId(account.id);
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

	const signFn = async (challenge: Uint8Array) => {
		const assertion = await signWithAny(toHex(challenge), credentials);

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

	const valueClean = stripHexPrefix(valueHex) || '0';

	let txResult;
	if (dataHex === '0x' || dataHex === '') {
		txResult = await sendNative(
			safeAddress,
			to,
			valueClean,
			effectiveChainId,
			publicKeyHex,
			signFn,
			maxFeeOverride,
			gasFeeToken,
			quotedFee
		);
	} else {
		const txData = fromHex(stripHexPrefix(dataHex));
		txResult = await sendContractCall(
			safeAddress,
			to,
			valueClean,
			txData,
			effectiveChainId,
			publicKeyHex,
			signFn,
			maxFeeOverride,
			gasFeeToken,
			quotedFee
		);
	}

	// Op is signed + accepted by the bundler here; the receipt wait can take a while.
	// Remember the chain so a later eth_getTransactionReceipt(userOpHash) poll can be
	// translated to the real bundle tx on the ORIGINAL chain (not the current one).
	rememberUserOpChain(txResult.userOpHash, effectiveChainId);
	// Report the hash so the UI can show "submitted, waiting" instead of a blank spin.
	onSubmitted?.(txResult.userOpHash);
	return await txResult.waitForTxHash();
}

/**
 * Handle a generic sign request.
 * Returns a full Safe contract signature (EIP-1271 compatible).
 */
export async function handleGenericSign(
	request: DAppRequest,
	account: SigningAccount,
	safeAddress: string,
	chainId: number
): Promise<string> {
	assertChainSupported(chainId);

	const jsonStr = JSON.stringify(request.params);
	const jsonBytes = new TextEncoder().encode(jsonStr);
	const originalHash = keccak256(jsonBytes);

	const safeHash = attestedMessageHash(originalHash, chainId, safeAddress);
	const { assertion, signerAddress } = await signSafeMessage(account, toHex(safeHash));
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
	onSubmitted?: (userOpHash: string) => void,
	// In-band chains only: settle gas in this whitelisted stablecoin (null/omitted
	// = native). Only meaningful for the tx/batch methods.
	gasFeeToken?: string | null,
	// In-band: the displayed fee (amount + recipient) — signed verbatim.
	quotedFee?: QuotedInBandFee,
	// Who already enforced the never-unlimited mandate for this request. See
	// {@link SubmitGuardOwner}; omitting it keeps the guard here.
	guardOwner: SubmitGuardOwner = 'ts'
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
			quotedFee
		);
	} else if (method === 'wallet_sendCalls') {
		return handleSendCalls(
			request,
			account,
			safeAddress,
			chainId,
			gasFeeToken,
			quotedFee,
			guardOwner
		);
	} else if (method === 'personal_sign') {
		return handlePersonalSign(request, account, safeAddress, chainId);
	} else if (method.includes('signTypedData')) {
		return handleSignTypedData(request, account, safeAddress, chainId);
	} else {
		return handleGenericSign(request, account, safeAddress, chainId);
	}
}

/**
 * Handle a wallet_sendCalls request (EIP-5792 batched atomic calls).
 * Executes multiple calls as a single UserOp via the Safe account.
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
	guardOwner: SubmitGuardOwner = 'ts'
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
	const stored = await findAccountByCredentialId(account.id);
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

	const signFn = async (challenge: Uint8Array) => {
		const assertion = await signWithAny(toHex(challenge), credentials);

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

	// Single call → use existing send logic
	if (calls.length === 1) {
		const call = calls[0];
		const to = call.to ?? '';
		const valueHex = call.value ?? '0x0';
		const dataHex = call.data ?? '0x';
		const valueClean = stripHexPrefix(valueHex) || '0';

		let txResult;
		if (dataHex === '0x' || dataHex === '') {
			txResult = await sendNative(
				safeAddress,
				to,
				valueClean,
				effectiveChainId,
				publicKeyHex,
				signFn,
				undefined,
				gasFeeToken,
				quotedFee
			);
		} else {
			const txData = fromHex(stripHexPrefix(dataHex));
			txResult = await sendContractCall(
				safeAddress,
				to,
				valueClean,
				txData,
				effectiveChainId,
				publicKeyHex,
				signFn,
				undefined,
				gasFeeToken,
				quotedFee
			);
		}
		rememberUserOpChain(txResult.userOpHash, effectiveChainId);
		return txResult.userOpHash;
	}

	// Multiple calls → batch via Safe multiSend
	const txResult = await sendBatchCalls(
		safeAddress,
		calls.map((c) => ({
			to: c.to,
			value: stripHexPrefix(c.value ?? '0x0') || '0',
			data: c.data ?? '0x'
		})),
		effectiveChainId,
		publicKeyHex,
		signFn,
		undefined,
		gasFeeToken,
		quotedFee
	);
	rememberUserOpChain(txResult.userOpHash, effectiveChainId);
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

/**
 * Read methods that MAY be forwarded to a public RPC endpoint — an explicit
 * allowlist (the base read-only set of the WalletPair Ethereum protocol). The
 * spec forbids inferring safety from "not a signing method" or an `eth_` prefix,
 * so an unknown method is never blindly forwarded.
 */
export const FORWARDABLE_READ_METHODS = new Set([
	'web3_clientVersion',
	'eth_syncing',
	'eth_blockNumber',
	'eth_call',
	'eth_estimateGas',
	'eth_createAccessList',
	'eth_feeHistory',
	'eth_gasPrice',
	'eth_maxPriorityFeePerGas',
	'eth_getBalance',
	'eth_getCode',
	'eth_getStorageAt',
	'eth_getProof',
	'eth_getTransactionCount',
	'eth_getBlockByHash',
	'eth_getBlockByNumber',
	'eth_getBlockTransactionCountByHash',
	'eth_getBlockTransactionCountByNumber',
	'eth_getTransactionByHash',
	'eth_getTransactionByBlockHashAndIndex',
	'eth_getTransactionByBlockNumberAndIndex',
	'eth_getTransactionReceipt',
	'eth_getLogs'
]);

/**
 * Cache of the wallet's own deployed Safe code, keyed by `${chainId}|${address}`.
 * A deployed Safe's code is immutable, so once observed we answer eth_getCode for
 * our own address without re-querying the RPC (defense-in-depth for older SDKs that
 * still forward eth_getCode for the wallet's own address). Keyed by address as well
 * as chain so switching accounts never serves another account's code. Undeployed
 * accounts are intentionally NOT cached — they may deploy at any time.
 */
const deployedSelfCode = new Map<string, string>();

/** Reset module-level read-only caches. Tests only. */
export function __resetReadOnlyCache(): void {
	deployedSelfCode.clear();
}

/**
 * Handle a read-only RPC method. Returns the result or null if not handled.
 */
export async function handleReadOnlyRPC(
	method: string,
	params: unknown[],
	address: string,
	chainId: number
): Promise<{ handled: true; result: unknown } | { handled: false }> {
	// Counterfactual smart-account override: when a dApp queries eth_getCode for
	// THIS wallet's own address and the Safe proxy isn't deployed yet (real code
	// is empty), return the Safe proxy runtime bytecode so the dApp detects a
	// smart contract wallet (EIP-1271) instead of an EOA. Other addresses pass
	// through to the normal RPC forward below.
	if (method === 'eth_getCode') {
		const target = (params?.[0] as string | undefined)?.toLowerCase();
		if (target && address && target === address.toLowerCase()) {
			const cacheKey = `${chainId}|${target}`;
			const cached = deployedSelfCode.get(cacheKey);
			if (cached) return { handled: true, result: cached };
			try {
				const res = await rpcCall('eth_getCode', params ?? [target, 'latest'], chainId);
				const code = res.result as string | undefined;
				if (code && code !== '0x' && code.length > 2) {
					deployedSelfCode.set(cacheKey, code);
					return { handled: true, result: code };
				}
			} catch {
				/* fall through to runtime code */
			}
			return { handled: true, result: SAFE_PROXY_RUNTIME_CODE };
		}
	}

	switch (method) {
		case 'eth_accounts':
		case 'eth_requestAccounts':
			// Never let an unhydrated/no-longer-selected account produce
			// `[undefined]`: that is outside the encrypted protocol's JSON data model
			// and would otherwise make the dApp wait until its own timeout.
			return { handled: true, result: address ? [address] : [] };
		case 'eth_chainId':
			return { handled: true, result: '0x' + chainId.toString(16) };
		case 'net_version':
			return { handled: true, result: String(chainId) };
		// EIP-2255 compatibility shims — not protocol methods (the WalletPair
		// pairing IS the authorization), but kept so dApps that call them don't
		// break. Not advertised in capabilities.methods.
		case 'wallet_getPermissions':
		case 'wallet_requestPermissions':
			return { handled: true, result: [{ parentCapability: 'eth_accounts' }] };
		case 'wallet_addEthereumChain':
			return { handled: true, result: null };
		case 'wallet_getCapabilities':
			return {
				handled: true,
				result: {
					['0x' + chainId.toString(16)]: { atomic: { status: 'supported' } }
				}
			};
		// EIP-5792: poll the status of a prior wallet_sendCalls batch by its ID
		// (the userOpHash returned from handleSendCalls).
		case 'wallet_getCallsStatus': {
			const id = params?.[0] as string | undefined;
			// Resolve the chain the batch was submitted on (the wallet may have since
			// switched networks); fall back to the current chain for unknown ids.
			const batchChain = resolveUserOpChain(id) ?? chainId;
			const hexChain = '0x' + batchChain.toString(16);
			const pending = {
				version: '2.0.0',
				id: id ?? '0x',
				chainId: hexChain,
				status: 100,
				atomic: true,
				receipts: [] as unknown[]
			};
			if (!id) return { handled: true, result: pending };
			try {
				// ID is a userOpHash — query the bundler for the UserOp receipt
				const res = await rpcCall('eth_getUserOperationReceipt', [id], batchChain);
				const opReceipt = res.result as {
					success?: boolean;
					receipt?: {
						status?: string;
						logs?: unknown[];
						blockHash?: string;
						blockNumber?: string;
						gasUsed?: string;
						transactionHash?: string;
					};
				} | null;
				if (!opReceipt?.receipt) return { handled: true, result: pending };
				const receipt = opReceipt.receipt;
				const ok = opReceipt.success !== false && receipt.status === '0x1';
				return {
					handled: true,
					result: {
						version: '2.0.0',
						id,
						chainId: hexChain,
						status: ok ? 200 : 500,
						atomic: true,
						receipts: [
							{
								logs: receipt.logs ?? [],
								status: receipt.status ?? '0x0',
								blockHash: receipt.blockHash,
								blockNumber: receipt.blockNumber,
								gasUsed: receipt.gasUsed,
								transactionHash: receipt.transactionHash ?? id
							}
						]
					}
				};
			} catch {
				return { handled: true, result: pending };
			}
		}
		// A dApp polling a receipt by a hash WE handed back (a wallet_sendCalls batch
		// id, or a userOpHash) would otherwise hit the public RPC — where that hash
		// matches no on-chain tx — and poll forever ("submitted but never confirms").
		// Translate our hashes to the real bundle tx via the bundler, on the chain the
		// op was submitted on, and forward the AUTHENTIC on-chain receipt (never a
		// synthesized one) so the dApp gets a complete, spec-shaped result.
		case 'eth_getTransactionReceipt':
		case 'eth_getTransactionByHash': {
			const hash = params?.[0] as string | undefined;
			const opChain = resolveUserOpChain(hash);
			if (hash && opChain !== undefined) {
				try {
					const opRes = await rpcCall('eth_getUserOperationReceipt', [hash], opChain);
					const realHash = (opRes.result as { receipt?: { transactionHash?: string } } | null)
						?.receipt?.transactionHash;
					// Not landed yet (or a transient bundler blip) — return null so the dApp
					// keeps polling, exactly as it would for an unmined tx.
					if (!realHash) return { handled: true, result: null };
					const real = await rpcCall(method, [realHash], opChain);
					return { handled: true, result: real.result ?? null };
				} catch {
					return { handled: true, result: null };
				}
			}
			// Not one of our hashes — forward untouched on the current chain.
			try {
				const res = await rpcCall(method, params ?? [], chainId);
				return { handled: true, result: res.result ?? null };
			} catch {
				return { handled: false };
			}
		}
		default:
			// Forward ONLY explicitly allow-listed read methods. Never blindly forward
			// an unknown method (which could reach debug/trace/admin namespaces) just
			// because it is not a signing method.
			if (FORWARDABLE_READ_METHODS.has(method)) {
				try {
					const res = await rpcCall(method, params ?? [], chainId);
					return { handled: true, result: res.result ?? null };
				} catch {
					return { handled: false };
				}
			}
			return { handled: false };
	}
}
