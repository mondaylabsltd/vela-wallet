/**
 * r4C-2 / r4C-9: an outside owner acts on a transaction from the connect
 * page alone. What the mailbox holds is believed only once the core has
 * re-hashed it (the link's hash) and re-verified every approval; the
 * owner's own signature is checked before it is handed on; executing is
 * `execTransaction` from the owner's wallet; rejecting proposes the
 * same-number cancellation. The real core throughout; the mailbox, the
 * chain and the wallet stood in for.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '$lib/i18n/wasm-init.server';
import { secp256k1 } from '@noble/curves/secp256k1.js';
import { keccak_256 } from '@noble/hashes/sha3.js';

const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => storage.get(k) ?? null,
	setItem: (k: string, v: string) => void storage.set(k, v),
	removeItem: (k: string) => void storage.delete(k)
});

const world = vi.hoisted(() => ({
	proposals: [] as unknown[],
	submitted: [] as { safeTx: { to: string; nonce: string }; approval: { signature: string } }[],
	busy: 0,
	/** A mailbox that takes only typed data (as the service does today): v 31/32 refused. */
	typedOnly: false,
	owners: [] as string[],
	sent: [] as { from: string; to: string; data: string }[]
}));

vi.mock('../core/service', () => {
	class ServiceUnavailableError extends Error {}
	class ServiceBusyError extends ServiceUnavailableError {
		constructor(readonly waitMs: number | null) {
			super('busy');
		}
	}
	return {
		ServiceUnavailableError,
		ServiceBusyError,
		fetchGenesis: async () => ({ found: false }),
		fetchProposals: async () => ({ members: {}, proposals: world.proposals }),
		submitApproval: async (req: (typeof world.submitted)[number]) => {
			if (world.typedOnly && /(1f|20)$/.test(req.approval.signature)) {
				return {
					ok: false,
					refusal: { kind: 'approval', error: { kind: 'notAnEip712Signature' } }
				};
			}
			if (world.busy > 0) {
				world.busy -= 1;
				throw new ServiceBusyError(1_000);
			}
			world.submitted.push(req);
			return { ok: true, status: 'approved', proposal: {} };
		}
	};
});
vi.mock('../core/chain', () => ({
	hasCode: async () => true,
	readMembers: async () => ({ deployed: true, owners: world.owners, threshold: 2, nonce: 3n })
}));
vi.mock('./onchain', async (original) => ({
	...(await original<typeof import('./onchain')>()),
	onChainApprovals: async () => []
}));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (method: string) =>
		method === 'eth_getTransactionReceipt'
			? { result: { status: '0x1' } }
			: method === 'eth_gasPrice'
				? { result: '0x3b9aca00' }
				: { result: '0x5208' }
}));

const kernel = await import('../core/kernel');
const { OutsideTransaction } = await import('./outside.svelte');

const CHAIN = 5042002;
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const hexOf = (b: Uint8Array) => `0x${Buffer.from(b).toString('hex')}`;
const bytesOf = (h: string) => Uint8Array.from(Buffer.from(h.replace(/^0x/, ''), 'hex'));

function eoa(seed: number) {
	const key = new Uint8Array(32).fill(seed);
	const address = hexOf(keccak_256(secp256k1.getPublicKey(key, false).slice(1)).slice(12));
	const sign = (digest: Uint8Array) => {
		const sig = secp256k1.sign(digest, key, { prehash: false }) as unknown as {
			r: bigint;
			s: bigint;
			recovery: number;
		};
		const word = (n: bigint) => n.toString(16).padStart(64, '0');
		return `0x${word(sig.r)}${word(sig.s)}${(27 + sig.recovery).toString(16)}`;
	};
	return { key, address, sign };
}

const A = eoa(7);
const B = eoa(8);
const STRANGER = eoa(9);

const TX = {
	to: '0x000000000000000000000000000000000000dEaD',
	value: '0x2386f26fc10000',
	data: '0x',
	operation: 0,
	safeTxGas: '0x0',
	baseGas: '0x0',
	gasPrice: '0x0',
	gasToken: '0x0000000000000000000000000000000000000000',
	refundReceiver: '0x0000000000000000000000000000000000000000',
	nonce: '0x3'
};
const HASH = () => kernel.safeTxHash(CHAIN, SAFE, TX);

/** A wallet holding `who`: it signs typed data by its own EIP-712 hash, as a real one does. */
function walletOf(
	who: ReturnType<typeof eoa>,
	{ typed = true, signer = who }: { typed?: boolean; signer?: ReturnType<typeof eoa> } = {}
) {
	return {
		request: vi.fn(async ({ method, params }: { method: string; params?: unknown[] }) => {
			switch (method) {
				case 'eth_chainId':
					return `0x${CHAIN.toString(16)}`;
				case 'eth_signTypedData_v4': {
					if (!typed) throw Object.assign(new Error('Method not supported'), { code: 4200 });
					const typedData = JSON.parse(params![1] as string);
					const { domain, message } = typedData;
					const hash = kernel.safeTxHash(Number(domain.chainId), domain.verifyingContract, {
						...message,
						value: `0x${BigInt(message.value).toString(16)}`,
						safeTxGas: '0x0',
						baseGas: '0x0',
						gasPrice: '0x0',
						nonce: `0x${BigInt(message.nonce).toString(16)}`
					});
					return signer.sign(bytesOf(hash));
				}
				case 'personal_sign': {
					const prefix = new TextEncoder().encode('\x19Ethereum Signed Message:\n32');
					return signer.sign(
						keccak_256(new Uint8Array([...prefix, ...bytesOf(params![0] as string)]))
					);
				}
				case 'eth_sendTransaction':
					world.sent.push(params![0] as never);
					return `0x${'77'.repeat(32)}`;
				default:
					throw new Error(`unexpected ${method}`);
			}
		})
	};
}

function stored(approvals: unknown[]) {
	return {
		chainId: CHAIN,
		safe: SAFE,
		safeTxHash: HASH(),
		safeTx: TX,
		proposer: A.address,
		approvals,
		createdMs: 1,
		updatedMs: 1
	};
}

const link = () => ({ chainId: CHAIN, multisig: SAFE, safeTxHash: HASH() });

beforeEach(() => {
	world.owners = [A.address, B.address];
	world.submitted = [];
	world.sent = [];
	world.busy = 0;
	world.typedOnly = false;
	world.proposals = [
		stored([{ owner: A.address, signature: A.sign(bytesOf(HASH())), createdMs: 1 }])
	];
});

describe('reading the transaction from the mailbox alone (r4C-2)', () => {
	it("is the link's transaction, its approvals verified and counted by the chain's owners", async () => {
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.load.kind).toBe('ready');
		expect(tx.view!.item.approved).toBe(1);
		expect(tx.view!.hashes.safeTxHash).toBe(HASH());
		expect(tx.canApprove(B.address)).toBe(true);
		expect(tx.canApprove(A.address)).toBe(false);
		expect(tx.canApprove(STRANGER.address)).toBe(false);
		// B executing counts as the second approval (msg.sender).
		expect(tx.canExecute(B.address)).toBe(true);
		expect(tx.canExecute(STRANGER.address)).toBe(false);
	});

	it('refuses what the mailbox holds under the hash when it is another transaction', async () => {
		world.proposals = [{ ...stored([]), safeTx: { ...TX, value: '0x1' } }];
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.load.kind).toBe('mismatch');
	});

	it('an approval in the mailbox that does not verify is not counted', async () => {
		world.proposals = [
			stored([{ owner: A.address, signature: STRANGER.sign(bytesOf(HASH())), createdMs: 1 }])
		];
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.view!.item.approved).toBe(0);
		expect(tx.canExecute(B.address)).toBe(false);
	});

	it('the mailbox lacking it is said as missing', async () => {
		world.proposals = [];
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.load.kind).toBe('missing');
	});
});

describe('approving from the page (r4C-2)', () => {
	it('signs typed data, checks it in the core, hands it to the mailbox', async () => {
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		await tx.approve(walletOf(B), B.address);
		expect(tx.step).toEqual({ kind: 'approved', form: 'typedData' });
		expect(world.submitted).toHaveLength(1);
		const posted = world.submitted[0].approval.signature;
		expect(
			kernel
				.verifyEcdsaApproval(HASH(), { owner: B.address, signature: posted })
				.owner.toLowerCase()
		).toBe(B.address);
	});

	it("a signature that is not the owner's for this transaction is never handed on", async () => {
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		await tx.approve(walletOf(B, { signer: STRANGER }), B.address);
		expect(tx.step).toMatchObject({ kind: 'failed', reason: 'badSignature' });
		expect(world.submitted).toHaveLength(0);
	});

	it("r4C-16: a wallet without typed data is offered the hash; Safe's eth_sign form is posted", async () => {
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		const wallet = walletOf(B, { typed: false });
		await tx.approve(wallet, B.address);
		expect(tx.step).toEqual({ kind: 'unsupported' });
		await tx.approve(wallet, B.address, 'ethSign');
		expect(tx.step).toEqual({ kind: 'approved', form: 'ethSign' });
		expect(world.submitted[0].approval.signature.slice(-2)).toMatch(/^(1f|20)$/);
	});

	it('r4C-16: a mailbox that refuses the hash form is said as that (approve on chain is offered)', async () => {
		world.typedOnly = true;
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		await tx.approve(walletOf(B, { typed: false }), B.address, 'ethSign');
		expect(tx.step).toMatchObject({ kind: 'failed', reason: 'noEthSign', problem: null });
	});

	it('a busy mailbox is waited out and sent again, nothing signed twice', async () => {
		world.busy = 1;
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		const wallet = walletOf(B);
		await tx.approve(wallet, B.address);
		expect(tx.step).toEqual({ kind: 'approved', form: 'typedData' });
		expect(world.submitted).toHaveLength(1);
		expect(
			wallet.request.mock.calls.filter(([r]) => r.method === 'eth_signTypedData_v4')
		).toHaveLength(1);
	});

	it('a declined signature is said as declined in the wallet', async () => {
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		const wallet = {
			request: async ({ method }: { method: string }) => {
				if (method === 'eth_chainId') return `0x${CHAIN.toString(16)}`;
				throw Object.assign(new Error('User rejected'), { code: 4001 });
			}
		};
		await tx.approve(wallet, B.address);
		expect(tx.step).toMatchObject({ kind: 'failed', problem: { kind: 'declined' } });
	});
});

describe('executing and rejecting from the page (r4C-9)', () => {
	it("executes with the owner's own wallet: execTransaction to the multisig, the owner as msg.sender", async () => {
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		const fee = await tx.fee(B.address);
		expect(fee).toEqual({ wei: 21000n * 1_000_000_000n, count: 1, partial: false });
		await tx.execute(walletOf(B), B.address);
		expect(tx.step).toMatchObject({ kind: 'executed', txHash: `0x${'77'.repeat(32)}` });
		expect(world.sent).toHaveLength(1);
		expect(world.sent[0].to.toLowerCase()).toBe(SAFE.toLowerCase());
		expect(world.sent[0].from).toBe(B.address);
		// execTransaction(...)
		expect(world.sent[0].data.slice(0, 10)).toBe('0x6a761202');
		// The signatures hold A's own and B as msg.sender (v = 1).
		expect(world.sent[0].data).toContain(A.sign(bytesOf(HASH())).slice(2));
	});

	it('rejects by proposing the same-number cancellation, approved by the owner', async () => {
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.canReject(B.address)).toBe(true);
		await tx.reject(walletOf(B), B.address);
		expect(tx.step).toEqual({ kind: 'rejected' });
		expect(world.submitted).toHaveLength(1);
		const cancel = world.submitted[0].safeTx;
		expect(cancel.to.toLowerCase()).toBe(SAFE.toLowerCase());
		expect(BigInt(cancel.nonce)).toBe(3n);
	});
});

describe("the mailbox's refusal, in one line", () => {
	it('its own words where it has them, else its kind', async () => {
		const { shortRefusal, refusedForm } = await import('./outside.svelte');
		expect(shortRefusal({ kind: 'transaction', detail: 'delegatecall is refused' })).toBe(
			'delegatecall is refused'
		);
		expect(shortRefusal({ kind: 'approval', error: { kind: 'notAnOwner' } })).toBe('notAnOwner');
		expect(shortRefusal({ kind: 'full' })).toBe('full');
		expect(refusedForm({ kind: 'approval', error: { kind: 'notAnEip712Signature' } })).toBe(true);
		expect(refusedForm({ kind: 'approval', error: { kind: 'badSignature' } })).toBe(false);
	});
});
