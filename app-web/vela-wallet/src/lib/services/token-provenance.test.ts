/**
 * r4a-2: the note a token nobody vouches for wears — the core's ruling
 * (`token_provenance`), worded. The round-4 case: a stranger's "USDC" on Arc,
 * where USDC is the chain's own coin.
 */
import { describe, expect, it } from 'vitest';
// The core, initialised at import for the build-time resolver.
import '$lib/i18n/engine.server';
import { shortenAddress } from '$lib/wallet/identity';
import { provenanceNote, provenanceOf } from './token-provenance';

const ARC = 5042002;
const FAKE = '0x57e64022d8d9a420dFE08a22d6a687caaBc6B7C6';
const words = { unverified: 'Unverified token', lookalike: 'Not the real {{symbol}}' };

describe('token provenance', () => {
	it('the native coin and a registry token are vouched for', () => {
		expect(provenanceOf(ARC, null, 'USDC').trust).toBe('native');
		expect(provenanceOf(8453, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913', 'USDC').trust).toBe(
			'vouched'
		);
		expect(provenanceNote(ARC, null, 'USDC', words)).toBeNull();
	});

	it('a lookalike of the native coin says it is not the real one, in the danger tone', () => {
		expect(provenanceOf(ARC, FAKE, 'USDC')).toEqual({ trust: 'unverified', imitates: 'USDC' });
		expect(provenanceNote(ARC, FAKE, 'usdc', words)).toEqual({
			text: 'Not the real USDC',
			tone: 'danger',
			contract: FAKE,
			short: shortenAddress(FAKE),
			explorerUrl: `https://testnet.arcscan.app/address/${FAKE}`
		});
	});

	it('any other unverified token shows its contract', () => {
		const note = provenanceNote(ARC, FAKE, 'R4T', words);
		expect(note?.text).toBe('Unverified token');
		expect(note?.tone).toBe('caution');
		expect(note?.short).toBe(shortenAddress(FAKE));
	});
});
