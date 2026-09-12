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
		const opJson = JSON.stringify({
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
