/**
 * Issue 262: a receipt that is late is not a confirmation.
 *
 * The bundler accepted the op and the receipt wait ran out. The page still
 * needs a hash, so the core answers with the op hash — but only if the shell
 * TELLS it the receipt is pending (`receipt_pending`). Reported as
 * `succeeded`, the core flips the record to "confirmed" with the op hash as
 * its tx hash while nothing may have landed.
 */
import { describe, expect, it, vi } from 'vitest';

const submit = vi.hoisted(() => ({ impl: null as null | (() => Promise<unknown>) }));

vi.mock('$lib/services/dapp-submit', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/services/dapp-submit')>();
	return { ...actual, handleDAppRequest: () => submit.impl!() };
});

import { DAppReceiptPendingError, receiptStillOutstanding } from '$lib/services/dapp-submit';
import { UserOpFeeHoldError, UserOpRejectedError } from '$lib/services/safe-transaction';
import { createSignExecutor } from './sign-executor';
import type { SignEffect } from './sign-types';

const ports = {
	transportFor: () => null,
	opSubmitted: () => {},
	assetSim: () => null,
	switchActiveAccount: async () => {}
};

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
		const result = await createSignExecutor(ports).execute(signAndSubmit);
		expect(result).toMatchObject({
			type: 'submit',
			outcome: { type: 'receipt_pending', user_op_hash: '0xophash' }
		});
	});

	it('a receipt in time is succeeded with the tx hash', async () => {
		submit.impl = () => Promise.resolve('0xtxhash');
		const result = await createSignExecutor(ports).execute(signAndSubmit);
		expect(result).toMatchObject({
			type: 'submit',
			outcome: { type: 'succeeded', result: '0xtxhash' }
		});
	});

	it('only a relay rejection or a drop is a verdict; timeout, unreachable and fee-hold are not', () => {
		expect(receiptStillOutstanding(new UserOpRejectedError('refused'))).toBe(false);
		expect(
			receiptStillOutstanding(new Error('Transaction was dropped from the network. Try again.'))
		).toBe(false);
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
