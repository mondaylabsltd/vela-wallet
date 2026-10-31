/**
 * The multisig's first steps on its home (r4D-8), in a browser, with the real
 * `proposals` and store:
 *
 * - r5reg-17: the card went away only if the multisig was already followed
 *   when the home first drew it — `proposals.followed` was a plain field, so
 *   a card drawn before the host followed it never read the networks it was
 *   deployed on again, and stayed up for good. Followed, then read deployed:
 *   it goes, and stays gone.
 * - r5reg-18: a multisig of one has no members to invite.
 */
import { tick } from 'svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';
import { proposals, type ChainState } from '../core/proposals.svelte';
import { multisig, type MultisigAccount } from '../core/store.svelte';
import { loadCore } from '$lib/core/client';
import {
	memoryStorage,
	offlineDeps,
	startAccounts,
	stopAccounts,
	type AccountsMachineForTests
} from '../shell/accounts.testing';

// core-ms-2: whether it is set up is the proposals machine's word (`set_up`);
// here it is what the test's stubbed chains say, read as the machine would.
vi.mock('../core/presence.svelte', async () => {
	const { proposals } = await import('../core/proposals.svelte');
	return {
		presence: {
			follow: () => () => {},
			of: () => null,
			banner: null,
			get setUp() {
				return proposals.chains.some((c) => c.members.deployed);
			}
		},
		waitingWords: () => ''
	};
});

const { default: MultisigHomeCard } = await import('./MultisigHomeCard.svelte');
const { machines } = await import('../shell/machines.svelte');

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const A = '0x00000000000000000000000000000000000000aa';
const B = '0x00000000000000000000000000000000000000bb';

function words(): MultisigMessages {
	const out: Record<string, Record<string, unknown>> = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const [key, value] of Object.entries(keys)) {
			out[group][key] =
				typeof value === 'string'
					? `${group}.${key}`
					: { locale: 'en', forms: { _other: `${group}.${key}` } };
		}
	}
	return out as unknown as MultisigMessages;
}

const settle = async () => {
	for (let i = 0; i < 5; i += 1) {
		await tick();
		await new Promise((r) => setTimeout(r, 0));
	}
};

const account = (owners: string[]): MultisigAccount =>
	({
		address: SAFE,
		genesis: { factory: '0x01', singleton: '0x02', saltNonce: '0x1', initializer: '0x' },
		owners,
		threshold: 1,
		recorded: true,
		chainIds: [100],
		firstRun: true,
		addedMs: 1
	}) as unknown as MultisigAccount;

/** The accounts machine (the real wasm) holding `row`, shown. */
let machine: AccountsMachineForTests | null = null;
async function shown(row: MultisigAccount) {
	stopAccounts(machine);
	({ machine } = await startAccounts({
		seed: { 'vela.multisigAccounts': JSON.stringify([row]), 'vela.activeMultisig': SAFE }
	}));
}

beforeAll(() => loadCore());

const deployedOn = (chainId: number, owners: string[]): ChainState => ({
	chainId,
	members: { deployed: true, owners, threshold: 1, nonce: 1n },
	items: [],
	mailbox: true
});

const card = () => document.body.querySelector('[data-testid="multisig-first-run"]');
const realFetch = globalThis.fetch;

beforeEach(() => {
	// Nothing here answers: the card is read from what `proposals` holds.
	globalThis.fetch = (async () => {
		throw new TypeError('offline');
	}) as typeof fetch;
});

afterEach(() => {
	proposals.follow(null, []);
	proposals.chains = [];
	stopAccounts(machine);
	machine = null;
	globalThis.fetch = realFetch;
});

describe('the first-run card (r5reg-17, r5reg-18)', () => {
	it('goes once the multisig is read deployed — though the home drew it before it was followed', async () => {
		await shown(account([A, B]));
		const screen = render(MultisigHomeCard, { props: { m: words(), onfund: () => {} } });
		await settle();
		expect(card()).not.toBeNull();

		// The host follows it after the home has drawn; its networks are read.
		// (Following is the one host's proposals machine, over a world that
		// never answers; this test's accounts machine stays the store's.)
		const active = multisig.active!;
		await machines.boot({
			deps: offlineDeps(memoryStorage()),
			load: async () => {},
			facts: () => ({ serviceUrl: 'http://service.test', accounts: [], networks: [100] })
		});
		multisig.attach(machine);
		proposals.follow(active, []);
		await settle();
		expect(card(), 'not deployed anywhere yet').not.toBeNull();
		proposals.chains = [deployedOn(100, [A, B])];
		await settle();
		expect(card()).toBeNull();
		// And it stays gone: no longer this device's first run.
		expect(multisig.find(SAFE)?.firstRun).toBeFalsy();
		await screen.unmount();
	});

	it('can still be dismissed before then', async () => {
		await shown(account([A, B]));
		const screen = render(MultisigHomeCard, { props: { m: words(), onfund: () => {} } });
		await settle();
		[...document.body.querySelectorAll('button')]
			.find((b) => b.textContent?.trim() === 'create.firstRunDismiss')
			?.click();
		await settle();
		expect(card()).toBeNull();
		await screen.unmount();
	});

	it('a multisig of one: no members to invite', async () => {
		await shown(account([A]));
		const screen = render(MultisigHomeCard, { props: { m: words(), onfund: () => {} } });
		await settle();
		expect(card()).not.toBeNull();
		expect(card()?.textContent).not.toContain('create.firstRunInvite');
		expect(card()?.textContent).toContain('create.firstRunFund');
		await screen.unmount();

		await shown(account([A, B]));
		const two = render(MultisigHomeCard, { props: { m: words(), onfund: () => {} } });
		await settle();
		expect(card()?.textContent).toContain('create.firstRunInvite');
		await two.unmount();
	});

	it('S4 (r7): a step done is ticked — the invite handed on, funds in', async () => {
		await shown(account([A, B]));
		const screen = render(MultisigHomeCard, { props: { m: words(), onfund: () => {} } });
		await settle();
		const done = (id: string) =>
			document.body.querySelector(`[data-testid="${id}"]`)?.classList.contains('done');
		expect(done('first-run-invite')).toBe(false);
		expect(done('first-run-fund')).toBe(false);
		// The invite handed on: the accounts machine keeps it.
		multisig.inviteShared(SAFE);
		await settle();
		expect(done('first-run-invite')).toBe(true);
		await screen.unmount();
	});
});
