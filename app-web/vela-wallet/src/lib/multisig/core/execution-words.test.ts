/**
 * An execution's preview in words: why it would fail, what the fee is and
 * pays for (r4-5, r4-7). When it cannot run and each look's pace are the
 * machine's (app_multisig_presence.rs, app_multisig_proposals.rs).
 */
import { describe, expect, it } from 'vitest';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';
import { blockOf } from '../shell/proposal-failure';

const { feeCoin, feeWords, whyText } = await import('./execution-words');

type Priced = Extract<import('./execution').ExecutionPreview, { kind: 'priced' }>;

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
	it('says what the multisig holds and what it needs, in the core’s words (r12-1)', () => {
		const known = (base: bigint, text: string) => ({
			base_units: base.toString(),
			token: null,
			said: { kind: 'known' as const, text, number: text, symbol: 'USDC', decimals: 18 }
		});
		expect(
			whyText(words(), {
				kind: 'short',
				have: known((28n * USDC) / 100n, '0.28 USDC'),
				need: known(USDC / 2n, '0.5 USDC')
			})
		).toBe('the multisig holds 0.28 USDC, and this needs 0.5 USDC');
	});

	it('r12-1: a token nobody said the decimals of is said in base units — never scaled by 18', () => {
		const raw = (base: string) => ({
			base_units: base,
			token: '0x00000000000000000000000000000000000000ab',
			said: {
				kind: 'decimals_unknown' as const,
				key: 'home.multisig.review.reading.decimalsUnknown',
				raw: base
			}
		});
		const m = words();
		m.review.reading.decimalsUnknown = '{{raw}} base units · decimals unknown';
		expect(whyText(m, { kind: 'short', have: raw('0'), need: raw('10000') })).toBe(
			'the multisig holds 0 base units · decimals unknown, and this needs 10000 base units · decimals unknown'
		);
	});

	it('r12-1: a bare base-unit amount (an older core) is never scaled by a guess', () => {
		const block = blockOf({ kind: 'short', token: null, have: '0', need: '10000' });
		const m = words();
		m.review.reading.decimalsUnknown = '{{raw}} base units · decimals unknown';
		expect(whyText(m, block)).toBe(
			'the multisig holds 0 base units · decimals unknown, and this needs 10000 base units · decimals unknown'
		);
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
