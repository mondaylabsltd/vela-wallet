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
import { page } from 'vitest/browser';
import { chainFilter } from '$lib/wallet/chain-filter.svelte';
import { identiconViewer } from '$lib/wallet/identicon-viewer.svelte';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';
import type { MultisigAccount } from '../core/store.svelte';

const ME = '0x00000000000000000000000000000000000000aa';
const TWO = '0x00000000000000000000000000000000000000bb';
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const DAN = '0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65';

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
	created: [] as { chainId: number }[],
	/** r7: what the chains say of each address (the accounts machine's read), and the networks asked. */
	evidence: {} as Record<string, 'vela' | 'contract' | 'wallet' | null>,
	evidenceOn: [] as number[][],
	/** r7: the kinds the person picked (`pick_kind`). */
	picked: [] as [string, string | null][]
}));
vi.mock('$lib/services/networks', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/networks')>()),
	spaceNetworksSync: () => seams.networks
}));
/**
 * The core's accounts machine as the sheet reads it: the draft's address,
 * the Safe contracts per network (the core's verdict, `seams.contracts`),
 * and a creation kept here at once — answered as the machine would.
 */
const facade = vi.hoisted(() => ({ accounts: [] as { address: string }[] }));
vi.mock('../core/store.svelte', async () => {
	const find = (a: string) =>
		facade.accounts.find((x) => x.address.toLowerCase() === a.toLowerCase());
	/**
	 * r7: the machine's `member_kinds` — each member asked about, as the
	 * machine answers it: what the chains said (`seams.evidence`), else the
	 * person's pick, else asked.
	 */
	const { SvelteMap } = await import('svelte/reactivity');
	const kinds = new SvelteMap<string, unknown>();
	const answer = (address: string, picked: string | null = null) => {
		const said = seams.evidence[address];
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
			get accounts() {
				return facade.accounts;
			},
			set accounts(rows: { address: string }[]) {
				facade.accounts = rows;
			},
			find,
			row: (a: string) => (find(a) ? { shared: true } : null),
			get view() {
				return {
					create: {
						phase: 'idle',
						draft_address: SAFE,
						draft_error: null,
						address: null,
						refused: null,
						error: null,
						networks: Object.fromEntries(
							seams.networks.map((n) => {
								const kind = seams.contracts[n.chainId] ?? 'present';
								return [n.chainId, kind === 'missing' ? { kind, names: ['Safe L2'] } : { kind }];
							})
						)
					},
					add: { ens: null },
					member_kinds: Object.fromEntries(kinds)
				};
			},
			send: (event: {
				type: string;
				members?: { address: string; chain_ids: number[] }[];
				address?: string;
				kind?: string | null;
			}) => {
				if (event.type === 'read_kinds') {
					for (const m of event.members ?? []) {
						seams.evidenceOn.push([...m.chain_ids]);
						kinds.set(m.address.toLowerCase(), answer(m.address));
					}
				}
				if (event.type === 'pick_kind' && event.address) {
					seams.picked.push([event.address, event.kind ?? null]);
					kinds.set(event.address.toLowerCase(), answer(event.address, event.kind ?? null));
				}
			},
			request: async (event: { type: string; choice?: { chain_id: number } }) => {
				if (event.type !== 'create' || !event.choice) throw new Error(`not asked: ${event.type}`);
				seams.created.push({ ...event.choice, chainId: event.choice.chain_id });
				return { phase: 'done', address: SAFE, refused: null, error: null };
			},
			until: async () => ({ phase: 'done', address: SAFE, refused: null, error: null })
		}
	};
});
vi.mock('../shell/local-accounts', () => ({ localAccounts: () => [] }));
vi.mock('$lib/core/kernels', async (importOriginal) => {
	const { eip55 } = await import('../external/eip55');
	return {
		...(await importOriginal<typeof import('$lib/core/kernels')>()),
		checksumAddress: (a: string) => eip55(a)
	};
});
vi.mock('$lib/contacts/core/contacts-book.svelte', () => ({
	contactsBook: {
		ensure: async () => {},
		view: {
			contacts: [
				{
					address: '0x90f79bf6eb2c4f870365e785982e1f101e93b906',
					name: 'Carol',
					resolved_name: null
				}
			]
		}
	}
}));
vi.mock('$lib/wallet/core/balance.svelte', () => ({ balance: { view: { tokens: [] } } }));
vi.mock('../core/names.svelte', () => ({
	names: { label: (a: string) => a.slice(0, 6), kindOf: () => null },
	keptMember: () => ({})
}));
vi.mock('../core/ens-name', () => ({
	looksLikeEns: (t: string) => t.trim().endsWith('.eth'),
	resolveEns: async (name: string) => ({ kind: 'found', name, address: DAN })
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
	const out: Record<string, Record<string, unknown>> = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const [key, value] of Object.entries(keys)) {
			// A plural key ships as its forms.
			out[group][key] =
				typeof value === 'string'
					? `${group}.${key}`
					: { locale: 'en', forms: { _other: `${group}.${key}` } };
		}
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
	seams.evidence = {};
	seams.evidenceOn = [];
	seams.picked = [];
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

		// The rule: every member must approve says so (no recovery to offer);
		// with one to spare, the passkey-native recovery is explained.
		press('create.next');
		await settle();
		const recovery = () => document.body.querySelector('[data-testid="create-recovery"]');
		expect(recovery()).toBeNull();
		document.body.querySelector<HTMLButtonElement>('button[aria-label="create.fewer"]')?.click();
		await settle();
		expect(recovery()?.textContent?.trim()).toBe('members.recovery');
		document.body.querySelector<HTMLButtonElement>('button[aria-label="create.more"]')?.click();
		await settle();

		// Review: the address it will have.
		press('create.next');
		await settle();
		document.body.querySelector<HTMLButtonElement>('.hero button.identicon')?.click();
		expect(identiconViewer.current?.address).toBe(SAFE);
		identiconViewer.close();

		// Created: the same address, the same viewer. Nothing said which network
		// (r4a-14): the person picks it.
		document.body.querySelector<HTMLButtonElement>('[role="radio"][data-chain="100"]')?.click();
		await settle();
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
			const rows = [...document.body.querySelectorAll('[role="radio"]')]
				.map((r) => r.textContent?.trim())
				.filter(Boolean);
			expect([...rows].sort()).toEqual(['Arbitrum', 'Base', 'Gnosis']);
			expect(document.body.textContent).not.toContain('create.noSafeNetworks');
			// r4a-14: nothing said which: none is picked for the person, Create waits.
			expect(document.body.querySelector('[role="radio"][aria-checked="true"]')).toBeNull();
			expect(document.body.textContent).toContain('create.networkNeeded');
			// Arbitrum, a network the service does not read, is picked and created on.
			document.body.querySelector<HTMLButtonElement>('[role="radio"][data-chain="42161"]')?.click();
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

const open = () =>
	render(CreateMultisigSheet, {
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
const type = (selector: string, value: string) => {
	const input = document.body.querySelector<HTMLInputElement>(selector);
	if (!input) throw new Error(`no ${selector}`);
	input.value = value;
	input.dispatchEvent(new Event('input', { bubbles: true }));
};
const addPressed = () =>
	[...document.body.querySelectorAll<HTMLButtonElement>('.typed button')]
		.find((b) => b.textContent?.trim() === 'create.addMember')
		?.click();

describe('r4a-14: what a creation can be, and how its members come', () => {
	it('one member is a 1 of 1 multisig, said what it means', async () => {
		const screen = open();
		await settle();
		type('.name-field', 'Solo');
		await settle();
		expect(document.body.querySelector('[data-testid="solo-hint"]')?.textContent).toContain(
			'create.solo'
		);
		press('create.next');
		await settle();
		expect(document.body.querySelector('output')?.textContent).toBe('1 of 1');
		expect(document.body.textContent).toContain('create.solo');
		press('create.next');
		await settle();
		document.body.querySelector<HTMLButtonElement>('[role="radio"]')?.click();
		await settle();
		press('create.create');
		await settle();
		expect(seams.created).toEqual([expect.objectContaining({ owners: [ME], threshold: 1 })]);
		await screen.unmount();
	});

	it('counts the name in bytes and says what is wrong with it, on the name', async () => {
		const screen = open();
		await settle();
		expect(document.body.querySelector('[data-testid="name-bytes"]')?.textContent).toContain(
			'create.nameBytes'
		);
		// Next with no name: the field says so.
		press('create.next');
		await settle();
		expect(document.body.querySelector('[data-testid="name-error"]')?.textContent).toBe(
			'create.nameNeeded'
		);
		type('.name-field', '团'.repeat(22)); // 66 bytes
		await settle();
		expect(document.body.querySelector('[data-testid="name-error"]')?.textContent).toBe(
			'create.nameTooLong'
		);
		await screen.unmount();
	});

	it('checksums a typed address, keeps its name, and reads an outside member in full on review', async () => {
		const screen = open();
		await settle();
		type('.name-field', 'Team');
		type('input.mono', '0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc');
		type('.name-field.small', 'Bob');
		await settle();
		addPressed();
		await settle();
		expect(document.body.querySelector('.owners')?.textContent).toContain('Bob');
		// From the contacts too, with the contact's name.
		[...document.body.querySelectorAll<HTMLButtonElement>('button.pick')][0]?.click();
		await settle();
		document.body
			.querySelector<HTMLButtonElement>('[data-testid="contacts-picker"] button')
			?.click();
		await settle();
		expect(document.body.querySelector('.owners')?.textContent).toContain('Carol');
		press('create.next');
		await settle();
		press('create.next');
		await settle();
		const full = [...document.body.querySelectorAll('[data-testid="review-full"]')].map(
			(e) => e.textContent
		);
		expect(full).toEqual([
			'0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC',
			'0x90F79bf6EB2c4f870365E785982E1f101E93b906'
		]);
		document.body.querySelector<HTMLButtonElement>('[role="radio"]')?.click();
		await settle();
		press('create.create');
		await settle();
		expect(seams.created[0]).toEqual(
			expect.objectContaining({
				owners: [
					ME,
					'0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC',
					'0x90F79bf6EB2c4f870365E785982E1f101E93b906'
				],
				members: expect.objectContaining({
					'0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC': { name: 'Bob' },
					'0x90F79bf6EB2c4f870365E785982E1f101E93b906': { name: 'Carol' }
				})
			})
		);
		await screen.unmount();
	});

	it('r7: each added member’s kind — found, a smart contract, hinted, or asked — kept only as declared', async () => {
		const BOB = '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC';
		const CAROL = '0x90F79bf6EB2c4f870365E785982E1f101E93b906';
		/** Another Safe as a member: code, no index record. */
		const SAFE_MEMBER = '0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc';
		seams.networks = [
			{ chainId: 100, displayName: 'Gnosis' },
			{ chainId: 8453, displayName: 'Base' }
		];
		seams.evidence = { [BOB]: 'wallet', [DAN]: 'vela', [SAFE_MEMBER]: 'contract' };
		const screen = open();
		await settle();
		type('.name-field', 'Team');
		for (const address of [BOB, CAROL, DAN, SAFE_MEMBER]) {
			type('input.mono', address);
			await settle();
			addPressed();
			await settle();
		}
		const rows = [...document.body.querySelectorAll<HTMLElement>('.owners li')];
		const kindOf = (row: HTMLElement) =>
			row.querySelector<HTMLElement>('[data-testid="member-kind"]')?.dataset.kind ?? null;
		// This device's account is a Vela account here: nothing asked of it.
		expect(rows.map(kindOf)).toEqual([null, 'unknown', 'unknown', 'passkey', 'contract']);
		// Bob: no Vela account found — "Other wallet" only hinted, and said so.
		expect(rows[1].querySelector('[data-testid="member-kind-likely"]')).not.toBeNull();
		expect(rows[1].querySelectorAll('[aria-checked="true"]')).toHaveLength(0);
		expect(rows[1].querySelector('[data-suggested]')?.textContent?.trim()).toBe(
			'members.kindWallet'
		);
		// Carol: it can't tell — asked, nothing picked.
		expect(rows[2].querySelector('[data-testid="member-kind-ask"]')).not.toBeNull();
		expect(rows[2].querySelectorAll('[aria-checked="true"]')).toHaveLength(0);
		// Dan: Vela found a Vela account — shown, nothing to pick.
		expect(rows[3].querySelectorAll('[role="radio"]')).toHaveLength(0);
		// The other Safe: a smart contract account, never "Vela account" — said honestly.
		expect(rows[4].querySelectorAll('[role="radio"]')).toHaveLength(0);
		expect(rows[4].textContent).toContain('members.kindContractNote');
		expect(rows[4].textContent).not.toContain('members.kindVela');
		// Looked for on every network it may be used on: the same address on each.
		expect([...(seams.evidenceOn.at(-1) ?? [])].sort()).toEqual([100, 8453]);
		// The person says Carol is a Vela account.
		[...rows[2].querySelectorAll<HTMLButtonElement>('[role="radio"]')]
			.find((b) => b.textContent?.trim() === 'members.kindVela')
			?.click();
		await settle();
		expect(kindOf(rows[2])).toBe('passkey');
		press('create.next');
		await settle();
		press('create.next');
		await settle();
		document.body.querySelector<HTMLButtonElement>('[role="radio"]')?.click();
		await settle();
		press('create.create');
		await settle();
		// What each is kept as is the machine's (its read, and the person's
		// word — `chosen_kinds`, Rust: app_multisig_accounts.rs): the sheet
		// said only Carol's, and Bob's hint was not picked.
		expect(seams.picked).toEqual([[CAROL, 'vela']]);
		expect(seams.created[0]).toEqual(
			expect.objectContaining({
				members: expect.objectContaining({
					[BOB]: {},
					[CAROL]: {},
					[DAN]: {},
					[SAFE_MEMBER]: {}
				})
			})
		);
		await screen.unmount();
	});

	it('r7: a member left unsaid keeps no kind — it is offered both ways later', async () => {
		const CAROL = '0x90F79bf6EB2c4f870365E785982E1f101E93b906';
		const screen = open();
		await settle();
		type('.name-field', 'Team');
		type('input.mono', CAROL);
		await settle();
		addPressed();
		await settle();
		press('create.next');
		await settle();
		press('create.next');
		await settle();
		document.body.querySelector<HTMLButtonElement>('[role="radio"]')?.click();
		await settle();
		press('create.create');
		await settle();
		const members = (seams.created[0] as unknown as { members: Record<string, unknown> }).members;
		expect(members[CAROL]).toEqual({});
		expect(seams.picked).toEqual([]);
		await screen.unmount();
	});

	it('r4D-5: an ENS name is shown as its address, in full, and the address is what is added', async () => {
		const screen = open();
		await settle();
		type('.name-field', 'Team');
		type('input.mono', 'dan.eth');
		await settle();
		addPressed();
		await settle();
		const answer = document.body.querySelector('[data-testid="ens-answer"]');
		expect(answer?.querySelector('.ens-address')?.textContent).toBe(DAN);
		// Not added until the person adds the address it showed.
		expect(document.body.querySelectorAll('.owners li')).toHaveLength(1);
		answer?.querySelector<HTMLButtonElement>('button')?.click();
		await settle();
		expect(document.body.querySelectorAll('.owners li')).toHaveLength(2);
		expect(document.body.querySelector('.owners')?.textContent).toContain('dan.eth');
		await screen.unmount();
	});
});

describe('round 6: the create sheet at a phone’s width, and a multisig of one', () => {
	afterEach(async () => {
		await page.viewport(414, 896);
		facade.accounts = [];
		chainFilter.select(null);
	});

	it('r5reg-19: at 390 px the address field is the row’s full width', async () => {
		await page.viewport(390, 844);
		const screen = render(CreateMultisigSheet, {
			props: {
				m: words(),
				locale: 'en',
				current: ME,
				wide: false,
				closeLabel: 'Close',
				onclose: () => {},
				oncreated: () => {}
			}
		});
		await settle();
		const field = document.body.querySelector<HTMLInputElement>('.typed input.mono');
		const row = field?.parentElement;
		expect(field && row).toBeTruthy();
		expect(field!.getBoundingClientRect().width).toBeGreaterThan(
			row!.getBoundingClientRect().width - 1
		);
		// The Add button beside the name keeps its own width; the name field takes the rest.
		const add = [...document.body.querySelectorAll<HTMLButtonElement>('.typed button')].find(
			(b) => b.textContent?.trim() === 'create.addMember'
		);
		expect(getComputedStyle(add!).flexGrow).toBe('0');
		const name = document.body.querySelector<HTMLInputElement>('.typed input.name-field');
		expect(getComputedStyle(name!).flexGrow).toBe('1');
		await screen.unmount();
	});

	it('r5reg-19: the network the wallet is on is in view, never under "More networks"', async () => {
		seams.networks = [
			{ chainId: 11155111, displayName: 'Sepolia' },
			{ chainId: 84532, displayName: 'Base Sepolia' },
			{ chainId: 421614, displayName: 'Arbitrum Sepolia' },
			{ chainId: 11155420, displayName: 'OP Sepolia' },
			{ chainId: 80002, displayName: 'Amoy' },
			{ chainId: 5042002, displayName: 'Arc Testnet' }
		];
		chainFilter.select(5042002);
		const screen = open();
		await settle();
		type('.name-field', 'Team');
		press('create.next');
		await settle();
		press('create.next');
		await settle();
		const arc = document.body.querySelector('[role="radio"][data-chain="5042002"]');
		expect(arc).not.toBeNull();
		// And it is the one picked: where the person is says which.
		expect(arc?.getAttribute('aria-checked')).toBe('true');
		await screen.unmount();
	});

	it('r5reg-18: a multisig of one is not told to send "the other members" anything', async () => {
		const screen = open();
		await settle();
		type('.name-field', 'Solo');
		press('create.next');
		await settle();
		press('create.next');
		await settle();
		document.body.querySelector<HTMLButtonElement>('[role="radio"]')?.click();
		await settle();
		// What the store holds once it is saved: a multisig of one.
		facade.accounts = [
			{
				address: SAFE,
				owners: [ME],
				threshold: 1,
				chainIds: [100],
				addedMs: 1
			} as unknown as MultisigAccount
		];
		press('create.create');
		await settle();
		expect(document.body.querySelector('[data-testid="created-body"]')?.textContent?.trim()).toBe(
			'create.createdBodySolo'
		);
		await screen.unmount();
	});
});
