import { beforeEach, describe, expect, it, vi } from 'vitest';

const EXECUTOR = '0x00000000000000000000000000000000000000aa';
/** The fee the in-band path fixes before signing: its token (null: the coin) and amount. */
const fee = { token: null as string | null, amount: 0n };
const balance = { value: '0x0' };
const sent = { count: 0 };
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
	// As the in-band path does: the fee is fixed, then `beforeSign`, then the passkey.
	sendBatchCalls: async (
		_from: string,
		_calls: unknown,
		_chain: number,
		_signer: unknown,
		signFn: ((c: Uint8Array) => Promise<unknown>) & {
			beforeSign?: (f: { token: string | null; amount: bigint; value: bigint }) => Promise<void>;
		}
	) => {
		if (relay.down > 0 && relay.before) {
			relay.down -= 1;
			throw new classes.PoolFailedError('relay unreachable');
		}
		await signFn.beforeSign?.({ token: fee.token, amount: fee.amount, value: 0n });
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
vi.mock('./chain', () => ({ hasCode: async () => true }));
vi.mock('./kernel', () => ({
	executionCalls: () => [
		{ to: '0x00000000000000000000000000000000000000bb', value: '0', data: '0x6a761202' }
	],
	isEcdsaApproval: () => true,
	sameAddress: (a: string, b: string) => a.toLowerCase() === b.toLowerCase()
}));

vi.mock('$lib/services/tx-reconciler', () => ({ pollUserOpStatus: async () => null }));

const { submitExecution, ExecutorCannotPayError, relayRefused, NOT_TAKEN_AFTER_MS } =
	await import('./execute');

const input = () =>
	({
		chainId: 5042002,
		multisig: '0x00000000000000000000000000000000000000cc',
		genesis: {},
		members: { owners: [EXECUTOR], threshold: 1, deployed: true },
		tx: {},
		approvals: [],
		executor: { address: EXECUTOR }
	}) as unknown as Parameters<typeof submitExecution>[0];

beforeEach(() => {
	sent.count = 0;
	relay.down = 0;
	relay.before = true;
	relay.signed = 0;
});

describe('executing a multisig transaction', () => {
	it('says the executor cannot pay before any passkey is asked', async () => {
		fee.token = null;
		fee.amount = 207n * 10n ** 15n;
		balance.value = `0x${(79n * 10n ** 15n).toString(16)}`;
		const refused = await submitExecution(input()).catch((e: unknown) => e);
		expect(refused).toBeInstanceOf(ExecutorCannotPayError);
		expect(refused).toMatchObject({ need: 207n * 10n ** 15n, have: 79n * 10n ** 15n });
		expect(sent.count, 'nothing signed, nothing sent').toBe(0);
	});

	it('sends when the executor holds the fee', async () => {
		fee.token = null;
		fee.amount = 10n ** 15n;
		balance.value = `0x${(10n ** 17n).toString(16)}`;
		await submitExecution(input());
		expect(sent.count).toBe(1);
	});

	it('leaves a fee paid in a token, or a balance it cannot read, to the relay', async () => {
		fee.token = '0x00000000000000000000000000000000000000dd';
		fee.amount = 10n ** 18n;
		balance.value = '0x0';
		await submitExecution(input());
		fee.token = null;
		balance.value = 'unreadable';
		await submitExecution(input());
		expect(sent.count).toBe(2);
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

	it('stops when the receipt came first', async () => {
		const watch = new AbortController();
		const refused = relayRefused('0x01', 100, watch.signal, answers(null), 5);
		watch.abort();
		expect(await refused).toBe(false);
	});
});

describe('a relay out of reach for a moment (r4a-23)', () => {
	it('before anything is signed: asked once more on its own, one passkey in all', async () => {
		fee.token = null;
		fee.amount = 10n ** 15n;
		balance.value = `0x${(10n ** 17n).toString(16)}`;
		relay.down = 1;
		await submitExecution(input());
		expect(sent.count).toBe(1);
		expect(relay.signed).toBe(1);
	});

	it('twice in a row: said, not retried without end', async () => {
		relay.down = 2;
		await expect(submitExecution(input())).rejects.toBeInstanceOf(classes.PoolFailedError);
		expect(relay.signed).toBe(0);
	});

	it('after the passkey: never retried behind the person’s back (it would ask again)', async () => {
		fee.token = null;
		fee.amount = 10n ** 15n;
		balance.value = `0x${(10n ** 17n).toString(16)}`;
		relay.before = false;
		relay.down = 1;
		await expect(submitExecution(input())).rejects.toBeInstanceOf(classes.UserOpNotSentError);
		expect(relay.signed).toBe(1);
	});
});
