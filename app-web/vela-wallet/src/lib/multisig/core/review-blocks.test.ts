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
