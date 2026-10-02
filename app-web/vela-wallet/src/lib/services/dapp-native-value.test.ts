/**
 * Spec 096 F1 — a dApp call that sends native coin reaches the signature.
 *
 * Found on BNB Chain, 2026-10-02: PancakeSwap's BNB → USDC asked for
 * `value: "0xaa87bee538000"` (0.003 BNB). The submit stripped the `0x`, the
 * MultiSend call carried `"aa87bee538000"`, and the inner-call gas floor read it
 * with `BigInt(...)` — `Cannot convert aa87bee538000 to a BigInt` — so every
 * such request died before the passkey, and the page was told "could not
 * estimate gas". Same on Uniswap and Aave's `depositETH`.
 *
 * These run the REAL path — `handleDAppRequest` → `dapp-submit` →
 * `safe-transaction` (estimate, the gas floor, the core's attestation, the
 * signature envelope, the POST) — with only the network, the stored account
 * and the passkey stood in for, on an in-band chain (BNB Chain: native fee and
 * a stablecoin fee) and on Tempo.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x88cca0eedbf2c4426110bbfc998f048689266894';
const ACCOUNT = { id: 'cred-a', address: SAFE, publicKeyHex: '04' + '11'.repeat(64) };
vi.mock('./accounts', () => ({
	findAccountByAddress: () => ACCOUNT,
	findAccountByCredentialId: () => ACCOUNT
}));
vi.mock('./networks', async (importOriginal) => ({
	...(await importOriginal<typeof import('./networks')>()),
	getAllNetworksSync: () => [{ chainId: 56 }, { chainId: 4217 }]
}));
vi.mock('./bundler-service', async (importOriginal) => ({
	...(await importOriginal<typeof import('./bundler-service')>()),
	fetchBundlerAccountInfo: vi.fn(async () => ({
		depositAddress: '0x' + '99'.repeat(20),
		settlementRecipient: '0x' + '99'.repeat(20)
	}))
}));
vi.mock('$lib/core/kernels', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/kernels')>()),
	verifySafeWebAuthn: () => ({ ok: true })
}));

/** What the "network" was asked, by method. */
const asked = vi.hoisted(() => new Map<string, unknown[][]>());
const OP_HASH = '0x' + 'ab'.repeat(32);
const TX_HASH = '0x' + 'cd'.repeat(32);
vi.mock('./rpc-adapter', () => ({
	rpcCall: vi.fn(async (method: string, params: unknown[]) => {
		asked.set(method, [...(asked.get(method) ?? []), params]);
		const ok = (result: unknown) => ({ jsonrpc: '2.0', id: 1, result });
		switch (method) {
			case 'eth_getCode':
				return ok('0x6080604052');
			case 'eth_call':
				return ok('0x' + '0'.repeat(64));
			case 'eth_estimateUserOperationGas':
				return ok({
					verificationGasLimit: '0x30000',
					callGasLimit: '0x40000',
					preVerificationGas: '0x10000'
				});
			case 'eth_estimateGas':
				return ok('0x30d40');
			case 'eth_gasPrice':
				return ok('0x3b9aca00');
			case 'eth_getBlockByNumber':
				return ok({ baseFeePerGas: '0x3b9aca00' });
			case 'eth_maxPriorityFeePerGas':
				return ok('0x0');
			case 'eth_blockNumber':
				return ok('0x10');
			case 'eth_sendUserOperation':
				return ok(OP_HASH);
			case 'eth_getUserOperationReceipt':
				return ok({ success: true, receipt: { transactionHash: TX_HASH, status: '0x1' } });
			default:
				return { jsonrpc: '2.0', id: 1, error: { code: -32601, message: `no ${method}` } };
		}
	})
}));

/** The passkey: a well-formed assertion over whatever challenge it is shown. */
const signed = vi.hoisted(() => ({ operations: [] as unknown[] }));
vi.mock('$lib/signing/sign-challenge', () => ({
	signChallenge: vi.fn(async (challenge: Uint8Array) => {
		const b64url = btoa(String.fromCharCode(...challenge))
			.replace(/\+/g, '-')
			.replace(/\//g, '_')
			.replace(/=+$/, '');
		const clientData = JSON.stringify({ type: 'webauthn.get', challenge: b64url });
		const hex = (bytes: Uint8Array) =>
			Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
		// DER(r, s), both 32 bytes with the high bit clear.
		const der = '3044' + '0220' + '11'.repeat(32) + '0220' + '22'.repeat(32);
		return {
			signatureHex: der,
			authenticatorDataHex: '00'.repeat(37),
			clientDataJSONHex: hex(new TextEncoder().encode(clientData)),
			credentialId: 'cred-a'
		};
	})
}));

import { handleDAppRequest } from './dapp-submit';
import * as safeTransaction from './safe-transaction';

const PCS_ROUTER = '0x13f4EA83D0bd40E75C8222255bc855a974568Dd4';
const AAVE_GATEWAY = '0x0c2C95b24529664fE55D4437D7A31175CFE6c4f7';
const WBNB = '0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c';
const USDC_BSC = '0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d';
/** 0.003 BNB — the value PancakeSwap asked for. */
const VALUE = '0xaa87bee538000';
const VALUE_WEI = '3000000000000000';
const VALUE_WORD = 'aa87bee538000'.padStart(64, '0');
const FEE = { amount: 1_000_000_000_000n, recipient: '0x' + '99'.repeat(20) };

function run(
	method: string,
	params: unknown[],
	chainId: number,
	gasFeeToken: string | null = null
) {
	return handleDAppRequest(
		{ id: 'req-1', method, params, origin: 'https://pancakeswap.finance' },
		{ id: 'cred-a' },
		SAFE,
		chainId,
		undefined,
		undefined,
		gasFeeToken,
		FEE,
		'core'
	);
}

/** The callData of the one op POSTed. */
function postedCallData(): string {
	const posts = asked.get('eth_sendUserOperation') ?? [];
	expect(posts).toHaveLength(1);
	return ((posts[0] as [Record<string, string>])[0].callData ?? '').toLowerCase();
}

/** The inner-call gas measurements (the floor that threw), as asked. */
function measured(): { to: string; value: string }[] {
	return (asked.get('eth_estimateGas') ?? []).map((p) => (p as [{ to: string; value: string }])[0]);
}

beforeEach(() => {
	asked.clear();
	signed.operations.length = 0;
});

describe('a native value reaches the signature (spec 096 F1)', () => {
	it('PancakeSwap BNB → USDC on BNB Chain: priced, signed and sent as 0.003 BNB', async () => {
		const signSpy = vi.spyOn(safeTransaction, 'buildUserOpSignature');
		const answer = await run(
			'eth_sendTransaction',
			[{ from: SAFE, to: PCS_ROUTER, value: VALUE, data: '0x3593564c' + '00'.repeat(64) }],
			56
		);
		expect(answer).toBe(TX_HASH);
		// The floor measured the call with its real value, `0x`-prefixed.
		expect(measured()).toEqual([expect.objectContaining({ to: PCS_ROUTER, value: VALUE })]);
		// The signed batch carries the value word, and the fee leg after it.
		expect(postedCallData()).toContain(PCS_ROUTER.slice(2).toLowerCase() + VALUE_WORD);
		signSpy.mockRestore();
	});

	it("Aave's depositETH sends its value, and the page gets the transaction", async () => {
		// depositETH(address,address,uint16) — the referral code 0 is not the value.
		const data =
			'0x474cf53d' +
			'6807dc923806fe8fd134338eabca509979a7e0cb'.padStart(64, '0') +
			SAFE.slice(2).padStart(64, '0') +
			'0'.repeat(64);
		const answer = await run(
			'eth_sendTransaction',
			[{ from: SAFE, to: AAVE_GATEWAY, value: VALUE, data }],
			56
		);
		expect(answer).toBe(TX_HASH);
		expect(measured()[0]).toEqual(expect.objectContaining({ to: AAVE_GATEWAY, value: VALUE }));
		expect(postedCallData()).toContain(AAVE_GATEWAY.slice(2).toLowerCase() + VALUE_WORD);
	});

	it('a batch with a native leg, the fee paid in a stablecoin', async () => {
		const answer = await run(
			'wallet_sendCalls',
			[
				{
					version: '2.0.0',
					chainId: '0x38',
					from: SAFE,
					calls: [
						{ to: WBNB, value: '0x0', data: '0x095ea7b3' + '00'.repeat(64) },
						{ to: PCS_ROUTER, value: VALUE, data: '0x3593564c' + '00'.repeat(64) }
					]
				}
			],
			56,
			USDC_BSC
		);
		expect(answer).toBe(OP_HASH);
		expect(measured().map((m) => m.value)).toEqual(['0x0', VALUE]);
		const callData = postedCallData();
		expect(callData).toContain(PCS_ROUTER.slice(2).toLowerCase() + VALUE_WORD);
		expect(callData).toContain(USDC_BSC.slice(2).toLowerCase() + '0'.repeat(64));
	});

	it('on Tempo, a contract call with a value is measured and signed the same way', async () => {
		const answer = await run(
			'eth_sendTransaction',
			[{ from: SAFE, to: PCS_ROUTER, value: '0x2a', data: '0x3593564c' }],
			4217
		);
		expect(answer).toBe(TX_HASH);
		expect(measured()[0]).toEqual(expect.objectContaining({ value: '0x2a' }));
		expect(postedCallData()).toContain(PCS_ROUTER.slice(2).toLowerCase() + '2a'.padStart(64, '0'));
	});

	it('what the passkey is shown is the value in base units', async () => {
		const { signChallenge } = await import('$lib/signing/sign-challenge');
		await run('eth_sendTransaction', [{ to: PCS_ROUTER, value: VALUE, data: '0x3593564c' }], 56);
		const signer = vi.mocked(signChallenge).mock.calls.at(-1)?.[1] as {
			request: { params: { value: string }[] };
		};
		// The request the passkey sheet names is the dApp's own, untouched.
		expect(signer.request.params[0].value).toBe(VALUE);
		expect(BigInt(VALUE).toString()).toBe(VALUE_WEI);
	});

	it('an unreadable value never reaches a signature', async () => {
		const { signChallenge } = await import('$lib/signing/sign-challenge');
		vi.mocked(signChallenge).mockClear();
		// The stripped text the submit used to build, and decimal text.
		for (const value of ['aa87bee538000', '3000000000000000']) {
			await expect(
				run('eth_sendTransaction', [{ to: PCS_ROUTER, value, data: '0x3593564c' }], 56)
			).rejects.toThrow('Invalid transaction params');
		}
		expect(vi.mocked(signChallenge)).not.toHaveBeenCalled();
		expect(asked.get('eth_sendUserOperation')).toBeUndefined();
	});
});
