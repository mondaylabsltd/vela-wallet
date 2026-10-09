/**
 * The pure kernels of the Rust core, behind the legacy TypeScript signatures
 * the money path was written against.
 *
 * Ported from src/services/vela-core/{index,types,convert,js-helpers}.ts @
 * f9bcb278. The web's initialization is `loadCore()` in `client.ts` (async,
 * idempotent) — every caller here runs after a boot that awaited it, so the
 * Expo module's import-time `initSync` and its Node byte-planting are gone;
 * nothing else changed. `client.ts` keeps the onboarding/identicon/registry
 * exports; money code imports THIS module and nothing from the wasm glue
 * directly.
 */
import * as wasm from '../../../../../rust/pkg-web/vela_core.js';
import type { Assertion } from '$lib/onboarding/core/passkey';
import type { SigningPage } from '$lib/core/generated/SigningPage';
import type { SigningPlan } from '$lib/core/generated/SigningPlan';
import type { SigningVenue } from '$lib/core/generated/SigningVenue';
import type { VenueBlock } from '$lib/core/generated/VenueBlock';
import type { VenueChoice } from '$lib/core/generated/VenueChoice';
import type { FeeCall } from '$lib/core/generated/FeeCall';
import type { FeeFailure } from '$lib/core/generated/FeeFailure';
import type { FeedDappContent } from '$lib/core/generated/FeedDappContent';
import type { ReadSlot } from '$lib/core/generated/ReadSlot';
import type { SignEnding } from '$lib/core/generated/SignEnding';
import type { SignEndingState } from '$lib/core/generated/SignEndingState';
import type { SignErrorKind } from '$lib/core/generated/SignErrorKind';
import type { SignResponsePayload } from '$lib/core/generated/SignResponsePayload';
import type { StableRef } from '$lib/core/generated/StableRef';
import type { TokenRef } from '$lib/core/generated/TokenRef';
import type { TrackEntryView } from '$lib/core/generated/TrackEntryView';
import type { TrackStatusAnswer } from '$lib/core/generated/TrackStatusAnswer';
import type { ConfirmState } from '$lib/core/generated/ConfirmState';
import type { LandingPace } from '$lib/core/generated/LandingPace';
import type { DappChainAsk } from '$lib/core/generated/DappChainAsk';
import type { DappAddOutcome } from '$lib/core/generated/DappAddOutcome';
import type { SignView } from '$lib/core/generated/SignView';
import type { GuardView } from '$lib/core/generated/GuardView';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { MarkView } from '$lib/core/generated/MarkView';

export {
	PROXY_CREATION_CODE,
	SAFE_PROXY_RUNTIME_CODE,
	SAFE_PROXY_FACTORY,
	SAFE_SINGLETON,
	FALLBACK_HANDLER,
	ENTRY_POINT,
	SAFE_4337_MODULE,
	SAFE_MODULE_SETUP,
	WEBAUTHN_SIGNER,
	MULTI_SEND,
	VELA_SPLITTER_FACTORY,
	VELA_SPLITTER_SALT,
	VELA_SPLITTER_CREATION_CODE
} from './safe-constants';

// ---------------------------------------------------------------------------
// Types (vela-core/types.ts)
// ---------------------------------------------------------------------------

/** Decoded-calldata tree node — see specs/001-rust-core-bindings/data-model.md. */
export interface AbiValue {
	kind: string;
	name: string;
	value: string;
	children: AbiValue[];
}

export interface CoreErrorShape {
	code: string;
	message: string;
}

export interface AbiParam {
	type: string;
	name: string;
	components?: AbiParam[];
}

/** EIP-712 typed data as received from dApps. */
export interface TypedData {
	types: Record<string, TypedDataField[]>;
	primaryType: string;
	domain: Record<string, unknown>;
	message: Record<string, unknown>;
}

export interface TypedDataField {
	name: string;
	type: string;
}

export interface VerifyResult {
	ok: boolean;
	reason?: string;
}

export interface RecoverableAssertion {
	signatureHex: string;
	authenticatorDataHex: string;
	clientDataJSONHex: string;
}

/** Legacy decoded value union (mirrors abi-decode.ts DecodedValue). */
export type DecodedValue =
	| string
	| bigint
	| boolean
	| Array<string | bigint | boolean | Record<string, unknown>>
	| Record<string, unknown>;

// ---------------------------------------------------------------------------
// Conversions (vela-core/convert.ts)
// ---------------------------------------------------------------------------

/**
 * Hex → bytes for values crossing INTO the core. Strict on purpose: a lenient
 * parser would re-introduce silence one layer below the strict core.
 */
export function bytesFromHex(hex: string): Uint8Array {
	const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
	if (clean.length % 2 !== 0) {
		throw new Error(`vela-core: odd-length hex string (${clean.length} chars)`);
	}
	const out = new Uint8Array(clean.length / 2);
	for (let i = 0; i < out.length; i++) {
		const pair = clean.slice(i * 2, i * 2 + 2);
		if (!/^[0-9a-fA-F]{2}$/.test(pair)) {
			throw new Error(`vela-core: invalid hex pair \`${pair}\``);
		}
		out[i] = parseInt(pair, 16);
	}
	return out;
}

function nodeToLegacy(node: AbiValue): DecodedValue {
	if (node.kind === 'tuple') return tupleToRecord(node);
	if (node.kind.endsWith(']'))
		return node.children.map((child) => nodeToLegacy(child)) as DecodedValue;
	if (node.kind === 'address') return node.value.toLowerCase();
	if (node.kind === 'bool') return node.value === 'true';
	if (node.kind.startsWith('uint') || node.kind.startsWith('int')) {
		return node.value.startsWith('-') ? -BigInt(node.value.slice(1)) : BigInt(node.value);
	}
	return node.value;
}

function tupleToRecord(node: AbiValue): Record<string, DecodedValue> {
	const out: Record<string, DecodedValue> = {};
	node.children.forEach((child, index) => {
		out[child.name || `_${index}`] = nodeToLegacy(child);
	});
	return out;
}

export function abiTreeToLegacyRecord(tree: AbiValue): Record<string, DecodedValue> {
	return tupleToRecord(tree);
}

// ---------------------------------------------------------------------------
// The handful of helpers the core deliberately does NOT own (js-helpers.ts)
// ---------------------------------------------------------------------------

export function addHexPrefix(hex: string): string {
	return hex.startsWith('0x') ? hex : `0x${hex}`;
}

export function stripHexPrefix(hex: string): string {
	return hex.startsWith('0x') ? hex.slice(2) : hex;
}

export function concatBytes(...arrays: Uint8Array[]): Uint8Array {
	const totalLength = arrays.reduce((sum, arr) => sum + arr.length, 0);
	const result = new Uint8Array(totalLength);
	let offset = 0;
	for (const arr of arrays) {
		result.set(arr, offset);
		offset += arr.length;
	}
	return result;
}

/** "transfer(address _to, uint256 _value)" → name + params. */
export function parseSignature(sig: string): { name: string; params: AbiParam[] } {
	const parenIdx = sig.indexOf('(');
	if (parenIdx === -1) return { name: sig, params: [] };
	const name = sig.slice(0, parenIdx);
	const body = sig.slice(parenIdx + 1, sig.lastIndexOf(')'));
	return { name, params: parseParamList(body) };
}

function parseParamList(body: string): AbiParam[] {
	if (!body.trim()) return [];
	const params: AbiParam[] = [];
	let depth = 0;
	let current = '';
	for (const ch of body) {
		if (ch === '(') depth++;
		if (ch === ')') depth--;
		if (ch === ',' && depth === 0) {
			params.push(parseOneParam(current.trim()));
			current = '';
		} else {
			current += ch;
		}
	}
	if (current.trim()) params.push(parseOneParam(current.trim()));
	return params;
}

function parseOneParam(raw: string): AbiParam {
	if (raw.startsWith('(')) {
		const closeIdx = findMatchingParen(raw, 0);
		const tupleBody = raw.slice(1, closeIdx);
		const rest = raw.slice(closeIdx + 1).trim();
		let arrayStr = '';
		let name: string;
		if (rest.startsWith('[')) {
			const bIdx = rest.indexOf(']');
			arrayStr = rest.slice(0, bIdx + 1);
			name = rest.slice(bIdx + 1).trim();
		} else {
			name = rest.replace(/^\s+/, '');
		}
		return { type: 'tuple' + arrayStr, name, components: parseParamList(tupleBody) };
	}
	const parts = raw.split(/\s+/);
	if (parts.length === 1) return { type: parts[0], name: '' };
	return { type: parts[0], name: parts.slice(1).join(' ') };
}

function findMatchingParen(s: string, start: number): number {
	let depth = 0;
	for (let i = start; i < s.length; i++) {
		if (s[i] === '(') depth++;
		if (s[i] === ')') {
			depth--;
			if (depth === 0) return i;
		}
	}
	return s.length - 1;
}

// ---------------------------------------------------------------------------
// Error translation
// ---------------------------------------------------------------------------

const USER_FACING: Record<string, string> = {
	Eip712NonCanonicalDomain:
		"This site's signature request uses a non-standard EIP-712 domain, so the signature it produced could not be verified by the site itself. Vela declined to sign it.",
	Eip712Parse: "This site's signature request is malformed and cannot be signed.",
	AbiParse: 'The function signature for this call could not be parsed.',
	AbiDecode: 'This transaction data does not match the function it claims to call.',
	InvalidClientData: 'Your passkey provider returned a response Safe contracts cannot verify.',
	InvalidPublicKey: 'The passkey public key could not be read.'
};

function translateCoreError(e: unknown): unknown {
	if (typeof e !== 'object' || e === null || !('code' in e)) return e;
	const { code, message } = e as { code?: unknown; message?: unknown };
	if (typeof code !== 'string') return e;
	const friendly = USER_FACING[code];
	const error = new Error(friendly ?? (typeof message === 'string' ? message : code), { cause: e });
	(error as Error & { coreCode?: string }).coreCode = code;
	return error;
}

function translated<T>(run: () => T): T {
	try {
		return run();
	} catch (e) {
		throw translateCoreError(e);
	}
}

// ---------------------------------------------------------------------------
// primitives
// ---------------------------------------------------------------------------

export function keccak256(data: Uint8Array): Uint8Array {
	return wasm.keccak256(data);
}

export function sha256(data: Uint8Array): Uint8Array {
	return wasm.sha256(data);
}

export function toHex(data: Uint8Array): string {
	return wasm.toHex(data, false);
}

export function fromHex(hex: string): Uint8Array {
	return translated(() => wasm.fromHex(hex));
}

export function toQuantity(value: string | number | bigint | undefined | null): string {
	let asString: string;
	if (value === undefined || value === null) asString = '';
	else if (typeof value === 'bigint') asString = value.toString();
	else if (typeof value === 'number')
		asString = Number.isInteger(value) ? BigInt(value).toString() : String(value);
	else asString = value;
	return translated(() => wasm.toQuantity(asString));
}

export function toBase64Url(data: Uint8Array): string {
	return wasm.toBase64Url(data);
}

export function fromBase64Url(s: string): Uint8Array {
	return translated(() => wasm.fromBase64Url(s));
}

export function checksumAddress(address: string): string {
	return translated(() => wasm.checksumAddress(address));
}

export function functionSelector(signature: string): Uint8Array {
	return translated(() => wasm.functionSelector(signature));
}

export function create2Address(
	factory: string,
	salt: Uint8Array,
	initCodeHash: Uint8Array
): string {
	return translated(() => wasm.create2Address(factory, salt, initCodeHash));
}

export function abiEncodeAddress(address: string): Uint8Array {
	return translated(() => wasm.abiEncodeAddress(address));
}

export function abiEncodeUint256(value: bigint | number): Uint8Array {
	const hex = `0x${BigInt(value).toString(16)}`;
	return wasm.abiEncodeUint256(hex);
}

export function abiEncodeUint256Hex(hex: string): Uint8Array {
	return wasm.abiEncodeUint256(hex);
}

export function abiEncodeBytes32(data: Uint8Array): Uint8Array {
	return translated(() => wasm.abiEncodeBytes32(data));
}

export function keccak256Hex(hex: string): Uint8Array {
	return keccak256(fromHex(hex));
}

/**
 * One MultiSend sub-transaction: operation(1) ‖ to(20) ‖ value(32, zero) ‖
 * dataLength(32) ‖ data. Assembly, not computation — the single implementation.
 */
export function encodeMultiSendTx(to: string, data: Uint8Array, operation: number): Uint8Array {
	const toBytes = fromHex(stripHexPrefix(to));
	const operationByte = new Uint8Array([operation]);
	const value = new Uint8Array(32);
	const lenBytes = abiEncodeUint256(data.length);
	return concatBytes(operationByte, toBytes, value, lenBytes, data);
}

// ---------------------------------------------------------------------------
// abi
// ---------------------------------------------------------------------------

export function canonicalize(sig: string): string {
	return translated(() => wasm.canonicalizeSignature(sig));
}

/** Legacy contract: bare hex, NO 0x prefix. */
export function computeSelector(sig: string): string {
	return translated(() => wasm.computeSelector(sig).slice(2));
}

/** Legacy contract: `null` on any failure so the sheet falls back to raw calldata. */
export function decodeCalldata(calldata: string, sig: string): Record<string, DecodedValue> | null {
	try {
		return abiTreeToLegacyRecord(wasm.decodeCalldata(sig, bytesFromHex(calldata)) as AbiValue);
	} catch {
		return null;
	}
}

export function matchSelector(calldata: string, signatures: string[]): string | null {
	const bytes = bytesFromHex(calldata);
	for (const sig of signatures) {
		try {
			if (wasm.matchSelector(sig, bytes)) return sig;
		} catch {
			/* an unparseable candidate cannot match */
		}
	}
	return null;
}

// ---------------------------------------------------------------------------
// eip712
// ---------------------------------------------------------------------------

/**
 * The ONE document a typed-data request is read as — the core's
 * `typed_data_request`, the same bytes `signMessageHash` covers: the four
 * methods, exactly two params, a real account in its slot, one EIP-712
 * document. `null` when the request is not one (the core refuses it before a
 * sheet). The audit of 2026-10-01: the sheet previewed the first string while
 * the passkey signed `params[1] ?? params[0]`.
 */
export function typedDataDocument(method: string, paramsJson: string): string | null {
	return wasm.typedDataDocument(method, paramsJson) ?? null;
}

/**
 * The calls a dApp transaction request sends — the core's ONE reading
 * (`tx_request::calls_of`, spec 096 F1): `params[0].calls` of a
 * `wallet_sendCalls`, else `params[0]`, every one or none, `value` in DECIMAL
 * wei (the `FeeCall` convention). `null` when a call is unreadable, names no
 * recipient, or there are none. Each reader of `value` used to be its own —
 * the fee quote read `"1000"` as decimal, the submit as hex, and the gas
 * floor threw on PancakeSwap's `0xaa87bee538000` once its prefix was gone.
 */
export function dappRequestCalls(method: string, paramsJson: string): FeeCall[] | null {
	const json = wasm.dappRequestCalls(method, paramsJson);
	return json === undefined ? null : (JSON.parse(json) as FeeCall[]);
}

export function hashTypedData(typedData: TypedData): Uint8Array {
	return translated(() => wasm.hashTypedData(JSON.stringify(typedData)));
}

// ---------------------------------------------------------------------------
// safe
// ---------------------------------------------------------------------------

export function computeAddress(publicKeyHex: string): string {
	return translated(() => {
		const key = wasm.parsePublicKey(publicKeyHex);
		return wasm.computeSafeAddress(bytesFromHex(key.x), bytesFromHex(key.y)).address;
	});
}

export function parsePublicKey(hex: string): { x: Uint8Array; y: Uint8Array } {
	return translated(() => {
		const key = wasm.parsePublicKey(hex);
		return { x: bytesFromHex(key.x), y: bytesFromHex(key.y) };
	});
}

export function calculateSaltNonce(x: Uint8Array, y: Uint8Array): Uint8Array {
	return translated(() => bytesFromHex(wasm.computeSafeAddress(x, y).salt_nonce));
}

export function encodeSetupData(x: Uint8Array, y: Uint8Array): Uint8Array {
	return translated(() => bytesFromHex(wasm.computeSafeAddress(x, y).setup_data));
}

function concatKeyBlocks(publicKeyHexes: string[]): Uint8Array {
	const blocks = new Uint8Array(publicKeyHexes.length * 64);
	publicKeyHexes.forEach((hex, index) => {
		const key = wasm.parsePublicKey(hex);
		blocks.set(bytesFromHex(key.x), index * 64);
		blocks.set(bytesFromHex(key.y), index * 64 + 32);
	});
	return blocks;
}

/** The counterfactual Safe for a founding key set; N=1 is byte-identical to `computeAddress`. */
export function computeSafeAddressMulti(publicKeyHexes: string[]): {
	address: string;
	saltNonce: Uint8Array;
	setupData: Uint8Array;
} {
	return translated(() => {
		const info = wasm.computeSafeAddressMulti(concatKeyBlocks(publicKeyHexes));
		return {
			address: info.address,
			saltNonce: bytesFromHex(info.salt_nonce),
			setupData: bytesFromHex(info.setup_data)
		};
	});
}

export function computeAddressMulti(publicKeyHexes: string[]): string {
	return computeSafeAddressMulti(publicKeyHexes).address;
}

/** The per-key WebAuthn signer proxy a NON-first founding key verifies through. */
export function computeWebauthnSignerAddress(publicKeyHex: string): string {
	return translated(() => {
		const key = wasm.parsePublicKey(publicKeyHex);
		return wasm.computeWebauthnSignerAddress(bytesFromHex(key.x), bytesFromHex(key.y));
	});
}

export function computeSplitterAddress(treasury: string): string {
	return translated(() => wasm.computeSplitterAddress(treasury));
}

export function encodeSplitterDeployCall(treasury: string): Uint8Array {
	return translated(() => wasm.encodeSplitterDeployCall(treasury));
}

// ---------------------------------------------------------------------------
// webauthn
// ---------------------------------------------------------------------------

export function extractPublicKey(
	attestationObject: Uint8Array
): { x: Uint8Array; y: Uint8Array } | null {
	try {
		const key = wasm.extractAttestationPublicKey(attestationObject);
		return { x: bytesFromHex(key.x), y: bytesFromHex(key.y) };
	} catch {
		return null;
	}
}

/** Legacy contract: `null` on malformed DER. */
export function derSignatureToRaw(derSig: Uint8Array): Uint8Array | null {
	try {
		return wasm.derSignatureToRawLowS(derSig);
	} catch {
		return null;
	}
}

/** Whether a WebAuthn assertion is one Safe's signer contract can verify. */
export function verifySafeWebAuthn(
	assertion: Pick<Assertion, 'clientDataJSONHex' | 'authenticatorDataHex'>
): VerifyResult {
	try {
		wasm.validateClientData(
			'get',
			bytesFromHex(assertion.clientDataJSONHex),
			bytesFromHex(assertion.authenticatorDataHex)
		);
		return { ok: true };
	} catch (e) {
		const message =
			typeof e === 'object' && e !== null && 'message' in e
				? String((e as { message: unknown }).message)
				: String(e);
		return { ok: false, reason: message };
	}
}

/** Uncompressed `04||x||y` hex, or null when not unique. */
export function recoverPublicKeyFromAssertions(
	first: RecoverableAssertion,
	second: RecoverableAssertion
): string | null {
	try {
		const key = wasm.recoverPublicKeyFromAssertions(
			bytesFromHex(first.authenticatorDataHex),
			bytesFromHex(first.clientDataJSONHex),
			bytesFromHex(first.signatureHex),
			bytesFromHex(second.authenticatorDataHex),
			bytesFromHex(second.clientDataJSONHex),
			bytesFromHex(second.signatureHex)
		);
		if (!key) return null;
		return `04${key.x.slice(2)}${key.y.slice(2)}`;
	} catch {
		return null;
	}
}

// ---------------------------------------------------------------------------
// user_op — the core's assembly, for checking the shell's (spec 028 Phase 8)
// ---------------------------------------------------------------------------

/** One sub-call as the shell built it (`MultiSendCall`'s shape). */
export interface AttestCall {
	to: string;
	/** Hex, `0x` optional, empty meaning zero. */
	value: string;
	data: Uint8Array;
}

/** The in-band fee leg by its inputs — the core builds the leg itself. */
export interface AttestFeeLeg {
	gasFeeToken: string | null;
	recipient: string;
	amount: bigint;
}

export interface AttestCalls {
	inner: AttestCall[];
	fee: AttestFeeLeg | null;
	alwaysMultiSend: boolean;
}

/** The operation as it will be hashed; `signature` is not part of the hash. */
export interface AttestOp {
	sender: string;
	nonce: string;
	initCode: Uint8Array;
	callData: Uint8Array;
	verificationGasLimit: bigint;
	callGasLimit: bigint;
	preVerificationGas: bigint;
	maxFeePerGas: bigint;
	maxPriorityFeePerGas: bigint;
	paymasterAndData: Uint8Array;
}

/**
 * The SafeOp hash the core computes for `op` — after rebuilding the calldata
 * from `calls` and refusing when the bytes differ. `calls === null` attests
 * the hash alone (the legacy path hands over finished calldata).
 */
export function attestSafeOpHash(
	op: AttestOp,
	calls: AttestCalls | null,
	chainId: number
): Uint8Array {
	return translated(() => {
		const opJson = attestOpJson(op);
		const callsJson =
			calls === null
				? ''
				: JSON.stringify({
						inner: calls.inner.map((call) => ({
							to: call.to,
							value_hex: call.value,
							data_hex: toHex(call.data)
						})),
						fee:
							calls.fee === null
								? null
								: {
										gas_fee_token: calls.fee.gasFeeToken,
										recipient: calls.fee.recipient,
										amount_hex: '0x' + calls.fee.amount.toString(16)
									},
						always_multi_send: calls.alwaysMultiSend
					});
		return wasm.attestSafeOpHash(opJson, callsJson, BigInt(chainId));
	});
}

/** The Safe message hash (EIP-1271) as the core computes it. */
export function attestSafeMessageHash(
	originalHash: Uint8Array,
	chainId: number,
	safeAddress: string
): Uint8Array {
	return translated(() => wasm.attestSafeMessageHash(originalHash, BigInt(chainId), safeAddress));
}

// ---------------------------------------------------------------------------
// Where a signature goes (spec 102)
// ---------------------------------------------------------------------------

/**
 * How an account signs on this device — `vela_core::app::Account::
 * signing_plan`: its signing DOMAIN (the RP ID its keys live under), its
 * VENUE (where transactions and messages are reviewed and signed: in Vela, or
 * a trusted page) and its KEY route (the key it was created or last signed in
 * with here, and the place that reached it).
 *
 * `account` is the record as STORED: the core's reader migrates a record
 * written before spec 102 (`signed_in_with`, a key's `signer_origin`) on the
 * way in, so this is the one place the web learns any of the three — never
 * the raw record's fields. `null` for a record this build cannot read.
 *
 * `surface: 'web'` is the plan as THIS shell must follow it
 * (`SigningPlan::on_web`): the web opens no signing page, so a `getvela.app`
 * account whose venue is a page signs in Vela, and a custom-domain account
 * comes back `blocked` (`not_on_web`) — sign nothing, and tell the sign/send
 * core `venue_blocked` so the sheet says why. Without it, the plan an app
 * would follow (which the web still asks when it only wants the key route).
 */
export function signingPlan(account: unknown, surface?: 'web'): SigningPlan | null {
	const plan = wasm.signingPlan(JSON.stringify(account), surface);
	return plan === undefined ? null : (JSON.parse(plan) as SigningPlan);
}

/**
 * Spec 102 R1 + R2: every venue an account on `domain` could pick, each
 * reachable or not and why. `surface: 'web'` adds the web's own reason: every
 * page row R1 does not already block is blocked `not_on_web` — the rows are
 * shown, disabled, with "Signing pages open from the Vela apps" (D-16).
 */
export function signingVenueChoices(
	domain: string,
	active: SigningVenue,
	saved: SigningPage[],
	surface?: 'web'
): VenueChoice[] {
	const choices = wasm.signingVenueChoices(
		domain,
		JSON.stringify(active),
		JSON.stringify(saved),
		surface
	);
	return choices === undefined ? [] : (JSON.parse(choices) as VenueChoice[]);
}

/**
 * Spec 102 R1: why `venue` cannot reach the keys of an account on `domain`,
 * or `null` when it can. The web asks it of `in_vela` to tell a custom-domain
 * account in Settings (P2-10: its keys say "Keys on {{domain}}" once). Whether
 * the web may SIGN for an account is the web plan's (`signingPlan(record,
 * 'web')`), not this.
 */
export function signingVenueBlock(domain: string, venue: SigningVenue): VenueBlock | null {
	const block = wasm.signingVenueBlock(domain, JSON.stringify(venue));
	return block === undefined ? null : (JSON.parse(block) as VenueBlock);
}

/**
 * A venue refusal's sentence as the core says it (`VenueBlock::key()` with
 * `VenueBlock::vars()`): the corpus key of the line, and the values it takes
 * by the corpus's own names (`domain`, `pageDomain`). Translate `key`, fill it
 * with `vars`, and the sentence is the core's whole — no shell decides which
 * fact fills which placeholder.
 */
export interface VenueBlockLine {
	key: string;
	vars: Record<string, string>;
}

/** The words of a venue refusal (spec 102), or `null` for something that is not a `VenueBlock`. */
export function venueBlockLine(block: VenueBlock): VenueBlockLine | null {
	const line = wasm.venueBlockLine(JSON.stringify(block));
	return line === undefined ? null : (JSON.parse(line) as VenueBlockLine);
}

/** The domain whose keys a page at `url` can use: its host, or `getvela.app` for `*.getvela.app`. */
export function signingPageDomain(url: string): string {
	return wasm.signingPageDomain(url);
}

/** A venue row's corpus keys (`{title_key, line_key, line_name}`), or `null` for a name the core does not know. */
export interface VenueWords {
	title_key: string;
	line_key: string;
	line_name: string;
}

/**
 * `signing_page` is the apps' create / sign-in entry "Use a trusted signing
 * page" (D6). The web's choosers do not offer it (P2b-W3: it opens no page),
 * so the web asks only the two venue rows' words.
 */
export function venueWords(row: 'in_vela' | 'page' | 'signing_page'): VenueWords | null {
	const words = wasm.venueWords(row);
	return words === undefined ? null : (JSON.parse(words) as VenueWords);
}

/**
 * `{{time}}` in a signing page's integrity line, "… · checked {{time}}"
 * (D-13) — `launch::checked_time`: the clock time in the person's format when
 * the check ran today, else the date and the time. `date` / `time` are the
 * person's presets with `auto` resolved; `utcOffsetMinutes` is
 * `-new Date().getTimezoneOffset()`. The web checks no page; its gallery
 * boards (the design the apps' screens build to) draw their lines with it.
 */
export function signerIntegrityTime(
	checkedAtMs: number,
	nowMs: number,
	utcOffsetMinutes: number,
	formats: { date: string; time: string },
	language: string
): string {
	return wasm.signerIntegrityTime(
		checkedAtMs,
		nowMs,
		utcOffsetMinutes,
		formats.date,
		formats.time,
		language
	);
}

// ---------------------------------------------------------------------------
// Keys and relying parties
// ---------------------------------------------------------------------------

/** One of the account's keys, as a signature by it is checked against them. */
export interface TrustedSignerKey {
	credentialId: string;
	publicKeyHex: string;
}

/**
 * The relying party a ceremony that runs on `page` is filed and proven under,
 * `null` for the app's own (spec 102 R3: `page` is set only for a wallet on a
 * custom domain, whose keys only its page can mint or use).
 *
 * A key made on a page is signed under THAT page's domain, so a challenge
 * fetched under the wallet's own could never match the answer.
 */
export function trustedSignerRegistryRpId(page: string | null): string | null {
	return wasm.trustedSignerRegistryRpId(page ?? undefined) ?? null;
}

// ---------------------------------------------------------------------------
// Chain gas floor (spec 060)
// ---------------------------------------------------------------------------

/**
 * The lowest gas price a chain will ACCEPT, in wei.
 *
 * `0n` on every chain but Arc, which discards an operation priced under its
 * 20 gwei minimum base fee SILENTLY — no error, no trace, a payment that looks
 * submitted and never happens. The number lives in the core
 * (`fee_policy::min_gas_price_wei`); this is the money path's door to it, not a
 * second copy.
 */
export function minGasPriceWei(chainId: number): bigint {
	return BigInt(wasm.minGasPriceWei(chainId));
}

/**
 * The USD price of a native gas coin that IS a dollar stablecoin — Tempo's
 * `USD`, Arc's `USDC`. `null` means "not pegged": the caller falls through to
 * the Chainlink/DEX ladder unchanged.
 *
 * One table, in the core, because this rule used to be a `symbol === 'USD'`
 * literal in each of the four shells (spec 060).
 */
export function peggedNativeUsd(symbol: string): number | null {
	return wasm.peggedNativeUsd(symbol) ?? null;
}

/**
 * How long a submitted operation usually takes to land on a chain, in seconds.
 *
 * `0` where Vela ships no estimate — the receipt's ring then circles instead of
 * filling, which is the honest drawing of a wallet that does not know.
 *
 * The send receipt gets this number inside its own `SendReceiptView`. A dApp
 * transaction lands on the SAME receipt but arrives through `tx_tracker`, whose
 * entries carry no estimate, so spec 077's landing asks the core here rather
 * than the web keeping a second copy of the chain table.
 */
export function typicalInclusionSeconds(chainId: number): number {
	return wasm.typicalInclusionSeconds(chainId);
}

/**
 * Spec 099 R7: may the signing confirm open, and if not, why — the core's one
 * gate (`sign_confirm::confirm_state`), the same on every client. `null`
 * when a view does not read: the confirm stays shut.
 */
export function signConfirmState(
	sign: SignView,
	guard: GuardView,
	clear: ClearSigningView,
	fee: FeeView | null,
	speedTier: FeeTier | null
): ConfirmState | null {
	const out = wasm.signConfirmState(
		JSON.stringify(sign),
		JSON.stringify(guard),
		JSON.stringify(clear),
		fee === null ? undefined : JSON.stringify(fee),
		speedTier ?? undefined
	);
	return out ? (JSON.parse(out) as ConfirmState) : null;
}

/**
 * Spec 099 R6: the landing's countdown and ring, counted from when the relay
 * put the bundle on the network (`TrackEntryView.relay_sent_at_ms`) — the
 * core's one ladder. `typicalS` `0` = no usual time for this chain.
 */
export function landingPace(sentAtMs: number | null, typicalS: number, nowMs: number): LandingPace {
	return JSON.parse(wasm.landingPace(sentAtMs ?? undefined, typicalS, nowMs)) as LandingPace;
}

/**
 * A `FeeFailure` as the wasm exports take it: the wire name for the plain
 * variants, the JSON for `ChainRead` (spec 082 RJ13), which is an object.
 */
function feeFailureWire(failure: FeeFailure): string {
	return typeof failure === 'string' ? failure : JSON.stringify(failure);
}

/**
 * The wait before automatic fee re-quote `attempt` (1-based) after `failure`,
 * or `null` when no retry can fix it (`fee_policy::requote_delay_ms`: 3 s,
 * 6 s, then every 8 s for a relay out of reach, a busy estimate or a chain
 * read that got no answer — spec 082 RJ12; never for a missing public key or
 * a calculation that cannot come out). The signing sheet re-asks on the
 * schedule every other client uses.
 */
export function feeRequoteDelayMs(failure: FeeFailure, attempt: number): number | null {
	return wasm.feeRequoteDelayMs(feeFailureWire(failure), attempt) ?? null;
}

/**
 * The bound on each automatic fee re-quote, in ms (`fee_policy::REQUOTE_TIMEOUT_MS`,
 * spec 082 RJ12): a re-ask that has not settled by then is given up and the
 * next one is scheduled, so the fee is back within wait + bound of the relay
 * returning.
 */
export function feeRequoteTimeoutMs(): number {
	return wasm.feeRequoteTimeoutMs();
}

/**
 * The corpus key of the reason line under a failed fee, or `null` for none
 * (`fee_policy::failure_reason_key`, spec 082 RJ13): the relay out of reach,
 * a rate-limited chain node, or a chain node out of reach (`explore.chainDown`,
 * whose `{{chain}}` the shell fills). The shell never picks these words itself.
 */
export function feeFailureReasonKey(failure: FeeFailure): string | null {
	return wasm.feeFailureReasonKey(feeFailureWire(failure)) ?? null;
}

/**
 * Issue 212's fee-signal cache is a SHELL cache with core rules: how long a
 * chain's gas signals and the relay's quote may be held, and which readings
 * may be held at all (`fee_policy::FEE_SIGNALS_CACHE_TTL_MS`,
 * `gas_signals_cacheable`, `bundler_quote_cacheable`). Every shell asks the
 * same three questions of the core instead of carrying its own answers.
 */
export function feeSignalsCacheTtlMs(): number {
	return wasm.feeSignalsCacheTtlMs();
}

/** A gas-signal read may be held only when it is complete and real. Decimal wei. */
export function gasSignalsCacheable(
	ethGasPrice: string | null,
	blockAnswered: boolean,
	wantTip: boolean,
	priorityFee: string | null
): boolean {
	return wasm.gasSignalsCacheable(ethGasPrice, blockAnswered, wantTip, priorityFee);
}

/** The relay's gas quote for one tier may be held only when its cap is real. */
export function bundlerQuoteCacheable(maxFeePerGas: string): boolean {
	return wasm.bundlerQuoteCacheable(maxFeePerGas);
}

/**
 * Spec 073: an amount field's text as the core reads it — ASCII digits and one
 * `.` — or `null` for a paste with no reading as one figure, which the field
 * refuses whole (`l10n::amount_text` says why; issue 231). `preset` is the
 * resolved number preset (`resolvedFormatKeys().number`); `previous` the
 * field's text before this edit, which is how one keystroke is told from a
 * paste or an autofill.
 */
export function amountTextClean(
	raw: string,
	preset: string,
	pasted: boolean,
	previous?: string
): string | null {
	return wasm.amountTextClean(raw, preset, previous, pasted) ?? null;
}

/**
 * Where the caret belongs in `clean`, having been at `caret` in `raw`: after as
 * many KEPT characters as stood before it (UTF-16 units, as `selectionStart`).
 */
export function amountTextCaret(raw: string, clean: string, caret: number): number {
	return wasm.amountTextCaret(raw, clean, caret);
}

/** The corpus's `time.now` / `time.minutesShort` / `time.hoursShort`, `{{n}}` unfilled. */
export interface RelativeTimeWords {
	now: string;
	minutes: string;
	hours: string;
}

/**
 * The core's compact relative time (`I18n::format_relative_time`): "now"
 * under 45 s, then rounded minutes and hours, a short weekday under a week,
 * else the date — one rule for every app. The web carries no catalog, so it
 * hands the core the three words its prerendered messages hold; `language`
 * (the page's locale) names the weekday. `atMs` is the moment, `nowMs` the
 * clock; `utcOffsetMinutes` what to add to UTC for local time;
 * `dateFormat` the person's preset with `auto` resolved
 * (`resolvedFormatKeys().date`).
 */
export function formatRelativeTime(
	atMs: number,
	nowMs: number,
	utcOffsetMinutes: number,
	dateFormat: string,
	language: string,
	words: RelativeTimeWords
): string {
	// The core reads the moment in WHOLE seconds; milliseconds there would
	// read as the future, which is "now".
	return translated(() =>
		wasm.formatRelativeTime(
			Math.floor(atMs / 1000),
			nowMs,
			utcOffsetMinutes,
			dateFormat,
			language,
			words.now,
			words.minutes,
			words.hours
		)
	);
}

// ---------------------------------------------------------------------------
// Spec 082 — submit, answer, reads: the core's rules the web shell draws from
// ---------------------------------------------------------------------------

/** The operation in the `attestSafeOpHash` wire shape (gas decimal, bytes hex). */
function attestOpJson(op: AttestOp): string {
	return JSON.stringify({
		sender: op.sender,
		nonce: op.nonce,
		init_code_hex: toHex(op.initCode),
		call_data_hex: toHex(op.callData),
		verification_gas_limit: op.verificationGasLimit.toString(),
		call_gas_limit: op.callGasLimit.toString(),
		pre_verification_gas: op.preVerificationGas.toString(),
		max_fee_per_gas: op.maxFeePerGas.toString(),
		max_priority_fee_per_gas: op.maxPriorityFeePerGas.toString(),
		paymaster_and_data_hex: toHex(op.paymasterAndData)
	});
}

/**
 * The EntryPoint v0.7 `getUserOpHash` of `op` on `chainId`, 0x-lowercase —
 * `user_op::user_op_hash` (RA6). Known before the POST, so an op whose reply is
 * lost still has a name to be followed by. The relay's own hash always wins.
 */
export function userOpHash(op: AttestOp, chainId: number): string {
	return translated(() => wasm.userOpHash(attestOpJson(op), BigInt(chainId)));
}

/** The gas limits an operation is signed with. */
export interface SignedGasLimits {
	verificationGasLimit: bigint;
	callGasLimit: bigint;
	preVerificationGas: bigint;
}

/**
 * The gas limits a submit signs, from the relay's estimate — the core's ONE
 * rule (`user_op::in_band_gas_limits`, or Tempo's own padding on a Tempo
 * chain), raised to the inner calls' measured floor when there is one. No
 * shell pads the relay's answer itself: the limits it signs are the limits the
 * fee was priced on. `subCalls` is the person's calls plus the fee leg (Tempo's
 * call floor grows with it); `settlementGas` is the relay's, `null` when it
 * published none.
 */
export function userOpGasLimits(
	chainId: number,
	deployed: boolean,
	subCalls: number,
	estimate: {
		verificationGasLimit: bigint;
		callGasLimit: bigint;
		preVerificationGas: bigint;
		settlementGas: bigint | null;
	},
	innerFloor: bigint | null
): SignedGasLimits {
	const limits = JSON.parse(
		translated(() =>
			wasm.userOpGasLimits(
				chainId,
				deployed,
				subCalls,
				estimate.verificationGasLimit.toString(),
				estimate.callGasLimit.toString(),
				estimate.preVerificationGas.toString(),
				estimate.settlementGas?.toString() ?? undefined,
				innerFloor?.toString() ?? undefined
			)
		)
	) as Record<'verificationGasLimit' | 'callGasLimit' | 'preVerificationGas', string>;
	return {
		verificationGasLimit: BigInt(limits.verificationGasLimit),
		callGasLimit: BigInt(limits.callGasLimit),
		preVerificationGas: BigInt(limits.preVerificationGas)
	};
}

/** What one POST of `eth_sendUserOperation` came back with (`SubmitReply`). */
export type SubmitReply =
	| { hash: string }
	/** The JSON-RPC `error` member, as the relay sent it. */
	| { error: unknown }
	/** The pool gave up: no endpoint answered. */
	| 'no_answer';

/** Why the relay refused an op it never queued (`RelayRejection`). */
export type RelayRejection =
	| 'relayer_unavailable'
	| 'bundler_underfunded'
	| { nonce_held: { user_op_hash: string } }
	| { other: string };

/** The end of one submit (`SubmitVerdict`). Hand-written: no ts-rs type exists. */
export type SubmitVerdict =
	| { type: 'accepted'; user_op_hash: string }
	/** The reply was lost after bytes may have left: `user_op_hash` is the local hash. */
	| { type: 'maybe_sent'; user_op_hash: string }
	/** Provably not queued. `null` = the relay was never reached. */
	| { type: 'not_sent'; rejection: RelayRejection | null };

/** One step of the submit loop (`SubmitStep`). */
export type SubmitStep = { retry_after: { delay_ms: number } } | { done: SubmitVerdict };

/**
 * One step of the submit loop — `user_op::submit_step` (RA1). `attempt` is the
 * 0-based count of POSTs of this op, the one just answered included;
 * `maybeDelivered` the OR over every POST of this op of the pool's verdict.
 */
export function userOpSubmitStep(
	reply: SubmitReply,
	attempt: number,
	maybeDelivered: boolean,
	localHash: string
): SubmitStep {
	return translated(
		() =>
			JSON.parse(
				wasm.userOpSubmitStep(JSON.stringify(reply), attempt, maybeDelivered, localHash)
			) as SubmitStep
	);
}

/** The dApp's `-32603` detail for an op that was not sent and has no refusal to quote (RA10). */
export function userOpNotSentDetail(): string {
	return wasm.userOpNotSentDetail();
}

/**
 * The core's sentence for a request that could not go out behind another of
 * the account's operations (083, `nonce_held`) — never that operation's hash.
 */
export function userOpPreviousPendingDetail(): string {
	return wasm.userOpPreviousPendingDetail();
}

/**
 * The dApp's `-32603` detail for an op the relay refused (spec 082 RJ3,
 * `user_op::REFUSED_DAPP_DETAIL`): "the network refused this transaction;
 * nothing was sent".
 */
export function userOpRefusedDappDetail(): string {
	return wasm.userOpRefusedDappDetail();
}

/**
 * How long a shell waits for the write-ahead's clearance after `OpSigned`
 * (`user_op::WRITE_AHEAD_WAIT_MS`, spec 082 RJ1). None in time → no POST,
 * and the submit is reported as not sent.
 */
export function userOpWriteAheadWaitMs(): number {
	return wasm.userOpWriteAheadWaitMs();
}

/**
 * What a failed relay estimate says (`user_op::EstimateFailure`, spec 082
 * RJ19). Hand-written: the core type carries no ts-rs derive. `reason` is
 * the revert's own words, already cleaned by the core (RG8), or `null`.
 */
export type EstimateFailure = { type: 'reverts'; reason: string | null } | { type: 'unavailable' };

/**
 * Classify a failed `eth_estimateUserOperationGas` (`user_op::estimate_failure`):
 * `errorJson` is the JSON-RPC `error` member, or the whole body, or `''` for
 * no answer. A revert ("reverted", AA23, -32521) → `reverts`; a timeout, an
 * exhausted pool, a rate limit, a signature (AA21) → `unavailable`.
 */
export function userOpEstimateFailure(errorJson: string): EstimateFailure {
	try {
		const parsed = JSON.parse(wasm.userOpEstimateFailure(errorJson)) as EstimateFailure;
		if (parsed.type === 'reverts') return { type: 'reverts', reason: parsed.reason ?? null };
		return { type: 'unavailable' };
	} catch {
		return { type: 'unavailable' };
	}
}

/**
 * A signed balance change, as the sheet writes it (`l10n::format_signed_token_amount`,
 * spec 082 RJ15): `null` for zero (never drawn), U+2212 for a minus, `+` for a
 * gain, and a dust amount written exactly instead of `−0`. `preset` is the
 * number preset's wire name (`comma_dot` when unknown).
 */
export function formatSignedTokenAmount(
	deltaBaseUnits: string | bigint,
	decimals: number,
	preset: string
): string | null {
	return wasm.formatSignedTokenAmount(String(deltaBaseUnits), decimals, preset) ?? null;
}

/** The relay's lifecycle-status method (RA7) — the only spelling it serves. */
export function userOpStatusMethod(): string {
	return wasm.userOpStatusMethod();
}

/** One parsed status answer (`TrackStatusAnswer`), or `null` for an unknown status. */
export function parseUserOpStatus(resultJson: string): TrackStatusAnswer | null {
	const answer = wasm.parseUserOpStatus(resultJson);
	return answer === undefined ? null : (JSON.parse(answer) as TrackStatusAnswer);
}

/**
 * The hash (or signature) an answer to a page names — a batch's id read out
 * of EIP-5792 2.0.0's `{ id }`, which the core shaped (spec 097 G) —
 * `SignResponsePayload::answered`. `null` for an error or a `null` answer.
 */
export function signAnswered(payload: SignResponsePayload): string | null {
	return wasm.signAnswered(JSON.stringify(payload)) ?? null;
}

/**
 * The message a page reads when signing ended in the error the core named
 * `kind`, with the notice's `detail` — `dapp_rpc::sign_error_words`. For the
 * dApp's developer, not the person: EIP-1193 messages are not UI (the person
 * reads the sheet, in their language). `null` for a kind this core does not
 * know.
 */
export function signErrorWords(kind: SignErrorKind, detail?: string | null): string | null {
	return wasm.signErrorWords(kind, detail ?? undefined) ?? null;
}

/** What the answer to a request stands for, before the tracker is asked (RA8). */
export function signEndingOf(
	method: string,
	payload: SignResponsePayload,
	submittedUserOp: string | null
): SignEnding | null {
	return translated(() => {
		const ending = wasm.signEndingOf(method, JSON.stringify(payload), submittedUserOp);
		return ending === undefined ? null : (JSON.parse(ending) as SignEnding);
	});
}

/** What the sheet draws for an ending once the tracker had its say (RA8). */
export function signEndingState(
	ending: SignEnding,
	entry: TrackEntryView | null | undefined
): SignEndingState {
	return translated(
		() =>
			JSON.parse(
				wasm.signEndingState(JSON.stringify(ending), entry ? JSON.stringify(entry) : null)
			) as SignEndingState
	);
}

/**
 * A dApp record's stored request as its "Technical details" show it (spec
 * 093, `dapp_activity::request_display`): typed data as its document,
 * pretty-printed; a message as its text (or its hex); call data as the
 * params, pretty-printed. `storedRequest` is the params' JSON text as the
 * record kept it — `''` when it kept nothing. `null` when there is nothing to
 * show; the shell then says it was not recorded. No shell formats it itself.
 */
export function dappRequestDisplay(content: FeedDappContent, storedRequest: string): string | null {
	return wasm.dappRequestDisplay(content, storedRequest) ?? null;
}

/** How long the dApp's receipt wait may still run, `elapsedMs` after approval (RA12). */
export function dappReceiptWaitMs(elapsedMs: number): number {
	return wasm.dappReceiptWaitMs(elapsedMs);
}

/**
 * The page's "not confirmed yet" for an operation that may still land —
 * `sign_request::not_confirmed_detail` (083). The extension's worker mirrors
 * it (`maybeSentPayload`); `protocol.test.ts` pins the mirror here.
 */
export function signNotConfirmedDetail(userOpHash: string): string {
	return wasm.signNotConfirmedDetail(userOpHash);
}

/** The extension's request lifetime — the core's `EXTENSION_REQUEST_TTL_MS` (RB11). */
export function signRequestTtlMs(): number {
	return wasm.signRequestTtlMs();
}

/** The bound on one whole fee quote — the core's `QUOTE_DEADLINE_MS` (spec 094 S9). */
export function feeQuoteDeadlineMs(): number {
	return wasm.feeQuoteDeadlineMs();
}

/**
 * How long one chain's balance read may take before the round counts it failed
 * — the core's `CHAIN_READ_DEADLINE_MS` (spec 092), the same on every shell.
 */
export function balanceChainReadDeadlineMs(): number {
	return wasm.balanceChainReadDeadlineMs();
}

/** One endpoint's read budget — the core's `RPC_READ_TIMEOUT_MS` (RF2). */
export function rpcReadTimeoutMs(): number {
	return wasm.rpcReadTimeoutMs();
}

/** An endpoint's cool-down after `consecutiveFailures` in a row (RF2). */
export function rpcCooldownMs(consecutiveFailures: number): number {
	return wasm.rpcCooldownMs(consecutiveFailures);
}

/** A site's name and the line under it (`SiteLabel`, hand-written — no ts-rs type). */
export interface SiteLabel {
	name: string;
	/** `null` when the name already is the host, so it is said once. */
	host_line: string | null;
}

/** `browser_load::site_label` (RE7). */
export function browserSiteLabel(title: string, host: string): SiteLabel {
	return JSON.parse(wasm.browserSiteLabel(title, host)) as SiteLabel;
}

/**
 * How long a failed remote mark stays failed (RE10): ms, or `null` for the
 * rest of the session. The web's `<img onerror>` has no status → `unknown`.
 */
export function markMissTtlMs(kind: string, status?: number | null): number | null {
	return wasm.markMissTtlMs(kind, status ?? null) ?? null;
}

/**
 * What a COIN's circle wears (`remote_mark::token_mark`): its glyph, its logo
 * candidates best first, and the corner badge's chain and logo (`null` = no
 * badge). `ethereumDataUrl` is the person's endpoint as stored; a blank one
 * is the built-in host, the core's call. `tokenAddress` is `null` for the
 * chain's own coin, never `''`. `named` are logos the API already named.
 */
export function tokenMark(
	ethereumDataUrl: string,
	chainId: number,
	symbol: string,
	tokenAddress: string | null,
	named: readonly string[]
): MarkView {
	return wasm.tokenMark(ethereumDataUrl, chainId, symbol, tokenAddress, [...named]) as MarkView;
}

/** What a NETWORK's circle wears (`remote_mark::chain_mark`): its own logo, never a badge. */
export function chainMark(
	ethereumDataUrl: string,
	chainId: number,
	nativeSymbol: string
): MarkView {
	return wasm.chainMark(ethereumDataUrl, chainId, nativeSymbol) as MarkView;
}

/** A chain's logo on the endpoint (`remote_mark::chain_logo_url`); none for chain 0. */
export function chainLogoUrl(ethereumDataUrl: string, chainId: number): string | undefined {
	return wasm.chainLogoUrl(ethereumDataUrl, chainId) ?? undefined;
}

/** Which balances one chain's read covers, in order (RE9). */
export function balanceReadPlan(
	chainId: number,
	stables: StableRef[],
	wrappedNative: string | null,
	custom: TokenRef[]
): ReadSlot[] {
	return translated(
		() =>
			JSON.parse(
				wasm.balanceReadPlan(
					chainId,
					JSON.stringify(stables),
					wrappedNative,
					JSON.stringify(custom)
				)
			) as ReadSlot[]
	);
}

/**
 * Spec 100: a page's `wallet_addEthereumChain` params, as the core reads them
 * (`dapp_rpc::add_chain_ask`) — the ask, or the words of a -32602. The
 * extension has no debug mode (spec 091 is the in-app browsers'), so a page's
 * http RPC is usable only on loopback.
 */
export function dappAddChainAsk(
	params: unknown,
	debugMode = false
): { ok: DappChainAsk } | { error: string } {
	return JSON.parse(wasm.dappAddChainAsk(JSON.stringify(params ?? null), debugMode)) as
		{ ok: DappChainAsk } | { error: string };
}

/**
 * Spec 100: how a page's add-network request is answered for `outcome` — the
 * core's one table (`dapp_rpc::add_outcome_error`): `null` when added, else
 * the error. The in-app browsers answer from the same table.
 */
export function dappAddOutcomeError(
	outcome: DappAddOutcome,
	chainId: number
): { code: number; message: string } | null {
	return JSON.parse(wasm.dappAddOutcomeError(JSON.stringify(outcome), chainId)) as {
		code: number;
		message: string;
	} | null;
}
