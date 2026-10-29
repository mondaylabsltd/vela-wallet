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

vi.mock('$lib/onboarding/core/storage', () => ({
	loadAccounts: () => [
		{ address: ME, name: 'Mine' },
		{ address: TWO, name: 'Two' }
	]
}));
vi.mock('$lib/services/networks', () => ({
	spaceNetworksSync: () => [{ chainId: 100, displayName: 'Gnosis' }]
}));
vi.mock('../core/accounts', () => ({
	newSaltNonce: () => '1',
	draftGenesis: () => ({ address: SAFE }),
	createMultisig: async () => ({ address: SAFE })
}));
vi.mock('../core/owners', () => ({
	localOwners: (owners: string[]) =>
		owners.map((address) => ({ address, name: address.slice(0, 6), account: { address } }))
}));
vi.mock('../core/display', () => ({
	memberLabel: (a: string) => a.slice(0, 6),
	shortenAddress: (a: string) => a.slice(0, 8)
}));
vi.mock('../core/kernel', () => ({
	sameAddress: (a: string, b: string) => a.toLowerCase() === b.toLowerCase(),
	validName: () => true
}));
vi.mock('../core/members', () => ({ memberProblem: () => null }));
vi.mock('$lib/wallet/identicon', () => ({
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

afterEach(() => identiconViewer.close());

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
