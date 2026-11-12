/**
 * R22 (docs/multisig-machines.md §36): a plain verdict stands on two
 * independent sources — the cross-shell vector
 * (`rust/crates/vela-core/tests/vectors/rules/signing-truth.json`, `sim`)
 * replayed through this shell's binding (`simOutcome`) and the envelope it
 * builds (`rpcCorroboratedEnvelope`); and what the sheet then says: the
 * unconfirmed caution with where a trusted node is set, and "Checked by your
 * own node only" under an own-node check.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import vectors from '../../../../../rust/crates/vela-core/tests/vectors/rules/signing-truth.json';
import { judgedByFreshMachine, rpcCorroboratedEnvelope, simOutcome } from '$lib/core/kernels';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { landedVerdict } from './verdict';
import { ran, nativeLog } from './sim-replies';
import { ownNodeAlone, singleNode, witnessed } from './sim-witnessed';
import { corroboratedEnvelope } from '$lib/services/corroborated-read';

const USER = vectors.user;

describe('signing truth: the shared `sim` vectors (simOutcome over the corroborated envelope)', () => {
	for (const c of vectors.sim) {
		it(c.name, () => {
			const verdict = simOutcome(USER, JSON.stringify(c.envelope));
			expect(verdict).not.toBeNull();
			expect(verdict!.kind).toBe(c.expect.kind);
			expect(verdict!.source ?? null).toBe(c.expect.source);
			expect(verdict!.notice_key).toBe(c.expect.notice_key);
			expect(verdict!.deltas.map((d) => d.delta)).toEqual(c.expect.deltas);
			const sourceKey = (c.expect as { source_key?: string }).source_key ?? null;
			expect(verdict!.source_key ?? null).toBe(sourceKey);
		});
	}
});

describe('the envelope this shell hands the core (rpcCorroboratedEnvelope)', () => {
	const SITE = '0x76875e38fc6bc2dedcaed807ce00782db5c0d141';
	const body = (reply: string) => ({ jsonrpc: '2.0', id: 1, ...JSON.parse(reply) });

	it('a reply without a witness is unconfirmed; with one that agrees, a check', () => {
		const reply = ran(nativeLog(USER, SITE, 1_000n));
		expect(simOutcome(USER, singleNode(reply))?.kind).toBe('unconfirmed');
		expect(simOutcome(USER, witnessed(reply))?.kind).toBe('deltas');
		expect(simOutcome(USER, witnessed(reply))?.source).toBe('corroborated');
		expect(simOutcome(USER, ownNodeAlone(reply))?.source).toBe('own_node');
	});

	it('the pool giving up is unreachable; a pool call is enveloped with both bodies', async () => {
		expect(rpcCorroboratedEnvelope(null, null, false)).toBe('{"unreachable":true}');
		const reply = ran(nativeLog(USER, SITE, 5n));
		const both = await corroboratedEnvelope('eth_simulateV1', [], 1, async () => ({
			body: body(reply),
			witnessBody: body(reply),
			ownNode: false,
			soleProvider: false
		}));
		expect(simOutcome(USER, both)?.kind).toBe('deltas');
		const none = await corroboratedEnvelope('eth_simulateV1', [], 1, async () => {
			throw new Error('All RPC endpoints failed');
		});
		expect(JSON.parse(none)).toEqual({ unreachable: true });
		expect(simOutcome(USER, none)?.kind).toBe('unreachable');
	});
});

describe('what the sheet says of it (landedVerdict)', () => {
	const m = resolveSigningMessages('en');
	const SITE = '0x76875e38fc6bc2dedcaed807ce00782db5c0d141';
	const ctx = {
		m,
		nativeSymbol: 'ETH',
		preset: 'comma_dot',
		ownNodeHref: '/en/settings?network=1'
	};

	it('unconfirmed: the core’s caution, "Check again", and where a trusted node is set', () => {
		const verdict = simOutcome(USER, singleNode(ran(nativeLog(USER, SITE, 1n))))!;
		expect(verdict.recheck).toBe(true);
		const said = landedVerdict({ verdict, judged: null }, ctx);
		expect(said).toEqual({
			kind: 'notice',
			block: {
				kind: 'warning',
				tone: 'caution',
				text: m.simUnconfirmed,
				ownNode: { text: m.ownNodeHint, action: m.ownNodeAction, href: '/en/settings?network=1' }
			}
		});
		expect(m.simUnconfirmed).toMatch(/Only one node answered/);
		expect(m.ownNodeAction).toBe('Network settings');
	});

	it('disagreed: its own caution', () => {
		const verdict = simOutcome(USER, witnessed(ran(nativeLog(USER, SITE, 1_000_000n)), ran()))!;
		expect(verdict.notice_key).toBe('componentsUi.signing.simDisagreed');
		const said = landedVerdict({ verdict, judged: null }, ctx);
		expect(said.kind === 'notice' && said.block.text).toBe(m.simDisagreed);
	});

	it('own node: the check, and the quiet line under the balance changes', () => {
		const verdict = simOutcome(USER, ownNodeAlone(ran(nativeLog(USER, SITE, 10n ** 18n))))!;
		const judged = judgedByFreshMachine(USER, 1, verdict.deltas);
		const said = landedVerdict({ verdict, judged }, ctx);
		expect(said.kind).toBe('moves');
		expect(said.block).toMatchObject({ kind: 'balances', source: m.simOwnNode });
		expect(m.simOwnNode).toBe('Checked by your own node only');
		// Corroborated: no line about who checked.
		const two = simOutcome(USER, witnessed(ran(nativeLog(USER, SITE, 10n ** 18n))))!;
		const plain = landedVerdict(
			{ verdict: two, judged: judgedByFreshMachine(USER, 1, two.deltas) },
			ctx
		);
		expect(plain.block).not.toHaveProperty('source');
	});
});
