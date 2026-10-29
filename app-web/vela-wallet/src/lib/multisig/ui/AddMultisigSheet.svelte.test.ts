/**
 * Adding a multisig (r4b-8, r4D-4, r4b-13): whose network nothing says, the
 * sheet asks — the likely networks first, in view — and a pick only picks:
 * the person's Add adds. An invite opens the sheet with what it carries and
 * adds nothing on its own; its network is the one used. A damaged invite
 * says so the moment it opens.
 */
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { MultisigMessages } from '../messages';

const SAFE = '0x013124f0f8b38d42a2e192bcc26337ee9d7ffbf1';
const calls = vi.hoisted(() => [] as (number | undefined)[]);
const used = vi.hoisted(() => [] as number[]);
vi.mock('../core/accounts', () => ({
	addMultisig: async (address: string, _ids: number[], network?: number) => {
		calls.push(network);
		return network === undefined
			? { ok: false, reason: 'chooseNetwork' }
			: { ok: true, account: { address } };
	},
	importExported: async () => ({ ok: true })
}));
vi.mock('$lib/services/networks', () => ({
	spaceNetworksSync: () => [
		{ chainId: 11155111, displayName: 'Sepolia' },
		{ chainId: 84532, displayName: 'Base Sepolia' },
		{ chainId: 5042002, displayName: 'Arc Testnet' }
	],
	chainName: (id: number) => `Chain ${id}`,
	networkForChainId: (id: number) =>
		[11155111, 84532, 5042002].includes(id) ? { chainId: id } : null,
	networkRank: (id: number) => [11155111, 84532, 5042002].indexOf(id)
}));
vi.mock('$lib/core/kernels', () => ({ addressEntry: () => 'valid' }));
vi.mock('$lib/wallet/core/balance.svelte', () => ({ balance: { view: { tokens: [] } } }));
vi.mock('$app/navigation', () => ({ goto: async () => {} }));
vi.mock('../core/activity.svelte', () => ({
	indexName: async () => null,
	missingTitle: (_m: unknown, id: number) => `chain ${id}`
}));
vi.mock('../core/store.svelte', () => ({
	multisig: {
		accounts: [],
		find: () => undefined,
		useChain: (_a: string, id: number) => void used.push(id)
	}
}));

const { default: AddMultisigSheet } = await import('./AddMultisigSheet.svelte');

const m = {
	add: {
		title: 'Add a multisig',
		intro: 'intro',
		placeholder: 'address',
		add: 'Add',
		notFound: 'nf',
		unavailable: 'un',
		unsupported: 'us',
		notMultisig: 'nm',
		chooseNetwork: 'Choose its network:',
		badInvite: 'This invite link is damaged',
		usedOn: 'Used on {{networks}}',
		networkMissing: 'Missing {{network}}',
		moreNetworks: 'More networks',
		invitedMembers: 'Members:',
		kindPasskey: 'Vela account',
		kindWallet: 'Outside wallet',
		labelsNote: 'labels'
	},
	action: { network: 'Network' },
	proposal: { addNetwork: 'Add network' },
	send: { invalidAddress: 'bad', invalidChecksum: 'checksum' },
	service: { customNote: '({{host}})' },
	import: { or: 'or', body: 'b', choose: 'c' }
} as unknown as MultisigMessages;

const settle = async () => {
	for (let i = 0; i < 4; i += 1) {
		await new Promise((r) => setTimeout(r, 0));
		await tick();
	}
};
const root = () => document.body;
const addButton = () =>
	[...root().querySelectorAll<HTMLButtonElement>('button')].find(
		(b) => b.textContent?.trim() === 'Add'
	)!;
const radios = () =>
	[...root().querySelectorAll<HTMLButtonElement>('[role="radio"]')].map((r) =>
		r.textContent?.trim()
	);

beforeEach(() => {
	calls.length = 0;
	used.length = 0;
});

describe('a multisig whose network nothing says', () => {
	it('is asked about, and added on the person’s Add — never on the pick', async () => {
		const added: string[] = [];
		const screen = render(AddMultisigSheet, {
			props: {
				m,
				wide: true,
				closeLabel: 'Close',
				onclose: () => {},
				onadded: (a: string) => added.push(a)
			}
		});
		await tick();
		const input = screen.container.querySelector<HTMLInputElement>('input.mono')!;
		input.value = SAFE;
		input.dispatchEvent(new Event('input', { bubbles: true }));
		addButton().click();
		await settle();
		expect(root().textContent).toContain('Choose its network:');
		expect(added).toEqual([]);
		// Every network in view, no menu to open; nothing picked, nothing to add.
		expect(radios()).toEqual(['Sepolia', 'Base Sepolia', 'Arc Testnet']);
		expect(addButton().disabled).toBe(true);

		root().querySelector<HTMLButtonElement>('[role="radio"][data-chain="5042002"]')?.click();
		await settle();
		// Picked: nothing added yet (r4b-8).
		expect(calls).toEqual([undefined]);
		expect(added).toEqual([]);
		addButton().click();
		await settle();
		expect(calls).toEqual([undefined, 5042002]);
		expect(added).toEqual([SAFE]);
		await screen.unmount();
	});
});

describe('an invite (r4D-4)', () => {
	it('opens with its network and members, adds nothing until Add, then adds on its network', async () => {
		const added: string[] = [];
		const screen = render(AddMultisigSheet, {
			props: {
				m,
				prefill: SAFE,
				link: {
					multisig: SAFE,
					chainIds: [5042002],
					members: [
						{ name: 'Alice', kind: 'passkey' },
						{ name: 'Bob', kind: 'wallet' }
					]
				},
				wide: true,
				closeLabel: 'Close',
				onclose: () => {},
				onadded: (a: string) => added.push(a)
			}
		});
		await settle();
		const summary = root().querySelector('[data-testid="invite-summary"]')?.textContent ?? '';
		expect(summary).toContain('Used on Chain 5042002');
		expect(summary).toContain('Alice');
		expect(summary).toContain('Outside wallet');
		// r4b-8: opening it adds nothing.
		expect(calls).toEqual([]);
		addButton().click();
		await settle();
		// Its network is the one used: never a guess, never the person's chore.
		expect(calls).toEqual([5042002]);
		expect(used).toEqual([5042002]);
		expect(added).toEqual([SAFE]);
		await screen.unmount();
	});

	it('says a damaged invite is damaged the moment it opens (r4b-13)', async () => {
		const screen = render(AddMultisigSheet, {
			props: {
				m,
				prefill: '0x0131',
				broken: true,
				wide: true,
				closeLabel: 'Close',
				onclose: () => {},
				onadded: () => {}
			}
		});
		await tick();
		expect(root().querySelector('[role="alert"]')?.textContent).toContain(
			'This invite link is damaged'
		);
		expect(calls).toEqual([]);
		await screen.unmount();
	});
});
