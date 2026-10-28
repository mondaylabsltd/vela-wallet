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
			members: [ALICE.toLowerCase(), BOB.toLowerCase()]
		});
	});

	it('a room paired before: the members alone; none named: the plain page', () => {
		expect(readConnectHash(new URL(connectLink(PAGE, null, [ALICE])).hash)).toEqual({
			invite: null,
			members: [ALICE.toLowerCase()]
		});
		expect(connectLink(PAGE, null, [])).toBe(PAGE);
	});

	it('an older link (the invite alone) still opens, with no one to check against', () => {
		const old = `#${encodeURIComponent(INVITE)}`;
		expect(readConnectHash(old)).toEqual({ invite: INVITE, members: [] });
		expect(isExpected([], BOB)).toBe(true);
	});

	it('drops what does not read — never a guess', () => {
		expect(readConnectHash('#for=0x123,nonsense&%E0%A4%A')).toEqual({ invite: null, members: [] });
		expect(readConnectHash('')).toEqual({ invite: null, members: [] });
		expect(connectLink(PAGE, null, ['0xnope'])).toBe(PAGE);
	});

	it('a member is one the link named, in any case', () => {
		const members = [ALICE.toLowerCase()];
		expect(isExpected(members, ALICE)).toBe(true);
		expect(isExpected(members, BOB)).toBe(false);
	});
});
