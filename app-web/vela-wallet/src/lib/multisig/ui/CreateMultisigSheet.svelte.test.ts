/**
 * Creating a multisig: every artwork drawn from an address opens the
 * identicon viewer (founder call, 2026-09-05) — the chosen members, the
 * review's address and the created one — except the picker's, whose whole
 * row is a checkbox's label: a button inside it would be nested interactive
 * content and steal the row's tap, so it stays a picture.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { identiconViewer } from '$lib/wallet/identicon-viewer.svelte';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const ME = '0x00000000000000000000000000000000000000aa';
const TWO = '0x00000000000000000000000000000000000000bb';
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';

vi.mock('$lib/onboarding/core/storage', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/onboarding/core/storage')>()),
	loadAccounts: () => [
		{ address: ME, name: 'Mine' },
		{ address: TWO, name: 'Two' }
	]
}));
const seams = vi.hoisted(() => ({
	networks: [{ chainId: 100, displayName: 'Gnosis' }] as { chainId: number; displayName: string }[],
	/** The core's verdict per chain on the Safe v1.4.1 contracts. */
	contracts: {} as Record<number, 'present' | 'missing' | 'unknown'>,
	created: [] as { chainId: number }[]
}));
vi.mock('$lib/services/networks', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/networks')>()),
	spaceNetworksSync: () => seams.networks
}));
vi.mock('../core/chain', async (importOriginal) => ({
	...(await importOriginal<typeof import('../core/chain')>()),
	safeContractsOn: async (chainId: number) => {
		const kind = seams.contracts[chainId] ?? 'present';
		return kind === 'missing' ? { kind, names: ['Safe L2'] } : { kind };
	}
}));
vi.mock('../core/accounts', async (importOriginal) => ({
	...(await importOriginal<typeof import('../core/accounts')>()),
	newSaltNonce: () => '1',
	draftGenesis: () => ({ address: SAFE }),
	createMultisig: async (choice: { chainId: number }) => {
		seams.created.push(choice);
		return { address: SAFE };
	}
}));
vi.mock('../core/owners', async (importOriginal) => ({
	...(await importOriginal<typeof import('../core/owners')>()),
	localOwners: (owners: string[]) =>
		owners.map((address) => ({ address, name: address.slice(0, 6), account: { address } }))
}));
vi.mock('../core/display', async (importOriginal) => ({
	...(await importOriginal<typeof import('../core/display')>()),
	memberLabel: (a: string) => a.slice(0, 6),
	shortenAddress: (a: string) => a.slice(0, 8)
}));
vi.mock('../core/kernel', async (importOriginal) => ({
	...(await importOriginal<typeof import('../core/kernel')>()),
	sameAddress: (a: string, b: string) => a.toLowerCase() === b.toLowerCase(),
	validName: () => true
}));
vi.mock('../core/members', async (importOriginal) => ({
	...(await importOriginal<typeof import('../core/members')>()),
	memberProblem: () => null
}));
vi.mock('$lib/wallet/identicon', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/wallet/identicon')>()),
	identiconSvgForClient: (a: string) => `<svg data-seed="${a}"></svg>`
}));

const { default: CreateMultisigSheet } = await import('./CreateMultisigSheet.svelte');

function words(): MultisigMessages {
	const out: Record<string, Record<string, string>> = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const key of Object.keys(keys)) out[group][key] = `${group}.${key}`;
	}
	out.panel.title = '{{m}} of {{n}}';
	out.create.thresholdValue = '{{m}} of {{n}}';
	return out as unknown as MultisigMessages;
}

const settle = async () => {
	for (let i = 0; i < 5; i += 1) {
		await tick();
		await new Promise((r) => setTimeout(r, 0));
	}
};
const press = (label: string) =>
	[...document.body.querySelectorAll('button')]
		.find((b) => b.textContent?.trim() === label)
		?.click();

afterEach(() => {
	identiconViewer.close();
	seams.networks = [{ chainId: 100, displayName: 'Gnosis' }];
	seams.contracts = {};
	seams.created = [];
});

describe('creating a multisig', () => {
	it('opens the viewer from every artwork but the picker row, which stays a picture', async () => {
		const screen = render(CreateMultisigSheet, {
			props: {
				m: words(),
				locale: 'en',
				current: ME,
				wide: true,
				closeLabel: 'Close',
				onclose: () => {},
				oncreated: () => {}
			}
		});
		await settle();

		// The picker: each row is a label around a checkbox. Its artwork is a
		// picture — no button, nothing interactive inside the label but the box.
		const labels = [...document.body.querySelectorAll('.choices label')];
		expect(labels).toHaveLength(2);
		for (const label of labels) {
			expect(label.querySelector('button')).toBeNull();
			expect(label.querySelector('.identicon')).not.toBeNull();
		}

		// Pick the second account and name it.
		labels[1].querySelector('input')?.click();
		const name = document.body.querySelector<HTMLInputElement>('.name-field');
		if (name) {
			name.value = 'Team';
			name.dispatchEvent(new Event('input', { bubbles: true }));
		}
		await settle();

		// The members chosen: each artwork opens the viewer, the remove
		// button stays beside it.
		const owners = [...document.body.querySelectorAll('.owners li')];
		expect(owners).toHaveLength(2);
		owners[1].querySelector<HTMLButtonElement>('button.identicon')?.click();
		expect(identiconViewer.current?.address).toBe(TWO);
		expect(document.body.querySelector('button button')).toBeNull();

		// Review: the address it will have.
		press('create.next');
		await settle();
		press('create.next');
		await settle();
		document.body.querySelector<HTMLButtonElement>('.hero button.identicon')?.click();
		expect(identiconViewer.current?.address).toBe(SAFE);
		identiconViewer.close();

		// Created: the same address, the same viewer.
		press('create.create');
		await settle();
		expect(document.body.textContent).toContain('create.created');
		document.body.querySelector<HTMLButtonElement>('.hero button.identicon')?.click();
		expect(identiconViewer.current?.address).toBe(SAFE);
		await screen.unmount();
	});
});

/**
 * The registry is an index, not a home: the multisig's record goes to the
 * index chain whatever network is picked, so every network the person has is
 * offered — not only those the service's health table lists — but one the
 * core finds without the Safe v1.4.1 contracts.
 */
describe("the create sheet's networks", () => {
	it('offers every network with the Safe contracts, whatever the service reads', async () => {
		seams.networks = [
			{ chainId: 100, displayName: 'Gnosis' },
			{ chainId: 42161, displayName: 'Arbitrum' },
			{ chainId: 8453, displayName: 'Base' },
			{ chainId: 999, displayName: 'No Safe' }
		];
		// Base did not answer: nothing says it lacks them, so it stays offered.
		seams.contracts = { 8453: 'unknown', 999: 'missing' };
		// The service reads Gnosis only; nothing here asks it.
		const fetches: string[] = [];
		const realFetch = globalThis.fetch;
		globalThis.fetch = (async (input: RequestInfo | URL) => {
			fetches.push(String(input));
			return new Response(
				JSON.stringify({ service: 'safe-genesis-registry', chains: [{ chainId: 100 }] })
			);
		}) as typeof fetch;
		try {
			const screen = render(CreateMultisigSheet, {
				props: {
					m: words(),
					locale: 'en',
					current: ME,
					wide: true,
					closeLabel: 'Close',
					onclose: () => {},
					oncreated: () => {}
				}
			});
			await settle();
			document.body.querySelectorAll<HTMLInputElement>('.choices input')[1]?.click();
			const name = document.body.querySelector<HTMLInputElement>('.name-field');
			if (name) {
				name.value = 'Team';
				name.dispatchEvent(new Event('input', { bubbles: true }));
			}
			await settle();
			press('create.next');
			await settle();
			press('create.next');
			await settle();
			document.body.querySelector<HTMLButtonElement>('.select [aria-haspopup="listbox"]')?.click();
			await settle();
			const rows = [...document.body.querySelectorAll('[role="listbox"] [role="option"]')]
				.map((r) => r.textContent?.trim())
				.filter(Boolean);
			expect(rows).toEqual(['Gnosis', 'Arbitrum', 'Base']);
			expect(document.body.textContent).not.toContain('create.noSafeNetworks');
			// Arbitrum, a network the service does not read, is picked and created on.
			[...document.body.querySelectorAll<HTMLElement>('[role="listbox"] [role="option"]')]
				.find((r) => r.textContent?.trim() === 'Arbitrum')
				?.click();
			await settle();
			press('create.create');
			await settle();
			expect(seams.created).toEqual([expect.objectContaining({ chainId: 42161 })]);
			expect(fetches.filter((u) => u.includes('/api/health'))).toEqual([]);
			await screen.unmount();
		} finally {
			globalThis.fetch = realFetch;
		}
	});

	it('says so when none of the networks has them, and creates nothing', async () => {
		seams.networks = [{ chainId: 999, displayName: 'No Safe' }];
		seams.contracts = { 999: 'missing' };
		const screen = render(CreateMultisigSheet, {
			props: {
				m: words(),
				locale: 'en',
				current: ME,
				wide: true,
				closeLabel: 'Close',
				onclose: () => {},
				oncreated: () => {}
			}
		});
		await settle();
		document.body.querySelectorAll<HTMLInputElement>('.choices input')[1]?.click();
		const name = document.body.querySelector<HTMLInputElement>('.name-field');
		if (name) {
			name.value = 'Team';
			name.dispatchEvent(new Event('input', { bubbles: true }));
		}
		await settle();
		press('create.next');
		await settle();
		press('create.next');
		await settle();
		expect(document.body.textContent).toContain('create.noSafeNetworks');
		press('create.create');
		await settle();
		expect(seams.created).toEqual([]);
		await screen.unmount();
	});
});
