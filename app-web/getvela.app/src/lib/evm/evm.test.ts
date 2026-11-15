import { describe, expect, it } from 'vitest';
import {
	decodeAbiParameters,
	decodeFunctionData,
	decodeFunctionResult,
	encodeAbiParameters,
	encodeFunctionData,
	type AbiParameter
} from './abi';
import { createClient, MULTICALL3_ADDRESS, type ReadContractArgs } from './client';
import { create2Address, getAddress, keccak256, selectorOf } from './hash';
import { stringToHex, type Hex } from './hex';
import { generatePrivateKey, privateKeyToAddress, rlpEncode, signLegacyTx, txHash } from './tx';

/*
 * Reference vectors. The ABI, CREATE2 and signed-transaction bytes below were
 * produced by viem 2.54.6 (the library this module replaced) on 2026-11-15;
 * the checksum and hash vectors come from their specs (EIP-55, Keccak).
 * Signatures are RFC 6979 deterministic, so the bytes must match exactly.
 */

const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';
const Z = '0x0000000000000000000000000000000000000000';
const ANVIL_KEY = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80';

const SETUP_ABI = [
	{
		type: 'function',
		name: 'setup',
		stateMutability: 'nonpayable',
		inputs: [
			{ name: 'owners', type: 'address[]' },
			{ name: 'threshold', type: 'uint256' },
			{ name: 'to', type: 'address' },
			{ name: 'data', type: 'bytes' },
			{ name: 'fallbackHandler', type: 'address' },
			{ name: 'paymentToken', type: 'address' },
			{ name: 'payment', type: 'uint256' },
			{ name: 'paymentReceiver', type: 'address' }
		],
		outputs: []
	}
] as const;

const SETUP_CALL =
	'0xb63e800d000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000020000000000000000000000002222222222222222222222222222222222222222000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000111111111111111111111111111111111111111100000000000000000000000022222222222222222222222222222222222222220000000000000000000000000000000000000000000000000000000000000004deadbeef00000000000000000000000000000000000000000000000000000000';

const GROUPS_CALL =
	'0x847c874f0000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000002800000000000000000000000000000000000000000000000000000000000000140000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000b67657476656c612e617070000000000000000000000000000000000000000000';

const TUPLE_PARAMS: AbiParameter[] = [
	{ type: 'uint256' },
	{
		type: 'tuple[]',
		components: [
			{ name: 'a', type: 'bytes' },
			{ name: 'b', type: 'uint32' },
			{ name: 'c', type: 'string' }
		]
	},
	{ type: 'bytes32' }
];
const TUPLE_ENCODED =
	'0x00000000000000000000000000000000000000000000000000000000000000050000000000000000000000000000000000000000000000000000000000000060abababababababababababababababababababababababababababababababab0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000000700000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000000101000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006e98791e5ba93000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000';

describe('hashing and addresses', () => {
	it('is Keccak-256, not SHA3-256', () => {
		expect(keccak256('0x')).toBe(
			'0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470'
		);
		expect(keccak256(stringToHex('abc'))).toBe(
			'0x4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45'
		);
	});

	it('checksums per EIP-55 and refuses a wrong checksum', () => {
		for (const a of [
			'0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed',
			'0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359',
			'0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB',
			'0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb'
		]) {
			expect(getAddress(a.toLowerCase())).toBe(a);
			expect(getAddress(a)).toBe(a);
		}
		expect(() => getAddress('0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAeD')).toThrow();
		expect(() => getAddress('0x1234')).toThrow();
	});

	it('computes selectors and CREATE2 addresses', () => {
		expect(selectorOf('SINGLETON()')).toBe('0x5a28a1db');
		expect(
			create2Address(
				'0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7',
				`0x${'00'.repeat(32)}`,
				keccak256('0x6080604052')
			)
		).toBe('0xdF722A0651579950B4A4E19D79C3a6ea9eaF4605');
	});
});

describe('ABI', () => {
	it('encodes calls byte-for-byte as viem did', () => {
		expect(encodeFunctionData(SETUP_ABI, 'setup', [[A, B], 2n, B, '0xdeadbeef', Z, Z, 0n, Z])).toBe(
			SETUP_CALL
		);
		const groups = [
			{
				type: 'function',
				name: 'getGroupsByRpId',
				stateMutability: 'view',
				inputs: [
					{ name: 'rpId', type: 'string' },
					{ name: 'offset', type: 'uint256' },
					{ name: 'limit', type: 'uint256' },
					{ name: 'desc', type: 'bool' }
				],
				outputs: []
			}
		] as const;
		expect(encodeFunctionData(groups, 'getGroupsByRpId', ['getvela.app', 40n, 20n, true])).toBe(
			GROUPS_CALL
		);
	});

	it('round-trips tuples, arrays, bytes, strings', () => {
		const values = [
			5n,
			[
				{ a: '0x01', b: 7, c: '金库' },
				{ a: '0x', b: 0, c: '' }
			],
			`0x${'ab'.repeat(32)}`
		];
		expect(encodeAbiParameters(TUPLE_PARAMS, values)).toBe(TUPLE_ENCODED);
		expect(decodeAbiParameters(TUPLE_PARAMS, TUPLE_ENCODED as Hex)).toEqual(values);
	});

	it('decodes calldata by selector, with checksummed addresses', () => {
		const { functionName, args } = decodeFunctionData(SETUP_ABI, SETUP_CALL as Hex);
		expect(functionName).toBe('setup');
		expect(args).toEqual([
			[getAddress(A), getAddress(B)],
			2n,
			getAddress(B),
			'0xdeadbeef',
			Z,
			Z,
			0n,
			Z
		]);
	});

	it('refuses what Solidity would refuse', () => {
		const one: AbiParameter[] = [{ type: 'address' }];
		expect(() => decodeAbiParameters(one, `0x${'ff'.repeat(32)}` as Hex)).toThrow(/dirty/);
		expect(() =>
			decodeAbiParameters([{ type: 'uint32' }], `0x${'00'.repeat(27)}0100000000` as Hex)
		).toThrow(/dirty/);
		expect(() =>
			decodeAbiParameters([{ type: 'bool' }], `0x${'00'.repeat(31)}02` as Hex)
		).toThrow();
		// Truncated: the offset points past the end.
		expect(() =>
			decodeAbiParameters([{ type: 'bytes' }], SETUP_CALL.slice(0, 74) as Hex)
		).toThrow();
	});

	it('refuses an array length the data cannot hold', () => {
		const huge = `0x${(32).toString(16).padStart(64, '0')}${'ff'.repeat(32)}` as Hex;
		expect(() => decodeAbiParameters([{ type: 'uint256[]' }], huge)).toThrow(/out of bounds/);
	});

	it('refuses offsets that alias, which would let a tiny answer decode into gigabytes', () => {
		// bytes[] of 3 elements, every one pointing at the same 32-byte payload.
		const w = (n: number) => n.toString(16).padStart(64, '0');
		const data = `0x${w(32)}${w(3)}${w(96)}${w(96)}${w(96)}${w(32)}${'aa'.repeat(32)}` as Hex;
		expect(() => decodeAbiParameters([{ type: 'bytes[]' }], data)).toThrow(/more than once/);
	});

	it('says "no data" instead of decoding an empty answer', () => {
		const abi = [
			{
				type: 'function',
				name: 'n',
				stateMutability: 'view',
				inputs: [],
				outputs: [{ name: '', type: 'uint256' }]
			}
		] as const;
		expect(() => decodeFunctionResult(abi, 'n', '0x')).toThrow(/no data/);
		expect(decodeFunctionResult(abi, 'n', `0x${'00'.repeat(31)}07`)).toBe(7n);
	});
});

describe('transactions', () => {
	it('derives the address of a key', () => {
		expect(privateKeyToAddress(ANVIL_KEY)).toBe('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266');
	});

	it('refuses keys outside [1, n) instead of reducing them', () => {
		expect(() => privateKeyToAddress(`0x${'00'.repeat(32)}`)).toThrow();
		expect(() => privateKeyToAddress(`0x${'ff'.repeat(32)}`)).toThrow();
		expect(() =>
			signLegacyTx(
				{ chainId: 1, nonce: 0, gasPrice: 1n, gas: 21000n, to: A },
				`0x${'00'.repeat(32)}`
			)
		).toThrow();
	});

	it('draws fresh, valid keys', () => {
		const a = generatePrivateKey();
		const b = generatePrivateKey();
		expect(a).toMatch(/^0x[0-9a-f]{64}$/);
		expect(a).not.toBe(b);
		expect(privateKeyToAddress(a)).toMatch(/^0x[0-9a-fA-F]{40}$/);
	});

	it('RLP-encodes per the yellow paper examples', () => {
		const enc = (s: string) => new TextEncoder().encode(s);
		expect(rlpEncode(enc('dog'))).toEqual(Uint8Array.of(0x83, 0x64, 0x6f, 0x67));
		expect(rlpEncode([enc('cat'), enc('dog')])).toEqual(
			Uint8Array.of(0xc8, 0x83, 0x63, 0x61, 0x74, 0x83, 0x64, 0x6f, 0x67)
		);
		expect(rlpEncode(new Uint8Array(0))).toEqual(Uint8Array.of(0x80));
		expect(rlpEncode([])).toEqual(Uint8Array.of(0xc0));
		expect(rlpEncode(Uint8Array.of(0x0f))).toEqual(Uint8Array.of(0x0f));
		const long = enc('Lorem ipsum dolor sit amet, consectetur adipisicing elit');
		expect(Array.from(rlpEncode(long).subarray(0, 2))).toEqual([0xb8, 0x38]);
	});

	it('signs legacy EIP-155 transactions byte-for-byte as viem did', () => {
		expect(
			signLegacyTx(
				{
					chainId: 31337,
					nonce: 0,
					gasPrice: 1_000_000_000n,
					gas: 21000n,
					to: B,
					value: 10n ** 18n
				},
				ANVIL_KEY
			)
		).toBe(
			'0xf86d80843b9aca00825208942222222222222222222222222222222222222222880de0b6b3a76400008082f4f5a0af0975d53b39c1d067cad30a51ea098bb5540f96f277dbfceeb8d96636089698a05e8099f89e17939d16fbc1be26f23225e04be6552dd0cc5d4f428d33052d99c7'
		);
		expect(
			signLegacyTx(
				{
					chainId: 1440000,
					nonce: 300,
					gasPrice: 7n,
					gas: 1_300_000n,
					to: '0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7',
					data: `0x${'00'.repeat(32)}6080604052${'ff'.repeat(60)}`
				},
				ANVIL_KEY
			)
		).toBe(
			'0xf8c782012c078313d62094914d7fec6aac8cd542e72bca78b30650d45643d780b86100000000000000000000000000000000000000000000000000000000000000006080604052ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff832bf223a00ec9f8414f1b0de20d3c4621411b019cafa5d5a0ea97edc6ddb0e8410e9a7c23a0709459bbda00fe1a8946c79825b78827607aed837840cb8392a280f07d4744a4'
		);
		const raw = signLegacyTx(
			{ chainId: 100, nonce: 1, gasPrice: 2_500_000_000n, gas: 50000n, to: A, value: 123456789n },
			ANVIL_KEY
		);
		expect(raw).toBe(
			'0xf86801849502f90082c35094111111111111111111111111111111111111111184075bcd158081eba00f63947f1b3168cb451c2b7f6f6aa5c5adcde4b0aa2267414868c51db3ab4d77a007401d069fe7385fad1302ae3bb929cb6b54a98d44107698b76150d1fec05656'
		);
		expect(txHash(raw)).toMatch(/^0x[0-9a-f]{64}$/);
	});
});

describe('client', () => {
	type Handler = (body: unknown) => unknown;
	function fakeFetch(perUrl: Record<string, Handler | 'down'>, log: string[] = []) {
		return (async (url: string, init: { body: string }) => {
			log.push(url);
			const h = perUrl[url];
			if (!h || h === 'down') throw new Error('unreachable');
			return { ok: true, json: async () => h(JSON.parse(init.body)) };
		}) as unknown as typeof fetch;
	}
	const answer = (result: unknown) => (b: unknown) => {
		const one = (r: { id: number }) => ({ jsonrpc: '2.0', id: r.id, result });
		return Array.isArray(b) ? b.map(one) : one(b as { id: number });
	};

	it('fails over to the next endpoint, and throws only when all fail', async () => {
		const log: string[] = [];
		const c = createClient(['a', 'b'], { fetch: fakeFetch({ a: 'down', b: answer('0x1') }, log) });
		expect(await c.request({ method: 'eth_chainId' })).toBe('0x1');
		expect(log).toEqual(['a', 'b']);
		const dead = createClient(['a'], { retryCount: 1, fetch: fakeFetch({ a: 'down' }) });
		await expect(dead.request({ method: 'eth_chainId' })).rejects.toThrow('unreachable');
	});

	it('treats an error answer as a failed endpoint', async () => {
		const err = () => ({ jsonrpc: '2.0', id: 1, error: { code: -32000, message: 'out of gas' } });
		const c = createClient(['a', 'b'], { fetch: fakeFetch({ a: err, b: answer('0x2') }) });
		expect(await c.request({ method: 'eth_call' })).toBe('0x2');
		const only = createClient(['a'], { retryCount: 0, fetch: fakeFetch({ a: err }) });
		await expect(only.request({ method: 'eth_call' })).rejects.toThrow('out of gas');
	});

	it('batches same-tick requests into one round-trip', async () => {
		const bodies: unknown[] = [];
		const handler = (b: unknown) => (
			bodies.push(b),
			answer({ number: '0x5', timestamp: '0x10' })(b)
		);
		const c = createClient(['a'], { batchSize: 50, fetch: fakeFetch({ a: handler }) });
		const blocks = await Promise.all([1n, 2n, 3n].map((n) => c.getBlock({ blockNumber: n })));
		expect(blocks.map((b) => b.timestamp)).toEqual([16n, 16n, 16n]);
		expect(bodies).toHaveLength(1);
		expect((bodies[0] as unknown[]).length).toBe(3);
	});

	it('decodes contract reads and refuses a non-hex answer', async () => {
		const abi = [
			{
				type: 'function',
				name: 'totalRecords',
				stateMutability: 'view',
				inputs: [],
				outputs: [{ name: '', type: 'uint256' }]
			}
		] as const;
		const ok = createClient(['a'], { fetch: fakeFetch({ a: answer(`0x${'00'.repeat(31)}2a`) }) });
		expect(await ok.readContract({ address: A, abi, functionName: 'totalRecords' })).toBe(42n);
		const bad = createClient(['a'], { retryCount: 0, fetch: fakeFetch({ a: answer({ x: 1 }) }) });
		await expect(
			bad.readContract({ address: A, abi, functionName: 'totalRecords' })
		).rejects.toThrow();
	});

	describe('readContracts (Multicall3)', () => {
		// aggregate3((address,bool,bytes)[]) as published by mds1/multicall3 —
		// written out here, independently of the client's own copy.
		const AGGREGATE3 = [
			{
				type: 'function',
				name: 'aggregate3',
				stateMutability: 'payable',
				inputs: [
					{
						name: 'calls',
						type: 'tuple[]',
						components: [
							{ name: 'target', type: 'address' },
							{ name: 'allowFailure', type: 'bool' },
							{ name: 'callData', type: 'bytes' }
						]
					}
				],
				outputs: [
					{
						name: 'returnData',
						type: 'tuple[]',
						components: [
							{ name: 'success', type: 'bool' },
							{ name: 'returnData', type: 'bytes' }
						]
					}
				]
			}
		] as const;
		const abi = [
			{
				type: 'function',
				name: 'valueOf',
				stateMutability: 'view',
				inputs: [{ name: 'n', type: 'uint256' }],
				outputs: [{ name: '', type: 'uint256' }]
			}
		] as const;
		const call = (n: number): ReadContractArgs => ({
			address: A,
			abi,
			functionName: 'valueOf',
			args: [BigInt(n)]
		});

		/** A Multicall3 that answers `valueOf(n)` with n * 10, reverting where told. */
		function multicall(log: { to: string; calls: number }[], reverts: bigint[] = [], drop = 0) {
			return (b: unknown) => {
				const one = (r: { id: number; params: [{ to: string; data: Hex }] }) => {
					const [{ to, data }] = r.params;
					expect(data.slice(0, 10)).toBe('0x82ad56cb');
					const [calls] = decodeFunctionData(AGGREGATE3, data).args as [
						{ target: string; allowFailure: boolean; callData: Hex }[]
					];
					log.push({ to, calls: calls.length });
					const answers = calls.slice(drop).map((c) => {
						const [n] = decodeFunctionData(abi, c.callData).args as [bigint];
						expect(c.target).toBe(A);
						return reverts.includes(n)
							? { success: false, returnData: '0x' }
							: {
									success: true,
									returnData: encodeAbiParameters([{ type: 'uint256' }], [n * 10n])
								};
					});
					const result = encodeAbiParameters(AGGREGATE3[0].outputs, [answers]);
					return { jsonrpc: '2.0', id: r.id, result };
				};
				return Array.isArray(b) ? b.map(one) : one(b as Parameters<typeof one>[0]);
			};
		}

		it('reads many calls in one eth_call to Multicall3, answers in order', async () => {
			const log: { to: string; calls: number }[] = [];
			const c = createClient(['a'], { fetch: fakeFetch({ a: multicall(log) }) });
			expect(await c.readContracts([3, 1, 2].map(call))).toEqual([30n, 10n, 20n]);
			expect(log).toEqual([{ to: MULTICALL3_ADDRESS, calls: 3 }]);
			expect(await c.readContracts([])).toEqual([]);
			expect(log).toHaveLength(1);
		});

		it('splits a long list, and the parts share one request when batching', async () => {
			const log: { to: string; calls: number }[] = [];
			const bodies: unknown[] = [];
			const h = multicall(log);
			const c = createClient(['a'], {
				batchSize: 10,
				fetch: fakeFetch({ a: (b) => (bodies.push(b), h(b)) })
			});
			const ns = Array.from({ length: 250 }, (_, i) => i);
			expect(await c.readContracts(ns.map(call))).toEqual(ns.map((n) => BigInt(n) * 10n));
			expect(log.map((l) => l.calls)).toEqual([100, 100, 50]);
			expect(bodies).toHaveLength(1);
		});

		it('refuses the whole read when one call reverts, naming it', async () => {
			const c = createClient(['a'], { fetch: fakeFetch({ a: multicall([], [2n]) }) });
			await expect(c.readContracts([1, 2].map(call))).rejects.toThrow('valueOf reverted');
		});

		it('refuses an answer that drops calls', async () => {
			const c = createClient(['a'], { fetch: fakeFetch({ a: multicall([], [], 1) }) });
			await expect(c.readContracts([1, 2].map(call))).rejects.toThrow('answered 1 of 2');
		});
	});
});
