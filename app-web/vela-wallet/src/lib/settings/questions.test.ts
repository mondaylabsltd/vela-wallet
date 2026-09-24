/**
 * The questions destructive rows ask first (spec 058, spec 072): titled with
 * what goes, worded from what the row already says.
 */
import { describe, expect, it } from 'vitest';
import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import { buildDesktopState, buildMobileState } from './fixtures';
import { removeNetworkQuestion, storageClearQuestion } from './questions';

const m = resolveSettingsMessages('en');
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;

describe('storageClearQuestion', () => {
	it('names the row, carries its group’s warning, and confirms with its own word', () => {
		const storage = buildMobileState('st13', m, IDENTICON).storage;
		expect(storageClearQuestion(storage, 'contacts', m.common.cancel)).toEqual({
			title: m.storage.itemContacts,
			body: m.storage.userData,
			confirm: m.storage.clear,
			cancel: m.common.cancel,
			tone: 'danger'
		});
		// A cache's Clear is not destructive, and its question says so by tone.
		expect(storageClearQuestion(storage, 'rates', m.common.cancel)?.tone).toBe('accent');
		expect(storageClearQuestion(storage, 'nothing-drawn', m.common.cancel)).toBeUndefined();
	});
});

describe('removeNetworkQuestion', () => {
	it('is titled with the network, in the corpus’s own words', () => {
		const networks = buildDesktopState('dst4', m, IDENTICON).networks;
		expect(removeNetworkQuestion(networks, 'xlayer')).toEqual({
			title: 'X Layer',
			body: m.networks.removeBody,
			confirm: m.networks.removeConfirm,
			cancel: m.networks.removeCancel,
			tone: 'danger'
		});
		expect(removeNetworkQuestion(networks, 'unknown')).toBeUndefined();
	});
});
