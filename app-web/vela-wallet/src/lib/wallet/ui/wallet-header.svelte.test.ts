/**
 * ob-2: the header's line under a wallet saved here whose registration has
 * not landed. Quiet while the session still tries on its own; once it has
 * failed a while, "not synced yet" and a retry that asks the session now.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import WalletHeader from './WalletHeader.svelte';
import type { WalletHeaderModel } from '../model';

const syncPending = vi.hoisted(() => vi.fn());
vi.mock('$lib/session/core/session.svelte', () => ({ session: { syncPending } }));

const base: WalletHeaderModel = {
	name: 'Everyday',
	addressDisplay: '0xAAAA00…000001',
	addressFull: '0xAAAA000000000000000000000000000000000001',
	identiconSvg: '<svg></svg>'
};

describe('the header’s sync line (ob-2)', () => {
	it('draws nothing for a synced wallet', () => {
		const screen = render(WalletHeader, { props: { header: base, onclick: () => {} } });
		expect(screen.container.querySelector('.sync[role="status"]')).toBeNull();
	});

	it('says syncing, with no button, while the session still tries', () => {
		const screen = render(WalletHeader, {
			props: {
				header: { ...base, sync: { text: 'Saved on this device · syncing…', stalled: false } },
				onclick: () => {}
			}
		});
		expect(screen.container.querySelector('.sync[role="status"]')?.textContent).toContain(
			'Saved on this device · syncing…'
		);
		expect(screen.container.querySelector('.retry')).toBeNull();
	});

	it('offers a retry once it has failed a while, and asks the session', () => {
		syncPending.mockClear();
		const screen = render(WalletHeader, {
			props: {
				header: {
					...base,
					sync: { text: 'Saved on this device · not synced yet', stalled: true, retry: 'Retry now' }
				},
				onclick: () => {}
			}
		});
		const retry = screen.container.querySelector<HTMLButtonElement>('.retry');
		expect(retry?.textContent?.trim()).toBe('Retry now');
		retry?.click();
		expect(syncPending).toHaveBeenCalledOnce();
	});
});
