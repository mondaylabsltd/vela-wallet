/**
 * An execution's preview in words: why it would fail, what the fee is and
 * pays for (r4-5, r4-7). When it cannot run and each look's pace are the
 * machine's (app_multisig_presence.rs, app_multisig_proposals.rs).
 */
import { describe, expect, it } from 'vitest';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const { feeCoin, feeWords, whyText } = await import('./execution-words');

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

