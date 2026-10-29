/**
 * A member typed as a name: resolved forward by the real core's walk to the
 * address its resolver gives (shown before it is used), "not found" when it
 * has none, "unavailable" when nobody answers — never an address made up.
 * The RPC is a script: each `eth_call` the core asks for is answered by id.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';

const RESOLVER = '0x231b0ee14048e9dccd1d247744d114a4eb5e8e63';
const THEIRS = '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045';
const word = (address: string) => `0x${address.slice(2).toLowerCase().padStart(64, '0')}`;

const seams = vi.hoisted(() => ({
	answers: {} as Record<string, string | null>,
	asked: [] as { chain_id: number; to: string }[]
}));
vi.mock('$lib/services/networks', () => ({ networkForChainId: () => ({}) }));
vi.mock('$lib/services/core-walk', () => ({
	runWalk: async (step: (answers: string) => string) => {
		const answers: unknown[] = [];
		for (let round = 0; round < 16; round++) {
			const next = JSON.parse(step(JSON.stringify(answers)));
			if (next.type === 'done') return next;
			for (const r of next.requests) {
				seams.asked.push(r);
				const body = seams.answers[r.id];
				answers.push(
					body == null
						? { id: r.id, outcome: 'failed', body: null }
						: { id: r.id, outcome: 'ok', body }
				);
			}
		}
		return null;
	}
}));

const { looksLikeName, resolveTypedName } = await import('./member-entry');

describe('a member typed as a name', () => {
	it('is told apart from an address', () => {
		expect(looksLikeName('vitalik.eth')).toBe(true);
		expect(looksLikeName('bob.base.eth')).toBe(true);
		expect(looksLikeName(THEIRS)).toBe(false);
		expect(looksLikeName('vitalik')).toBe(false);
	});

	it('resolves to the address its resolver gives, on Ethereum (or Base for a Basename)', async () => {
		seams.answers = { 'resolver:0': word(RESOLVER), addr: word(THEIRS) };
		seams.asked = [];
		expect(await resolveTypedName('Vitalik.eth')).toEqual({
			state: 'resolved',
			address: THEIRS,
			name: 'vitalik.eth'
		});
		expect(seams.asked[0].chain_id).toBe(1);
		seams.asked = [];
		await resolveTypedName('bob.base.eth');
		expect(seams.asked[0].chain_id).toBe(8453);
	});

	it('says not found or unavailable, never an address', async () => {
		seams.answers = { 'resolver:0': word(RESOLVER), addr: `0x${'0'.repeat(64)}` };
		expect(await resolveTypedName('nobody.eth')).toEqual({ state: 'not_found' });
		seams.answers = {};
		expect(await resolveTypedName('vitalik.eth')).toEqual({ state: 'unavailable' });
	});
});
