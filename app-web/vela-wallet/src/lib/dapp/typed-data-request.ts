/**
 * One canonical reading of a typed-data request for preview, policy and signing.
 *
 * v4 is deliberately strict: `[address, typedData]`, exactly. The document is
 * parsed once, serialized once, and its EIP-712 digest becomes a commitment
 * shared by the confirmation sheet and the final submit path.
 */
import { hashTypedData, toHex, type TypedData } from '$lib/core/kernels';

const ADDRESS = /^0x[0-9a-fA-F]{40}$/;

export class InvalidTypedDataRequest extends Error {
	readonly code = -32602;
	constructor(message = 'Invalid eth_signTypedData_v4 params') {
		super(message);
		this.name = 'InvalidTypedDataRequest';
	}
}

export interface CanonicalTypedDataRequest {
	address: string | null;
	typedData: TypedData;
	typedDataJson: string;
	digest: Uint8Array;
	digestHex: string;
	params: unknown[];
	paramsJson: string;
}

interface Commitment {
	method: string;
	paramsJson: string;
	digestHex: string;
}

const commitments = new Map<string, Commitment>();
const keyOf = (id: string, origin: string | undefined) => `${origin ?? ''}\u0000${id}`;

function object(value: unknown): value is Record<string, unknown> {
	return !!value && typeof value === 'object' && !Array.isArray(value);
}

function parseTypedData(raw: unknown): TypedData {
	let parsed: unknown = raw;
	if (typeof raw === 'string') {
		try {
			parsed = JSON.parse(raw);
		} catch {
			throw new InvalidTypedDataRequest('Typed data is not valid JSON');
		}
	}
	if (
		!object(parsed) ||
		!object(parsed.types) ||
		typeof parsed.primaryType !== 'string' ||
		parsed.primaryType.length === 0 ||
		!object(parsed.domain) ||
		!object(parsed.message)
	) {
		throw new InvalidTypedDataRequest('Typed data is missing its EIP-712 fields');
	}
	return parsed as unknown as TypedData;
}

/** Parse and normalize one request, without mutating the caller's params. */
export function canonicalTypedDataRequest(
	method: string,
	params: unknown[],
	expectedAddress?: string | null
): CanonicalTypedDataRequest {
	let address: string | null = null;
	let raw: unknown;
	if (method === 'eth_signTypedData_v4') {
		if (params.length !== 2 || typeof params[0] !== 'string' || !ADDRESS.test(params[0])) {
			throw new InvalidTypedDataRequest();
		}
		address = params[0].toLowerCase();
		if (expectedAddress && address !== expectedAddress.toLowerCase()) {
			throw new InvalidTypedDataRequest('Typed-data account is not the connected account');
		}
		raw = params[1];
	} else if (method === 'eth_signTypedData' || method === 'eth_signTypedData_v1') {
		raw = params[0];
	} else {
		raw = params[1];
	}

	const typedData = parseTypedData(raw);
	const typedDataJson = JSON.stringify(typedData);
	const digest = hashTypedData(typedData);
	const digestHex = `0x${toHex(digest)}`;
	const canonicalParams =
		method === 'eth_signTypedData_v4' ? [address, typedDataJson] : [...params];
	const paramsJson = JSON.stringify(canonicalParams);
	return {
		address,
		typedData,
		typedDataJson,
		digest,
		digestHex,
		params: canonicalParams,
		paramsJson
	};
}

/** Record the exact digest the confirmation pipeline is about to display. */
export function commitTypedDataRequest(
	id: string,
	origin: string | undefined,
	method: string,
	params: unknown[],
	expectedAddress?: string | null
): CanonicalTypedDataRequest {
	const canonical = canonicalTypedDataRequest(method, params, expectedAddress);
	const key = keyOf(id, origin);
	const existing = commitments.get(key);
	if (
		existing &&
		(existing.method !== method ||
			existing.paramsJson !== canonical.paramsJson ||
			existing.digestHex !== canonical.digestHex)
	) {
		throw new InvalidTypedDataRequest('Typed data changed after it entered the signing flow');
	}
	commitments.set(key, {
		method,
		paramsJson: canonical.paramsJson,
		digestHex: canonical.digestHex
	});
	return canonical;
}

/** Recompute and compare immediately before the passkey challenge is built. */
export function requireCommittedTypedData(
	id: string,
	origin: string | undefined,
	method: string,
	params: unknown[],
	expectedAddress?: string | null
): CanonicalTypedDataRequest {
	const canonical = canonicalTypedDataRequest(method, params, expectedAddress);
	const expected = commitments.get(keyOf(id, origin));
	if (
		!expected ||
		expected.method !== method ||
		expected.paramsJson !== canonical.paramsJson ||
		expected.digestHex !== canonical.digestHex
	) {
		throw new InvalidTypedDataRequest('Preview and signing digest do not match');
	}
	return canonical;
}

export function clearTypedDataCommitment(id: string, origin: string | undefined): void {
	commitments.delete(keyOf(id, origin));
}
