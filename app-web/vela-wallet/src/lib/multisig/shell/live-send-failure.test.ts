/**
 * r17-12: an execution that stopped before its passkey was asked — the relay
 * refused its fee estimate, or the network's reads failed — was said as
 * "Something went wrong, so nothing was sent." on the web, while desktop said
 * which. The shell hands the core the kind (the desktop's r16-18 rule): a read
 * on a network the pool finds failing is `chain_unreachable`, the relay
 * refusing or not reached is `not_sent`; the sheet words each kind from the
 * catalog (`review.failure`).
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => storage.get(k) ?? null,
	setItem: (k: string, v: string) => void storage.set(k, v),
	removeItem: (k: string) => void storage.delete(k)
});

let thrown: unknown = null;
vi.mock('$lib/services/safe-transaction', async (original) => ({
	...(await original<typeof import('$lib/services/safe-transaction')>()),
	sendBatchCalls: async () => {
		throw thrown;
	}
}));
let failing = false;
vi.mock('$lib/services/rpc-pool', async (original) => ({
	...(await original<typeof import('$lib/services/rpc-pool')>()),
	chainFailing: () => failing
}));

const { EstimateGasError, UserOpNotSentError } = await import('$lib/services/safe-transaction');
const { failureSaid } = await import('../core/review-door');
const { detailError, ExecutionExpiredError } = await import('./proposal-failure');
const { PoolFailedError } = await import('$lib/services/rpc-pool');
const { liveDeps } = await import('./live');
const deps = liveDeps({ peer: {} as never, notice: () => {} });

const KEY = '04' + '11'.repeat(64);
const FROM = '0x8F1E0bC5D9aA3F6e2B7c4D1a9E8f3B2c5A6d7E01';

beforeEach(() => {
	storage.clear();
	storage.set(
		'vela.accounts',
		JSON.stringify([
			{
				id: 'aa01',
				name: 'Alice',
				address: FROM,
				public_key_hex: KEY,
				created_at_iso: '2026-10-29T00:00:00.000Z',
				keys: [
					{ credential_id: 'aa01', public_key_hex: KEY, name: 'Alice', transports: 'internal' }
				],
				sign_in_key: { credential_id: 'aa01', method: 'platform', transports: 'internal' }
			}
		])
	);
	failing = false;
});

async function executeFailing(error: unknown) {
	thrown = error;
	const result = await deps.sendCalls(
		{
			type: 'send_calls',
			chain_id: 5042002,
			from: FROM,
			calls: [{ to: '0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69', value: '0x0', data: '0x' }],
			fee: { amount: '1', recipient: FROM, tier: 'standard', token: null },
			purpose: { kind: 'execute' }
		} as never,
		new AbortController().signal
	);
	expect(result.type).toBe('calls_failed');
	const detail = (result as { detail: string }).detail;
	return JSON.parse(detail) as { kind: string; chain_id?: number };
}

describe('an execution stopped before its passkey (r17-12)', () => {
	it('a relay that refused its fee estimate is "not sent", never "something went wrong"', async () => {
		const said = await executeFailing(new EstimateGasError('refused', '{"code":-32602}'));
		expect(said.kind).toBe('not_sent');
	});

	it('a network the pool finds failing is "can’t reach" that network', async () => {
		failing = true;
		const said = await executeFailing(
			new PoolFailedError('All RPC endpoints failed for chain 5042002', {
				maybeDelivered: false,
				rateLimited: false
			})
		);
		expect(said).toMatchObject({ kind: 'chain_unreachable', chain_id: 5042002 });
	});

	it('the relay not reached (its pool failed, the network answering) is "not sent"', async () => {
		const said = await executeFailing(
			new PoolFailedError('All bundler endpoints failed for chain 5042002', {
				maybeDelivered: false,
				rateLimited: false
			})
		);
		expect(said.kind).toBe('not_sent');
	});

	it('a failure inside the app stays "something went wrong"', async () => {
		const said = await executeFailing(
			new PoolFailedError('RPC pool fault', {
				maybeDelivered: true,
				rateLimited: false,
				internal: true
			})
		);
		expect(said.kind).toBe('unknown');
	});
});

/**
 * r21-1: an execution the relay refused with AA22 ("expired or not due") —
 * its window closed before the EntryPoint saw it — is said as the Send says
 * it: "Not sent", calmly, `componentsUi.signing.expired`, and pressing again
 * signs anew. Never "Something went wrong" or "The network didn't take it".
 */
describe('an expired execution (r21-1)', () => {
	const AA22 = 'SUBMIT REFUSED by simulation: AA22 expired or not due';

	it('the send pipeline’s Expired is the shell’s `expired`, said in the Send’s words', async () => {
		const said = await executeFailing(new UserOpNotSentError('expired', AA22));
		expect(said.kind).toBe('expired');
		const detail = JSON.stringify(said);
		expect(failureSaid(detail)).toEqual({ kind: 'expired', key: 'componentsUi.signing.expired' });
		expect(detailError(detail)).toBeInstanceOf(ExecutionExpiredError);
	});

	it('the relay’s AA22 carried in a not-sent detail is the same expiry', async () => {
		const said = await executeFailing(new UserOpNotSentError({ other: AA22 }, AA22));
		const detail = JSON.stringify(said);
		expect(failureSaid(detail).key).toBe('componentsUi.signing.expired');
		expect(detailError(detail)).toBeInstanceOf(ExecutionExpiredError);
		// …and only AA22: a plain refusal is still "not sent".
		expect(detailError('{"kind":"not_sent"}')).not.toBeInstanceOf(ExecutionExpiredError);
	});
});
