/**
 * §3.6: the one review, in the signing sheet's own blocks — and parity: the
 * same transfer, approve and call, drawn for a Vela account (the signing
 * sheet's model) and for a multisig member (the call's reading + the one
 * review), show the same block kinds, the multisig's beside them (the
 * state-diff guard). The core is the real one; the chain answers are the
 * fork's own (`rust/crates/vela-core/tests/vectors/multisig-review/`), and
 * the core reads every call itself (r8-1, `reading-blocks.ts`).
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { resolveMultisigMessages, resolveSigningMessages } from '$lib/i18n/engine.server';
import type { Block } from '$lib/signing/model';
import type { SimEnded } from '$lib/signing/verdict';
import { concernBlocks, readingBlocks, readingTitle, type ReadingContext } from './reading-blocks';
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
	movesBlocks,
	partyAddresses,
	recipientBadges,
	recipientNames,
	reviewBlocks,
	reviewTech,
	simWithheld,
	stateBlocks,
	writeLine,
	type ReviewContext
} from './review-blocks';

const m = resolveMultisigMessages('en');
const signing = resolveSigningMessages('en');
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

	it('a refused delegatecall: nothing in the place of what moves — never "No asset changes"', () => {
		const noChange = signing.simSaid['componentsUi.signing.simResultNoChange'];
		expect(noChange).toBeTruthy();
		for (const name of ['fork-bybit-probe', 'fork-bybit-trace']) {
			const review = multisigReview(vector(name));
			expect(review.refused?.why).toBe('delegatecall');
			expect(review.simulation.kind).toBe('withheld');
			expect(review.simulation.no_change_key).toBeNull();
			expect(simWithheld(review)).toBe(true);
			// Landed or not, the verdict's place holds nothing: no card, no
			// "Checking…", no could-not-check, no "read from the probe" line
			// (the state guard keeps its own source line).
			for (const sim of [simOf(review), null]) {
				expect(movesBlocks(review, sim, ctx)).toEqual([]);
				const blocks = reviewBlocks(review, sim, ctx);
				expect(blocks.some((b) => b.kind === 'verdict')).toBe(false);
				expect(JSON.stringify(blocks)).not.toContain(noChange);
				expect(JSON.stringify(blocks)).not.toContain(m.review.movesProbe);
			}
			expect(review.concerns.some((c) => c.kind === 'unsimulated')).toBe(false);
		}
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

		/**
		 * r4a-18 with r5reg-15: when the call's own row names the address (and
		 * the review's row is left out), that row reads by the name the review
		 * knows — "R5 Bob · Your account", never a bare address beside it.
		 */
		it('the call row wears the known name; an unknown address has none to wear', () => {
			const own = reviewTo(true, [{ address: BOB, label: 'R5 Bob', kind: 'own' }]);
			expect(recipientNames(own, ctx)).toEqual({ [BOB.toLowerCase()]: 'R5 Bob' });
			expect(recipientNames(reviewTo(false, []), ctx)).toEqual({});
			// This device's own name for it comes first.
			const named = { ...ctx, nameOf: () => 'Bob (this device)' };
			expect(recipientNames(own, named)[BOB.toLowerCase()]).toBe('Bob (this device)');
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

// -- r8-1: the core's own reading of every call, drawn ---------------------------------

/** The shared vectors every platform replays (`tests/vectors/multisig/reading.json`). */
const READING = JSON.parse(
	readFileSync('../../rust/crates/vela-core/tests/vectors/multisig/reading.json', 'utf8')
) as {
	cases: {
		name: string;
		args: { chainId: number; safe: string; tx: SafeTx; tokenFacts?: unknown[] };
		expect: { reading: unknown; anyway: boolean; refused: boolean };
	}[];
};
const readingCtx: ReadingContext = {
	r: m.review.reading,
	callNumber: m.review.callNumber,
	nativeSymbol: 'USDC'
};
const reviewOf = (c: (typeof READING.cases)[number]) =>
	multisigReview({
		chainId: c.args.chainId,
		safe: c.args.safe,
		tx: c.args.tx,
		owners: ['0x1111111111111111111111111111111111111111'],
		threshold: 1,
		deployed: true,
		simReply: '{"unreachable":true}',
		tokenFacts: c.args.tokenFacts ?? []
	} as never);
const text = (blocks: Block[]) => JSON.stringify(blocks);

describe('the core reads every call itself, the same on every platform (r8-1)', () => {
	for (const c of READING.cases) {
		it(c.name, () => {
			const review = reviewOf(c);
			// The review carries the very reading the shared vectors pin.
			expect(review.reading).toEqual(c.expect.reading);
			if (!c.expect.refused) expect(review.sign.anyway).toBe(c.expect.anyway);
			else expect(review.sign.allowed).toBe(false);
			// Every call is drawn; none as an empty review.
			const blocks = readingBlocks(review, readingCtx);
			expect(blocks.length > 0).toBe(review.reading.calls.length > 0);
			// Nothing is left as a template.
			expect(text([...blocks, ...concernBlocks(review, readingCtx)])).not.toContain('{{');
		});
	}

	const named = (name: string) => READING.cases.find((c) => c.name.startsWith(name))!;

	it("approve(MAX) of Arc's USDC: Approve, Unlimited in red, its spender, and the danger line", () => {
		const review = reviewOf(named("approve(spender, 2^256-1) of Arc's USDC"));
		const blocks = readingBlocks(review, readingCtx);
		expect(blocks[0]).toEqual({ kind: 'intent', text: m.review.reading.approve, tone: 'danger' });
		const rows = blocks.flatMap((b) => (b.kind === 'rows' ? b.rows : []));
		expect(rows).toContainEqual({
			label: m.review.reading.allowance,
			value: m.review.reading.unlimited,
			valueTone: 'danger'
		});
		expect(rows).toContainEqual({ label: m.review.reading.token, value: 'USDC' });
		const lines = concernBlocks(review, readingCtx);
		expect(lines.some((b) => b.kind === 'warning' && b.tone === 'danger')).toBe(true);
		expect(anywayLabel('approve', 'Approve', review, m)).toBe(m.review.approveAnyway);
	});

	it('a token nobody read: its amount raw with "decimals unknown", never 18 assumed (r8-2)', () => {
		const review = reviewOf(named('a token nobody read'));
		const said = text(readingBlocks(review, readingCtx));
		expect(said).toContain('10000');
		expect(said).toContain('decimals');
		expect(said).not.toContain('0.00000000000001');
	});

	it("Arc's USDC transfer of 10000: 0.01 USDC (r8-2, r8-20)", () => {
		const review = reviewOf(named("Arc's USDC ERC-20 transfer"));
		expect(readingTitle(review, readingCtx)).toBe(`${m.review.reading.send} 0.01 USDC`);
	});

	it('a nested batch: every leg of every level, and "… anyway" (r8-5)', () => {
		const review = reviewOf(named('a batch whose leg calls multiSend'));
		const cards = readingBlocks(review, readingCtx).filter((b) => b.kind === 'card');
		const titles = cards.map((b) => (b.kind === 'card' ? b.title : ''));
		expect(titles).toHaveLength(3);
		expect(titles[0]).toContain('1');
		expect(titles[1]).toContain('1.1');
		expect(titles[2]).toContain('1.2');
		expect(review.sign.anyway).toBe(true);
	});

	it('a batch nested too deep: refused, nothing signed (r8-5) — said once, by the refusal (r10)', () => {
		const review = reviewOf(named('multiSend called four deep'));
		expect(review.sign.allowed).toBe(false);
		// One refusal box: the core says it as the refusal (`refused.why =
		// tooDeep`) and leaves the "can't be read" concern out at the source.
		expect(review.refused?.why).toBe('tooDeep');
		expect(text(concernBlocks(review, readingCtx))).not.toContain(
			m.review.reading.unreadableLine
		);
	});
});

const TOKEN = '0x3600000000000000000000000000000000000000';
const TO = '0x4444444444444444444444444444444444444444';
const TRANSFER = {
	to: TOKEN,
	value: '0',
	data: `0xa9059cbb${TO.slice(2).padStart(64, '0')}${5n.toString(16).padStart(64, '0')}`
};

/**
 * r5reg-15: a transfer's review named its recipient twice — the call's
 * "Recipient 0xd9c7…" and the review's "To 0xd9c7…". Said once: the call's
 * row, wearing what the review knows of it ("First time sending here").
 */
describe('who it reaches, said once (r5reg-15)', () => {
	it('an ERC-20 transfer: the call names the recipient; the review adds its badge, not a second row', () => {
		const review = multisigReview({
			chainId: 5042002,
			safe: '0x0e96ddc491878645dB7E3a47d2a6Ccbc94f17f32',
			tx: txOf(TRANSFER.to, '0x0', TRANSFER.data),
			owners: ['0x1111111111111111111111111111111111111111'],
			threshold: 1,
			deployed: true,
			simReply: '{"unreachable":true}',
			recipientFacts: [{ address: TO, isContract: false, firstTime: true }]
		});
		const call = readingBlocks(review, readingCtx);
		const drawn = partyAddresses(call);
		expect(drawn).toContain(TO.toLowerCase());
		const namedRows = (blocks: Block[]) =>
			blocks.filter((b) => b.kind === 'party' && b.address?.toLowerCase() === TO.toLowerCase());
		// Without the call's reading the review names it; with it, it does not.
		expect(namedRows(reviewBlocks(review, simOf(review), ctx))).toHaveLength(1);
		expect(namedRows(reviewBlocks(review, simOf(review), ctx, drawn))).toHaveLength(0);
		// One row in all — and it still says it is the first time.
		expect(namedRows([...call, ...reviewBlocks(review, simOf(review), ctx, drawn)])).toHaveLength(
			1
		);
		expect(recipientBadges(review, ctx)[TO.toLowerCase()]?.text).toBe(signing.tagFirstTime);
	});
});
