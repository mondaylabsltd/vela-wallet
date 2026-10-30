/**
 * r4D-5: the names typed in Settings → Add member travel with the next
 * invite, by address, with the founding members'; the device that opens it
 * keeps them as labels only — for addresses that are members, never over
 * its own names.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AddLink } from './link';

const SAFE = '0x710311c8d71CC405c5B7782BE1367eAFA3d78937';
const ALICE = '0xe331F2Ef7A6f4ED426529c8C10642b68084771A2';
const CAROL = '0xa1D99c2066BF6C4664981529FfB788c1bc5104F1';
const EVE = '0x34a0f6Ed36F63cc20B462803eBE13FF5C12c1e19';
/** Added in Settings after the multisig was made. */
const BOB = '0x412f2165f299Dfc2363ED08F8Fc6DE4353E81fF6';
const STRANGER = '0x0000000000000000000000000000000000000bad';

const seams = vi.hoisted(() => ({
	named: [] as { address: string; known: Record<string, { name?: string; kind?: string }> }[],
	account: null as null | { address: string; owners: string[]; chainIds: number[] }
}));

vi.mock('./names.svelte', () => ({
	names: {
		label: (a: string) =>
			({ [ALICE]: 'Alice', [BOB]: 'R5 Bob', [EVE]: 'Eve MetaMask' })[a] ?? `${a.slice(0, 8)}…`,
		kindOf: (a: string) => (a === EVE ? 'wallet' : a === CAROL ? null : 'passkey')
	}
}));
vi.mock('./display', () => ({ shortenAddress: (a: string) => `${a.slice(0, 8)}…` }));
vi.mock('./proposals.svelte', () => ({ membersAnywhere: () => [ALICE, CAROL, EVE, BOB] }));
vi.mock('./store.svelte', () => ({
	multisig: {
		find: () => seams.account,
		nameMembers: (address: string, known: Record<string, { name?: string }>) =>
			void seams.named.push({ address, known })
	}
}));

const { invitedMembers, inviteLink, keepInvited } = await import('./invite');
const { parseAddLink } = await import('./link');

beforeEach(() => {
	seams.named = [];
	seams.account = { address: SAFE, owners: [ALICE, CAROL, EVE], chainIds: [5042002] };
});

describe('the invite carries every member by address (r4D-5)', () => {
	it('the founding members, then the one added in Settings with its typed name', () => {
		expect(invitedMembers({ address: SAFE, owners: [ALICE, CAROL, EVE] })).toEqual([
			{ address: ALICE, name: 'Alice', kind: 'passkey' },
			{ address: CAROL },
			{ address: EVE, name: 'Eve MetaMask', kind: 'wallet' },
			{ address: BOB, name: 'R5 Bob', kind: 'passkey' }
		]);
		const link = inviteLink('https://x', 'en', {
			address: SAFE,
			owners: [ALICE, CAROL, EVE],
			chainIds: [5042002]
		});
		expect(parseAddLink(new URL(link).hash)?.members?.[3]).toEqual({
			address: BOB,
			name: 'R5 Bob',
			kind: 'passkey'
		});
	});
});

describe('the device that opens it keeps the labels — of members only', () => {
	const link = (members: AddLink['members']): AddLink => ({
		multisig: SAFE,
		chainIds: [5042002],
		members
	});

	it('a founding member at once; one added since once a network says it is a member', async () => {
		const read = vi.fn(async () => [ALICE, CAROL, EVE, BOB]);
		await keepInvited(
			SAFE,
			link([
				{ address: ALICE, name: 'Alice', kind: 'passkey' },
				{ address: BOB, name: 'R5 Bob', kind: 'passkey' },
				// Not a member anywhere: its label lands nowhere.
				{ address: STRANGER, name: 'Alice' }
			]),
			read
		);
		expect(read).toHaveBeenCalledTimes(1);
		expect(seams.named).toEqual([
			{ address: SAFE, known: { [ALICE]: { name: 'Alice', kind: 'passkey' } } },
			{ address: SAFE, known: { [BOB]: { name: 'R5 Bob', kind: 'passkey' } } }
		]);
	});

	it('a network that cannot be read keeps only the founding members’', async () => {
		await keepInvited(
			SAFE,
			link([
				{ address: ALICE, name: 'Alice' },
				{ address: BOB, name: 'R5 Bob' }
			]),
			async () => {
				throw new Error('down');
			}
		);
		expect(seams.named).toEqual([{ address: SAFE, known: { [ALICE]: { name: 'Alice' } } }]);
	});

	it('a link from before r6: by the genesis’s order, only when the counts match', async () => {
		const read = vi.fn(async () => []);
		await keepInvited(SAFE, link([{ name: 'Alice' }, {}, { name: 'Eve', kind: 'wallet' }]), read);
		expect(seams.named).toEqual([
			{
				address: SAFE,
				known: { [ALICE]: { name: 'Alice' }, [CAROL]: {}, [EVE]: { name: 'Eve', kind: 'wallet' } }
			}
		]);
		seams.named = [];
		await keepInvited(SAFE, link([{ name: 'Alice' }]), read);
		expect(seams.named).toEqual([]);
		expect(read).not.toHaveBeenCalled();
	});
});
