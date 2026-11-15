/**
 * A throwaway secp256k1 key and the one kind of transaction it signs: a
 * legacy EIP-155 transaction (chain id bound into the signature, so it cannot
 * be replayed on another chain).
 *
 * Legacy rather than EIP-1559 on purpose: every EVM chain accepts it —
 * including a freshly launched one the chain-setup page is preparing — and the
 * fee is exactly `gas × gasPrice`, the same figure the page quotes for funding.
 *
 * The curve math runs in rust/vela-site-crypto: RustCrypto's `k256`, the
 * crate vela-core already ships (constant-time, RFC 6979 deterministic
 * nonces, low-s). Only the RLP framing is here.
 */
import { getAddress, type Address } from './hash';
import { bigintToBytes, bytesToHex, concatBytes, hexToBytes, type Hex } from './hex';
import { keccak256Wasm as keccak256Bytes, secretKeyAddress, signPrehash } from './wasm';

// ── RLP ──────────────────────────────────────────────────────────────────────

type RlpItem = Uint8Array | RlpItem[];

function rlpLength(len: number, offset: number): Uint8Array {
	if (len <= 55) return Uint8Array.of(offset + len);
	const lenBytes = bigintToBytes(BigInt(len));
	return concatBytes(Uint8Array.of(offset + 55 + lenBytes.length), lenBytes);
}

export function rlpEncode(item: RlpItem): Uint8Array {
	if (Array.isArray(item)) {
		const payload = concatBytes(...item.map(rlpEncode));
		return concatBytes(rlpLength(payload.length, 0xc0), payload);
	}
	if (item.length === 1 && item[0] < 0x80) return item;
	return concatBytes(rlpLength(item.length, 0x80), item);
}

// ── Keys ─────────────────────────────────────────────────────────────────────

/**
 * 32 bytes from the platform CSPRNG. The module has no randomness of its
 * own; it only judges the draw — the rare value outside [1, n) is refused
 * and drawn again, never reduced (reduction would bias the key).
 */
export function generatePrivateKey(): Hex {
	for (;;) {
		const secret = crypto.getRandomValues(new Uint8Array(32));
		const valid = secretKeyAddress(secret) !== null;
		const hex = bytesToHex(secret);
		secret.fill(0);
		if (valid) return hex;
	}
}

export function privateKeyToAddress(privateKey: Hex): Address {
	const address = secretKeyAddress(hexToBytes(privateKey));
	if (!address) throw new Error('not a valid secp256k1 private key');
	return getAddress(bytesToHex(address));
}

// ── Transactions ─────────────────────────────────────────────────────────────

export interface LegacyTx {
	chainId: number;
	nonce: bigint | number;
	gasPrice: bigint;
	gas: bigint;
	to: Address;
	value?: bigint;
	data?: Hex;
}

/** Sign and serialize; the result is what `eth_sendRawTransaction` takes. */
export function signLegacyTx(tx: LegacyTx, privateKey: Hex): Hex {
	if (!Number.isSafeInteger(tx.chainId) || tx.chainId <= 0) throw new Error('bad chain id');
	const chainId = BigInt(tx.chainId);
	const fields: Uint8Array[] = [
		bigintToBytes(BigInt(tx.nonce)),
		bigintToBytes(tx.gasPrice),
		bigintToBytes(tx.gas),
		hexToBytes(getAddress(tx.to)),
		bigintToBytes(tx.value ?? 0n),
		hexToBytes(tx.data ?? '0x')
	];
	// EIP-155: sign over the fields plus (chainId, 0, 0).
	const digest = keccak256Bytes(
		rlpEncode([...fields, bigintToBytes(chainId), new Uint8Array(0), new Uint8Array(0)])
	);
	const sig = signPrehash(hexToBytes(privateKey), digest);
	if (!sig) throw new Error('not a valid secp256k1 private key');
	const r = sig.subarray(0, 32);
	const s = sig.subarray(32, 64);
	const recovery = BigInt(sig[64]);
	const v = recovery + chainId * 2n + 35n;
	return bytesToHex(
		rlpEncode([
			...fields,
			bigintToBytes(v),
			bigintToBytes(BigInt(bytesToHex(r))),
			bigintToBytes(BigInt(bytesToHex(s)))
		])
	);
}

/** The hash a node reports for a signed transaction. */
export function txHash(raw: Hex): Hex {
	return bytesToHex(keccak256Bytes(hexToBytes(raw)));
}
