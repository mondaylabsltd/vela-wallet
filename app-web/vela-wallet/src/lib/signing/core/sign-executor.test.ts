/**
 * What the core hears about a dApp submit.
 *
 * Issue 262: a receipt that is late is not a confirmation. The bundler
 * accepted the op and the receipt wait ran out. The page still needs a hash,
 * so the core answers with the op hash — but only if the shell TELLS it the
 * receipt is pending (`receipt_pending`). Reported as `succeeded`, the core
 * flips the record to "confirmed" with the op hash as its tx hash while
 * nothing may have landed.
 *
 * Spec 082 (RB5, RA1, RA8): a page that is gone gets nothing signed and
 * nothing sent (`asker_gone`); a submit that was provably not sent says the
 * relay's words or the core's fixed sentence; a landed revert is not
 * "dropped, try again".
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';

const submit = vi.hoisted(() => ({
	impl: null as null | ((...args: unknown[]) => Promise<unknown>),
	args: [] as unknown[]
}));

vi.mock('$lib/services/dapp-submit', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/services/dapp-submit')>();
	return {
		...actual,
		handleDAppRequest: (...args: unknown[]) => {
			submit.args = args;
			return submit.impl!(...args);
		}
	};
});

import {
	DAppReceiptPendingError,
	guardedSign,
	receiptStillOutstanding,
	type DAppSubmitHooks
} from '$lib/services/dapp-submit';
import {
	UserOpFeeHoldError,
	UserOpNotSentError,
	UserOpRejectedError,
	UserOpRevertedError
} from '$lib/services/safe-transaction';
import { createSignExecutor } from './sign-executor';
import {
	AskerGoneError,
	claimThrough,
	type SignEffect,
	type SignResponder,
	type SignShellPorts
} from './sign-types';

function makePorts(over: Partial<SignShellPorts> = {}): SignShellPorts {
	return {
		transportFor: () => null,
		opSubmitted: () => {},
		ceremony: () => {},
		askerLive: async () => true,
		approvedAtMs: () => null,
		requestOrigin: () => 'https://app.example',
		assetSim: () => null,
		switchActiveAccount: async () => {},
		recordsWritten: () => {},
		...over
	};
}

const signAndSubmit: SignEffect = {
	id: 1,
	operation: {
		type: 'sign_and_submit',
		id: 'req-1',
		method: 'eth_sendTransaction',
		params_json: '[{"to":"0x0000000000000000000000000000000000000001","value":"0x1"}]',
		chain_id: 100,
		address: '0x88cCA0EeDbF2C4426110bbFc998F048689266894',
		credential_id: 'cred-1',
		max_fee_per_gas: null,
		gas_fee_token: null,
		quoted_fee: null
	}
};

describe('the receipt wait, as the core hears it', () => {
	it('a late receipt is receipt_pending with the op hash, never succeeded', async () => {
		submit.impl = () => Promise.reject(new DAppReceiptPendingError('0xophash'));
		const result = await createSignExecutor(makePorts()).execute(signAndSubmit);
		expect(result).toMatchObject({
			type: 'submit',
			outcome: { type: 'receipt_pending', user_op_hash: '0xophash' }
		});
	});

	it('a receipt in time is succeeded with the tx hash', async () => {
		submit.impl = () => Promise.resolve('0xtxhash');
		const result = await createSignExecutor(makePorts()).execute(signAndSubmit);
		expect(result).toMatchObject({
			type: 'submit',
			outcome: { type: 'succeeded', result: '0xtxhash' }
		});
	});

	it('only a relay rejection is a "not sent" verdict; a revert is answered, the rest stays in flight', () => {
		expect(receiptStillOutstanding(new UserOpRejectedError('refused'))).toBe(false);
		// A landed revert is answered its tx hash (ruling 9), not retried.
		expect(receiptStillOutstanding(new UserOpRevertedError('0x' + 'ab'.repeat(32)))).toBe(false);
		expect(receiptStillOutstanding(new UserOpFeeHoldError('queued'))).toBe(true);
		expect(
			receiptStillOutstanding(
				new Error('Transaction submitted (0xab…) but not confirmed within 120s.')
			)
		).toBe(true);
		expect(
			receiptStillOutstanding(new Error("Couldn't reach the bundler to confirm transaction 0xab…"))
		).toBe(true);
	});
});

describe('the op hash an answer carries (RF3)', () => {
	it('a late receipt answers the op hash WITH its chain; a tx hash carries none', async () => {
		const sent: { result: unknown; opHash: unknown }[] = [];
		const transport: SignResponder = {
			sendResponse: (_id, result, _error, opHash) => sent.push({ result, opHash })
		};
		const executor = createSignExecutor(makePorts({ transportFor: () => transport }));
		submit.impl = () => Promise.reject(new DAppReceiptPendingError('0xOPHASH'));
		await executor.execute(signAndSubmit);
		await executor.execute({
			id: 2,
			operation: {
				type: 'send_response',
				transport_id: 't1',
				id: 'req-1',
				payload: { type: 'ok', result: '0xOPHASH' }
			}
		});
		await executor.execute({
			id: 3,
			operation: {
				type: 'send_response',
				transport_id: 't1',
				id: 'req-2',
				payload: { type: 'ok', result: '0xtxhash' }
			}
		});
		expect(sent).toEqual([
			{ result: '0xOPHASH', opHash: { chainId: 100 } },
			{ result: '0xtxhash', opHash: undefined }
		]);
	});
});

describe('a page that is gone (spec 082 RB5)', () => {
	it('not live → asker_gone: nothing answered, nothing recorded', async () => {
		submit.impl = () => Promise.reject(new AskerGoneError('sign'));
		const result = await createSignExecutor(makePorts()).execute(signAndSubmit);
		expect(result).toMatchObject({ type: 'submit', outcome: { type: 'asker_gone' } });
	});

	it('the submit asks the transport through the port, by request id and phase', async () => {
		const askerLive = vi.fn(async () => false);
		const ceremony = vi.fn();
		submit.impl = () => Promise.resolve('0xtx');
		await createSignExecutor(makePorts({ askerLive, ceremony })).execute(signAndSubmit);
		const hooks = submit.args[9] as DAppSubmitHooks;
		await expect(hooks.claim('sign')).resolves.toBe(false);
		expect(askerLive).toHaveBeenCalledWith('req-1', 'sign');
		hooks.ceremony('started');
		expect(ceremony).toHaveBeenCalledWith('req-1', 'started');
	});

	it('the receipt wait is what is LEFT of the window since the approval (RA12)', async () => {
		submit.impl = () => Promise.resolve('0xtx');
		const approvedAt = Date.now() - 50_000;
		await createSignExecutor(makePorts({ approvedAtMs: () => approvedAt })).execute(signAndSubmit);
		const hooks = submit.args[9] as DAppSubmitHooks;
		const wait = hooks.receiptWaitMs();
		expect(wait).toBeGreaterThan(60_000);
		expect(wait).toBeLessThanOrEqual(70_000);
	});

	it('not live at sign → the signer is never called', async () => {
		const signer = vi.fn(async () => 'sig');
		const order: string[] = [];
		const hooks: DAppSubmitHooks = {
			claim: async (phase) => {
				order.push(`claim:${phase}`);
				return false;
			},
			ceremony: (stage) => order.push(`ceremony:${stage}`),
			receiptWaitMs: () => 0
		};
		await expect(guardedSign(signer, hooks, true)()).rejects.toBeInstanceOf(AskerGoneError);
		expect(signer).not.toHaveBeenCalled();
		expect(order).toEqual(['claim:sign']);
	});

	it('not live at submit → the signature exists, and nothing after it runs (no relay POST)', async () => {
		const order: string[] = [];
		const post = vi.fn();
		const hooks: DAppSubmitHooks = {
			claim: async (phase) => {
				order.push(`claim:${phase}`);
				return phase !== 'submit';
			},
			ceremony: (stage) => order.push(`ceremony:${stage}`),
			receiptWaitMs: () => 0
		};
		const signFn = guardedSign(
			async () => {
				order.push('passkey');
				return 'sig';
			},
			hooks,
			true
		);
		// The submit builder: sign, then POST — the POST is never reached.
		const submitting = (async () => {
			await signFn();
			post();
		})();
		await expect(submitting).rejects.toMatchObject({ name: 'AskerGoneError', phase: 'submit' });
		expect(post).not.toHaveBeenCalled();
		// The ceremony order the sheet's words follow (RA9).
		expect(order).toEqual([
			'claim:sign',
			'ceremony:started',
			'passkey',
			'ceremony:done',
			'claim:submit'
		]);
	});

	it('a message is claimed before its passkey and never "submitted"', async () => {
		const phases: string[] = [];
		const hooks: DAppSubmitHooks = {
			claim: async (phase) => {
				phases.push(phase);
				return true;
			},
			ceremony: () => {},
			receiptWaitMs: () => 0
		};
		await expect(guardedSign(async () => 'sig', hooks, false)()).resolves.toBe('sig');
		expect(phases).toEqual(['sign']);
	});

	it('no claim port (the web wallet) → unchanged: always live, no hooks → the signer as is', async () => {
		await expect(claimThrough(undefined, 'r', 'sign')).resolves.toBe(true);
		await expect(claimThrough({ sendResponse: () => {} }, 'r', 'submit')).resolves.toBe(true);
		const signer = async () => 'sig';
		expect(guardedSign(signer, undefined, true)).toBe(signer);
	});

	it('a claim that throws is not a yes', async () => {
		const transport: SignResponder = {
			sendResponse: () => {},
			claim: async () => {
				throw new Error('channel closed');
			}
		};
		await expect(claimThrough(transport, 'r', 'sign')).resolves.toBe(false);
	});
});

describe('a submit that was not sent (spec 082 RA1)', () => {
	it('answers the core’s fixed sentence, never the pool’s text', async () => {
		submit.impl = () =>
			Promise.reject(new UserOpNotSentError(null, 'relay unreachable; nothing was sent'));
		const result = await createSignExecutor(makePorts()).execute(signAndSubmit);
		expect(result).toMatchObject({
			type: 'submit',
			outcome: { type: 'failed', message: 'relay unreachable; nothing was sent' }
		});
	});

	it('a relay refusal keeps the relay’s own sentence', async () => {
		submit.impl = () =>
			Promise.reject(new UserOpNotSentError({ other: 'AA25 invalid nonce' }, 'AA25 invalid nonce'));
		const result = await createSignExecutor(makePorts()).execute(signAndSubmit);
		expect(result).toMatchObject({ outcome: { type: 'failed', message: 'AA25 invalid nonce' } });
	});
});

describe('the Activity row follows every record write (RG3)', () => {
	it('persist and patch both poke the feed', async () => {
		const recordsWritten = vi.fn();
		const executor = createSignExecutor(makePorts({ recordsWritten }));
		await executor.execute({
			id: 1,
			operation: { type: 'update_record', record_id: 'dapp-1-tx', close: { type: 'failed' } }
		});
		expect(recordsWritten).toHaveBeenCalledTimes(1);
	});
});
