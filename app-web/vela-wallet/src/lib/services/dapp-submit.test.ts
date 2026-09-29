/**
 * The dApp path signs for the wallet a request is FOR (spec 062, found on
 * Base): one passkey founds any number of wallets, so a credential id does
 * not name a wallet — an address does.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';

const ONE = { id: 'cred-a', address: '0x' + 'd4'.repeat(20), publicKeyHex: '04' + '11'.repeat(64) };
const MULTI = {
	id: 'cred-a', // the SAME founding credential as ONE
	address: '0x' + '88'.repeat(20),
	publicKeyHex: ONE.publicKeyHex,
	keys: [
		{ credentialId: 'cred-a', publicKeyHex: ONE.publicKeyHex },
		{ credentialId: 'cred-b', publicKeyHex: '04' + '22'.repeat(64) }
	]
};
vi.mock('./accounts', () => ({
	findAccountByAddress: (address: string) =>
		[ONE, MULTI].find((a) => a.address === address.toLowerCase()),
	findAccountByCredentialId: (id: string) => [ONE, MULTI].find((a) => a.id === id)
}));
vi.mock('$lib/onboarding/core/passkey', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/onboarding/core/passkey')>()),
	signWithAny: vi.fn()
}));
vi.mock('./rpc-adapter', () => ({ rpcCall: vi.fn() }));
vi.mock('./safe-transaction', async (importOriginal) => ({
	...(await importOriginal<typeof import('./safe-transaction')>()),
	keySetOf: vi.fn(() => ({ keys: [] })),
	sendBatchCalls: vi.fn(),
	sendContractCall: vi.fn(),
	sendNative: vi.fn()
}));
vi.mock('./networks', async (importOriginal) => ({
	...(await importOriginal<typeof import('./networks')>()),
	getAllNetworksSync: () => [{ chainId: 100 }]
}));
const passkey = vi.hoisted(() => ({ calls: 0 }));
vi.mock('$lib/signing/sign-challenge', () => ({
	signChallenge: vi.fn(async () => {
		passkey.calls += 1;
		return {
			signatureHex: '30',
			authenticatorDataHex: '00',
			clientDataJSONHex: '00',
			credentialId: 'cred-a'
		};
	})
}));
vi.mock('$lib/core/kernels', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/kernels')>()),
	verifySafeWebAuthn: () => ({ ok: true })
}));

import {
	DAppReceiptPendingError,
	handleDAppRequest,
	storedWalletFor,
	type DAppSubmitHooks
} from './dapp-submit';
import {
	sendNative,
	UserOpRevertedError,
	type SignFn,
	type SubmitResult
} from './safe-transaction';
import { AskerGoneError, WriteAheadTimeoutError } from '$lib/signing/core/sign-types';

describe('storedWalletFor', () => {
	it('two wallets founded by one passkey: the request names which by ADDRESS', () => {
		// By credential alone both requests would have resolved to ONE — and a
		// request for MULTI would have deployed ONE's Safe (AA14).
		expect(storedWalletFor({ id: 'cred-a' }, MULTI.address)?.keys).toHaveLength(2);
		expect(storedWalletFor({ id: 'cred-a' }, ONE.address)?.keys).toBeUndefined();
		expect(
			storedWalletFor({ id: 'cred-a' }, MULTI.address.toUpperCase().replace('0X', '0x'))?.address
		).toBe(MULTI.address);
	});

	it('an address no record carries falls back to the credential, as before', () => {
		expect(storedWalletFor({ id: 'cred-a' }, '0x' + '00'.repeat(20))?.address).toBe(ONE.address);
		expect(storedWalletFor({ id: 'nobody' }, '0x' + '00'.repeat(20))).toBeUndefined();
	});
});

/**
 * Spec 082 T083: exactly one answer after a lost reply or a revert, and the
 * ceremony order the sheet's words follow.
 */
describe('one answer for an on-chain request (spec 082 RA2, RA8, RA9)', () => {
	const LOCAL = '0x' + 'c1'.repeat(32);
	const TX = '0x' + 'f6'.repeat(32);
	const request = {
		id: 'req-1',
		method: 'eth_sendTransaction',
		params: [{ to: '0x' + '76'.repeat(20), value: '0x38d7ea4c68000' }],
		origin: 'http://127.0.0.1:8137'
	};

	function hooks(
		order: string[],
		live: (phase: string) => boolean = () => true,
		over: Partial<DAppSubmitHooks> = {}
	): DAppSubmitHooks {
		return {
			claim: async (phase, submit) => {
				order.push(submit ? `claim:${phase}:${submit.opHash}:${submit.chainId}` : `claim:${phase}`);
				return live(phase);
			},
			ceremony: (stage) => order.push(`ceremony:${stage}`),
			receiptWaitMs: () => 108_000,
			...over
		};
	}

	/**
	 * The submit builder, as `submitUserOp` runs it: sign, hash, the gate before
	 * the first POST (the write-ahead, then the last claim), then POST, then
	 * hand back the op.
	 */
	function submitsWith(result: Partial<SubmitResult>, post: () => void = () => {}) {
		vi.mocked(sendNative).mockImplementation(async (...args: unknown[]) => {
			const signFn = args[5] as SignFn;
			await signFn(new Uint8Array(32));
			await signFn.beforePost?.({ userOpHash: LOCAL, submitBlock: 7, chainId: 100 });
			post();
			return {
				userOpHash: LOCAL,
				maybeSent: false,
				submitBlock: null,
				waitForTxHash: async () => TX,
				...result
			} as SubmitResult;
		});
	}

	function run(onSubmitted: (...args: unknown[]) => void, h?: DAppSubmitHooks) {
		return handleDAppRequest(
			request,
			{ id: ONE.id },
			ONE.address,
			100,
			undefined,
			onSubmitted,
			null,
			undefined,
			'core',
			h
		);
	}

	it('mute: exactly one Ok(op hash), inside the window — never 4900 or -32603', async () => {
		const waitFor = vi.fn(async () => {
			throw new Error('Transaction submitted (0xc1…) but not confirmed within 108s.');
		});
		submitsWith({ maybeSent: true, submitBlock: 48_478_700, waitForTxHash: waitFor });
		const onSubmitted = vi.fn();
		const error = await run(onSubmitted, hooks([])).catch((e: unknown) => e);
		// The executor answers `receipt_pending` → the page gets the op hash, once.
		expect(error).toBeInstanceOf(DAppReceiptPendingError);
		expect((error as DAppReceiptPendingError).userOpHash).toBe(LOCAL);
		expect(onSubmitted).toHaveBeenCalledTimes(1);
		expect(onSubmitted).toHaveBeenCalledWith(LOCAL, true, 48_478_700);
		// The wait is what is left of the window, never a fresh 120 s.
		expect(waitFor).toHaveBeenCalledWith(108_000, undefined);
	});

	it('revert inside the wait: Ok(tx hash), not -32603', async () => {
		submitsWith({
			waitForTxHash: async () => {
				throw new UserOpRevertedError(TX);
			}
		});
		await expect(run(vi.fn(), hooks([]))).resolves.toBe(TX);
	});

	it('the ceremony order: claim, prompt up, signature, claim again, then the POST', async () => {
		const order: string[] = [];
		submitsWith({}, () => order.push('post'));
		await expect(run(vi.fn(), hooks(order))).resolves.toBe(TX);
		expect(order).toEqual([
			'claim:sign',
			'ceremony:started',
			'ceremony:done',
			// RJ2: the last claim carries the op hash and chain.
			`claim:submit:${LOCAL}:100`,
			'post'
		]);
	});

	it('write-ahead: nothing is POSTed before the record is on disk (RJ1)', async () => {
		const order: string[] = [];
		let clear!: () => void;
		const cleared = new Promise<void>((resolve) => (clear = resolve));
		submitsWith({}, () => order.push('post'));
		const writeAhead = vi.fn(async (hash: string, block: number | null) => {
			order.push(`write-ahead:${hash}:${block}`);
			await cleared;
		});
		const running = run(
			vi.fn(),
			hooks(order, () => true, { writeAhead })
		);
		await vi.waitFor(() => expect(writeAhead).toHaveBeenCalledTimes(1));
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(order).not.toContain('post');
		clear();
		await expect(running).resolves.toBe(TX);
		expect(order).toEqual([
			'claim:sign',
			'ceremony:started',
			'ceremony:done',
			`write-ahead:${LOCAL}:7`,
			`claim:submit:${LOCAL}:100`,
			'post'
		]);
	});

	it('write-ahead: no clearance → zero POSTs, nothing reported sent', async () => {
		const order: string[] = [];
		submitsWith({}, () => order.push('post'));
		const onSubmitted = vi.fn();
		const error = await run(
			onSubmitted,
			hooks(order, () => true, {
				writeAhead: async () => {
					throw new WriteAheadTimeoutError(LOCAL);
				}
			})
		).catch((e: unknown) => e);
		expect(error).toBeInstanceOf(WriteAheadTimeoutError);
		expect(order).not.toContain('post');
		expect(order.some((line) => line.startsWith('claim:submit'))).toBe(false);
		expect(onSubmitted).not.toHaveBeenCalled();
	});

	it('the receipt wait stops when the core answered through the tracker (RJ4)', async () => {
		const answered = new AbortController();
		const waitFor = vi.fn(async (_ms?: number, signal?: AbortSignal) => {
			expect(signal).toBe(answered.signal);
			throw new Error('aborted');
		});
		submitsWith({ waitForTxHash: waitFor });
		const error = await run(
			vi.fn(),
			hooks([], () => true, { answered: answered.signal })
		).catch((e: unknown) => e);
		expect(error).toBeInstanceOf(DAppReceiptPendingError);
		expect(waitFor).toHaveBeenCalledWith(108_000, answered.signal);
	});

	it('not live at sign: no passkey, no POST, asker gone', async () => {
		const order: string[] = [];
		const before = passkey.calls;
		submitsWith({}, () => order.push('post'));
		const error = await run(
			vi.fn(),
			hooks(order, (phase) => phase !== 'sign')
		).catch((e: unknown) => e);
		expect(error).toBeInstanceOf(AskerGoneError);
		expect(passkey.calls).toBe(before);
		expect(order).toEqual(['claim:sign']);
	});

	it('not live at submit: the signature exists, the relay POST never happens', async () => {
		const order: string[] = [];
		submitsWith({}, () => order.push('post'));
		const onSubmitted = vi.fn();
		const error = await run(
			onSubmitted,
			hooks(order, (phase) => phase !== 'submit')
		).catch((e: unknown) => e);
		expect(error).toMatchObject({ name: 'AskerGoneError', phase: 'submit' });
		expect(order).not.toContain('post');
		expect(onSubmitted).not.toHaveBeenCalled();
	});
});
