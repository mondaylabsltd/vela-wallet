/**
 * r4D-5: an ENS name typed as a member is told from an address here (which
 * field it is); reading it on Ethereum (registry → resolver → addr, a
 * checksummed address; no address, Ethereum not answering and a lookalike
 * each said as themselves) is the core accounts machine's `resolve_ens`
 * (Rust: `app_multisig_accounts.rs`, `ens_names_are_read_on_ethereum`,
 * `an_ens_names_address_is_read_and_checksummed`).
 */
import { describe, expect, it, vi } from 'vitest';

vi.mock('./store.svelte', () => ({ multisig: {} }));

const { looksLikeEns } = await import('./ens-name');

describe('an ENS name as a member (r4D-5)', () => {
	it('is told from an address', () => {
		expect(looksLikeEns('vitalik.eth')).toBe(true);
		expect(looksLikeEns(' Team.Alice.ETH ')).toBe(true);
		expect(looksLikeEns('0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045')).toBe(false);
	});
});
