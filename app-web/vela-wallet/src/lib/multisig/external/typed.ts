/**
 * r4C-7: what Vela asks the connect page to have signed, read before the
 * wallet is asked — the SafeTx inside an `eth_signTypedData_v4` request.
 *
 * The page believes nothing it is sent: the typed data is taken apart into
 * a SafeTx, and the core builds the typed data for that SafeTx itself
 * (`safeTxTypedData`); only a request that is exactly that (same domain, same
 * types, same fields) is read as one. Anything else is "a request this page
 * cannot read", said, and only declined.
 */
import { safeTxTypedData, type SafeTx } from '../core/kernel';

export interface SafeTxRequest {
	chainId: number;
	safe: string;
	tx: SafeTx;
}

const FIELDS = [
	'to',
	'value',
	'data',
	'operation',
	'safeTxGas',
	'baseGas',
	'gasPrice',
	'gasToken',
	'refundReceiver',
	'nonce'
] as const;

function hexQuantity(value: unknown): string {
	return `0x${BigInt(String(value)).toString(16)}`;
}

function same(a: unknown, b: unknown): boolean {
	return String(a).toLowerCase() === String(b).toLowerCase();
}

/**
 * The SafeTx an `eth_signTypedData_v4` request's typed data (its second
 * param, a JSON string or an object) asks for, checked against the core's own
 * typed data for it — or `null` when it is not exactly a SafeTx.
 */
export function readSafeTxRequest(typedData: unknown): SafeTxRequest | null {
	try {
		const parsed = (typeof typedData === 'string' ? JSON.parse(typedData) : typedData) as {
			primaryType?: unknown;
			domain?: { chainId?: unknown; verifyingContract?: unknown };
			message?: Record<string, unknown>;
			types?: Record<string, unknown>;
		};
		if (parsed?.primaryType !== 'SafeTx' || !parsed.domain || !parsed.message) return null;
		const chainId = Number(BigInt(String(parsed.domain.chainId)));
		const safe = String(parsed.domain.verifyingContract);
		if (!Number.isSafeInteger(chainId) || !/^0x[0-9a-fA-F]{40}$/.test(safe)) return null;
		const m = parsed.message;
		const tx: SafeTx = {
			to: String(m.to),
			value: hexQuantity(m.value),
			data: String(m.data || '0x'),
			operation: Number(m.operation),
			safeTxGas: hexQuantity(m.safeTxGas),
			baseGas: hexQuantity(m.baseGas),
			gasPrice: hexQuantity(m.gasPrice),
			gasToken: String(m.gasToken),
			refundReceiver: String(m.refundReceiver),
			nonce: hexQuantity(m.nonce)
		};
		// The core's typed data for this SafeTx: the request must be it, field for field.
		const own = safeTxTypedData(chainId, safe, tx) as {
			domain: Record<string, unknown>;
			message: Record<string, unknown>;
			types: Record<string, unknown>;
		};
		if (JSON.stringify(own.types) !== JSON.stringify(parsed.types)) return null;
		if (!same(own.domain.verifyingContract, safe) || Number(own.domain.chainId) !== chainId) {
			return null;
		}
		if (Object.keys(parsed.domain).length !== 2) return null;
		for (const field of FIELDS) {
			const mine = own.message[field];
			const theirs = m[field];
			const numeric = typeof mine === 'string' && /^\d+$/.test(mine);
			if (numeric ? BigInt(String(theirs)) !== BigInt(mine) : !same(mine, theirs)) return null;
		}
		return { chainId, safe, tx };
	} catch {
		return null;
	}
}

/**
 * A `personal_sign` answer over a safeTxHash, as the Safe reads Safe's
 * `eth_sign` form: `v` made 27/28, then `+ 4` (31/32). The core checks it.
 */
export function ethSignForm(signature: string): string {
	const clean = signature.startsWith('0x') ? signature.slice(2) : signature;
	if (!/^[0-9a-fA-F]{130}$/.test(clean)) throw new Error('not a 65-byte signature');
	let v = parseInt(clean.slice(128), 16);
	if (v < 27) v += 27;
	if (v !== 27 && v !== 28) throw new Error(`v ${v} is not a personal_sign answer's`);
	return `0x${clean.slice(0, 128)}${(v + 4).toString(16)}`;
}
