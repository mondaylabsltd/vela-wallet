/**
 * Inviting members: the multisig's artwork opens the identicon viewer on
 * its address, as every artwork does (founder call, 2026-09-05).
 */
import { tick } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { identiconViewer } from '$lib/wallet/identicon-viewer.svelte';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
/** r4D-8: the multisigs whose invite was handed on (`multisig.inviteShared`). */
const invited = vi.hoisted(() => [] as string[]);

vi.mock('../core/store.svelte', () => ({
	multisig: {
		find: () => null,
		row: () => null,
		shared: () => true,
		inviteShared: (address: string) => invited.push(address),
		accounts: []
	}
}));
vi.mock('$lib/services/public-key-index', () => ({
	queryWalletName: async () => null,
	lookupWalletName: async () => null
}));
vi.mock('$lib/wallet/identicon', () => ({
	identiconSvgForClient: (a: string) => `<svg data-seed="${a}"></svg>`
}));

const { default: InviteSheet } = await import('./InviteSheet.svelte');
const { loadCore } = await import('$lib/core/client');

// The invite is the core's codec: loaded, as the wallet loads it before any sheet.
beforeAll(() => loadCore());

function words(): MultisigMessages {
	const out: Record<string, Record<string, string>> = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const key of Object.keys(keys)) out[group][key] = `${group}.${key}`;
	}
	return out as unknown as MultisigMessages;
}

afterEach(() => identiconViewer.close());

describe('the invite sheet', () => {
	it("opens the viewer on the multisig's address", async () => {
		render(InviteSheet, {
			props: {
				m: words(),
				account: { address: SAFE, genesis: {}, owners: [], threshold: 1, chainIds: [] } as never,
				locale: 'en',
				wide: true,
				closeLabel: 'Close',
				onclose: () => {}
			}
		});
		await tick();
		const art = document.body.querySelector<HTMLButtonElement>('.hero button.identicon');
		expect(art).not.toBeNull();
		art?.click();
		expect(identiconViewer.current?.address).toBe(SAFE);
	});

	it('r4D-16: a code to scan, the network, the outside-wallet hint, and a link that carries them', async () => {
		let copied = '';
		const clip = vi
			.spyOn(navigator.clipboard, 'writeText')
			.mockImplementation(async (t: string) => void (copied = t));
		render(InviteSheet, {
			props: {
				m: words(),
				account: {
					address: SAFE,
					genesis: {},
					owners: ['0x70997970C51812dc3A010C7d01b50e0d17dc79C8'],
					threshold: 1,
					chainIds: [5042002]
				} as never,
				locale: 'en',
				wide: true,
				closeLabel: 'Close',
				onclose: () => {}
			}
		});
		await tick();
		expect(document.body.querySelector('[data-testid="invite-qr"] path')).not.toBeNull();
		expect(document.body.querySelector('[data-testid="invite-network"]')?.textContent).toContain(
			'add.usedOn'
		);
		expect(document.body.textContent).toContain('invite.outsideHint');
		[...document.body.querySelectorAll('button')]
			.find((b) => b.textContent?.trim() === 'proposal.copyLink')
			?.click();
		await vi.waitFor(() => expect(copied).not.toBe(''));
		expect(new URL(copied).hash.startsWith(`#msa=5042002:${SAFE}`)).toBe(true);
		// r4D-8: handed on — its first steps tick off.
		expect(invited).toContain(SAFE);
		clip.mockRestore();
	});
});
