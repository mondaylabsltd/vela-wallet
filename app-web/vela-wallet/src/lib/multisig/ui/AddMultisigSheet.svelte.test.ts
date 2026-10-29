/**
 * Adding a multisig whose network nothing says: the sheet asks which one —
 * it is not guessed — and adds it on the person's pick. And who can act for
 * it (§3.5): a refused genesis is said with why and cannot be added; a
 * high-risk one is shown and added only on the person's explicit tap.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { MultisigMessages } from '../messages';

const REFUSED = '0x2f53cc750594bb97c21bf3c18a4837bb5ad911f6';
const RISKY = '0x0cb9aef9713c4a2ea2995fca68d7355ce5e27d83';
const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';
const calls = vi.hoisted(() => [] as (number | undefined)[]);
const accepted = vi.hoisted(() => [] as boolean[]);
vi.mock('../core/accounts', () => ({
	addMultisig: async (
		address: string,
		_ids: number[],
		network?: number,
		options: { acceptRisks?: boolean } = {}
	) => {
		if (address === '0x2f53cc750594bb97c21bf3c18a4837bb5ad911f6') {
			return {
				ok: false,
				reason: 'refused',
				findings: [
					{
						from: 'genesis',
						chainId: 100,
						reason: { kind: 'module', address: '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1' }
					}
				]
			};
		}
		if (address === '0x0cb9aef9713c4a2ea2995fca68d7355ce5e27d83') {
			accepted.push(!!options.acceptRisks);
			return options.acceptRisks
				? { ok: true, account: { address } }
				: {
						ok: false,
						reason: 'highRisk',
						findings: [
							{
								from: 'chain',
								chainId: 100,
								risk: { kind: 'module', address: '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1' }
							}
						],
						account: { address }
					};
		}
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
	import: { or: 'or', body: 'b', choose: 'c' },
	authority: {
		refusedTitle: 'Vela won’t use this multisig',
		refusedBody: 'refused body',
		module: 'Its setup turns on a module: {{address}}',
		riskTitle: 'Something besides its members can act',
		riskBody: 'risk body',
		riskModule: 'On {{network}}, a module: {{address}}',
		riskConfirm: 'I understand — add it anyway'
	}
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

describe('who can act for the multisig (§3.5)', () => {
	const open = async (address: string) => {
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
		input.value = address;
		input.dispatchEvent(new Event('input', { bubbles: true }));
		const button = (label: string) =>
			[...document.body.querySelectorAll<HTMLButtonElement>('button')].find(
				(b) => b.textContent?.trim() === label
			);
		button('Add')!.click();
		await settle();
		return { added, button, screen };
	};

	it('a refused genesis: said with why, and nothing can add it', async () => {
		const { added, button, screen } = await open(REFUSED);
		const notice = screen.container.querySelector('[data-testid="authority-refused"]');
		expect(notice?.textContent).toContain('Vela won’t use this multisig');
		expect(notice?.textContent).toContain(`Its setup turns on a module: ${MODULE}`);
		expect(button('Add')?.disabled).toBe(true);
		expect(button('I understand — add it anyway')).toBeUndefined();
		expect(added).toEqual([]);
	});

	it('high risk: shown in full, added only on the explicit tap', async () => {
		const { added, button, screen } = await open(RISKY);
		const notice = screen.container.querySelector('[data-testid="authority-risk"]');
		expect(notice?.textContent).toContain(`a module: ${MODULE}`);
		expect(added).toEqual([]);
		button('I understand — add it anyway')!.click();
		await settle();
		expect(accepted).toEqual([false, true]);
		expect(added).toEqual([RISKY]);
	});
});

/** r4b-13: what a link's own lookup found is said at once — never looked up twice. */
describe('a link the wallet already looked up, or a damaged one', () => {
	it('says what was found at once, and asks the service nothing until Add', async () => {
		calls.length = 0;
		const screen = render(AddMultisigSheet, {
			props: {
				m,
				prefill: '0x013124f0f8b38d42a2e192bcc26337ee9d7ffbf1',
				prefillFailure: 'nf',
				wide: true,
				closeLabel: 'Close',
				onclose: () => {},
				onadded: () => {}
			}
		});
		await settle();
		expect(document.body.textContent).toContain('nf');
		expect(calls).toEqual([]);
		await screen.unmount();
	});
});
