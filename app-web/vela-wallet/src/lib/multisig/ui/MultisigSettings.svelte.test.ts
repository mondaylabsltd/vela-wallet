/**
 * A multisig's members in Settings: each member's artwork opens the
 * identicon viewer on that member's address (founder call, 2026-09-05 —
 * every artwork drawn from an address, not just the header's). The owner
 * found these were pictures that did nothing.
 *
 * Its name in the registry (v4): renamed on chain only by a proposal of its
 * own, on a network that holds its record; and a multisig the service has no
 * record of is said as not shared, with "Share it again".
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { identiconViewer } from '$lib/wallet/identicon-viewer.svelte';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const ME = '0x00000000000000000000000000000000000000aa';
const OTHER = '0x00000000000000000000000000000000000000bb';
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';

const seams = vi.hoisted(() => ({
	shared: true,
	recordedOn: [100] as number[],
	networks: [100] as number[],
	renamed: [] as [string, number, string][],
	opened: [] as unknown[],
	sharedAgain: 0
}));
vi.mock('../core/proposals.svelte', () => ({
	proposals: { chains: [] },
	MailboxUnreadableError: class extends Error {},
	proposeRename: async (address: string, chainId: number, name: string) => {
		seams.renamed.push([address, chainId, name]);
		return { safeTxHash: '0xabc' };
	}
}));
vi.mock('../core/accounts', () => ({
	recordHomeChain: async () => 100,
	shareAgain: async () => {
		seams.sharedAgain += 1;
		return true;
	}
}));
vi.mock('../core/service', () => ({
	fetchGenesis: async () => ({ found: true, recordedOn: seams.recordedOn })
}));
vi.mock('$lib/services/networks', () => ({
	networkForChainId: (id: number) => (seams.networks.includes(id) ? { chainId: id } : null),
	chainName: (id: number) => `Chain ${id}`
}));
vi.mock('../core/display', () => ({ chainName: (id: number) => `Chain ${id}` }));
vi.mock('../core/kernel', () => ({
	validName: (n: string) => n.length > 0 && n.length <= 64 && n.trim() === n
}));
vi.mock('../core/ui.svelte', () => ({
	multisigUi: { open: (sheet: unknown) => seams.opened.push(sheet) }
}));
vi.mock('../core/backup', () => ({
	checkBackup: async () => 'unavailable',
	startBackup: async () => ({ kind: 'cancelled' })
}));
vi.mock('../core/owners', () => ({
	localOwners: (owners: string[]) =>
		owners
			.filter((o) => o.toLowerCase() === ME)
			.map((address) => ({ address, name: 'Mine', account: { address } }))
}));
vi.mock('../core/names.svelte', () => ({ names: { label: (a: string) => a.slice(0, 6) } }));
vi.mock('../core/store.svelte', () => ({
	isShared: () => seams.shared,
	multisig: { find: () => null, remove: () => {} }
}));
vi.mock('$lib/wallet/identicon', () => ({
	identiconSvgForClient: (a: string) => `<svg data-seed="${a}"></svg>`
}));

const { default: MultisigSettings } = await import('./MultisigSettings.svelte');

function words(): MultisigMessages {
	const out: Record<string, Record<string, string>> = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const key of Object.keys(keys)) out[group][key] = `${group}.${key}`;
	}
	out.panel.title = '{{m}} of {{n}}';
	return out as unknown as MultisigMessages;
}

afterEach(() => {
	identiconViewer.close();
	identiconViewer.openLabel = '';
});

describe('the members list (Settings → Account)', () => {
	it('opens the viewer on the member whose artwork is tapped', async () => {
		identiconViewer.openLabel = 'View identicon';
		const screen = render(MultisigSettings, {
			props: {
				m: words(),
				account: { address: SAFE, genesis: {}, owners: [ME, OTHER], threshold: 2 } as never,
				payer: null,
				cancelLabel: 'Cancel'
			}
		});
		await tick();
		const rows = [...screen.container.querySelectorAll('.members li')];
		expect(rows).toHaveLength(2);
		const art = rows.map((li) => li.querySelector<HTMLButtonElement>('button.identicon'));
		expect(art.every((b) => b !== null)).toBe(true);
		// Keyboard and screen reader: a button, named for what it opens.
		expect(art[1]?.getAttribute('aria-label')).toBe('View identicon');
		art[1]?.click();
		expect(identiconViewer.current).toEqual({
			address: OTHER,
			identiconSvg: `<svg data-seed="${OTHER}"></svg>`
		});
		art[0]?.click();
		expect(identiconViewer.current?.address).toBe(ME);
	});
});

describe('the name in the registry (v4)', () => {
	const props = (name = 'Team fund') => ({
		m: words(),
		account: {
			address: SAFE,
			genesis: { name },
			owners: [ME, OTHER],
			threshold: 2,
			chainIds: [100]
		} as never,
		payer: null,
		cancelLabel: 'Cancel'
	});

	afterEach(() => {
		seams.shared = true;
		seams.recordedOn = [100];
		seams.networks = [100];
		seams.renamed = [];
		seams.opened = [];
		seams.sharedAgain = 0;
	});

	it('is renamed by a proposal of its own, on the network holding its record', async () => {
		const screen = render(MultisigSettings, { props: props() });
		await expect.element(screen.getByTestId('registry-name')).toHaveTextContent('Team fund');
		await screen.getByRole('button', { name: 'settings.renameAction' }).click();
		const field = screen.getByRole('textbox', { name: 'create.name' });
		await expect.element(field).toHaveValue('Team fund');
		const propose = screen.getByRole('button', { name: 'send.propose' });
		// The same name: nothing to propose.
		await expect.element(propose).toBeDisabled();
		await field.fill('Team treasury');
		await propose.click();
		await vi.waitFor(() => expect(seams.renamed).toEqual([[SAFE, 100, 'Team treasury']]));
		// The proposal opens, for the members to approve; the name shown is
		// still the record's until it is executed.
		expect(seams.opened).toEqual([{ kind: 'proposal', chainId: 100, safeTxHash: '0xabc' }]);
		await expect.element(screen.getByTestId('registry-name')).toHaveTextContent('Team fund');
	});

	it('says it cannot be renamed here when no network of this wallet holds its record', async () => {
		seams.networks = [];
		const screen = render(MultisigSettings, { props: props() });
		await expect.element(screen.getByText('settings.renameUnavailable')).toBeVisible();
		expect(screen.getByRole('button', { name: 'settings.renameAction' }).elements()).toHaveLength(
			0
		);
	});

	it('a multisig the service has no record of is said as not shared, and shared again', async () => {
		seams.shared = false;
		seams.recordedOn = [];
		const screen = render(MultisigSettings, { props: props() });
		await expect.element(screen.getByText('panel.notShared')).toBeVisible();
		await expect.element(screen.getByText('settings.shareAgainNote')).toBeVisible();
		await screen.getByRole('button', { name: 'settings.shareAgain' }).click();
		await vi.waitFor(() => expect(seams.sharedAgain).toBe(1));
	});
});
