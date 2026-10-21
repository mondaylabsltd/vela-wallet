/**
 * Network changes through the intents (spec 103): the network core's own
 * events, and — for an RPC its save gate refuses — the field put back to what
 * is in effect, so the person's card keeps no refused draft.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { NetEvent } from '$lib/core/generated/NetEvent';

const ledger = {
	events: [] as NetEvent[],
	/** What the gate decides for an RPC on blur. */
	gate: 'save' as 'save' | 'mismatch',
	saved: 'https://rpc.arc.example',
	draft: null as string | null,
	mismatch: false
};

vi.mock('$lib/settings/core/network-admin.svelte', () => ({
	networkAdmin: {
		boot: async () => {},
		dispatch: (event: NetEvent) => {
			ledger.events.push(event);
			if (event.type === 'override_field_edited') {
				ledger.draft = event.value;
				ledger.mismatch = false; // the next keystroke clears the refusal
			}
			if (event.type === 'override_blurred' && ledger.draft !== null) {
				if (ledger.gate === 'mismatch') ledger.mismatch = true;
				else ledger.saved = ledger.draft;
			}
		},
		get view() {
			return {
				networks: [
					{
						chain_id: 5042002,
						rpc_url: ledger.draft ?? ledger.saved,
						explorer_url: 'https://scan.arc.example',
						rpc_chain_mismatch: ledger.mismatch ? {} : null,
						rpc_save_deferred: false
					}
				]
			};
		}
	}
}));

const { setNetworkEndpoint } = await import('./networks');

beforeEach(() => {
	Object.assign(ledger, {
		events: [],
		gate: 'save',
		saved: 'https://rpc.arc.example',
		draft: null,
		mismatch: false
	});
});

describe('changing a network’s RPC (spec 103)', () => {
	it('opens the card, edits the field and leaves it — which saves', async () => {
		expect(await setNetworkEndpoint(5042002, 'rpc', 'https://other.arc.example')).toBeNull();
		expect(ledger.events.map((event) => event.type)).toEqual([
			'override_expanded',
			'override_field_edited',
			'override_blurred'
		]);
		expect(ledger.saved).toBe('https://other.arc.example');
	});

	it('puts the field back when the gate refuses an RPC for another chain', async () => {
		ledger.gate = 'mismatch';
		const result = await setNetworkEndpoint(5042002, 'rpc', 'https://mainnet.example');
		expect(result).toMatchObject({ code: 'refused' });
		expect(ledger.saved, 'nothing written').toBe('https://rpc.arc.example');
		expect(ledger.draft, 'the card shows what is in effect again').toBe('https://rpc.arc.example');
		expect(ledger.mismatch).toBe(false);
	});
});
