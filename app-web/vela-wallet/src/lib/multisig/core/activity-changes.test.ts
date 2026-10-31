/**
 * A waiting transaction's Home row says what must be read first (r4a-1,
 * r4a-2): a member change names who and the rule before → after on its
 * network, with "Changes who controls this multisig"; a transfer of a token
 * nobody vouches for carries its contract, and a lookalike says it is not
 * the real one. The core reads both; nothing here is stood in for but the
 * chain and the mailbox.
 */
import { describe, expect, it, vi } from 'vitest';
import { resolveMultisigMessages } from '$lib/i18n/engine.server';
import { shortenAddress } from '$lib/wallet/identity';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';
const C = '0x3333333333333333333333333333333333333333';
const D = '0x4444444444444444444444444444444444444444';
const FAKE = '0x57e64022d8d9a420dFE08a22d6a687caaBc6B7C6';
const ARC = 5042002;

const seams = vi.hoisted(() => ({ open: [] as unknown[] }));
vi.mock('./proposals.svelte', () => ({
	proposals: {
		chain: () => ({ members: { owners: [A, B, C], threshold: 2, nonce: 0n, deployed: true } }),
		get open() {
			return seams.open;
		},
		missing: [],
		stale: false,
		unreachable: false,
		offline: false,
		unreadable: [],
		historyUnreadable: [],
		cached: null,
		loading: false,
		readAt: null
	}
}));
vi.mock('./names.svelte', () => ({ names: { label: (a: string) => shortenAddress(a) } }));
vi.mock('./reading', () => ({ readTx: () => null, asReview: () => null }));
vi.mock('$lib/onboarding/core/storage', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/onboarding/core/storage')>()),
	loadAccounts: () => []
}));
// The fake token answers `symbol()` "USDC" and `decimals()` 6.
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (_method: string, params: [{ data: string }]) => ({
		result:
			params[0].data === '0x95d89b41'
				? `0x${'20'.padStart(64, '0')}${'4'.padStart(64, '0')}${'55534443'.padEnd(64, '0')}`
				: `0x${'6'.padStart(64, '0')}`
	})
}));

const { callsTx, reviewTx } = await import('./kernel');
const { waitingRows } = await import('./activity.svelte');
const m = resolveMultisigMessages('en');
const settle = () => new Promise((done) => setTimeout(done, 0));

function waiting(tx: ReturnType<typeof callsTx>, id: string) {
	return {
		chainId: ARC,
		safeTxHash: `0x${id.padStart(64, '0')}`,
		review: reviewTx(ARC, SAFE, tx),
		nonce: 0n,
		approved: 1,
		threshold: 2,
		status: 'pending',
		canApprove: [],
		canExecute: [],
		sameNonce: [],
		executing: false,
		unsent: false
	};
}

describe('a waiting member change on Home', () => {
	it('names who, the rule before and after, the network, and the caution', async () => {
		const add = `0x0d582f13${D.slice(2).padStart(64, '0')}${'2'.padStart(64, '0')}`;
		seams.open = [waiting(callsTx([{ to: SAFE, value: '0', data: add }], '0x0'), 'a1')];
		waitingRows.follow(m);
		await settle();
		const row = waitingRows.group(m)!.rows[0];
		expect(row.title).toBe(`Add ${shortenAddress(D)}`);
		expect(row.subtitle).toBe(
			// r4D-13: a plain wait says its count alone ("waiting for others" by itself).
			'1/2 approved · Approvals: 2 of 3 → 2 of 4 · Arc Testnet'
		);
		expect(row.caution).toBe('Changes who controls this multisig');
	});
});

describe('a waiting transfer of a token nobody vouches for', () => {
	it('carries the contract, and says a lookalike is not the real one', async () => {
		const transfer = `0xa9059cbb${B.slice(2).padStart(64, '0')}${(1000n * 10n ** 6n).toString(16).padStart(64, '0')}`;
		seams.open = [waiting(callsTx([{ to: FAKE, value: '0', data: transfer }], '0x0'), 'b2')];
		waitingRows.follow(m);
		await settle();
		const row = waitingRows.group(m)!.rows[0];
		expect(row.title).toBe('Send 1,000 USDC');
		expect(row.caution).toBe(
			`Not the real USDC — a different contract uses this name · ${shortenAddress(FAKE)}`
		);
	});
});
