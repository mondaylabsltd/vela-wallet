import { describe, expect, it } from 'vitest';
import {
	addLink,
	brokenAddText,
	brokenLink,
	carriedLinkHash,
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
		expect(parseAddLink(new URL(link).hash)).toBe(SAFE);
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
		expect(parseAddLink(`#msa%3D${SAFE}`)).toBe(SAFE);
		expect(parseAddLink(`# msa=${SAFE} `)).toBe(SAFE);
	});

	it('one with a part too many is damaged, and said so', () => {
		expect(parseProposalLink(`#ms=100.${SAFE}.${HASH}.extra`)).toBeNull();
		expect(brokenLink(`#ms=100.${SAFE}.${HASH}.extra`)).toBe('proposal');
		expect(brokenLink(`#msa%3D${SAFE.slice(0, 30)}`)).toBe('add');
		expect(brokenAddText(`#msa%3D${SAFE.slice(0, 30)}`)).toBe(SAFE.slice(0, 30));
	});

	it('a link opened on another page (`/en#msa=…`) is carried to the wallet', () => {
		expect(carriedLinkHash(`#msa=${SAFE}`)).toBe(`#msa=${SAFE}`);
		expect(carriedLinkHash(`#ms%3D100.${SAFE}.${HASH}`)).toBe(`#ms=100.${SAFE}.${HASH}`);
		expect(carriedLinkHash('#intro')).toBe('');
		expect(carriedLinkHash('')).toBe('');
	});
});
