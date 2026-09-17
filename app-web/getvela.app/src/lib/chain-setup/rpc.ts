/**
 * The reads this page makes, over plain JSON-RPC.
 *
 * Deliberately not viem for the probes: the wallet's checker speaks raw
 * `eth_getCode` / `eth_call` and judges the raw strings (`is_code_deployed`,
 * `p256_call_indicates_support` in `network_admin.rs`), and this page must
 * reach the same verdict from the same bytes. viem is used only where a key
 * has to sign (`deployer-wallet.ts`).
 */
import { keccak256 } from 'viem';
import { EXPECTED_CODE_HASH, EXPECTED_RUNTIME_CODE } from './deployers';
import {
	P256_PRECOMPILE,
	REQUIRED_CONTRACTS,
	VALID_P256_CALL,
	type RequiredContract
} from './required-contracts';

export class RpcError extends Error {
	constructor(
		message: string,
		public readonly code?: number
	) {
		super(message);
	}
}

export async function rpcCall<T = unknown>(
	url: string,
	method: string,
	params: unknown[] = [],
	timeoutMs = 12_000
): Promise<T> {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeoutMs);
	try {
		const res = await fetch(url, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
			signal: controller.signal
		});
		if (!res.ok) throw new RpcError(`HTTP ${res.status}`);
		const json = (await res.json()) as { result?: T; error?: { code?: number; message?: string } };
		if (json.error) throw new RpcError(json.error.message ?? 'RPC error', json.error.code);
		return json.result as T;
	} finally {
		clearTimeout(timer);
	}
}

export interface RpcProbe {
	url: string;
	/** `null` = did not answer, or answered for a different chain. */
	latencyMs: number | null;
	chainId: number | null;
}

/** Race every endpoint; keep the ones that answer for THIS chain, fastest first. */
export async function probeRpcs(urls: string[], expectChainId?: number): Promise<RpcProbe[]> {
	const probes = await Promise.all(
		urls.map(async (url): Promise<RpcProbe> => {
			const start = performance.now();
			try {
				const hex = await rpcCall<string>(url, 'eth_chainId', [], 8_000);
				const chainId = Number.parseInt(hex, 16);
				const ok =
					Number.isFinite(chainId) && (expectChainId === undefined || chainId === expectChainId);
				return {
					url,
					latencyMs: ok ? Math.round(performance.now() - start) : null,
					chainId: ok ? chainId : null
				};
			} catch {
				return { url, latencyMs: null, chainId: null };
			}
		})
	);
	return probes.sort((a, b) => (a.latencyMs ?? Infinity) - (b.latencyMs ?? Infinity));
}

/** The wallet's `is_code_deployed`: empty / `0x` / `0x0` ⇒ no; anything longer ⇒ yes. */
export function isCodeDeployed(code: string | null | undefined): boolean {
	if (code == null || code === '' || code === '0x' || code === '0x0') return false;
	return code.length > 2;
}

/** The wallet's strategy 1: a ≥66-char result whose value is exactly 1. */
export function p256CallIndicatesSupport(result: string | null | undefined): boolean {
	if (!result || result === '0x' || result.length < 66) return false;
	try {
		return BigInt(result) === 1n;
	} catch {
		return false;
	}
}

export type P256Status = 'precompile' | 'contract' | 'missing';

/**
 * Two strategies, in the wallet's order. Strategy 1 asks the precompile to
 * verify a known-good signature. Strategy 2 — only if that did not answer 1 —
 * checks whether code sits at `0x100` at all, which is how a chain that ships
 * a verifier contract there (rather than a native precompile) still passes.
 * The `gas` is set because zkSync-style chains refuse the call without it.
 */
export async function probeP256(url: string): Promise<P256Status> {
	try {
		const result = await rpcCall<string>(url, 'eth_call', [
			{ to: P256_PRECOMPILE, data: VALID_P256_CALL, gas: '0x100000' },
			'latest'
		]);
		if (p256CallIndicatesSupport(result)) return 'precompile';
	} catch {
		/* falls through to strategy 2 */
	}
	try {
		const code = await rpcCall<string>(url, 'eth_getCode', [P256_PRECOMPILE, 'latest']);
		return isCodeDeployed(code) ? 'contract' : 'missing';
	} catch {
		return 'missing';
	}
}

export interface ContractStatus {
	contract: RequiredContract;
	/** Code is there AND, for the keyless ones, it is the code we expect. */
	deployed: boolean;
	/** Code is there but it is not the contract — the address is taken by something else. */
	mismatch: boolean;
	/** The read itself failed; not the same as "absent". */
	unknown: boolean;
}

export async function checkContract(
	url: string,
	contract: RequiredContract
): Promise<ContractStatus> {
	let code: string;
	try {
		code = await rpcCall<string>(url, 'eth_getCode', [contract.address, 'latest']);
	} catch {
		return { contract, deployed: false, mismatch: false, unknown: true };
	}
	if (!isCodeDeployed(code)) return { contract, deployed: false, mismatch: false, unknown: false };
	const expectedCode = EXPECTED_RUNTIME_CODE[contract.key];
	const expectedHash = EXPECTED_CODE_HASH[contract.key];
	if (expectedCode) {
		const ok = code.toLowerCase() === expectedCode.toLowerCase();
		return { contract, deployed: ok, mismatch: !ok, unknown: false };
	}
	if (expectedHash) {
		const ok = keccak256(code as `0x${string}`).toLowerCase() === expectedHash.toLowerCase();
		return { contract, deployed: ok, mismatch: !ok, unknown: false };
	}
	// CREATE2: the address is a hash of the code, so code there IS the contract.
	return { contract, deployed: true, mismatch: false, unknown: false };
}

export interface CheckResult {
	contracts: ContractStatus[];
	p256: P256Status;
}

export async function checkChain(url: string): Promise<CheckResult> {
	const [contracts, p256] = await Promise.all([
		Promise.all(REQUIRED_CONTRACTS.map((c) => checkContract(url, c))),
		probeP256(url)
	]);
	return { contracts, p256 };
}

export async function getBalance(url: string, address: string): Promise<bigint> {
	return BigInt(await rpcCall<string>(url, 'eth_getBalance', [address, 'latest']));
}

export async function getGasPrice(url: string): Promise<bigint> {
	return BigInt(await rpcCall<string>(url, 'eth_gasPrice'));
}

export async function sendRaw(url: string, rawTx: string): Promise<`0x${string}`> {
	return rpcCall<`0x${string}`>(url, 'eth_sendRawTransaction', [rawTx]);
}

/** Poll for a receipt. `null` = not mined within the window (not a failure verdict). */
export async function waitForReceipt(
	url: string,
	hash: string,
	timeoutMs = 120_000,
	everyMs = 2_500
): Promise<boolean | null> {
	const start = Date.now();
	while (Date.now() - start < timeoutMs) {
		try {
			const receipt = await rpcCall<{ status: string } | null>(url, 'eth_getTransactionReceipt', [
				hash
			]);
			if (receipt) return receipt.status === '0x1';
		} catch {
			/* not yet */
		}
		await new Promise((r) => setTimeout(r, everyMs));
	}
	return null;
}
