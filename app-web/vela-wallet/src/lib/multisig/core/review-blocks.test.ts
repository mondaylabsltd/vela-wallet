/**
 * §3.6: the one review, in the signing sheet's own blocks — and parity: the
 * same transfer, approve and call, drawn for a Vela account (the signing
 * sheet's model) and for a multisig member (the call's reading + the one
 * review), show the same block kinds, the multisig's beside them (the
 * state-diff guard). The core is the real one; the chain answers are the
 * fork's own (`rust/crates/vela-core/tests/vectors/multisig-review/`), and
 * the clear-signing machines run to rest (`call-review.testing.ts`).
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SignView } from '$lib/core/generated/SignView';
import { resolveMultisigMessages, resolveSigningMessages } from '$lib/i18n/engine.server';
import { buildSigningModel } from '$lib/signing/live';
import type { Block } from '$lib/signing/model';
import type { SimEnded } from '$lib/signing/verdict';
import { callReviewBlocks } from './call-review.svelte';
import { APPROVE_MAX, BLIND, CHAIN, MULTISIG, TOKEN, read } from './call-review.testing';
import {
	multisigReview,
	type CriticalWrite,
	type MultisigReviewOut,
	type ReviewInput,
	type SafeTx
} from './kernel';
import {
	anywayLabel,
	hashRows,
	partyAddresses,
	recipientBadges,
	reviewBlocks,
	reviewTech,
	stateBlocks,
	writeLine,
	type ReviewContext
} from './review-blocks';

const m = resolveMultisigMessages('en');
const signing = resolveSigningMessages('en');
const CURRENCY: CurrencyView = { code: 'USD', rate: 1, committed: true, pending: null };
const ctx: ReviewContext = {
	m,
	signing,
	network: 'Gnosis',
	nativeSymbol: 'xDAI',
	preset: 'comma_dot',
	nameOf: () => null
};

const VECTORS = '../../rust/crates/vela-core/tests/vectors/multisig-review';
const vector = (name: string) =>
	JSON.parse(readFileSync(`${VECTORS}/${name}.json`, 'utf8')).input as ReviewInput;

const ZERO = '0x0000000000000000000000000000000000000000';
const txOf = (to: string, value: string, data: string): SafeTx => ({
	to,
	value,
	data,
	operation: 0,
	safeTxGas: '0x0',
	baseGas: '0x0',
	gasPrice: '0x0',
	gasToken: ZERO,
	refundReceiver: ZERO,
	nonce: '0x0'
});
const simOf = (review: MultisigReviewOut): SimEnded => ({
	verdict: review.simulation,
	judged: null
});
const kinds = (blocks: Block[]) => new Set(blocks.map((b) => b.kind));

describe('the state-diff guard, said (§3.6)', () => {
	it('Bybit on the fork: a red "changes who controls it", the slot-0 write, its source — no signing', () => {
		const review = multisigReview(vector('fork-bybit-trace'));
		expect(review.sign).toEqual({ allowed: false, anyway: false });
		const blocks = stateBlocks(review, ctx);
		expect(blocks[0]).toEqual({ kind: 'intent', text: m.review.stateTitle, tone: 'danger' });
		const card = blocks.find((b) => b.kind === 'card');
		expect(card && card.kind === 'card' && card.tone).toBe('danger');
		expect(card && card.kind === 'card' && card.rows[0].label).toContain(
			'0xBAd00000000000000000000000000000000000ba'
		);
		expect(blocks).toContainEqual({ kind: 'source', text: m.review.sourceTrace, tone: 'neutral' });
		// The probe (Arc's source) says the same, as its own source.
		const probed = stateBlocks(multisigReview(vector('fork-bybit-probe')), ctx);
		expect(probed[0]).toMatchObject({ kind: 'intent', tone: 'danger' });
		expect(probed).toContainEqual({ kind: 'source', text: m.review.sourceProbe, tone: 'neutral' });
	});

	it('a declared member change: checked, exactly as it says', () => {
		const review = multisigReview(vector('fork-add-owner-trace'));
		expect(stateBlocks(review, ctx)[0]).toEqual({
			kind: 'positive',
			text: m.review.stateDeclared.replace('{{network}}', 'Gnosis')
		});
	});

	it('a member being added: the name typed for them, never "first time sending here" (r5reg-16)', () => {
		const input = vector('fork-add-owner-trace');
		const preview = multisigReview(input);
		const added = preview.recipients.find((r) => r.role === 'member');
		expect(added).toBeDefined();
		const review = multisigReview({
			...input,
			recipientFacts: [{ address: added!.address, isContract: true, firstTime: true }]
		});
		const blocks = reviewBlocks(review, simOf(review), {
			...ctx,
			nameOf: () => 'A local label',
			memberName: (a) => (a.toLowerCase() === added!.address.toLowerCase() ? 'Typed name' : null)
		});
		const party = blocks.find((b) => b.kind === 'party' && b.label === m.review.roleMember);
		expect(party).toMatchObject({ name: 'Typed name' });
		expect(party && party.kind === 'party' && party.badge).toBeUndefined();
		expect(review.concerns.some((c) => c.kind === 'firstTime')).toBe(false);
	});

	it('no source on the network: a caution, and only the reading stands', () => {
		const input = vector('fork-transfer-probe-only');
		const review = multisigReview({ ...input, probe: { error: { code: -32601, message: 'x' } } });
		expect(stateBlocks(review, ctx)).toEqual([
			{
				kind: 'warning',
				tone: 'caution',
				text: m.review.stateUnread.replace('{{network}}', 'Gnosis')
			}
		]);
	});

	it('every write kind has its own words, none left as a template', () => {
		const lines = [
			{ kind: 'singleton', before: '0xA', after: '0xB' },
			{ kind: 'module', address: '0xM', enabled: true },
			{ kind: 'module', address: '0xM', enabled: false },
			{ kind: 'guard', before: '0x0', after: '0xG' },
			{ kind: 'moduleGuard', before: '0x0', after: '0xG' },
			{ kind: 'handler', before: '0x0', after: '0xH' },
			{ kind: 'owners', added: ['0xN'], removed: ['0xO'] },
			{ kind: 'ownerEntry', address: '0xE' },
			{ kind: 'threshold', before: '1', after: '2' },
			{ kind: 'ownerCount', before: '2', after: '9' },
			{ kind: 'nonce' },
			{ kind: 'storage', slot: '0x9' }
		] satisfies CriticalWrite[];
		const words = lines.flatMap((w) => writeLine(w, m.review));
		expect(words).toHaveLength(13);
		for (const w of words) {
			expect(w.length).toBeGreaterThan(0);
			expect(w).not.toContain('{{');
		}
	});
});

describe('what moves, who it reaches, the bytes (§3.6)', () => {
	it('what moves read from the probe where nothing else answers: said so, under the verdict', () => {
		const review = multisigReview(vector('fork-transfer-probe-only'));
		const blocks = reviewBlocks(review, simOf(review), ctx);
		expect(blocks.some((b) => b.kind === 'verdict')).toBe(true);
		expect(blocks).toContainEqual({ kind: 'source', text: m.review.movesProbe, tone: 'neutral' });
	});

	it('a lookalike recipient: a danger line naming who it looks like; first time and contract as badges', () => {
		const poisoned = '0x1234000000000000000000000000000000005678';
		const review = multisigReview({
			...vector('fork-transfer-probe-only'),
			tx: txOf(poisoned, '0x1', '0x'),
			recipientFacts: [{ address: poisoned, isContract: true, firstTime: true }],
			known: [{ address: '0x1234ffffffffffffffffffffffffffffffff5678', label: 'Alice' }]
		});
		const blocks = reviewBlocks(review, simOf(review), ctx);
		const party = blocks.find((b) => b.kind === 'party');
		expect(party).toMatchObject({ kind: 'party', label: m.review.roleRecipient });
		expect(party && party.kind === 'party' && party.badge?.text).toContain(signing.tagFirstTime);
		const warning = blocks.find((b) => b.kind === 'warning' && b.tone === 'danger');
		expect(warning && warning.kind === 'warning' && warning.text).toContain('Alice');
		expect(review.sign.anyway).toBe(true);
		expect(anywayLabel('approve', m.proposal.approve, review, m)).toBe(m.review.approveAnyway);
	});

	/**
	 * r4a-18, r5reg-8: one's own Vela account and a member are smart accounts
	 * the person controls or knows — never "first time sending here" nor
	 * "coins sent to a contract are lost"; said as theirs. An unknown
	 * contract and an unknown plain account are said as before.
	 */
	describe('who a transfer reaches (r4a-18, r5reg-8)', () => {
		const BOB = '0x412f2165f299Dfc2363ED08F8Fc6DE4353E81fF6';
		const reviewTo = (isContract: boolean, known: ReviewInput['known']) =>
			multisigReview({
				...vector('fork-transfer-probe-only'),
				tx: txOf(BOB, '0x1', '0x'),
				recipientFacts: [{ address: BOB, isContract, firstTime: true }],
				known
			});
		const partyOf = (review: MultisigReviewOut) => {
			const blocks = reviewBlocks(review, simOf(review), ctx);
			const party = blocks.find((b) => b.kind === 'party');
			return { blocks, party: party?.kind === 'party' ? party : null };
		};
		const lost = (blocks: Block[]) =>
			blocks.some(
				(b) => b.kind === 'warning' && b.text === m.review.toContract.replace('{{address}}', BOB)
			);

		it('one of this device’s accounts: "Your account", by its name, nothing else', () => {
			const review = reviewTo(true, [{ address: BOB, label: 'R5 Bob', kind: 'own' }]);
			const { blocks, party } = partyOf(review);
			expect(party?.name).toBe('R5 Bob');
			expect(party?.badge).toEqual({ text: m.review.knownOwn, tone: 'neutral' });
			expect(lost(blocks)).toBe(false);
			expect(review.concerns.some((c) => c.kind === 'firstTime' || c.kind === 'toContract')).toBe(
				false
			);
		});

		it('a member: "Member", nothing else', () => {
			const review = reviewTo(true, [{ address: BOB, label: 'Bob', kind: 'member' }]);
			const { blocks, party } = partyOf(review);
			expect(party?.badge).toEqual({ text: m.review.knownMember, tone: 'neutral' });
			expect(lost(blocks)).toBe(false);
		});

		it('an unknown contract: first time and contract, and the coins-lost line', () => {
			const review = reviewTo(true, []);
			const { blocks, party } = partyOf(review);
			expect(party?.badge?.text).toContain(signing.tagFirstTime);
			expect(party?.badge?.text).toContain(signing.tagContract);
			expect(lost(blocks)).toBe(true);
		});

		it('an unknown plain account: first time only', () => {
			const review = reviewTo(false, []);
			const { blocks, party } = partyOf(review);
			expect(party?.badge?.text).toBe(signing.tagFirstTime);
			expect(lost(blocks)).toBe(false);
		});
	});

	it('technical details: the ten SafeTx fields, the typed data as JSON, the three hashes', () => {
		const review = multisigReview(vector('fork-add-owner-trace'));
		const tech = reviewTech(review, ctx);
		expect(tech.params.map((p) => p.label)).toEqual([
			'to',
			'value',
			'data',
			'operation',
			'safeTxGas',
			'baseGas',
			'gasPrice',
			'gasToken',
			'refundReceiver',
			'nonce'
		]);
		expect(JSON.parse(tech.raw!.hex)).toEqual(review.typedData);
		const hashes = hashRows(review, m);
		expect(hashes.map((h) => h.value)).toEqual([
			review.hashes.domainHash,
			review.hashes.messageHash,
			review.hashes.safeTxHash
		]);
		expect(hashes.every((h) => /^0x[0-9a-f]{64}$/.test(h.value))).toBe(true);
	});
});

// -- parity: a Vela account's sheet and a multisig member's review, same intent ----------

const NO_FEE: FeeView = {
	busy: false,
	failed: null,
	fee: null,
	stale: false,
	fee_token: null,
	options: [],
	confirm_fee_ready: false,
	no_coin_pays: false,
	nothing_to_pay_from: false,
	provisional: false,
	failure: null
};

function siteRequest(paramsJson: string, from: string): SignView {
	return {
		surface: 'sheet',
		request: {
			id: 'parity',
			method: 'eth_sendTransaction',
			kind: 'transaction',
			params_json: paramsJson,
			origin: 'https://app.example',
			dapp: null,
			chain_id: CHAIN,
			signer_address: from,
			first_party: false
		},
		is_signing: false,
		is_submitting: false,
		phase: 'idle',
		pending_op_hash: null,
		pending_op_maybe_sent: false,
		error: null,
		funding: null,
		confirm_gate_open: true,
		confirm_block: null,
		reconcile_pending: false,
		swipe_action: 'none',
		tracker_handoff: null,
		landing_closed: false,
		tracker_withdraw: null,
		failure_refused: false,
		failure_retryable: false,
		failure_refusal_key: null,
		failure_not_sent: false,
		passkey_note: null,
		sim_checking: false,
		sim_waited_out_key: null,
		notice: null,
		blocked: null,
		global_chain_id: CHAIN
	};
}

const TO = '0x4444444444444444444444444444444444444444';
const TRANSFER = {
	to: TOKEN,
	value: '0',
	data: `0xa9059cbb${TO.slice(2).padStart(64, '0')}${5n.toString(16).padStart(64, '0')}`
};

describe('parity: the same intent for a Vela account and a multisig (§3.6)', () => {
	for (const [name, call] of [
		['an ERC-20 transfer', TRANSFER],
		['an unlimited approve', APPROVE_MAX],
		['a call nobody can read', BLIND]
	] as const) {
		it(`${name}: every block kind the account's sheet draws, the multisig's review draws`, () => {
			const reading = read(call);
			const sim = { verdict: null, judged: null } as unknown as SimEnded;
			// The Vela account: the signing sheet's own model for a site's request.
			const account = buildSigningModel({
				sign: siteRequest(reading.paramsJson, '0x9999999999999999999999999999999999999999'),
				clear: reading.clear,
				guard: reading.guard,
				fee: NO_FEE,
				currency: CURRENCY,
				sim,
				m: signing,
				identity: {
					name: '',
					address: '0x9999999999999999999999999999999999999999',
					identiconSvg: ''
				},
				identicon: () => ''
			})!;
			// The multisig member: the call's reading, then the one review.
			const review = multisigReview({
				chainId: CHAIN,
				safe: MULTISIG,
				tx: txOf(call.to, `0x${BigInt(call.value).toString(16)}`, call.data),
				owners: ['0x1111111111111111111111111111111111111111'],
				threshold: 1,
				deployed: true,
				simReply: '{"unreachable":true}',
				blind: reading.clear.sign_anyway
			});
			const member = [
				...callReviewBlocks(reading, signing, CURRENCY),
				...reviewBlocks(review, { verdict: review.simulation, judged: null }, ctx)
			];
			// The one deliberate difference: the account picks its cap (`allowance`);
			// a member has none to pick and reads what it grants as rows (cs-12).
			const shown = (k: string) => (k === 'allowance' ? 'rows' : k);
			for (const kind of kinds(account.blocks)) expect(kinds(member)).toContain(shown(kind));
			// The same friction: "Sign anyway" on the account's sheet is "… anyway" here.
			expect(review.sign.anyway).toBe(reading.clear.sign_anyway);
			// Technical details on both, with the bytes signed.
			expect(account.tech.raw).toBeDefined();
			expect(reviewTech(review, ctx).raw?.hex).toContain('SafeTx');
		});
	}
});

/**
 * r5reg-15: a transfer's review named its recipient twice — the call's
 * "Recipient 0xd9c7…" and the review's "To 0xd9c7…". Said once: the call's
 * row, wearing what the review knows of it ("First time sending here").
 */
describe('who it reaches, said once (r5reg-15)', () => {
	it('an ERC-20 transfer: the call names the recipient; the review adds its badge, not a second row', () => {
		const reading = read(TRANSFER);
		const call = callReviewBlocks(reading, signing, CURRENCY);
		const drawn = partyAddresses(call);
		expect(drawn).toContain(TO.toLowerCase());
		const review = multisigReview({
			chainId: CHAIN,
			safe: MULTISIG,
			tx: txOf(TRANSFER.to, '0x0', TRANSFER.data),
			owners: ['0x1111111111111111111111111111111111111111'],
			threshold: 1,
			deployed: true,
			simReply: '{"unreachable":true}',
			blind: reading.clear.sign_anyway,
			recipientFacts: [{ address: TO, isContract: false, firstTime: true }]
		});
		const named = (blocks: Block[]) =>
			blocks.filter((b) => b.kind === 'party' && b.address?.toLowerCase() === TO.toLowerCase());
		// Without the call's reading the review names it; with it, it does not.
		expect(named(reviewBlocks(review, simOf(review), ctx))).toHaveLength(1);
		expect(named(reviewBlocks(review, simOf(review), ctx, drawn))).toHaveLength(0);
		// One row in all — and it still says it is the first time.
		expect(named([...call, ...reviewBlocks(review, simOf(review), ctx, drawn)])).toHaveLength(1);
		expect(recipientBadges(review, ctx)[TO.toLowerCase()]?.text).toBe(signing.tagFirstTime);
	});
});
