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

const store = vi.hoisted(() => ({
	saveTransaction: vi.fn<(tx: unknown) => Promise<void>>(async () => {}),
	deleteTransaction: vi.fn(async () => {}),
	updateTransaction: vi.fn(async () => {})
}));
vi.mock('$lib/services/records', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/records')>()),
	saveTransaction: store.saveTransaction,
	deleteTransaction: store.deleteTransaction,
	updateTransaction: store.updateTransaction
}));

import { userOpNotSentDetail, userOpWriteAheadWaitMs } from '$lib/core/kernels';
import {
	DAppReceiptPendingError,
	DAppRevertedError,
	guardedSign,
	type DAppSubmitHooks
} from '$lib/services/dapp-submit';
import { UserOpNotSentError } from '$lib/services/safe-transaction';
import { createSignExecutor } from './sign-executor';
import {
	AskerGoneError,
	WriteAheadTimeoutError,
	claimThrough,
	type SignEffect,
	type SignResponder,
	type SignShellPorts
} from './sign-types';

const OP = '0x' + '5a'.repeat(32);

function makePorts(over: Partial<SignShellPorts> = {}): SignShellPorts {
	return {
		transportFor: () => null,
		opSubmitted: () => {},
		opSigned: () => {},
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

describe('a batch id and a late receipt (083)', () => {
	// 083 H2 review: `wallet_sendCalls` resolves with its EIP-5792 batch id —
	// the op hash — at acceptance, with no receipt wait. As `succeeded` the
	// core closed the record "confirmed" under that hash (an explorer link to
	// nothing, never tracked); as `receipt_pending` the page still gets the id
	// and the tracker settles the record.
	it('a batch id is receipt_pending with that id, never succeeded', async () => {
		submit.impl = () => Promise.resolve('0xbatchid');
		const batch: SignEffect = {
			...signAndSubmit,
			operation: {
				...(signAndSubmit.operation as Extract<
					SignEffect['operation'],
					{ type: 'sign_and_submit' }
				>),
				method: 'wallet_sendCalls',
				params_json: '[{"calls":[{"to":"0x0000000000000000000000000000000000000001"}]}]'
			}
		};
		const result = await createSignExecutor(makePorts()).execute(batch);
		expect(result).toMatchObject({
			type: 'submit',
			outcome: { type: 'receipt_pending', user_op_hash: '0xbatchid' }
		});
	});

	// Spec 097 E: the core answers a batch with its id at `op_submitted` —
	// INSIDE the submit, before it resolves. The answer still carries the
	// batch's chain, so the extension knows the id for `wallet_getCallsStatus`
	// (it answered "Unknown bundle id" for an id it never recorded), and the
	// chain is used up by that one answer.
	it('a batch answered at op_submitted carries its chain, once', async () => {
		const sent: { result: unknown; opHash: unknown }[] = [];
		const transport: SignResponder = {
			sendResponse: (_id, result, _error, opHash) => sent.push({ result, opHash })
		};
		const executor = createSignExecutor(
			makePorts({
				transportFor: () => transport,
				// The core's answer, as `op_submitted` produces it.
				opSubmitted: (id, hash) =>
					void executor.execute({
						id: 9,
						operation: {
							type: 'send_response',
							transport_id: 't1',
							id,
							payload: { type: 'ok', result: hash }
						}
					})
			})
		);
		submit.impl = (...args: unknown[]) => {
			const onSubmitted = args[5] as (hash: string, maybeSent: boolean, block: number) => void;
			onSubmitted('0xBATCH', false, 7);
			return Promise.resolve('0xBATCH');
		};
		const batch: SignEffect = {
			...signAndSubmit,
			operation: {
				...(signAndSubmit.operation as Extract<
					SignEffect['operation'],
					{ type: 'sign_and_submit' }
				>),
				method: 'wallet_sendCalls',
				params_json: '[{"calls":[{"to":"0x0000000000000000000000000000000000000001"}]}]'
			}
		};
		await executor.execute(batch);
		await executor.execute({
			id: 10,
			operation: {
				type: 'send_response',
				transport_id: 't1',
				id: 'req-2',
				payload: { type: 'ok', result: '0xBATCH' }
			}
		});
		expect(sent).toEqual([
			{ result: '0xBATCH', opHash: { chainId: 100 } },
			{ result: '0xBATCH', opHash: undefined }
		]);
	});

	// A batch that may only have been sent is not answered at `op_submitted`:
	// its id goes when the submit returns, still carrying its chain, once.
	it('a maybe-sent batch carries its chain when the submit returns, once', async () => {
		const sent: { result: unknown; opHash: unknown }[] = [];
		const transport: SignResponder = {
			sendResponse: (_id, result, _error, opHash) => sent.push({ result, opHash })
		};
		const executor = createSignExecutor(makePorts({ transportFor: () => transport }));
		submit.impl = (...args: unknown[]) => {
			const onSubmitted = args[5] as (hash: string, maybeSent: boolean, block: number) => void;
			onSubmitted('0xLOCAL', true, 7);
			return Promise.resolve('0xLOCAL');
		};
		const batch: SignEffect = {
			...signAndSubmit,
			operation: {
				...(signAndSubmit.operation as Extract<
					SignEffect['operation'],
					{ type: 'sign_and_submit' }
				>),
				method: 'wallet_sendCalls',
				params_json: '[{"calls":[{"to":"0x0000000000000000000000000000000000000001"}]}]'
			}
		};
		const result = await executor.execute(batch);
		expect(result).toMatchObject({
			type: 'submit',
			outcome: { type: 'receipt_pending', user_op_hash: '0xLOCAL' }
		});
		for (const id of [11, 12]) {
			await executor.execute({
				id,
				operation: {
					type: 'send_response',
					transport_id: 't1',
					id: 'req-1',
					payload: { type: 'ok', result: '0xLOCAL' }
				}
			});
		}
		expect(sent).toEqual([
			{ result: '0xLOCAL', opHash: { chainId: 100 } },
			{ result: '0xLOCAL', opHash: undefined }
		]);
	});

	// 083, owner ruling 2026-10-01: an included op that REVERTED is reported
	// as such — the core answers the page the revert, never the tx hash a
	// site would read as done.
	it('a reverted receipt is reported reverted, with both hashes', async () => {
		submit.impl = () => Promise.reject(new DAppRevertedError('0xop', '0xtx'));
		const result = await createSignExecutor(makePorts()).execute(signAndSubmit);
		expect(result).toMatchObject({
			type: 'submit',
			outcome: { type: 'reverted', user_op_hash: '0xop', tx_hash: '0xtx' }
		});
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
		expect(askerLive).toHaveBeenCalledWith('req-1', 'sign', undefined);
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
			receiptWaitMs: () => 0,
			writeAhead: async () => {
				order.push('write-ahead');
			}
		};
		const signFn = guardedSign(
			async () => {
				order.push('passkey');
				return 'sig';
			},
			hooks,
			true
		);
		// The submit builder: sign, hash, the gate before the POST — the POST is
		// never reached.
		const submitting = (async () => {
			await signFn();
			await signFn.beforePost?.({ userOpHash: OP, submitBlock: 7, chainId: 100 });
			post();
		})();
		await expect(submitting).rejects.toMatchObject({ name: 'AskerGoneError', phase: 'submit' });
		expect(post).not.toHaveBeenCalled();
		// The ceremony order the sheet's words follow (RA9); the record is written
		// before the last claim (RJ1).
		expect(order).toEqual([
			'claim:sign',
			'ceremony:started',
			'passkey',
			'ceremony:done',
			'write-ahead',
			'claim:submit'
		]);
	});

	it('the submit claim carries the op hash and chain (RJ2), after the write-ahead', async () => {
		const claims: unknown[] = [];
		const hooks: DAppSubmitHooks = {
			claim: async (phase, submit) => {
				claims.push({ phase, submit });
				return true;
			},
			ceremony: () => {},
			receiptWaitMs: () => 0
		};
		const signFn = guardedSign(async () => 'sig', hooks, true);
		await signFn();
		await signFn.beforePost?.({ userOpHash: OP, submitBlock: null, chainId: 100 });
		expect(claims).toEqual([
			{ phase: 'sign', submit: undefined },
			{ phase: 'submit', submit: { opHash: OP, chainId: 100 } }
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

/**
 * Spec 082 RJ1 (T228): a record exists before the bytes leave. The executor
 * announces the signed op (`OpSigned`) and lets the POST go only on the core's
 * `ClearToPost` for that op — none in time, and nothing is sent.
 */
describe('the write-ahead (spec 082 RJ1)', () => {
	const OP_B = '0x' + '6b'.repeat(32);

	/** The core's `PersistRecord` for the write-ahead of `hash` (pending, may have been sent). */
	function persistAhead(hash: string, recordId = 'dapp-1-tx'): SignEffect {
		return {
			id: 90,
			operation: {
				type: 'persist_record',
				record: {
					record_id: recordId,
					kind: 'dapp_tx',
					method: 'eth_sendTransaction',
					params_json: '[{"to":"0x0000000000000000000000000000000000000001","value":"0x1"}]',
					result: '',
					from: '0x88cCA0EeDbF2C4426110bbFc998F048689266894',
					chain_id: 100,
					now_ms: 1,
					status: 'pending',
					user_op_hash: hash,
					dapp_origin: 'https://app.example',
					dapp_url: 'https://app.example',
					intent: null,
					maybe_sent: true,
					submit_block: 7,
					stored_request: '[{"to":"0x0000000000000000000000000000000000000001","value":"0x1"}]',
					request_truncated: false
				}
			}
		};
	}

	/**
	 * The core clears the POST on the store's acknowledgement, and the web
	 * acknowledges a write the store refused (logged, so the machine moves
	 * on). A refused write-ahead must still let nothing out: the POST would
	 * leave with no record behind it (the desktop's twin: DESK_B review (2)).
	 */
	it('a write-ahead record the store refused lets nothing out — the submit is "not sent"', async () => {
		const post = vi.fn();
		submit.impl = async (...args: unknown[]) => {
			await (args[9] as DAppSubmitHooks).writeAhead!(OP, 7);
			post();
			return '0xtx';
		};
		store.saveTransaction.mockRejectedValueOnce(new Error('QuotaExceededError'));
		const executor = createSignExecutor(makePorts());
		const running = executor.execute(signAndSubmit);
		expect(await executor.execute(persistAhead(OP))).toEqual({ type: 'record_persisted' });
		await executor.execute({
			id: 2,
			operation: { type: 'clear_to_post', id: 'req-1', user_op_hash: OP }
		});
		await expect(running).resolves.toMatchObject({
			type: 'submit',
			outcome: { type: 'failed', message: userOpNotSentDetail(), refused: false }
		});
		expect(post).not.toHaveBeenCalled();
	});

	it('the same op signed again is cleared by its own write, never by the last attempt’s', async () => {
		const post = vi.fn();
		submit.impl = async (...args: unknown[]) => {
			await (args[9] as DAppSubmitHooks).writeAhead!(OP, 7);
			post();
			return '0xtx';
		};
		const executor = createSignExecutor(makePorts());
		// Attempt 1: written and cleared (its clearance uses the mark up).
		const first = executor.execute(signAndSubmit);
		await executor.execute(persistAhead(OP, 'dapp-1-tx'));
		await executor.execute({
			id: 2,
			operation: { type: 'clear_to_post', id: 'req-1', user_op_hash: OP }
		});
		await first;
		expect(post).toHaveBeenCalledTimes(1);
		// Attempt 2, the same hash: the store refuses this one's record.
		store.saveTransaction.mockRejectedValueOnce(new Error('QuotaExceededError'));
		const second = executor.execute(signAndSubmit);
		await executor.execute(persistAhead(OP, 'dapp-2-tx'));
		await executor.execute({
			id: 3,
			operation: { type: 'clear_to_post', id: 'req-1', user_op_hash: OP }
		});
		await expect(second).resolves.toMatchObject({ outcome: { type: 'failed' } });
		expect(post).toHaveBeenCalledTimes(1);
	});

	it('announces the op, and the POST waits for its clearance', async () => {
		const opSigned = vi.fn();
		let hooks!: DAppSubmitHooks;
		let cleared = false;
		submit.impl = async (...args: unknown[]) => {
			hooks = args[9] as DAppSubmitHooks;
			await hooks.writeAhead!(OP, 7);
			cleared = true;
			return '0xtx';
		};
		const executor = createSignExecutor(makePorts({ opSigned }));
		const running = executor.execute(signAndSubmit);
		await vi.waitFor(() => expect(opSigned).toHaveBeenCalledWith('req-1', OP, 7));
		await executor.execute(persistAhead(OP));
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(cleared).toBe(false);
		// A clearance for another op clears nothing.
		await executor.execute({
			id: 2,
			operation: { type: 'clear_to_post', id: 'req-1', user_op_hash: OP_B }
		});
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(cleared).toBe(false);
		expect(
			await executor.execute({
				id: 3,
				operation: {
					type: 'clear_to_post',
					id: 'req-1',
					user_op_hash: OP.toUpperCase().replace('0X', '0x')
				}
			})
		).toEqual({ type: 'responded' });
		await expect(running).resolves.toMatchObject({ outcome: { type: 'succeeded' } });
		expect(cleared).toBe(true);
	});

	it('no clearance in time → no POST, and the core hears "not sent", never a refusal', async () => {
		vi.useFakeTimers();
		try {
			const post = vi.fn();
			submit.impl = async (...args: unknown[]) => {
				await (args[9] as DAppSubmitHooks).writeAhead!(OP, null);
				post();
				return '0xtx';
			};
			const running = createSignExecutor(makePorts()).execute(signAndSubmit);
			await vi.advanceTimersByTimeAsync(userOpWriteAheadWaitMs());
			await expect(running).resolves.toMatchObject({
				type: 'submit',
				outcome: { type: 'failed', message: userOpNotSentDetail(), refused: false }
			});
			expect(post).not.toHaveBeenCalled();
		} finally {
			vi.useRealTimers();
		}
		expect(new WriteAheadTimeoutError(OP).name).toBe('WriteAheadTimeoutError');
	});

	it('a withdrawn record is deleted; an admitted one drops "may have been sent" and stays pending', async () => {
		const recordsWritten = vi.fn();
		const executor = createSignExecutor(makePorts({ recordsWritten }));
		expect(
			await executor.execute({
				id: 1,
				operation: { type: 'delete_record', record_id: 'dapp-1-tx' }
			})
		).toEqual({ type: 'record_updated' });
		expect(store.deleteTransaction).toHaveBeenCalledWith('dapp-1-tx');
		await executor.execute({
			id: 2,
			operation: { type: 'update_record', record_id: 'dapp-2-tx', close: { type: 'admitted' } }
		});
		expect(store.updateTransaction).toHaveBeenCalledWith('dapp-2-tx', { maybeSent: false });
		expect(recordsWritten).toHaveBeenCalledTimes(2);
	});

	it('the receipt wait stops once the core answered the request itself (RJ4)', async () => {
		let hooks!: DAppSubmitHooks;
		submit.impl = (...args: unknown[]) => {
			hooks = args[9] as DAppSubmitHooks;
			return new Promise(() => {});
		};
		const executor = createSignExecutor(makePorts());
		void executor.execute(signAndSubmit);
		await vi.waitFor(() => expect(hooks).toBeDefined());
		expect(hooks.answered?.aborted).toBe(false);
		await executor.execute({
			id: 2,
			operation: {
				type: 'send_response',
				transport_id: 't1',
				id: 'req-1',
				payload: { type: 'ok', result: '0xtx' }
			}
		});
		expect(hooks.answered?.aborted).toBe(true);
	});
});

describe('a relay refusal is said as one (spec 082 RJ3)', () => {
	it('any rejection but "relayer unavailable" → refused', async () => {
		for (const [rejection, refused] of [
			[{ other: 'AA25 invalid nonce' }, true],
			['bundler_underfunded', true],
			['relayer_unavailable', false],
			[null, false]
		] as const) {
			submit.impl = () => Promise.reject(new UserOpNotSentError(rejection, 'words'));
			const result = await createSignExecutor(makePorts()).execute(signAndSubmit);
			expect(result, JSON.stringify(rejection)).toMatchObject({
				outcome: { type: 'failed', refused }
			});
		}
		// Four submits on real timers take ~2.3 s alone; a loaded runner
		// crossed the 5 s default (082 close-out).
	}, 20_000);
});

/**
 * Spec 093: the record the store keeps is the core's — its cut of the request,
 * whether it was cut, its summary and the sheet's balance changes, stored as
 * they came; a signature's record keeps no result. This side clips nothing.
 */
describe('the record persisted (spec 093)', () => {
	const typed = JSON.stringify({ primaryType: 'PermitSingle', message: { x: 'y'.repeat(20_000) } });

	it("stores the core's stored_request, request_truncated, summary and balance changes verbatim", async () => {
		store.saveTransaction.mockClear();
		const executor = createSignExecutor(makePorts());
		const summary = {
			action: 'permit' as const,
			calls: 0,
			spender: '0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad',
			symbol: 'USDC',
			decimals: 6,
			unlimited: true,
			primary_type: 'PermitSingle'
		};
		await executor.execute({
			id: 91,
			operation: {
				type: 'persist_record',
				record: {
					record_id: 'dapp-5-typed',
					kind: 'sign_typed_data',
					method: 'eth_signTypedData_v4',
					// The FULL request — larger than any cut.
					params_json: JSON.stringify(['0x88cCA0EeDbF2C4426110bbFc998F048689266894', typed]),
					result: '',
					from: '0x88cCA0EeDbF2C4426110bbFc998F048689266894',
					chain_id: 1,
					now_ms: 5,
					status: 'confirmed',
					user_op_hash: '',
					dapp_origin: 'Uniswap',
					dapp_url: 'https://app.uniswap.org',
					intent: null,
					maybe_sent: false,
					submit_block: null,
					summary,
					stored_request:
						'["0x88cCA0EeDbF2C4426110bbFc998F048689266894","{\\"primaryType\\":\\"PermitSingle\\"}"]',
					request_truncated: true
				}
			}
		});
		const saved = store.saveTransaction.mock.calls.at(-1)![0] as Record<string, unknown>;
		expect(saved).toMatchObject({
			id: 'dapp-5-typed',
			type: 'sign_typed_data',
			txHash: '',
			requestTruncated: true,
			dappSummary: summary,
			dappUrl: 'https://app.uniswap.org'
		});
		expect(saved.signedRequest).toEqual({
			method: 'eth_signTypedData_v4',
			params: ['0x88cCA0EeDbF2C4426110bbFc998F048689266894', '{"primaryType":"PermitSingle"}']
		});
		expect(JSON.stringify(saved.signedRequest).length).toBeLessThan(typed.length);
		expect(saved).not.toHaveProperty('balanceChanges');
	});

	it("a transaction's record keeps the core's result and the sheet's balance changes", async () => {
		store.saveTransaction.mockClear();
		const executor = createSignExecutor(makePorts());
		const changes = [
			{ type: 'native' as const, delta: '30000000000000000' },
			{ type: 'erc20_unverified' as const, token: null, delta: '-5' }
		];
		await executor.execute({
			id: 92,
			operation: {
				type: 'persist_record',
				record: {
					record_id: 'dapp-6-tx',
					kind: 'dapp_tx',
					method: 'eth_sendTransaction',
					params_json: '[{"to":"0x0000000000000000000000000000000000000001","value":"0x1"}]',
					result: '0x' + 'f1'.repeat(32),
					from: '0x88cCA0EeDbF2C4426110bbFc998F048689266894',
					chain_id: 1,
					now_ms: 6,
					status: 'confirmed',
					user_op_hash: OP,
					dapp_origin: 'https://app.uniswap.org',
					dapp_url: 'https://app.uniswap.org',
					intent: 'Swap',
					maybe_sent: false,
					submit_block: null,
					balance_changes: changes,
					summary: { action: 'call', calls: 1 },
					stored_request: '[{"to":"0x0000000000000000000000000000000000000001","value":"0x1"}]',
					request_truncated: false
				}
			}
		});
		const saved = store.saveTransaction.mock.calls.at(-1)![0] as Record<string, unknown>;
		expect(saved).toMatchObject({
			txHash: '0x' + 'f1'.repeat(32),
			intent: 'Swap',
			balanceChanges: changes,
			requestTruncated: false
		});
	});
});
