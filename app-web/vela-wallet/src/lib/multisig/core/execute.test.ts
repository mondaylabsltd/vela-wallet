import { beforeEach, describe, expect, it, vi } from 'vitest';

const EXECUTOR = '0x00000000000000000000000000000000000000aa';
const ALICE = '0x00000000000000000000000000000000000000a1';
const BOB = '0x00000000000000000000000000000000000000b0';
const CAROL = '0x00000000000000000000000000000000000000c0';
const SAFE = '0x00000000000000000000000000000000000000cc';
const RECIPIENT = '0x00000000000000000000000000000000000000fe';
const USDC = 10n ** 18n;

/** The fee the in-band path is handed (`quotedFee`), and the calls, as sent. */
const sent = {
	count: 0,
	calls: null as unknown,
	quoted: null as unknown,
	beforeSign: null as
		null | ((f: { token: string | null; amount: bigint; value: bigint }) => Promise<void>)
};
const balance = { value: '0x0' };
/** Accounts with code on the chain (deployed). */
const code = new Set<string>();

vi.mock('$lib/services/accounts', () => ({
	findAccountByAddress: () => ({ address: EXECUTOR, publicKeyHex: '0x04', keys: [] })
}));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async () => ({ result: balance.value })
}));
vi.mock('$lib/services/safe-transaction', () => ({
	keySetOf: () => ({ keys: [] }),
	sendBatchCalls: async (
		_from: string,
		calls: unknown,
		_chain: number,
		_signer: unknown,
		signFn: { beforeSign?: typeof sent.beforeSign },
		_max: unknown,
		_token: unknown,
		quoted: unknown
	) => {
		sent.calls = calls;
		sent.quoted = quoted;
		sent.beforeSign = signFn.beforeSign ?? null;
		sent.count += 1;
		return { userOpHash: '0x01', waitForTxHash: async () => '0x02' };
	}
}));
vi.mock('$lib/core/kernels', () => ({
	fromHex: () => new Uint8Array(),
	verifySafeWebAuthn: () => ({ ok: true })
}));
vi.mock('$lib/signing/sign-challenge', () => ({ signChallenge: async () => ({}) }));
vi.mock('./chain', () => ({ hasCode: async (_c: number, a: string) => code.has(a) }));
vi.mock('./display', () => ({ tokenInfo: async () => ({ symbol: 'USDC', decimals: 18 }) }));
vi.mock('./execution-fee', () => ({ quoteExecutionFee: async () => ({ kind: 'failed' }) }));
vi.mock('./kernel', () => ({
	// One deploy call per genesis, then execTransaction — the core's order.
	executionCalls: (plan: {
		deployMultisig: unknown;
		deployOwners: unknown[];
		approvals: { owner: string }[];
	}) => [
		...(plan.deployMultisig ? [{ to: '0xfactory', value: '0x0', data: '0xmultisig' }] : []),
		...plan.deployOwners.map((g) => ({ to: '0xfactory', value: '0x0', data: `0x${String(g)}` })),
		{
			to: SAFE,
			value: '0x0',
			data: `0x6a761202:${plan.approvals.map((a) => a.owner).join(',')}`
		}
	],
	isEcdsaApproval: (a: { signature?: string }) => 'signature' in a,
	sameAddress: (a: string, b: string) => a.toLowerCase() === b.toLowerCase()
}));

vi.mock('$lib/services/tx-reconciler', () => ({ pollUserOpStatus: async () => null }));

const {
	submitExecution,
	previewExecution,
	planExecution,
	spendingOf,
	revertText,
	plainReason,
	stoppedBeforeSigning,
	ExecutorCannotPayError,
	ExecutionWouldFailError,
	ExecutionUnpricedError,
	relayRefused,
	NOT_TAKEN_AFTER_MS
} = await import('./execute');
type Reads = NonNullable<Parameters<typeof previewExecution>[1]>;
type Input = Parameters<typeof submitExecution>[0];
type Priced = Extract<Awaited<ReturnType<typeof previewExecution>>, { kind: 'priced' }>;

const passkey = (owner: string) => ({ owner, ownerGenesis: owner.slice(-2) }) as never;
const ecdsa = (owner: string) => ({ owner, signature: '0x' }) as never;

const input = (over: Partial<Input> = {}) =>
	({
		chainId: 5042002,
		multisig: SAFE,
		genesis: {},
		members: { owners: [EXECUTOR, ALICE, BOB], threshold: 2, deployed: true, nonce: 0n },
		tx: { to: RECIPIENT, value: '0x0', data: '0x', operation: 0 },
		approvals: [],
		executor: { address: EXECUTOR },
		...over
	}) as unknown as Input;

/** The reads a preview makes, each set per test. */
function reads(over: Partial<Reads> = {}): Reads {
	return {
		quote: async () => ({
			kind: 'ok',
			fee: { amount: USDC / 20n, recipient: RECIPIENT, tier: 'standard' },
			totalGas: 500_000n,
			executorDeployed: true
		}),
		balanceOf: async () => 10n * USDC,
		callRefusal: async () => null,
		gasOf: async () => 221_000n,
		token: async () => ({ symbol: 'USDC', decimals: 18 }),
		...over
	};
}

beforeEach(() => {
	sent.count = 0;
	sent.calls = null;
	sent.quoted = null;
	sent.beforeSign = null;
	code.clear();
});

describe('which approvals an execution carries (r4-7: no member deployed for nothing)', () => {
	it('carries only what the threshold needs, those needing no deployment first', async () => {
		code.add(BOB);
		// 2-of-3 with the executor an owner: one other approval is needed.
		const plan = await planExecution(input({ approvals: [passkey(ALICE), passkey(BOB)] }));
		expect(plan.activates, 'Bob has an account there already; Alice is not deployed').toEqual([]);
		expect((plan.calls.at(-1) as { data: string }).data).toContain(BOB);
		expect((plan.calls.at(-1) as { data: string }).data).not.toContain(ALICE);
		expect(plan.calls).toHaveLength(1);
	});

	it('a plain account’s approval needs nothing deployed', async () => {
		const plan = await planExecution(input({ approvals: [passkey(ALICE), ecdsa(CAROL)] }));
		expect(plan.activates).toEqual([]);
	});

	it('activates the passkey member it needs, and the multisig on its first execution', async () => {
		const plan = await planExecution(
			input({
				approvals: [passkey(ALICE)],
				members: { owners: [EXECUTOR, ALICE, BOB], threshold: 2, deployed: false, nonce: 0n }
			})
		);
		expect(plan.deploysMultisig).toBe(true);
		expect(plan.activates).toEqual([ALICE]);
		expect(plan.calls).toHaveLength(3);
	});
});

describe('what a transaction spends', () => {
	it('sums a batch per coin: transfers, and calls that send the chain’s coin', () => {
		const action = {
			kind: 'batch',
			calls: [
				{
					to: BOB,
					value: '5',
					data: '0x',
					action: { kind: 'transfer', token: null, to: BOB, amount: '5' }
				},
				{
					to: '0xt',
					value: '0',
					data: '0x',
					action: { kind: 'transfer', token: '0xT', to: BOB, amount: '7' }
				},
				{
					to: '0xd',
					value: '3',
					data: '0x12',
					action: { kind: 'call', to: '0xd', value: '3', data: '0x12' }
				},
				{
					to: '0xt',
					value: '0',
					data: '0x',
					action: { kind: 'transfer', token: '0xt', to: CAROL, amount: '1' }
				}
			]
		} as never;
		expect(spendingOf(action)).toEqual([
			{ token: null, amount: 8n },
			{ token: '0xT', amount: 8n }
		]);
	});
});

describe('an execution looked at before its passkey (r4-5)', () => {
	it('that spends more than the multisig holds fails, in its own words, before any fee is asked', async () => {
		const quote = vi.fn();
		const preview = await previewExecution(
			input({ action: { kind: 'transfer', token: null, to: BOB, amount: (USDC / 2n).toString() } }),
			reads({ balanceOf: async () => (28n * USDC) / 100n, quote })
		);
		expect(preview).toEqual({
			kind: 'fails',
			why: {
				kind: 'short',
				symbol: 'USDC',
				decimals: 18,
				have: (28n * USDC) / 100n,
				need: USDC / 2n
			}
		});
		expect(quote).not.toHaveBeenCalled();
	});

	it('whose call the target refuses fails with that reason', async () => {
		const preview = await previewExecution(
			input(),
			reads({ callRefusal: async () => ({ kind: 'reverts', reason: 'paused' }) })
		);
		expect(preview).toEqual({ kind: 'fails', why: { kind: 'reverts', reason: 'paused' } });
	});

	it('the relay’s simulation reverting fails; its generic wrapper is not passed on as a reason', async () => {
		const preview = await previewExecution(
			input(),
			reads({
				quote: async () => ({
					kind: 'reverts',
					reason: 'Safe execution failed: the target call in executeUserOp reverted'
				})
			})
		);
		expect(preview).toEqual({ kind: 'fails', why: { kind: 'reverts', reason: null } });
	});

	it('no price: unpriced, never a guess', async () => {
		expect(
			await previewExecution(input(), reads({ quote: async () => ({ kind: 'failed' }) }))
		).toEqual({ kind: 'unpriced' });
	});

	it('is priced at the relay’s quote, with the one-time setup’s share measured', async () => {
		const preview = await previewExecution(
			input({
				approvals: [passkey(ALICE)],
				members: { owners: [EXECUTOR, ALICE, BOB], threshold: 2, deployed: false, nonce: 0n }
			}),
			reads({
				balanceOf: async (_c, _t, holder) => (holder === EXECUTOR ? USDC / 100n : 10n * USDC)
			})
		);
		expect(preview.kind).toBe('priced');
		if (preview.kind !== 'priced') return;
		expect(preview.fee.amount).toBe(USDC / 20n);
		// Two deployments of 221k each, less the 21k each carries alone: 400k of 500k.
		expect(preview.setup).toEqual({
			multisig: true,
			members: [ALICE],
			executor: false,
			amount: ((USDC / 20n) * 4n) / 5n
		});
		expect(preview.balance).toBe(USDC / 100n);
		expect(preview.canPay, 'holds 0.01 for a 0.05 fee').toBe(false);
	});

	it('says no setup share when the executor’s own account is deployed by it too', async () => {
		const preview = await previewExecution(
			input({ members: { owners: [EXECUTOR, ALICE], threshold: 1, deployed: false, nonce: 0n } }),
			reads({
				quote: async () => ({
					kind: 'ok',
					fee: { amount: USDC / 20n, recipient: RECIPIENT },
					totalGas: 500_000n,
					executorDeployed: false
				})
			})
		);
		expect(preview.kind === 'priced' && preview.setup).toEqual({
			multisig: true,
			members: [],
			executor: true,
			amount: null
		});
	});
});

describe('executing a multisig transaction', () => {
	const priced = (canPay: boolean): Priced =>
		({
			kind: 'priced',
			calls: [{ to: SAFE, value: '0x0', data: '0x6a761202' }],
			fee: { amount: 207n * 10n ** 15n, recipient: RECIPIENT, tier: 'standard' },
			balance: canPay ? USDC : 79n * 10n ** 15n,
			canPay,
			setup: { multisig: false, members: [], executor: false, amount: null }
		}) as Priced;

	it('says the executor cannot pay before any passkey is asked', async () => {
		const refused = await submitExecution(input(), priced(false)).catch((e: unknown) => e);
		expect(refused).toBeInstanceOf(ExecutorCannotPayError);
		expect(refused).toMatchObject({ need: 207n * 10n ** 15n, have: 79n * 10n ** 15n });
		expect(sent.count, 'nothing signed, nothing sent').toBe(0);
		expect(stoppedBeforeSigning(refused)).toBe(true);
	});

	it('signs the fee shown, on the calls priced — never a second, send-time price', async () => {
		await submitExecution(input(), priced(true));
		expect(sent.count).toBe(1);
		expect(sent.quoted).toEqual({
			amount: 207n * 10n ** 15n,
			recipient: RECIPIENT,
			tier: 'standard'
		});
		expect(sent.calls).toEqual([{ to: SAFE, value: '0x0', data: '0x6a761202' }]);
	});

	it('signs nothing for one that would fail, or that has no price', async () => {
		const fails = await submitExecution(input(), {
			kind: 'fails',
			why: { kind: 'reverts', reason: null }
		}).catch((e: unknown) => e);
		expect(fails).toBeInstanceOf(ExecutionWouldFailError);
		const unpriced = await submitExecution(input(), { kind: 'unpriced' }).catch((e: unknown) => e);
		expect(unpriced).toBeInstanceOf(ExecutionUnpricedError);
		expect(sent.count).toBe(0);
	});

	it('without a preview, looks first (here: no price) and signs nothing', async () => {
		const refused = await submitExecution(input()).catch((e: unknown) => e);
		expect(refused).toBeInstanceOf(ExecutionUnpricedError);
		expect(sent.count).toBe(0);
	});

	it('keeps the balance check at signing time, for a fee in the chain’s coin only', async () => {
		await submitExecution(input(), priced(true));
		const check = sent.beforeSign!;
		balance.value = '0x0';
		await expect(check({ token: null, amount: 1n, value: 0n })).rejects.toBeInstanceOf(
			ExecutorCannotPayError
		);
		await expect(check({ token: '0xdd', amount: 1n, value: 0n })).resolves.toBeUndefined();
		balance.value = 'unreadable';
		await expect(check({ token: null, amount: 1n, value: 0n })).resolves.toBeUndefined();
	});
});

describe('reasons, as a person reads them', () => {
	it('decodes Error(string)', () => {
		// Error("Pausable: paused")
		const data =
			'0x08c379a0' +
			'0000000000000000000000000000000000000000000000000000000000000020' +
			'0000000000000000000000000000000000000000000000000000000000000010' +
			'5061757361626c653a2070617573656400000000000000000000000000000000';
		expect(revertText(data)).toBe('Pausable: paused');
		expect(revertText('0x12345678')).toBeNull();
	});

	it('drops wrappers that only say it reverted', () => {
		expect(plainReason('the target call in executeUserOp reverted')).toBeNull();
		expect(plainReason('execution reverted')).toBeNull();
		expect(plainReason('ERC20: transfer amount exceeds balance')).toBe(
			'ERC20: transfer amount exceeds balance'
		);
	});
});

describe('an execution the relay may not have taken (ms-3)', () => {
	type Status = { status: string; txHash?: string } | null;
	const answers = (...list: Status[]) => {
		let i = 0;
		return (async () => list[Math.min(i++, list.length - 1)]) as unknown as Parameters<
			typeof relayRefused
		>[3];
	};

	it('is said at once when the relay refused it before any block', async () => {
		const signal = new AbortController().signal;
		expect(await relayRefused('0x01', 100, signal, answers({ status: 'rejected' }), 0)).toBe(true);
	});

	it('is said when the relay keeps never having heard of it', async () => {
		vi.useFakeTimers();
		try {
			const status = answers({ status: 'not_found' });
			const refused = relayRefused('0x01', 100, new AbortController().signal, status, 1_000);
			await vi.advanceTimersByTimeAsync(NOT_TAKEN_AFTER_MS + 1_000);
			expect(await refused).toBe(true);
		} finally {
			vi.useRealTimers();
		}
	});

	it('is followed on when the relay holds it, or names its bundle', async () => {
		const signal = new AbortController().signal;
		expect(await relayRefused('0x01', 100, signal, answers({ status: 'queued' }), 0)).toBe(false);
		expect(
			await relayRefused('0x01', 100, signal, answers({ status: 'rejected', txHash: '0x02' }), 0)
		).toBe(false);
		expect(
			await relayRefused(
				'0x01',
				100,
				signal,
				answers({ status: 'not_found' }, { status: 'submitted' }),
				0
			)
		).toBe(false);
	});

	it('one the relay took is followed past "held": a later refusal is said at once (r4-5)', async () => {
		const signal = new AbortController().signal;
		expect(
			await relayRefused(
				'0x01',
				100,
				signal,
				answers({ status: 'queued' }, { status: 'submitted' }, { status: 'rejected' }),
				0,
				true
			)
		).toBe(true);
		expect(
			await relayRefused(
				'0x01',
				100,
				signal,
				answers({ status: 'queued' }, { status: 'included', txHash: '0x02' }),
				0,
				true
			)
		).toBe(false);
	});

	it('stops when the receipt came first', async () => {
		const watch = new AbortController();
		const refused = relayRefused('0x01', 100, watch.signal, answers(null), 5);
		watch.abort();
		expect(await refused).toBe(false);
	});
});
