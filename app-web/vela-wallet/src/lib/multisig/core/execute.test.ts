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
/** r4a-23: the relay out of reach this many times, before the passkey (`before`) or after. */
const relay = vi.hoisted(() => ({ down: 0, before: true, signed: 0 }));
const classes = vi.hoisted(() => {
	class PoolFailedError extends Error {
		rateLimited = false;
		internal = false;
		maybeDelivered = false;
	}
	class UserOpNotSentError extends Error {
		rejection = null;
	}
	return { PoolFailedError, UserOpNotSentError };
});
/** §3.5: the core's verdict on the code read for a deployment, set per test. */
const authority = vi.hoisted(() => ({
	verdict: { kind: 'verified', owners: [], threshold: 1 } as unknown,
	reads: 0
}));

vi.mock('$lib/services/accounts', () => ({
	findAccountByAddress: () => ({ address: EXECUTOR, publicKeyHex: '0x04', keys: [] })
}));
vi.mock('$lib/services/rpc-pool', () => ({
	PoolFailedError: classes.PoolFailedError,
	poolRpcCall: async () => ({ result: balance.value })
}));
vi.mock('$lib/services/safe-transaction', () => ({
	UserOpNotSentError: classes.UserOpNotSentError,
	keySetOf: () => ({ keys: [] }),
	sendBatchCalls: async (
		_from: string,
		calls: unknown,
		_chain: number,
		_signer: unknown,
		signFn: ((c: Uint8Array) => Promise<unknown>) & { beforeSign?: typeof sent.beforeSign },
		_max: unknown,
		_token: unknown,
		quoted: unknown
	) => {
		if (relay.down > 0 && relay.before) {
			relay.down -= 1;
			throw new classes.PoolFailedError('relay unreachable');
		}
		sent.calls = calls;
		sent.quoted = quoted;
		sent.beforeSign = signFn.beforeSign ?? null;
		await signFn(new Uint8Array());
		relay.signed += 1;
		if (relay.down > 0) {
			relay.down -= 1;
			throw new classes.UserOpNotSentError('relay unreachable; nothing was sent');
		}
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
		executor: string | null;
	}) => [
		...(plan.deployMultisig ? [{ to: '0xfactory', value: '0x0', data: '0xmultisig' }] : []),
		...plan.deployOwners.map((g) => ({ to: '0xfactory', value: '0x0', data: `0x${String(g)}` })),
		{
			to: SAFE,
			value: '0x0',
			data: `0x6a761202:${plan.approvals.map((a) => a.owner).join(',')}|${plan.executor ?? ''}`
		}
	],
	counterfactualAuthority: () => authority.verdict,
	isEcdsaApproval: (a: { signature?: string }) => 'signature' in a,
	isPasskeyApproval: (a: { ownerGenesis?: unknown }) => 'ownerGenesis' in a,
	sameAddress: (a: string, b: string) => a.toLowerCase() === b.toLowerCase()
}));

vi.mock('./authority', () => ({
	counterfactualFacts: async () => {
		authority.reads += 1;
		return { codes: {} };
	}
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
	ExecutionUncheckedError,
	ExecutionSpentError,
	bounded,
	throwIfStopped,
	relayRefused,
	NOT_TAKEN_AFTER_MS
} = await import('./execute');
const { GenesisRefusedError } = await import('./refusal');
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
		tx: { to: RECIPIENT, value: '0x0', data: '0x', operation: 0, nonce: '0' },
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
		nonceOf: async () => 0n,
		...over
	};
}

beforeEach(() => {
	sent.count = 0;
	sent.calls = null;
	sent.quoted = null;
	sent.beforeSign = null;
	code.clear();
	relay.down = 0;
	relay.before = true;
	relay.signed = 0;
	authority.verdict = { kind: 'verified', owners: [], threshold: 1 };
	authority.reads = 0;
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

	it('r7-3: approvals that meet the threshold are all carried — an outside wallet’s too; the executor only sends it', async () => {
		code.add(ALICE);
		const plan = await planExecution(input({ approvals: [passkey(ALICE), ecdsa(BOB)] }));
		expect(plan.carries).toEqual([ALICE, BOB]);
		expect(plan.executorApproves).toBe(false);
		const exec = (plan.calls.at(-1) as { data: string }).data;
		expect(exec).toContain(BOB);
		expect(exec.endsWith('|'), 'no msg.sender approval taking a given one’s place').toBe(true);
		expect(plan.activates).toEqual([]);
	});

	it('r7-3: one short, an owner executing approves by sending it — no approval of its own asked', async () => {
		const plan = await planExecution(input({ approvals: [ecdsa(BOB)] }));
		expect(plan.carries).toEqual([BOB]);
		expect(plan.executorApproves).toBe(true);
		expect((plan.calls.at(-1) as { data: string }).data).toContain(`|${EXECUTOR}`);
	});

	it('r7-3: the executor’s own approval in the mailbox is not carried twice', async () => {
		code.add(EXECUTOR);
		const plan = await planExecution(input({ approvals: [passkey(EXECUTOR), ecdsa(BOB)] }));
		expect(plan.carries).toEqual([BOB]);
		expect(plan.executorApproves).toBe(true);
	});

	it('an executor that is no owner carries the threshold’s approvals, and approves nothing', async () => {
		const plan = await planExecution(
			input({
				approvals: [ecdsa(ALICE), ecdsa(BOB)],
				executor: { address: CAROL } as never
			})
		);
		expect(plan.carries).toEqual([ALICE, BOB]);
		expect(plan.executorApproves).toBe(false);
	});

	it('the look says what it carries: the approvals counted when it runs', async () => {
		const preview = await previewExecution(
			input({ approvals: [ecdsa(ALICE), ecdsa(BOB)] }),
			reads()
		);
		expect(preview).toMatchObject({
			kind: 'priced',
			carries: [ALICE, BOB],
			executorApproves: false
		});
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

	it('r5reg-21: a revert after its number was used is "spent" — another member ran it — never "would fail"', async () => {
		const nonceOf = vi.fn(async () => 1n);
		const reverts = reads({
			quote: async () => ({ kind: 'reverts', reason: 'GS026' }),
			nonceOf
		});
		expect(await previewExecution(input(), reverts)).toEqual({ kind: 'spent' });
		// The coin it sent is gone too: still what ran, not "the multisig holds less".
		expect(
			await previewExecution(
				input({ action: { kind: 'transfer', token: null, to: BOB, amount: USDC.toString() } }),
				reads({ balanceOf: async () => 0n, nonceOf })
			)
		).toEqual({ kind: 'spent' });
		// Its number still open: a failure is a failure.
		expect(await previewExecution(input(), { ...reverts, nonceOf: async () => 0n })).toMatchObject({
			kind: 'fails'
		});
		// The nonce unreadable: the failure stands (nothing is claimed).
		expect(
			await previewExecution(input(), { ...reverts, nonceOf: async () => null })
		).toMatchObject({ kind: 'fails' });
		// A look that runs never reads the nonce.
		nonceOf.mockClear();
		await previewExecution(input(), reads({ nonceOf }));
		expect(nonceOf).not.toHaveBeenCalled();
		// And Execute stops before any passkey, on its own error.
		expect(() => throwIfStopped({ kind: 'spent' })).toThrow(ExecutionSpentError);
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
			carries: [],
			executorApproves: true,
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

describe('a read that never answers (round 5)', () => {
	it('is bounded: past its time it said nothing, and the look goes on', async () => {
		vi.useFakeTimers();
		try {
			const never = new Promise<bigint | null>(() => {});
			const read = bounded(never, null, 8_000);
			await vi.advanceTimersByTimeAsync(8_000);
			expect(await read).toBeNull();
			expect(await bounded(Promise.resolve(5n), null, 8_000)).toBe(5n);
		} finally {
			vi.useRealTimers();
		}
	});

	it('a look that could not check it stops before the passkey, and the approval is kept', () => {
		expect(stoppedBeforeSigning(new ExecutionUncheckedError())).toBe(true);
	});

	it('throwIfStopped: every look but a priced, payable one stops, with the error the sheet says', () => {
		expect(() => throwIfStopped({ kind: 'unchecked' })).toThrow(ExecutionUncheckedError);
		expect(() => throwIfStopped({ kind: 'unpriced' })).toThrow(ExecutionUnpricedError);
		expect(() =>
			throwIfStopped({ kind: 'fails', why: { kind: 'unknown' } } as unknown as Parameters<
				typeof throwIfStopped
			>[0])
		).toThrow(ExecutionWouldFailError);
		const priced = (canPay: boolean) =>
			({
				kind: 'priced',
				calls: [],
				fee: { token: null, amount: 5n, value: 5n },
				canPay,
				balance: 1n
			}) as unknown as Parameters<typeof throwIfStopped>[0];
		expect(() => throwIfStopped(priced(false))).toThrow(ExecutorCannotPayError);
		expect(() => throwIfStopped(priced(true))).not.toThrow();
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

describe('a relay out of reach for a moment (r4a-23)', () => {
	const priced = (): Priced =>
		({
			kind: 'priced',
			calls: [{ to: SAFE, value: '0x0', data: '0x6a761202' }],
			fee: { amount: 10n ** 15n, recipient: RECIPIENT, tier: 'standard' },
			balance: USDC,
			canPay: true,
			carries: [],
			executorApproves: true,
			setup: { multisig: false, members: [], executor: false, amount: null }
		}) as Priced;

	it('before anything is signed: asked once more on its own, one passkey in all', async () => {
		relay.down = 1;
		await submitExecution(input(), priced());
		expect(sent.count).toBe(1);
		expect(relay.signed).toBe(1);
	});

	it('twice in a row: said, not retried without end', async () => {
		relay.down = 2;
		await expect(submitExecution(input(), priced())).rejects.toBeInstanceOf(
			classes.PoolFailedError
		);
		expect(relay.signed).toBe(0);
	});

	it('after the passkey: never retried behind the person’s back (it would ask again)', async () => {
		relay.before = false;
		relay.down = 1;
		await expect(submitExecution(input(), priced())).rejects.toBeInstanceOf(
			classes.UserOpNotSentError
		);
		expect(relay.signed).toBe(1);
	});
});

describe('deploying the multisig: only a genesis the core finds Verified, on code read now (§3.5)', () => {
	const undeployed = () =>
		input({
			approvals: [ecdsa(CAROL)],
			members: { owners: [EXECUTOR, ALICE, CAROL], threshold: 2, deployed: false, nonce: 0n }
		});
	const deploying = (): Priced =>
		({
			kind: 'priced',
			calls: [
				{ to: '0xfactory', value: '0x0', data: '0xmultisig' },
				{ to: SAFE, value: '0x0', data: '0x6a761202' }
			],
			fee: { amount: 10n ** 15n, recipient: RECIPIENT, tier: 'standard' },
			balance: USDC,
			canPay: true,
			carries: [],
			executorApproves: true,
			setup: { multisig: true, members: [], executor: false, amount: null }
		}) as Priced;

	it('a refused genesis: the preview says so (not "no price"), and nothing is signed', async () => {
		authority.verdict = { kind: 'refused', reasons: [{ kind: 'module', address: ALICE }] };
		const look = await previewExecution(undeployed(), reads());
		expect(look.kind).toBe('refused');
		const refused = await submitExecution(undeployed(), look).catch((e: unknown) => e);
		expect(refused).toBeInstanceOf(GenesisRefusedError);
		expect(sent.count).toBe(0);
	});

	it('a preview shown earlier: the code is read again right before the passkey', async () => {
		await submitExecution(undeployed(), deploying());
		expect(authority.reads).toBe(1);
		expect(sent.count).toBe(1);
		authority.verdict = { kind: 'unknown', unread: [SAFE] };
		const unverified = await submitExecution(undeployed(), deploying()).catch((e: unknown) => e);
		expect(unverified).toBeInstanceOf(GenesisRefusedError);
		expect(sent.count, 'nothing signed for a chain that did not answer').toBe(1);
	});

	it('a deployed multisig reads no deployment code', async () => {
		await submitExecution(input(), {
			...deploying(),
			setup: { ...deploying().setup, multisig: false }
		});
		expect(authority.reads).toBe(0);
	});
});
