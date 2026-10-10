/**
 * PR 2 note 1: a failed fee is asked again by the CORE, through the timer the
 * fee session answers — and only while the session lives.
 *
 * The signing sheet used to run its own re-quote scheduler (`FeeRequoteTimer`)
 * and the send form ran none at all: the row said "Retrying automatically"
 * over a footer that said "Tap it to retry", and the send form never retried.
 * The fee machine now asks for `start_ttl` after a failure that can pass —
 * 3 s, 6 s, then every 8 s — and the session's executor answers it with a
 * plain sleep. Every fee surface (the send form and confirm, the speed
 * previews, the signing sheet in the app and in the extension's request
 * window) runs this one session type, so pinning it here pins them all.
 *
 * Driven end to end: the REAL `fee_policy` core inside the REAL session and
 * executor; only the network reads are stood in for (the account read fails
 * as the chain out of reach), and the clock is the test's.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FeeView } from '$lib/core/generated/FeeView';

const seams = vi.hoisted(() => ({ deployed: vi.fn(), gasSignals: vi.fn() }));

vi.mock('$lib/services/bundler-service', () => ({
	fetchInBandGasQuotes: vi.fn(async () => null),
	fetchBundlerAccountInfo: vi.fn(async () => null)
}));
vi.mock('$lib/services/wallet-api', () => ({ getCachedNativePriceUsd: () => null }));
vi.mock('$lib/services/safe-transaction', () => ({
	accountIsDeployed: seams.deployed,
	fetchRawBundlerQuote: vi.fn(async () => null),
	fetchRawGasSignals: seams.gasSignals,
	keySetOf: vi.fn(),
	measureCallGasForQuote: vi.fn(),
	simulateUserOpGas: vi.fn()
}));
vi.mock('$lib/services/accounts', () => ({ findAccountByAddress: vi.fn(async () => null) }));

import { DeploymentReadError } from '$lib/services/deployment-read';
import { createFeeSession } from './fee-session';

const ACCOUNT = '0x1111111111111111111111111111111111111111';

function quoteRequested() {
	return {
		type: 'quote_requested' as const,
		chain_id: 1,
		account: ACCOUNT,
		deployed: false,
		read_deployment: true,
		public_key_available: true,
		tier: 'standard' as const,
		calls: [],
		fee_token: null,
		auto_fee_token: false,
		number: 'comma_dot' as const
	};
}

/** Let the effect loop's promises settle (the executor awaits the mocks). */
async function settle(): Promise<void> {
	for (let i = 0; i < 10; i += 1) await Promise.resolve();
}

describe('the core retries a failed fee through the session’s own timer', () => {
	let views: FeeView[];
	beforeEach(() => {
		vi.useFakeTimers();
		views = [];
		// Every node of the chain is away: the account read gets no answer.
		seams.deployed.mockReset().mockRejectedValue(new DeploymentReadError(false));
		seams.gasSignals.mockReset().mockResolvedValue({
			ethGasPrice: null,
			baseFee: null,
			priorityFee: null
		});
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	function session() {
		const s = createFeeSession({
			onView: (view) => views.push(view),
			onError: (error) => {
				throw error;
			},
			publicKeyHex: () => undefined
		});
		s.start(quoteRequested());
		return s;
	}
	const last = () => views[views.length - 1];

	it('asks again at 3 s, then 6 s — a real new read each time, the reason kept through it', async () => {
		const s = session();
		await settle();
		expect(seams.deployed).toHaveBeenCalledTimes(1);
		// One truth for the row and the footer: retrying by itself, so no tap.
		expect(last().failed).toEqual({ chain_read: { rate_limited: false } });
		expect(last().failure).toEqual({
			failure: { chain_read: { rate_limited: false } },
			reason_key: 'componentsUi.gas.reasonChainDown',
			auto_retry: true,
			retrying: false,
			figure_key: null,
			footer_key: 'componentsUi.signing.confirmBlock.feeRetrying',
			// PR 2 polish: a tap asks again; the question it answered.
			tap: 'retry',
			chain_id: 1,
			fee_token: null
		});

		await vi.advanceTimersByTimeAsync(2_999);
		expect(seams.deployed).toHaveBeenCalledTimes(1);
		await vi.advanceTimersByTimeAsync(1);
		await settle();
		// The re-ask is a fresh read (issue 483), and while it is out the row
		// keeps its reason and the footer its line — busy, `retrying`.
		expect(seams.deployed).toHaveBeenCalledTimes(2);
		expect(seams.deployed.mock.calls[1][2]).toEqual({ fresh: true });
		const during = views.find((view, i) => i > 0 && view.busy && view.failure?.retrying);
		expect(during?.failure).toMatchObject({
			reason_key: 'componentsUi.gas.reasonChainDown',
			retrying: true,
			footer_key: 'componentsUi.signing.confirmBlock.feeRetrying'
		});
		// Failed again: the next wait is the core's second step.
		expect(last().failure?.retrying).toBe(false);
		await vi.advanceTimersByTimeAsync(5_999);
		expect(seams.deployed).toHaveBeenCalledTimes(2);
		await vi.advanceTimersByTimeAsync(1);
		await settle();
		expect(seams.deployed).toHaveBeenCalledTimes(3);
		s.dispose();
	});

	it('a tap asks at once, and the pending timer goes with the attempt it moved on', async () => {
		const s = session();
		await settle();
		expect(seams.deployed).toHaveBeenCalledTimes(1);
		await vi.advanceTimersByTimeAsync(1_000);
		s.dispatch({ type: 'requote' });
		await settle();
		expect(seams.deployed).toHaveBeenCalledTimes(2);
		// The 3 s timer set before the tap does not fire a third read at 3 s.
		await vi.advanceTimersByTimeAsync(2_500);
		expect(seams.deployed).toHaveBeenCalledTimes(2);
		s.dispose();
	});

	it('a session that ended asks nothing more', async () => {
		const s = session();
		await settle();
		expect(seams.deployed).toHaveBeenCalledTimes(1);
		s.dispose();
		await vi.advanceTimersByTimeAsync(60_000);
		await settle();
		expect(seams.deployed).toHaveBeenCalledTimes(1);
	});

	it('a failure no retry can clear is never retried, and says tap', async () => {
		const s = createFeeSession({
			onView: (view) => views.push(view),
			publicKeyHex: () => undefined
		});
		// Undeployed, and no key to build its initCode with: only a tap helps.
		seams.deployed.mockReset().mockResolvedValue(false);
		s.start({ ...quoteRequested(), public_key_available: false });
		await settle();
		expect(last().failed).toBe('missing_public_key');
		expect(last().failure).toEqual({
			failure: 'missing_public_key',
			reason_key: null,
			auto_retry: false,
			retrying: false,
			figure_key: 'componentsUi.gas.estimateFailed',
			footer_key: 'componentsUi.signing.confirmBlock.feeFailed',
			tap: 'retry',
			chain_id: 1,
			fee_token: null
		});
		await vi.advanceTimersByTimeAsync(60_000);
		await settle();
		expect(seams.deployed).toHaveBeenCalledTimes(1);
		s.dispose();
	});
});
