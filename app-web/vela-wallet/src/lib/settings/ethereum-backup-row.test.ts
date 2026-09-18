/**
 * The Ethereum backup row (spec 062): one line, three states, a button only
 * while there is something to do — and nothing at all when the feature is dark.
 */
import { describe, expect, it } from 'vitest';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import { ethereumBackupRow, withLiveEthereumBackup } from './live';

const m = resolveSettingsMessages('en');
const model = {
	sections: [
		{ rows: [{ id: 'contacts', title: 'Contacts' }] },
		{ label: 'Advanced', rows: [{ id: 'storage', title: 'Storage' }] }
	]
};

describe('ethereumBackupRow', () => {
	it('says where the record stands, and is a button only when it is not backed up', () => {
		expect(ethereumBackupRow('not_backed_up', m)).toEqual({
			title: 'Back up keys to Ethereum',
			subtitle: 'Not backed up yet',
			tone: 'caution',
			actionable: true
		});
		expect(ethereumBackupRow('backed_up', m)).toMatchObject({
			subtitle: 'Backed up on Ethereum',
			tone: 'positive',
			actionable: false
		});
		// Silence is never drawn as a verdict — and never as something to tap.
		expect(ethereumBackupRow('could_not_check', m)).toMatchObject({
			subtitle: 'Could not check',
			actionable: false
		});
		expect(ethereumBackupRow('checking', m)).toMatchObject({ actionable: false });
	});

	it('draws nothing while the feature is dark or the wallet has no record', () => {
		expect(ethereumBackupRow('unavailable', m)).toBeUndefined();
		expect(ethereumBackupRow('not_registered', m)).toBeUndefined();
		expect(withLiveEthereumBackup(model, 'unavailable', m)).toBe(model);
	});

	it('joins the first block, with a chevron only when it can be tapped', () => {
		const live = withLiveEthereumBackup(model, 'not_backed_up', m);
		expect(live.sections[0].rows.map((r) => r.id)).toEqual(['contacts', 'ethereum-backup']);
		expect(live.sections[0].rows[1]).toMatchObject({ trailing: 'chevron' });
		expect(live.sections[1]).toBe(model.sections[1]);
		const done = withLiveEthereumBackup(model, 'backed_up', m);
		expect(done.sections[0].rows[1]).toMatchObject({ trailing: 'none' });
	});

	it('every locale has words for every state', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const words = resolveSettingsMessages(locale).backup;
			for (const value of Object.values(words)) expect(value, locale).not.toBe('');
		}
	});
});
