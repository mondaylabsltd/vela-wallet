/**
 * A multisig's members in Settings: each member's artwork opens the
 * identicon viewer on that member's address (founder call, 2026-09-05 —
 * every artwork drawn from an address, not just the header's). The owner
 * found these were pictures that did nothing.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { identiconViewer } from '$lib/wallet/identicon-viewer.svelte';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const ME = '0x00000000000000000000000000000000000000aa';
const OTHER = '0x00000000000000000000000000000000000000bb';
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';

vi.mock('../core/proposals.svelte', () => ({ proposals: { chains: [] } }));
vi.mock('../core/backup', () => ({
	checkBackup: async () => 'unavailable',
	startBackup: async () => ({ kind: 'cancelled' })
}));
vi.mock('../core/owners', () => ({
	localOwners: (owners: string[]) =>
		owners
			.filter((o) => o.toLowerCase() === ME)
			.map((address) => ({ address, name: 'Mine', account: { address } }))
}));
vi.mock('../core/names.svelte', () => ({ names: { label: (a: string) => a.slice(0, 6) } }));
vi.mock('../core/store.svelte', () => ({
	isShared: () => true,
	multisig: { find: () => null, remove: () => {} }
}));
vi.mock('$lib/wallet/identicon', () => ({
	identiconSvgForClient: (a: string) => `<svg data-seed="${a}"></svg>`
}));

const { default: MultisigSettings } = await import('./MultisigSettings.svelte');

function words(): MultisigMessages {
	const out: Record<string, Record<string, string>> = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const key of Object.keys(keys)) out[group][key] = `${group}.${key}`;
	}
	out.panel.title = '{{m}} of {{n}}';
	return out as unknown as MultisigMessages;
}

afterEach(() => {
	identiconViewer.close();
	identiconViewer.openLabel = '';
});

describe('the members list (Settings → Account)', () => {
	it('opens the viewer on the member whose artwork is tapped', async () => {
		identiconViewer.openLabel = 'View identicon';
		const screen = render(MultisigSettings, {
			props: {
				m: words(),
				account: { address: SAFE, genesis: {}, owners: [ME, OTHER], threshold: 2 } as never,
				payer: null,
				cancelLabel: 'Cancel'
			}
		});
		await tick();
		const rows = [...screen.container.querySelectorAll('.members li')];
		expect(rows).toHaveLength(2);
		const art = rows.map((li) => li.querySelector<HTMLButtonElement>('button.identicon'));
		expect(art.every((b) => b !== null)).toBe(true);
		// Keyboard and screen reader: a button, named for what it opens.
		expect(art[1]?.getAttribute('aria-label')).toBe('View identicon');
		art[1]?.click();
		expect(identiconViewer.current).toEqual({
			address: OTHER,
			identiconSvg: `<svg data-seed="${OTHER}"></svg>`
		});
		art[0]?.click();
		expect(identiconViewer.current?.address).toBe(ME);
	});
});
