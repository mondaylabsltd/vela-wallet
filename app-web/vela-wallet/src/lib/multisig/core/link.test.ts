import { describe, expect, it } from 'vitest';
import { addLink, parseAddLink, parseProposalLink, proposalLink } from './link';

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
});
