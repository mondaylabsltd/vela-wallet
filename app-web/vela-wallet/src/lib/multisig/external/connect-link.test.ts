/**
 * ms-14: the connect page's link carries the members Vela asks for in its
 * fragment, with the invite — never in the query, which a server logs and a
 * referrer repeats — and an older link (the invite alone) still opens.
 */
import { describe, expect, it } from 'vitest';
import { connectLink, isExpected, readConnectHash } from './connect-link';

const PAGE = 'https://wallet.getvela.app/en/connect';
const INVITE = `wss://relay.getvela.app/r/abc#${'A'.repeat(86)}`;
const ALICE = '0x70997970C51812dc3A010C7d01b50e0d17dc79C8';
const BOB = '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC';

describe('the connect link (ms-14)', () => {
	it('a new room: the invite and the members, all after #', () => {
		const link = connectLink(PAGE, INVITE, [ALICE, BOB]);
		const url = new URL(link);
		expect(url.search).toBe('');
		expect(url.origin + url.pathname).toBe(PAGE);
		// The invite first, as every link before this one began.
		expect(link).toMatch(/\/en\/connect#wss/);
		expect(readConnectHash(url.hash)).toEqual({
			invite: INVITE,
			members: [ALICE.toLowerCase(), BOB.toLowerCase()],
			tx: null,
			damaged: false
		});
	});

	it('a room paired before: the members alone; none named: the plain page', () => {
		expect(readConnectHash(new URL(connectLink(PAGE, null, [ALICE])).hash)).toEqual({
			invite: null,
			members: [ALICE.toLowerCase()],
			tx: null,
			damaged: false
		});
		expect(connectLink(PAGE, null, [])).toBe(PAGE);
	});

	it('an older link (the invite alone) still opens, with no one to check against', () => {
		const old = `#${encodeURIComponent(INVITE)}`;
		expect(readConnectHash(old)).toEqual({
			invite: INVITE,
			members: [],
			tx: null,
			damaged: false
		});
		expect(isExpected([], BOB)).toBe(true);
	});

	it('drops what does not read — never a guess', () => {
		expect(readConnectHash('#for=0x123,nonsense&%E0%A4%A')).toEqual({
			invite: null,
			members: [],
			tx: null,
			damaged: false
		});
		expect(readConnectHash('')).toEqual({ invite: null, members: [], tx: null, damaged: false });
		expect(readConnectHash(`#ms=0.${ALICE}.0x12`).tx).toBeNull();
		expect(connectLink(PAGE, null, ['0xnope'])).toBe(PAGE);
	});

	it('a member is one the link named, in any case', () => {
		const members = [ALICE.toLowerCase()];
		expect(isExpected(members, ALICE)).toBe(true);
		expect(isExpected(members, BOB)).toBe(false);
	});

	it('r4C-2: carries the transaction, so the page can approve it with no Vela on the other end', () => {
		const tx = { chainId: 5042002, multisig: ALICE, safeTxHash: `0x${'Ab'.repeat(32)}` };
		const live = connectLink(PAGE, INVITE, [BOB], tx);
		expect(readConnectHash(new URL(live).hash)).toEqual({
			invite: INVITE,
			members: [BOB.toLowerCase()],
			tx: { chainId: 5042002, multisig: ALICE.toLowerCase(), safeTxHash: `0x${'ab'.repeat(32)}` },
			damaged: false
		});
		// The approve link: no room at all.
		const later = connectLink(PAGE, null, [BOB], tx);
		expect(later).toMatch(/\/en\/connect#for=0x3c44.*&ms=5042002\.0x7099/);
		expect(readConnectHash(new URL(later).hash).invite).toBeNull();
		// A transaction that does not read is left out, never half-written.
		expect(connectLink(PAGE, null, [BOB], { ...tx, safeTxHash: '0x12' })).not.toContain('ms=');
	});

	it('round 6: an invite that does not read is said as damaged — dropped, never guessed', () => {
		const cut = `wss://relay.getvela.app/r/abc#${'A'.repeat(40)}`;
		expect(readConnectHash(`#${encodeURIComponent(cut)}&for=${ALICE}`)).toEqual({
			invite: null,
			members: [ALICE.toLowerCase()],
			tx: null,
			damaged: true
		});
		// Its encoding broken on the way.
		expect(readConnectHash('#wss%3A%2F%2Frelay%E0%A4%A').damaged).toBe(true);
		// Nothing that was meant as an invite: not damaged.
		expect(readConnectHash(`#for=${ALICE}`).damaged).toBe(false);
	});
});
