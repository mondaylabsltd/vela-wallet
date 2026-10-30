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
	sharedAgain: 0,
	/** A creation's proof is kept here: its record is on its way (r4a-12). */
	waiting: false,
	/** Whether it is deployed on the chain holding its record (this wallet's RPC). */
	deployed: true,
	/** Each network's members now, read through this wallet's RPC. */
	chains: [] as unknown[]
}));
vi.mock('../core/proposals.svelte', () => ({
	proposals: {
		get chains() {
			return seams.chains;
		},
		items: []
	},
	MailboxUnreadableError: class extends Error {},
	proposeMemberChange: async () => ({ safeTxHash: '0xabc' }),
	membersAnywhere: (a: { owners: string[] }) => a.owners,
	proposeRename: async (address: string, chainId: number, name: string) => {
		seams.renamed.push([address, chainId, name]);
		return { safeTxHash: '0xabc' };
	}
}));
// Changing the members is its own component's (MemberChanges.svelte.test.ts).
vi.mock('../core/member-entry', () => ({
	looksLikeName: () => false,
	resolveTypedName: async () => ({ state: 'unavailable' })
}));
vi.mock('$lib/contacts/core/contacts-book.svelte', () => ({ contactsBook: { view: null } }));
vi.mock('../core/accounts', () => ({
	recordHomeChain: async () => 100,
	recordWaiting: () => seams.waiting,
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
vi.mock('../core/display', () => ({
	chainName: (id: number) => `Chain ${id}`,
	shortenAddress: (a: string) => `${a.slice(0, 8)}…${a.slice(-6)}`,
	ruleTitle: (m: MultisigMessages, r: { m: number; n: number }) =>
		r.n === 1
			? m.panel.titleSolo
			: m.panel.title.replace('{{m}}', `${r.m}`).replace('{{n}}', `${r.n}`)
}));
vi.mock('../core/chain', () => ({ hasCode: async () => seams.deployed }));
// A rename's signature passes the gate in `proposeRename` (stood in above).
vi.mock('../core/gate.svelte', () => ({
	SigningRefusedError: class extends Error {},
	SigningUncheckedError: class extends Error {},
	SigningBlockedError: class extends Error {}
}));
vi.mock('../core/kernel', () => {
	const same = (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase();
	return {
		validName: (n: string) => n.length > 0 && n.length <= 64 && n.trim() === n,
		validOwner: () => true,
		MEMBER_CHANGES: ['addOwner', 'removeOwner', 'swapOwner', 'changeThreshold'],
		memberChangeTx: () => ({ kind: 'refused', refusal: { kind: 'unchanged' } }),
		sameAddress: same,
		// The core's rule, as it is.
		memberDrift: (
			was: { owners: string[]; threshold: number },
			is: { owners: string[]; threshold: number }
		) => {
			const added = is.owners.filter((o) => !was.owners.some((w) => same(w, o)));
			const removed = was.owners.filter((o) => !is.owners.some((w) => same(w, o)));
			return {
				same: added.length === 0 && removed.length === 0 && was.threshold === is.threshold,
				added,
				removed,
				thresholdWas: was.threshold,
				thresholdIs: is.threshold
			};
		}
	};
});
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
vi.mock('../core/names.svelte', () => ({
	names: { label: (a: string) => a.slice(0, 6) },
	keptMember: () => ({})
}));
// r7: Add member pre-fills a member's kind from what the wallet can tell — nothing, here.
vi.mock('../core/signers.svelte', () => ({ signers: { evidence: () => null } }));
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
	seams.chains = [];
	seams.deployed = true;
	seams.waiting = false;
	seams.shared = true;
	seams.recordedOn = [100];
	seams.networks = [100];
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

	it('offers nothing that deploys when it is not deployed where its record is', async () => {
		seams.deployed = false;
		const screen = render(MultisigSettings, { props: props() });
		await expect
			.element(screen.getByTestId('rename-not-deployed'))
			.toHaveTextContent('settings.renameNotDeployed');
		expect(screen.getByRole('button', { name: 'settings.renameAction' }).elements()).toHaveLength(
			0
		);
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

	it('r4a-12: a creation still sharing says so — no "older registry", no "not on a network"', async () => {
		seams.shared = false;
		seams.recordedOn = [];
		seams.waiting = true;
		const screen = render(MultisigSettings, { props: props() });
		await expect.element(screen.getByText('create.savedLater')).toBeVisible();
		expect(screen.getByText('settings.shareAgainNote').elements()).toHaveLength(0);
		await expect
			.element(screen.getByTestId('rename-blocked'))
			.toHaveTextContent('settings.renameNotRecorded');
		expect(screen.getByText('settings.renameUnavailable').elements()).toHaveLength(0);
	});
});

/**
 * Owners are each network's (owner, 2026-10-29): the original members, and
 * per network it is deployed on the members there now, with what changed;
 * a network it is not deployed on starts with the original members.
 */
describe('original and current members', () => {
	const NEW = '0x00000000000000000000000000000000000000cc';
	const account = { address: SAFE, genesis: {}, owners: [ME, OTHER], threshold: 2, chainIds: [] };

	it('shows each deployed network beside the original members, marked added and removed', async () => {
		seams.chains = [
			{ chainId: 100, members: { deployed: true, owners: [ME, NEW], threshold: 1, nonce: 3n } },
			{ chainId: 8453, members: { deployed: false, owners: [ME, OTHER], threshold: 2, nonce: 0n } },
			{ chainId: 10, members: { deployed: true, owners: [ME, OTHER], threshold: 2, nonce: 1n } }
		];
		const screen = render(MultisigSettings, {
			props: { m: words(), account: account as never, payer: null, cancelLabel: 'Cancel' }
		});
		await tick();
		const original = screen.container.querySelector('[data-testid="members-original"]')!;
		expect(original.textContent).toContain('settings.originalMembers');
		expect(original.textContent).toContain('2 of 2');
		expect(original.querySelectorAll('.members li')).toHaveLength(2);
		const current = [...screen.container.querySelectorAll('[data-testid="members-current"]')];
		expect(current).toHaveLength(2);
		// Gnosis: OTHER swapped for NEW, one approval now.
		expect(current[0].textContent).toContain('settings.currentOn');
		expect(current[0].textContent).toContain('1 of 2');
		expect(current[0].querySelectorAll('[data-testid="member-added"]')).toHaveLength(1);
		expect(current[0].querySelectorAll('[data-testid="member-removed"]')).toHaveLength(1);
		expect(current[0].textContent).toContain('settings.thresholdChanged');
		// Optimism: the same as the original members.
		expect(current[1].textContent).toContain('settings.sameAsOriginal');
		// Base, not deployed: starts with the original members.
		expect(screen.container.querySelectorAll('[data-testid="members-undeployed"]')).toHaveLength(1);
	});

	it('shows the original members alone before any network is read', async () => {
		const screen = render(MultisigSettings, {
			props: { m: words(), account: account as never, payer: null, cancelLabel: 'Cancel' }
		});
		await tick();
		expect(screen.container.querySelectorAll('[data-testid="members-current"]')).toHaveLength(0);
		expect(screen.container.querySelectorAll('[data-testid="members-original"]')).toHaveLength(1);
	});
});

describe('who can act for it (§3.5)', () => {
	const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

	it('says a refused network as read-only, and a risk found on another, each in its own notice', async () => {
		const screen = render(MultisigSettings, {
			props: {
				m: words(),
				account: {
					address: SAFE,
					genesis: {},
					owners: [ME, OTHER],
					threshold: 2,
					chainIds: [100, 8453],
					findings: [
						{ from: 'chain', chainId: 100, risk: { kind: 'module', address: MODULE } },
						{ from: 'genesis', chainId: 8453, reason: { kind: 'module', address: MODULE } }
					]
				} as never,
				payer: null,
				cancelLabel: 'Cancel'
			}
		});
		const refused = screen.getByTestId('authority-refused');
		await expect.element(refused).toHaveTextContent('authority.refusedTitle');
		await expect.element(refused).toHaveTextContent('authority.refusedOn');
		await expect.element(refused).toHaveTextContent('authority.module');
		const risk = screen.getByTestId('authority-risk');
		await expect.element(risk).toHaveTextContent('authority.riskModule');
		await expect.element(risk).not.toHaveTextContent('authority.module');
	});
});
