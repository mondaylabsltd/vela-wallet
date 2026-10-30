/**
 * An execution's preview in words, and the looks the sheet and the home
 * share: why it would fail, what the fee is and pays for (r4-5, r4-7), and
 * when the home says "Can't run now".
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

vi.mock('./execute', () => ({ previewExecution: async () => ({ kind: 'unpriced' }) }));

const { feeCoin, feeWords, whyText } = await import('./execution-words');
const { blockOf, preflight, previewKey } = await import('./preflight.svelte');

type Priced = Extract<import('./execute').ExecutionPreview, { kind: 'priced' }>;

const USDC = 10n ** 18n;
const ALICE = '0x00000000000000000000000000000000000000a1';
const BOB = '0x00000000000000000000000000000000000000b0';

/** The English words the corpus carries for these keys. */
function words(): MultisigMessages {
	const out = structuredClone(MULTISIG_SHAPE);
	Object.assign(out.proposal, {
		whyShort: 'the multisig holds {{have}}, and this needs {{need}}',
		whyReverts: 'the network refuses it — “{{reason}}”',
		whyRevertsPlain: 'it reverts when it runs, and the network gives no reason',
		feeLine: 'Network fee ≈ {{fee}}, paid by {{name}}',
		feeSetup: 'Includes one-time setup on {{network}} ≈ {{amount}}:',
		feeSetupPlain: 'Includes one-time setup on {{network}}:',
		setupMultisig: 'deploying this multisig',
		setupMember: 'activating {{name}}’s account — the multisig checks their approval through it',
		setupExecutor: 'activating {{name}}’s account, which pays the fee'
	});
	return out;
}

const priced = (
	setup: Partial<{
		multisig: boolean;
		members: string[];
		executor: boolean;
		amount: bigint | null;
	}> = {},
	canPay = true
) =>
	({
		kind: 'priced',
		calls: [],
		fee: { amount: (35n * USDC) / 1000n, recipient: '0xfe' },
		balance: USDC,
		canPay,
		carries: [],
		executorApproves: true,
		setup: { multisig: false, members: [], executor: false, amount: null, ...setup }
	}) as Priced;

const opts = {
	executor: ALICE,
	symbol: 'USDC',
	network: 'Arc Testnet',
	label: (a: string) => (a === ALICE ? 'Alice' : a === BOB ? 'Bob' : a)
};

describe('why it would fail, in words a person can act on', () => {
	it('says what the multisig holds and what it needs', () => {
		expect(
			whyText(words(), {
				kind: 'short',
				symbol: 'USDC',
				decimals: 18,
				have: (28n * USDC) / 100n,
				need: USDC / 2n
			})
		).toBe('the multisig holds 0.28 USDC, and this needs 0.5 USDC');
	});

	it('the network’s reason, or that it gave none', () => {
		expect(whyText(words(), { kind: 'reverts', reason: 'paused' })).toBe(
			'the network refuses it — “paused”'
		);
		expect(whyText(words(), { kind: 'reverts', reason: null })).toBe(
			'it reverts when it runs, and the network gives no reason'
		);
	});
});

describe('the fee line (r4-7)', () => {
	it('says the fee in the executor’s coin and who pays — no setup line when nothing is set up', () => {
		expect(feeWords(words(), priced(), opts)).toEqual({
			fee: 'Network fee ≈ 0.035 USDC, paid by Alice',
			setup: null,
			items: []
		});
	});

	it('says what the one-time setup is, and about how much of the fee', () => {
		const shown = feeWords(
			words(),
			priced({ multisig: true, members: [BOB], amount: (2n * USDC) / 100n }),
			opts
		);
		expect(shown.setup).toBe('Includes one-time setup on Arc Testnet ≈ 0.02 USDC:');
		expect(shown.items).toEqual([
			'deploying this multisig',
			'activating Bob’s account — the multisig checks their approval through it'
		]);
	});

	it('r5reg-15: reads in the wallet’s fee precision — never six decimals, never "0"', () => {
		const at = (wei: bigint) => ({ ...priced(), fee: { amount: wei, recipient: '0xfe' } });
		// 0.035412… USDC read "0.035412 USDC": four decimals, as the wallet's fee rows.
		expect(feeWords(words(), at(35_412_345_678_901_234n), opts).fee).toBe(
			'Network fee ≈ 0.0354 USDC, paid by Alice'
		);
		// A tiny fee widens to its first digit, so it never reads as free.
		expect(feeWords(words(), at(12_345_678_901_234n), opts).fee).toBe(
			'Network fee ≈ 0.000012 USDC, paid by Alice'
		);
		expect(feeCoin(35_412_345_678_901_234n, 'USDC')).toBe('0.0354 USDC');
	});

	it('names the executor’s own first deployment, and no share it could not measure', () => {
		const shown = feeWords(words(), priced({ executor: true }), opts);
		expect(shown.setup).toBe('Includes one-time setup on Arc Testnet:');
		expect(shown.items).toEqual(['activating Alice’s account, which pays the fee']);
	});
});

describe('when the home says it cannot run (r4-5)', () => {
	it('any look that fails: the transaction’s own fault', () => {
		const why = { kind: 'reverts', reason: null } as const;
		expect(blockOf([priced(), { kind: 'fails', why }])).toEqual({ kind: 'fails', why });
	});

	it('nobody here can pay, once every executor was priced', () => {
		expect(blockOf([priced({}, false), priced({}, false)])).toEqual({ kind: 'nobodyPays' });
		expect(blockOf([priced({}, false), priced({}, true)])).toBeNull();
		expect(blockOf([priced({}, false), null]), 'a look still out says nothing').toBeNull();
		expect(blockOf([{ kind: 'unpriced' }])).toBeNull();
		expect(blockOf([])).toBeNull();
	});
});

describe('the looks the sheet and the home share', () => {
	const input = (nonce = 0n) =>
		({
			chainId: 5042002,
			multisig: '0xcc',
			executor: { address: ALICE },
			members: { owners: [ALICE], threshold: 1, deployed: true, nonce },
			tx: { nonce: '0x0' },
			approvals: []
		}) as never;

	beforeEach(() => preflight._reset());

	it('takes one look per execution while it is fresh, and a new one at a new state', async () => {
		const look = vi.fn(async () => priced());
		preflight._reset(look);
		await preflight.check(input());
		await preflight.check(input());
		expect(look).toHaveBeenCalledTimes(1);
		expect(preflight.get(input())).toEqual(priced());
		await preflight.check(input(1n));
		expect(look).toHaveBeenCalledTimes(2);
		expect(previewKey(input())).not.toBe(previewKey(input(1n)));
		await preflight.check(input(), 0);
		expect(look, 'asked again on the person’s tap').toHaveBeenCalledTimes(3);
	});

	it('a look that throws is unpriced, never a hang', async () => {
		preflight._reset(async () => {
			throw new Error('down');
		});
		expect(await preflight.check(input())).toEqual({ kind: 'unpriced' });
	});

	/**
	 * Round 5 (S2): "Approve and execute" stayed disabled four minutes on a
	 * look that never answered. Every look is capped: past it, "couldn't
	 * check" — approving goes ahead, Execute looks again — and a late answer
	 * still lands.
	 */
	it('a look that never answers is said as unchecked at its cap — never endless', async () => {
		vi.useFakeTimers();
		try {
			const answers: ((p: Priced) => void)[] = [];
			preflight._reset(() => new Promise<Priced>((done) => answers.push(done)), 15_000);
			const looking = preflight.check(input());
			expect(preflight.get(input())).toBeNull();
			await vi.advanceTimersByTimeAsync(15_000);
			expect(await looking).toEqual({ kind: 'unchecked' });
			expect(preflight.get(input())).toEqual({ kind: 'unchecked' });
			// Not reused: Execute's own look (freshness 0) asks again, capped too.
			const again = preflight.check(input(), 0);
			expect(answers).toHaveLength(2);
			await vi.advanceTimersByTimeAsync(15_000);
			expect(await again).toEqual({ kind: 'unchecked' });
			// The first look answers at last: it lands.
			answers[0](priced());
			await vi.advanceTimersByTimeAsync(0);
			expect(preflight.get(input())).toEqual(priced());
		} finally {
			vi.useRealTimers();
		}
	});
});
