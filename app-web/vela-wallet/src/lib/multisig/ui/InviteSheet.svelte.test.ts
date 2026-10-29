/**
 * Inviting members: the multisig's artwork opens the identicon viewer on
 * its address, as every artwork does (founder call, 2026-09-05).
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { identiconViewer } from '$lib/wallet/identicon-viewer.svelte';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';

vi.mock('../core/store.svelte', () => ({
	isShared: () => true,
	multisig: { find: () => null }
}));
vi.mock('$lib/wallet/identicon', () => ({
	identiconSvgForClient: (a: string) => `<svg data-seed="${a}"></svg>`
}));

const { default: InviteSheet } = await import('./InviteSheet.svelte');

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
				account: { address: SAFE, genesis: {}, owners: [], threshold: 1 } as never,
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
});
