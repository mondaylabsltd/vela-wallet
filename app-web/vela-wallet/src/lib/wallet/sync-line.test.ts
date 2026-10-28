/**
 * ob-2: the home's line under a wallet saved here whose registration has not
 * landed — "syncing…" while the session still tries on its own, "not synced
 * yet" with a retry once it has failed a while, and nothing once it lands.
 */
import { describe, expect, it } from 'vitest';
import { desktopWithSync, homeWithSync, syncLine } from './identity';
import type { WalletDesktopModel, WalletHomeModel } from './model';

const words = { syncing: 'syncing…', syncStalled: 'not synced yet', syncRetry: 'Retry now' };
const ME = '0xAAAA000000000000000000000000000000000001';

describe('the sync line (ob-2)', () => {
	it('is nothing for a wallet that is synced', () => {
		expect(syncLine([], ME, words, 0)).toBeUndefined();
		expect(
			syncLine(
				[{ address: '0xBBBB000000000000000000000000000000000002', retrying: true }],
				ME,
				words,
				9
			)
		).toBeUndefined();
	});

	it('says syncing while the session still tries, then not synced yet with a retry', () => {
		const owed = [{ address: ME.toLowerCase(), retrying: true }];
		expect(syncLine(owed, ME, words, 0)).toEqual({ text: 'syncing…', stalled: false });
		expect(syncLine(owed, ME, words, 1)).toEqual({ text: 'syncing…', stalled: false });
		expect(syncLine(owed, ME, words, 2)).toEqual({
			text: 'not synced yet',
			stalled: true,
			retry: 'Retry now'
		});
	});

	it('offers no retry for one the session cannot finish on its own', () => {
		expect(syncLine([{ address: ME, retrying: false }], ME, words, 0)).toEqual({
			text: 'not synced yet',
			stalled: true
		});
	});

	it('rides on the header of either layout, and leaves it alone when there is none', () => {
		const sync = { text: 'syncing…', stalled: false };
		const header = { name: 'Me', addressDisplay: '', addressFull: ME, identiconSvg: '' };
		const home = { header } as unknown as WalletHomeModel;
		expect(homeWithSync(home, sync).header.sync).toEqual(sync);
		expect(homeWithSync(home, undefined)).toBe(home);
		const desktop = { sidebar: { header } } as unknown as WalletDesktopModel;
		expect(desktopWithSync(desktop, sync).sidebar.header.sync).toEqual(sync);
	});
});
