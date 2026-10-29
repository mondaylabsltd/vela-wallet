/**
 * Adding a multisig whose network nothing says: the sheet asks which one —
 * it is not guessed — and adds it on the person's pick.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { MultisigMessages } from '../messages';

const calls = vi.hoisted(() => [] as (number | undefined)[]);
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
		{ chainId: 5042002, displayName: 'Arc Testnet' }
	],
	chainName: (id: number) => `Chain ${id}`
}));
vi.mock('$lib/core/kernels', () => ({ addressEntry: () => 'valid' }));

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
		chooseNetwork: 'Choose its network:'
	},
	action: { network: 'Network' },
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

describe('a multisig whose network nothing says', () => {
	it('is asked about, and added on the person’s pick', async () => {
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
		const root = document.body;
		const input = screen.container.querySelector<HTMLInputElement>('input.mono')!;
		input.value = '0x013124f0f8b38d42a2e192bcc26337ee9d7ffbf1';
		input.dispatchEvent(new Event('input', { bubbles: true }));
		const addButton = () =>
			[...root.querySelectorAll<HTMLButtonElement>('button')].find(
				(b) => b.textContent?.trim() === 'Add'
			)!;
		addButton().click();
		await settle();
		expect(root.textContent).toContain('Choose its network:');
		expect(added).toEqual([]);
		// Nothing picked: nothing to add.
		expect(addButton().disabled).toBe(true);

		[...root.querySelectorAll<HTMLElement>('button')]
			.find((b) => b.textContent?.trim() === 'Network')
			?.click();
		await settle();
		[...root.querySelectorAll<HTMLElement>('[role="option"], button')]
			.find((b) => b.textContent?.trim() === 'Arc Testnet')
			?.click();
		await settle();
		addButton().click();
		await settle();
		expect(calls).toEqual([undefined, 5042002]);
		expect(added).toEqual(['0x013124f0f8b38d42a2e192bcc26337ee9d7ffbf1']);
	});
});
