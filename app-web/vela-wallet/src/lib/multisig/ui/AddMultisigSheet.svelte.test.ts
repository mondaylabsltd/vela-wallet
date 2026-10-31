/**
 * Adding a multisig (r4b-8, r4D-4, r4b-13): whose network nothing says, the
 * sheet asks — the likely networks first, in view — and a pick only picks:
 * the person's Add adds. An invite opens the sheet with what it carries and
 * adds nothing on its own; its network is the one used. A damaged invite
 * says so the moment it opens. And who can act
 * for it (§3.5): a refused genesis is said with why and cannot be added; a
 * high-risk one is shown and added only on the person's explicit tap.
 */
import { tick } from 'svelte';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import type { MultisigMessages } from '../messages';

const REFUSED = '0x2f53cc750594bb97c21bf3c18a4837bb5ad911f6';
const RISKY = '0x0cb9aef9713c4a2ea2995fca68d7355ce5e27d83';
const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';
const calls = vi.hoisted(() => [] as (number | undefined)[]);
const accepted = vi.hoisted(() => [] as boolean[]);
const SAFE = '0x013124f0f8b38d42a2e192bcc26337ee9d7ffbf1';
const used = vi.hoisted(() => [] as number[]);
/**
 * The core's accounts machine, as the sheet asks it (`multisig.request`):
 * what it answers each event with, in its own words.
 */
const machine = vi.hoisted(() => ({
	answer(event: { type: string; [k: string]: unknown }): unknown {
		if (event.type === 'add') {
			const address = event.address as string;
			const network = event.network as number | null;
			if (address === '0x2f53cc750594bb97c21bf3c18a4837bb5ad911f6') {
				return {
					kind: 'refused',
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
				accepted.push(!!event.accept_risks);
				return event.accept_risks
					? { kind: 'added', address }
					: {
							kind: 'highRisk',
							findings: [
								{
									from: 'chain',
									chainId: 100,
									risk: { kind: 'module', address: '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1' }
								}
							]
						};
			}
			// core-ms-2: the invite's own network, the machine's (`invite.on_device`).
			const invited =
				io.invite && address.toLowerCase() === io.invite.link.multisig.toLowerCase()
					? io.invite.on_device[0]
					: undefined;
			const on = network ?? invited ?? null;
			calls.push(on ?? undefined);
			if (on !== null && invited !== undefined) used.push(...io.invite!.on_device);
			return on === null ? { kind: 'chooseNetwork' } : { kind: 'added', address };
		}
		if (event.type === 'parse_export') {
			return { safes: io.exported, error: null, busy: null, outcomes: {} };
		}
		if (event.type === 'import') {
			io.imported.push(event.address as string);
			return { kind: 'imported' };
		}
		throw new Error(`not asked here: ${event.type}`);
	}
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
// r5reg-24: Carol's account is on this device.
const CAROL = vi.hoisted(() => '0xa1D99c2066BF6C4664981529FfB788c1bc5104F1');
vi.mock('../core/display', () => ({
	shortenAddress: (a: string) => `${a.slice(0, 8)}…${a.slice(-6)}`,
	localName: (a: string) => (a.toLowerCase() === CAROL.toLowerCase() ? 'R5 Carol' : null)
}));
/** r7: the import's seams — the export read, what the chains say, what is asked and picked, what is imported. */
const io = vi.hoisted(() => ({
	exported: [] as unknown[],
	evidence: {} as Record<string, 'vela' | 'contract' | 'wallet' | null>,
	/** The members whose kinds were read (`read_kinds`), with the networks each is read on. */
	asked: [] as [string, number[]][],
	/** The kinds the person picked (`pick_kind`). */
	picked: [] as [string, string | null][],
	imported: [] as string[],
	/** core-ms-2: the invite the accounts machine keeps (`view.invite`). */
	invite: null as null | {
		link: { multisig: string; chainIds: number[]; members: unknown };
		on_device: number[];
		missing: number | null;
		missing_name: string | null;
	}
}));
vi.mock('../core/store.svelte', async () => {
	/**
	 * r7: the accounts machine's `member_kinds` — each member asked about, as
	 * the machine answers it: what the chains said (`io.evidence`), else the
	 * person's pick, else asked. Keeping them with each Safe imported is the
	 * machine's (Rust: `an_import_keeps_its_members_kinds`).
	 */
	const { SvelteMap } = await import('svelte/reactivity');
	const kinds = new SvelteMap<string, unknown>();
	const answer = (address: string, picked: string | null = null) => {
		const said = io.evidence[address];
		if (said === 'vela') return { kind: 'vela', suggested: null, locked: true, note: 'known' };
		if (said === 'contract')
			return { kind: 'contract', suggested: null, locked: true, note: 'contract' };
		if (picked) return { kind: picked, suggested: null, locked: false, note: null };
		if (said === 'wallet')
			return { kind: null, suggested: 'wallet', locked: false, note: 'likely' };
		return { kind: null, suggested: null, locked: false, note: 'ask' };
	};
	return {
		multisig: {
			accounts: [],
			find: () => undefined,
			get view() {
				return { member_kinds: Object.fromEntries(kinds), invite: io.invite, used_networks: [] };
			},
			send: (event: {
				type: string;
				members?: { address: string; chain_ids: number[] }[];
				address?: string;
				kind?: string | null;
			}) => {
				if (event.type === 'read_kinds') {
					for (const m of event.members ?? []) {
						io.asked.push([m.address, m.chain_ids]);
						kinds.set(m.address.toLowerCase(), answer(m.address));
					}
				}
				if (event.type === 'pick_kind' && event.address) {
					io.picked.push([event.address, event.kind ?? null]);
					kinds.set(event.address.toLowerCase(), answer(event.address, event.kind ?? null));
				}
			},
			request: async (event: { type: string }) => machine.answer(event)
		}
	};
});
vi.mock('$lib/wallet/identicon', () => ({
	identiconSvgForClient: (a: string) => `<svg data-seed="${a}"></svg>`
}));
vi.mock('../core/names.svelte', () => ({
	names: { label: (a: string) => `${a.slice(0, 8)}…${a.slice(-6)}` },
	keptMember: () => ({})
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
		badChecksum: "This invite's address doesn't match its checksum",
		thatsYou: "That's you",
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
	import: {
		or: 'or',
		body: 'b',
		choose: 'c',
		pick: 'Choose the Safes to import',
		undeployed: 'undeployed',
		importSome: 'Import {{total}}',
		importing: 'Importing…',
		imported: 'Imported',
		already: 'Already added',
		kindsTitle: 'How does each member approve?',
		kindsBody: 'kinds body'
	},
	create: { next: 'Next', back: 'Back', done: 'Done', thresholdValue: '{{m}} of {{n}}' },
	members: {
		kind: 'Kind of member',
		kindVela: 'Vela account (passkey)',
		kindWallet: 'Other wallet',
		kindKnown: 'Vela found a Vela account',
		kindLikely: 'likely another wallet',
		kindAsk: 'Vela can’t tell',
		kindContract: 'Smart contract account',
		kindContractNote: 'Approves from its own app'
	},
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
const root = () => document.body;
const addButton = () =>
	[...root().querySelectorAll<HTMLButtonElement>('button')].find(
		(b) => b.textContent?.trim() === 'Add'
	)!;
const radios = () =>
	[...root().querySelectorAll<HTMLButtonElement>('[role="radio"]')].map((r) =>
		r.textContent?.trim()
	);

// The network order is the core’s rule (`networks.likely`, `multisigLinkCall`).
beforeAll(async () => {
	await loadCore();
});

beforeEach(() => {
	calls.length = 0;
	used.length = 0;
	io.invite = null;
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
		const link = {
			multisig: SAFE,
			chainIds: [5042002],
			members: [
				{ name: 'Alice', kind: 'passkey' as const },
				{ name: 'Bob', kind: 'wallet' as const }
			]
		};
		io.invite = { link, on_device: [5042002], missing: null, missing_name: null };
		const screen = render(AddMultisigSheet, {
			props: {
				m,
				prefill: SAFE,
				link,
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
		// Its network is the one used — the machine's, from the invite it
		// keeps: never a guess, never the person's chore.
		expect(calls).toEqual([5042002]);
		expect(used).toEqual([5042002]);
		io.invite = null;
		expect(added).toEqual([SAFE]);
		await screen.unmount();
	});

	it('says a damaged invite is damaged the moment it opens (r4b-13)', async () => {
		const screen = render(AddMultisigSheet, {
			props: {
				m,
				prefill: '0x0131',
				broken: 'damaged',
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

	it('refuses an invite whose address letters were changed, in plain words (r5reg-22)', async () => {
		const screen = render(AddMultisigSheet, {
			props: {
				m,
				prefill: '5042002:0x710311c8d71cc405c5B7782BE1367eAFA3d78937',
				broken: 'checksum',
				wide: true,
				closeLabel: 'Close',
				onclose: () => {},
				onadded: () => {}
			}
		});
		await tick();
		expect(root().querySelector('[role="alert"]')?.textContent).toContain(
			"This invite's address doesn't match its checksum"
		);
		expect(calls).toEqual([]);
		await screen.unmount();
	});

	it('tells the one who opens it which member is them (r5reg-24)', async () => {
		const screen = render(AddMultisigSheet, {
			props: {
				m,
				prefill: SAFE,
				link: {
					multisig: SAFE,
					chainIds: [5042002],
					members: [
						{
							address: '0xe331F2Ef7A6f4ED426529c8C10642b68084771A2',
							name: 'Alice',
							kind: 'passkey'
						},
						// Named "Carol" by the inviter; this device's own account.
						{ address: CAROL, name: 'Carol' },
						// Unnamed by the inviter, and not this device's: not listed.
						{ address: '0x34a0f6Ed36F63cc20B462803eBE13FF5C12c1e19' }
					]
				},
				wide: true,
				closeLabel: 'Close',
				onclose: () => {},
				onadded: () => {}
			}
		});
		await settle();
		const you = root().querySelectorAll('[data-testid="invite-you"]');
		expect(you).toHaveLength(1);
		expect(you[0].textContent).toContain('Carol');
		expect(you[0].textContent).toContain("That's you");
		await screen.unmount();
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

/**
 * r7: importing from Safe{Wallet} asks each member's kind before anything is
 * imported — pre-filled where Vela can tell, asked where it can't — and keeps
 * what was declared with each Safe it is a member of (a label, never
 * authority). A member on this device is not asked.
 */
describe('the Safe{Wallet} import asks each member’s kind (r7)', () => {
	const BOB = '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC';
	const DAN = '0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65';
	const ERIN = '0x90F79bf6EB2c4f870365E785982E1f101E93b906';
	const ONE = '0x1111111111111111111111111111111111111111';
	const TWO = '0x2222222222222222222222222222222222222222';

	beforeEach(() => {
		io.exported = [
			{
				address: ONE,
				name: 'Team',
				chains: [
					{
						chainId: 100,
						deployed: true,
						owners: [CAROL, BOB, DAN],
						threshold: 2,
						props: null,
						version: null
					}
				]
			},
			{
				address: TWO,
				name: 'Ops',
				chains: [
					{
						chainId: 8453,
						deployed: false,
						owners: [BOB, ERIN],
						threshold: 1,
						props: {},
						version: null
					}
				]
			}
		];
		io.evidence = { [BOB]: 'wallet', [DAN]: 'vela' };
		io.asked = [];
		io.picked = [];
		io.imported = [];
	});

	const button = (label: string) =>
		[...document.body.querySelectorAll<HTMLButtonElement>('button')].find(
			(b) => b.textContent?.trim() === label
		);

	async function chooseFile() {
		const screen = render(AddMultisigSheet, {
			props: { m, wide: true, closeLabel: 'Close', onclose: () => {}, onadded: () => {} }
		});
		await tick();
		const input = screen.container.querySelector<HTMLInputElement>('input[type=file]')!;
		const files = new DataTransfer();
		files.items.add(new File(['{}'], 'safes.json', { type: 'application/json' }));
		input.files = files.files;
		input.dispatchEvent(new Event('change', { bubbles: true }));
		await settle();
		return screen;
	}

	it('Next asks each member not on this device once — found, pre-filled, or asked', async () => {
		const screen = await chooseFile();
		expect(root().textContent).toContain('Choose the Safes to import');
		button('Next')!.click();
		await settle();
		expect(io.imported, 'nothing imported before the kinds are asked').toEqual([]);
		const step = screen.container.querySelector('[data-testid="import-kinds"]')!;
		expect(root().textContent).toContain('How does each member approve?');
		const rows = [...step.querySelectorAll<HTMLElement>('li')];
		// Carol is on this device: not asked. Bob is in both Safes: asked once,
		// on the networks he is a member on.
		expect(rows).toHaveLength(3);
		expect(io.asked).toEqual([
			[BOB, [100, 8453]],
			[DAN, [100]],
			[ERIN, [8453]]
		]);
		const kinds = rows.map(
			(r) => r.querySelector<HTMLElement>('[data-testid="member-kind"]')?.dataset.kind
		);
		// Bob: "Other wallet" only hinted — nothing picked.
		expect(kinds).toEqual(['unknown', 'passkey', 'unknown']);
		expect(rows[0].textContent).toContain('likely another wallet');
		expect(rows[0].querySelector('[data-suggested]')?.textContent?.trim()).toBe('Other wallet');
		expect(rows[1].querySelectorAll('[role="radio"]')).toHaveLength(0);
		expect(rows[2].textContent).toContain('Vela can’t tell');
		await screen.unmount();
	});

	it('imports with what was declared, kept with each Safe it is a member of', async () => {
		const screen = await chooseFile();
		button('Next')!.click();
		await settle();
		// Bob is a Vela account after all, says the person; Erin is left unsaid.
		const rows = [
			...screen.container.querySelectorAll<HTMLElement>('[data-testid="import-kinds"] li')
		];
		[...rows[0].querySelectorAll<HTMLButtonElement>('[role="radio"]')]
			.find((b) => b.textContent?.trim() === 'Vela account (passkey)')
			?.click();
		await settle();
		button('Import 2')!.click();
		await settle();
		expect(io.imported).toEqual([ONE, TWO]);
		// What was declared goes to the machine, which keeps it with each Safe.
		expect(io.picked).toEqual([[BOB, 'vela']]);
		// Back on the list, each Safe's outcome.
		expect(root().textContent).toContain('Imported');
		await screen.unmount();
	});

	it('another Safe as a member: a smart contract account, kept as one; the unpicked hint keeps nothing', async () => {
		io.evidence = { [BOB]: 'wallet', [DAN]: 'vela', [ERIN]: 'contract' };
		const screen = await chooseFile();
		button('Next')!.click();
		await settle();
		const rows = [
			...screen.container.querySelectorAll<HTMLElement>('[data-testid="import-kinds"] li')
		];
		expect(rows[2].querySelector<HTMLElement>('[data-testid="member-kind"]')?.dataset.kind).toBe(
			'contract'
		);
		expect(rows[2].textContent).toContain('Smart contract account');
		expect(rows[2].textContent).toContain('Approves from its own app');
		expect(rows[2].querySelectorAll('[role="radio"]')).toHaveLength(0);
		button('Import 2')!.click();
		await settle();
		expect(io.imported).toEqual([ONE, TWO]);
		// Nothing picked: what is kept is what the chains said (the machine's).
		expect(io.picked).toEqual([]);
		await screen.unmount();
	});

	it('Back returns to the list, and nothing is imported', async () => {
		const screen = await chooseFile();
		button('Next')!.click();
		await settle();
		button('Back')!.click();
		await settle();
		expect(root().textContent).toContain('Choose the Safes to import');
		expect(io.imported).toEqual([]);
		await screen.unmount();
	});
});
