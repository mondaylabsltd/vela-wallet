/**
 * The links are the core's codec (`multisig_link`; every method's vectors in
 * rust/crates/vela-core/tests/multisig_link_vectors.rs): here, only that the
 * web reaches it — a link made here reads back as the core reads links.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { addLink, proposalLink, shareOrigin } from './link';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const HASH = '0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa';

const read = <T>(method: string, hash: string) =>
	JSON.parse(wasm.multisigLinkCall(method, JSON.stringify({ hash }))) as T;

describe('links, by the core', () => {
	it('round-trip a transaction to approve', () => {
		const link = proposalLink('https://wallet.example', 'en', {
			chainId: 100,
			multisig: SAFE,
			safeTxHash: HASH
		});
		expect(link).toBe(`https://wallet.example/en/wallet#ms=100.${SAFE}.${HASH}`);
		expect(read('parseProposalLink', new URL(link).hash)).toEqual({
			chainId: 100,
			multisig: SAFE,
			safeTxHash: HASH
		});
	});

	it('round-trip a multisig to add, with its networks and members', () => {
		const link = addLink('https://wallet.example', 'en', SAFE, {
			chainIds: [5042002, 100],
			members: [{ address: SAFE.toLowerCase(), name: 'Alice', kind: 'passkey' }]
		});
		expect(read('parseAddLink', new URL(link).hash)).toEqual({
			multisig: SAFE,
			chainIds: [5042002, 100],
			members: [{ address: SAFE, name: 'Alice', kind: 'passkey' }]
		});
	});

	it('name the web wallet when made inside the extension, whose own address opens nowhere else', () => {
		expect(shareOrigin('https://wallet.example')).toBe('https://wallet.example');
		expect(shareOrigin('chrome-extension://abcdef')).toBe('https://wallet.getvela.app');
	});
});
