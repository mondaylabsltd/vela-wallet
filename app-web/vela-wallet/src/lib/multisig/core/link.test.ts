import { describe, expect, it } from 'vitest';
import {
	addLink,
	addLinkFault,
	brokenLink,
	checksumHolds,
	brokenLinkText,
	carriedLinkHash,
	isMultisigLink,
	parseAddLink,
	parseProposalLink,
	proposalLink,
	shareOrigin
} from './link';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const HASH = '0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa';

describe('links', () => {
	it('round-trip a transaction to approve', () => {
		const link = proposalLink('https://wallet.getvela.app', 'zh', {
			chainId: 100,
			multisig: SAFE,
			safeTxHash: HASH
		});
		expect(link).toBe(`https://wallet.getvela.app/zh/wallet#ms=100.${SAFE}.${HASH}`);
		expect(parseProposalLink(new URL(link).hash)).toEqual({
			chainId: 100,
			multisig: SAFE,
			safeTxHash: HASH
		});
	});

	it('round-trip a multisig to add', () => {
		const link = addLink('https://wallet.getvela.app', 'en', SAFE);
		expect(parseAddLink(new URL(link).hash)).toEqual({
			multisig: SAFE,
			chainIds: [],
			members: null
		});
		expect(parseProposalLink(new URL(link).hash)).toBeNull();
	});

	it('read nothing from anything else', () => {
		expect(parseProposalLink('')).toBeNull();
		expect(parseProposalLink('#ms=0.' + SAFE + '.' + HASH)).toBeNull();
		expect(parseProposalLink('#ms=100.0x1234.' + HASH)).toBeNull();
		expect(parseProposalLink('#ms=100.' + SAFE + '.0x12')).toBeNull();
		expect(parseProposalLink('#ms=1e2.' + SAFE + '.' + HASH)).toBeNull();
		expect(parseAddLink('#msa=not-an-address')).toBeNull();
		expect(parseAddLink('#ms=100.' + SAFE + '.' + HASH)).toBeNull();
	});

	it('name the web wallet when made inside the extension, whose own address opens nowhere else', () => {
		expect(shareOrigin('chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl')).toBe(
			'https://wallet.getvela.app'
		);
		expect(shareOrigin('moz-extension://abc')).toBe('https://wallet.getvela.app');
		// A web page's own origin is the one to share (a dev server, a preview).
		expect(shareOrigin('http://localhost:5173')).toBe('http://localhost:5173');
		expect(shareOrigin('https://wallet.getvela.app')).toBe('https://wallet.getvela.app');
		expect(
			proposalLink(shareOrigin('chrome-extension://x'), 'en', {
				chainId: 100,
				multisig: SAFE,
				safeTxHash: HASH
			})
		).toBe(`https://wallet.getvela.app/en/wallet#ms=100.${SAFE}.${HASH}`);
	});
});

describe('links as chat apps and mail clients carry them (r4b-17)', () => {
	it('a percent-encoded `=` (#ms%3D…, #msa%3D…) reads the same', () => {
		expect(parseProposalLink(`#ms%3D100.${SAFE}.${HASH}`)).toEqual({
			chainId: 100,
			multisig: SAFE,
			safeTxHash: HASH
		});
		expect(parseAddLink(`#msa%3D${SAFE}`)?.multisig).toBe(SAFE);
		expect(parseAddLink(`# msa=${SAFE} `)?.multisig).toBe(SAFE);
	});

	it('one with a part too many is damaged, and said so', () => {
		expect(parseProposalLink(`#ms=100.${SAFE}.${HASH}.extra`)).toBeNull();
		expect(brokenLink(`#ms=100.${SAFE}.${HASH}.extra`)).toBe('proposal');
		expect(brokenLink(`#msa%3D${SAFE.slice(0, 30)}`)).toBe('add');
		expect(brokenLinkText(`#msa%3D${SAFE.slice(0, 30)}`)).toBe(SAFE.slice(0, 30));
	});

	it('a link opened on another page (`/en#msa=…`) is carried to the wallet', () => {
		expect(carriedLinkHash(`#msa=${SAFE}`)).toBe(`#msa=${SAFE}`);
		expect(carriedLinkHash(`#ms%3D100.${SAFE}.${HASH}`)).toBe(`#ms=100.${SAFE}.${HASH}`);
		expect(carriedLinkHash('#intro')).toBe('');
		expect(carriedLinkHash('')).toBe('');
	});
});

/**
 * r4D-4, r4D-5, r4b-8, r4b-13, r4b-17: an invite carries the networks the
 * multisig is used on and the labels its inviter knows — all in the
 * fragment — and every older or mangled form either reads or says it is
 * damaged; none is silently dropped.
 */
describe('the invite link', () => {
	it('carries its networks, the first first, and its members’ labels — after # only', () => {
		const link = addLink('https://wallet.getvela.app', 'en', SAFE, {
			chainIds: [5042002, 100, 5042002],
			members: [
				{ name: 'Alice', kind: 'passkey' },
				{ name: 'Bob, the treasurer', kind: 'wallet' },
				{}
			]
		});
		const url = new URL(link);
		expect(url.search).toBe('');
		expect(url.pathname).toBe('/en/wallet');
		expect(url.hash).toBe(`#msa=5042002,100:${SAFE}&m=pAlice,wBob%2C%20the%20treasurer,-`);
		expect(parseAddLink(url.hash)).toEqual({
			multisig: SAFE,
			chainIds: [5042002, 100],
			members: [
				{ name: 'Alice', kind: 'passkey' },
				{ name: 'Bob, the treasurer', kind: 'wallet' },
				{}
			]
		});
	});

	it('names no members when the inviter knows none', () => {
		const link = addLink('https://x', 'en', SAFE, { chainIds: [100], members: [{}, {}] });
		expect(new URL(link).hash).toBe(`#msa=100:${SAFE}`);
	});

	it('reads the link from before networks rode along: no network, then the person is asked', () => {
		expect(parseAddLink(`#msa=${SAFE}`)).toEqual({ multisig: SAFE, chainIds: [], members: null });
		expect(brokenLink(`#msa=${SAFE}`)).toBeNull();
	});

	it('reads `=` encoded as %3D (r4b-17), whole or in part', () => {
		expect(parseAddLink(`#msa%3D${SAFE}`)?.multisig).toBe(SAFE);
		expect(parseAddLink(`#msa%3D5042002%3A${SAFE}`)?.chainIds).toEqual([5042002]);
		expect(parseProposalLink(`#ms%3D100.${SAFE}.${HASH}`)?.safeTxHash).toBe(HASH);
		expect(isMultisigLink(`#msa%3d${SAFE}`)).toBe(true);
	});

	it('says a damaged link is damaged, at once (r4b-13)', () => {
		expect(brokenLink(`#msa=${SAFE.slice(0, 30)}`)).toBe('add');
		expect(brokenLink(`#msa=5042x:${SAFE}`)).toBe('add');
		expect(brokenLink(`#msa=0:${SAFE}`)).toBe('add');
		expect(brokenLink(`#ms=100.${SAFE}.0x12`)).toBe('proposal');
		expect(brokenLinkText(`#msa=${SAFE.slice(0, 30)}`)).toBe(SAFE.slice(0, 30));
		expect(brokenLink('#something-else')).toBeNull();
		expect(isMultisigLink('#something-else')).toBe(false);
	});

	it('drops labels that do not read, and still opens the multisig', () => {
		expect(parseAddLink(`#msa=100:${SAFE}&m=xAlice`)).toEqual({
			multisig: SAFE,
			chainIds: [100],
			members: null
		});
		expect(parseAddLink(`#msa=100:${SAFE}&m=p%E0%A4%A`)?.members).toBeNull();
		expect(parseAddLink(`#msa=100:${SAFE}&m=p${'a'.repeat(65)}`)?.members).toBeNull();
	});

	it('carries every member by address: the founding ones and those added since (r4D-5)', () => {
		const ALICE = '0xe331F2Ef7A6f4ED426529c8C10642b68084771A2';
		const BOB = '0x412f2165f299Dfc2363ED08F8Fc6DE4353E81fF6';
		const link = addLink('https://x', 'en', SAFE, {
			chainIds: [5042002],
			members: [
				{ address: ALICE, name: 'Alice', kind: 'passkey' },
				// Added in Settings: its typed name rides along.
				{ address: BOB, name: 'R5 Bob', kind: 'passkey' },
				{ address: SAFE.toLowerCase() }
			]
		});
		const hash = new URL(link).hash;
		expect(hash).toBe(
			`#msa=5042002:${SAFE}&a=${ALICE.slice(2).toLowerCase()}pAlice,` +
				`${BOB.slice(2).toLowerCase()}pR5%20Bob,${SAFE.slice(2).toLowerCase()}-`
		);
		// Read back in their checksummed case.
		expect(parseAddLink(hash)?.members).toEqual([
			{ address: ALICE, name: 'Alice', kind: 'passkey' },
			{ address: BOB, name: 'R5 Bob', kind: 'passkey' },
			{ address: SAFE }
		]);
		// One that does not read drops the labels, never the multisig.
		expect(parseAddLink(`#msa=5042002:${SAFE}&a=12pAlice`)?.members).toBeNull();
		expect(parseAddLink(`#msa=5042002:${SAFE}&a=12pAlice`)?.multisig).toBe(SAFE);
	});

	it('r7: carries a smart contract member as `c`, and reads the links from before as they were', () => {
		const ALICE = '0xe331F2Ef7A6f4ED426529c8C10642b68084771A2';
		const BOB = '0x412f2165f299Dfc2363ED08F8Fc6DE4353E81fF6';
		const link = addLink('https://x', 'en', SAFE, {
			chainIds: [100],
			members: [
				{ address: ALICE, kind: 'contract', name: 'Treasury Safe' },
				{ address: BOB, kind: 'wallet' }
			]
		});
		const hash = new URL(link).hash;
		expect(hash).toBe(
			`#msa=100:${SAFE}&a=${ALICE.slice(2).toLowerCase()}cTreasury%20Safe,` +
				`${BOB.slice(2).toLowerCase()}w`
		);
		expect(parseAddLink(hash)?.members).toEqual([
			{ address: ALICE, name: 'Treasury Safe', kind: 'contract' },
			{ address: BOB, kind: 'wallet' }
		]);
		// A link from before r7 (p, w, -) reads unchanged; the genesis-order form too.
		expect(parseAddLink(`#msa=100:${SAFE}&m=pAlice,wBob,-`)?.members).toEqual([
			{ name: 'Alice', kind: 'passkey' },
			{ name: 'Bob', kind: 'wallet' },
			{}
		]);
		expect(parseAddLink(`#msa=100:${SAFE}&m=cSafe`)?.members).toEqual([
			{ name: 'Safe', kind: 'contract' }
		]);
	});

	it('refuses an address whose letter case was changed on the way (r5reg-22)', () => {
		// One letter's case flipped: not its checksum.
		const flipped = SAFE.replace('aEF', 'AEF');
		expect(flipped).not.toBe(SAFE);
		expect(checksumHolds(flipped)).toBe(false);
		expect(parseAddLink(`#msa=5042002:${flipped}`)).toBeNull();
		expect(brokenLink(`#msa=5042002:${flipped}`)).toBe('add');
		expect(addLinkFault(`#msa=5042002:${flipped}`)).toBe('checksum');
		// All one case carries no checksum, and reads.
		expect(parseAddLink(`#msa=5042002:${SAFE.toLowerCase()}`)?.multisig).toBe(SAFE.toLowerCase());
		expect(checksumHolds(SAFE)).toBe(true);
		// A cut-short one is damaged, not a checksum.
		expect(addLinkFault(`#msa=5042002:${SAFE.slice(0, 30)}`)).toBe('damaged');
		expect(addLinkFault(`#msa=5042002:${SAFE}`)).toBeNull();
	});
});
