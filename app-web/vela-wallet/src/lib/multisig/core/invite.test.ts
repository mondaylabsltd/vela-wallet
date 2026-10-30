/**
 * r4D-5: the names typed in Settings → Add member travel with the next
 * invite, by address, with the founding members'. The device that opens it
 * keeps them as labels only — for addresses that are members, never over
 * its own names — by the core's accounts machine (`keep_invited`; Rust:
 * `an_invites_labels_land_only_on_members`,
 * `an_invites_labels_for_members_added_since_and_by_order`).
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';

const SAFE = '0x710311c8d71CC405c5B7782BE1367eAFA3d78937';
const ALICE = '0xe331F2Ef7A6f4ED426529c8C10642b68084771A2';
const CAROL = '0xa1D99c2066BF6C4664981529FfB788c1bc5104F1';
const EVE = '0x34a0f6Ed36F63cc20B462803eBE13FF5C12c1e19';
/** Added in Settings after the multisig was made. */
const BOB = '0x412f2165f299Dfc2363ED08F8Fc6DE4353E81fF6';

vi.mock('./names.svelte', () => ({
	names: {
		label: (a: string) =>
			({ [ALICE]: 'Alice', [BOB]: 'R5 Bob', [EVE]: 'Eve MetaMask' })[a] ?? `${a.slice(0, 8)}…`,
		kindOf: (a: string) => (a === EVE ? 'wallet' : a === CAROL ? null : 'passkey')
	}
}));
vi.mock('./display', () => ({ shortenAddress: (a: string) => `${a.slice(0, 8)}…` }));
vi.mock('./proposals.svelte', () => ({ membersAnywhere: () => [ALICE, CAROL, EVE, BOB] }));
vi.mock('./store.svelte', () => ({ multisig: {} }));

const { invitedMembers, inviteLink } = await import('./invite');
const parseAddLink = (hash: string) =>
	JSON.parse(wasm.multisigLinkCall('parseAddLink', JSON.stringify({ hash }))) as {
		members?: unknown[];
	} | null;

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
