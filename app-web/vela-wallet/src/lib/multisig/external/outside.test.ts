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

const SAFE_ADDRESS = vi.hoisted(() => '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83');
const world = vi.hoisted(() => ({
	proposals: [] as unknown[],
	submitted: [] as { safeTx: { to: string; nonce: string }; approval: { signature: string } }[],
	busy: 0,
	/** A mailbox that takes only typed data (as the service does today): v 31/32 refused. */
	typedOnly: false,
	owners: [] as string[],
	sent: [] as { from: string; to: string; data: string }[],
	/** §3.5: the network's verdict as the chain would give it (`null`: not checkable). */
	findings: [] as unknown[] | null,
	/** Not deployed yet: deployed by an execution from here, from the registry's genesis. */
	deployed: true,
	/** The core's verdict on the code read right before a deploy. */
	authority: { kind: 'verified', owners: [], threshold: 2 } as unknown,
	/** Refusals the mailbox answers with, in turn, before it takes one. */
	refuse: [] as unknown[],
	/** Every submission, refused or taken, and whether it carried the setup. */
	tried: [] as { signature: string; genesis: boolean }[],
	/** The wallet's coin, as `eth_getBalance` answers. */
	balance: '0x0',
	/** The multisig's next number on the chain. */
	nonce: 3n
}));
vi.mock('../core/judge', () => ({ judgeOn: async () => world.findings }));
vi.mock('../core/authority', () => ({ counterfactualFacts: async () => ({ codes: {} }) }));
vi.mock('../core/kernel', async (original) => ({
	...(await original<typeof import('../core/kernel')>()),
	// The registry's genesis stands for this multisig (the core's derivation is its own test's).
	genesisAddress: () => SAFE_ADDRESS,
	counterfactualAuthority: () => world.authority,
	namedGenesis: (g: unknown) => g
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
		fetchGenesis: async () =>
			world.deployed ? { found: false } : { found: true, genesis: { name: 'Fixture' } },
		fetchProposals: async () => ({ members: {}, proposals: world.proposals }),
		submitApproval: async (req: (typeof world.submitted)[number] & { genesis?: unknown }) => {
			world.tried.push({ signature: req.approval.signature, genesis: 'genesis' in req });
			if (world.refuse.length > 0) return { ok: false, refusal: world.refuse.shift() };
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
	readMembers: async () => ({
		deployed: world.deployed,
		owners: world.owners,
		threshold: 2,
		nonce: world.nonce
	})
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
				: method === 'eth_getBalance'
					? { result: world.balance }
					: { result: '0x5208' }
}));

const kernel = await import('../core/kernel');
const { OutsideTransaction } = await import('./outside.svelte');

const CHAIN = 5042002;
const SAFE = SAFE_ADDRESS;
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
	world.findings = [];
	world.deployed = true;
	world.authority = { kind: 'verified', owners: [], threshold: 2 };
	world.refuse = [];
	world.tried = [];
	world.balance = '0x0';
	world.nonce = 3n;
	storage.clear();
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

	it('r7-3: approvals that already meet the threshold are what it carries — the executor only sends it', async () => {
		world.owners = [A.address, B.address, STRANGER.address];
		world.proposals = [
			stored([
				{ owner: A.address, signature: A.sign(bytesOf(HASH())), createdMs: 1 },
				{ owner: STRANGER.address, signature: STRANGER.sign(bytesOf(HASH())), createdMs: 2 }
			])
		];
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.view!.item.approved).toBe(2);
		await tx.execute(walletOf(B), B.address);
		const data = world.sent[0].data.toLowerCase();
		// Both given approvals counted on chain; no `msg.sender` approval
		// (v = 1, B's address as r) taking the place of one.
		expect(data).toContain(A.sign(bytesOf(HASH())).slice(2).toLowerCase());
		expect(data).toContain(STRANGER.sign(bytesOf(HASH())).slice(2).toLowerCase());
		expect(data).not.toContain(
			`${B.address.slice(2).toLowerCase().padStart(64, '0')}${'0'.repeat(64)}01`
		);
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

/**
 * §3.5 on the page: who can act for the multisig on that network is judged
 * as in Vela. A refused genesis makes it read-only (nothing approved,
 * executed or rejected from here); a network that cannot be checked acts on
 * nothing yet; a deploy is built only from a genesis Verified on code read
 * right before it.
 */
describe('who can act for it on this network (§3.5)', () => {
	const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

	it('a refused genesis: read-only — nothing to approve, execute or reject', async () => {
		world.findings = [
			{ from: 'genesis', chainId: CHAIN, reason: { kind: 'module', address: MODULE } }
		];
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.gate?.kind).toBe('refused');
		expect(tx.canApprove(B.address)).toBe(false);
		expect(tx.canExecute(B.address)).toBe(false);
		expect(tx.canReject(B.address)).toBe(false);
		await tx.approve(walletOf(B), B.address);
		await tx.execute(walletOf(B), B.address);
		expect(world.submitted).toEqual([]);
		expect(world.sent).toEqual([]);
	});

	it('not checkable yet: nothing acts', async () => {
		world.findings = null;
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.gate).toEqual({ kind: 'unchecked' });
		expect(tx.canApprove(B.address)).toBe(false);
	});

	it('a risk found there: said, and the owner may still act (the page asks each time)', async () => {
		world.findings = [{ from: 'chain', chainId: CHAIN, risk: { kind: 'module', address: MODULE } }];
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.gate?.kind).toBe('risk');
		expect(tx.canApprove(B.address)).toBe(true);
	});

	it('a deploy from a genesis the code read just then does not verify: never sent', async () => {
		world.deployed = false;
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.canExecute(B.address)).toBe(true);
		world.authority = { kind: 'refused', reasons: [{ kind: 'module', address: MODULE }] };
		await tx.execute(walletOf(B), B.address);
		expect(tx.step).toMatchObject({ kind: 'failed', reason: 'deployRefused' });
		expect(world.sent).toEqual([]);
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

describe('a refusal never loses what was signed (r5reg-12)', () => {
	const unchecked = {
		kind: 'genesis',
		detail: 'the proxy code of factory 0x4e1D… on chain 5042002 could not be read'
	};

	it('said by kind, never its raw text; kept, and sent again with nothing signed twice', async () => {
		const { refusalKind } = await import('./outside.svelte');
		expect(refusalKind(unchecked)).toBe('setupUnchecked');
		expect(refusalKind({ kind: 'genesis', detail: 'the genesis derives 0x1 on chain 1' })).toBe(
			'setup'
		);
		expect(refusalKind({ kind: 'approval', error: { kind: 'badSignature' } })).toBe('signature');
		expect(refusalKind({ kind: 'tooManyProposals', limit: 50 })).toBe('full');
		expect(refusalKind({ kind: 'transaction', detail: 'delegatecall' })).toBe('transaction');
		expect(refusalKind({ kind: 'error', status: 404, error: 'unknown Safe' })).toBe('other');

		world.refuse = [{ kind: 'full' }];
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		const wallet = walletOf(B);
		await tx.approve(wallet, B.address);
		expect(tx.step).toMatchObject({ kind: 'failed', refused: 'full', unsent: true });
		expect(storage.get('vela.connect.unsent.v1')).toContain(B.address);
		const signed = world.tried[0].signature;
		await tx.sendAgain();
		expect(tx.step).toEqual({ kind: 'approved', form: 'typedData' });
		expect(world.submitted.map((r) => r.approval.signature)).toEqual([signed]);
		expect(
			wallet.request.mock.calls.filter(([r]) => r.method === 'eth_signTypedData_v4')
		).toHaveLength(1);
		expect(storage.get('vela.connect.unsent.v1')).toBeUndefined();
	});

	it('a rejection refused is kept too, and a reload sends it again on its own', async () => {
		world.refuse = [{ kind: 'error', status: 503, error: null }];
		const first = new OutsideTransaction(link());
		await first.refresh();
		await first.reject(walletOf(B), B.address);
		expect(first.step).toMatchObject({ kind: 'failed', what: 'reject', unsent: true });
		// The page reloaded: the same link, a new page.
		const again = new OutsideTransaction(link());
		await again.refresh();
		await vi.waitFor(() => expect(again.step).toEqual({ kind: 'rejected' }));
		expect(world.submitted).toHaveLength(1);
		expect(world.submitted[0].safeTx.to.toLowerCase()).toBe(SAFE.toLowerCase());
	});

	it("refused for the multisig's setup alone: sent once more without it", async () => {
		world.deployed = false;
		world.refuse = [unchecked];
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.view!.genesis).not.toBeNull();
		await tx.approve(walletOf(B), B.address);
		expect(world.tried.map((t) => t.genesis)).toEqual([true, false]);
		expect(tx.step).toEqual({ kind: 'approved', form: 'typedData' });
	});

	it('kept for a transaction whose number is used since: dropped, nothing to send', async () => {
		world.refuse = [{ kind: 'full' }];
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		await tx.approve(walletOf(B), B.address);
		expect(storage.get('vela.connect.unsent.v1')).toBeDefined();
		world.nonce = 4n;
		const used = new OutsideTransaction(link());
		await used.refresh();
		expect(used.view!.item.status).toBe('done');
		expect(storage.get('vela.connect.unsent.v1')).toBeUndefined();
		expect(used.step).toEqual({ kind: 'idle' });
		expect(world.submitted).toHaveLength(0);
	});
});

describe('enough approvals, and paying for an execution (r4C-14, r5reg-13)', () => {
	it('approvals that meet the threshold: nothing to sign, executing is left', async () => {
		world.owners = [A.address, B.address, STRANGER.address];
		world.proposals = [
			stored([
				{ owner: A.address, signature: A.sign(bytesOf(HASH())), createdMs: 1 },
				{ owner: STRANGER.address, signature: STRANGER.sign(bytesOf(HASH())), createdMs: 2 }
			])
		];
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(tx.view!.item.approved).toBe(2);
		expect(tx.enough).toBe(true);
		expect(tx.canApprove(B.address)).toBe(false);
		expect(tx.canExecute(B.address)).toBe(true);
	});

	it("reads the wallet's coin, which pays the fee", async () => {
		world.balance = '0x2386f26fc10000';
		const tx = new OutsideTransaction(link());
		await tx.refresh();
		expect(await tx.balance(B.address)).toBe(10_000_000_000_000_000n);
	});
});

describe("Vela's request, counted before it is signed (r4C-14)", () => {
	it('read from the mailbox and the chain; not there yet: null', async () => {
		const { readTransaction } = await import('./outside.svelte');
		const read = await readTransaction(link());
		expect(read?.view?.item.approved).toBe(1);
		expect(read?.enough).toBe(false);
		world.proposals = [];
		expect(await readTransaction(link())).toBeNull();
	});
});
