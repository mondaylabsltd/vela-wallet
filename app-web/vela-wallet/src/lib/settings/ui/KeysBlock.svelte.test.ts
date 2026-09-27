/**
 * The key this device signs with stands out in Settings' keys list (founder,
 * 2026-09-26), in a real browser: the one FILLED pill, first among the row's
 * pills, with a check — and on no other row. A `.svelte.test.ts` because the
 * point is what a person sees; the node project does not render components.
 */
import { beforeAll, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import type { CreateKeyRow } from '$lib/onboarding/generated/CreateKeyRow';
import type { WalletKeyRowModel, WalletKeysModel } from '../model';
import KeysBlock from './KeysBlock.svelte';

beforeAll(() => loadCore());

const key: CreateKeyRow = {
	name: '',
	authenticator_attachment: 'platform',
	transports: 'internal',
	confirmed: true,
	synced: false,
	synced_known: true,
	aaguid: '',
	provider_name: '',
	method: 'platform',
	kind: 'platform'
};

const row = (name: string, pills: WalletKeyRowModel['pills']): WalletKeyRowModel => ({
	name,
	holderFallback: 'Built-in passkey',
	fingerprint: '197d…647b',
	pills,
	details: [],
	key: { ...key, name }
});

const model: WalletKeysModel = {
	title: 'Keys',
	subtitle: 'Any one of these passkeys can sign for this wallet, on every network.',
	count: '3',
	loading: false,
	rows: [
		row('Parallel One', [{ text: 'Device-bound', tone: 'local' }]),
		row('Parallel Two', [
			{ text: 'Signed in', tone: 'signs_here' },
			{ text: 'Device-bound', tone: 'local' }
		]),
		row('Parallel Three', [{ text: 'Device-bound', tone: 'local' }])
	],
	backupExplain: '',
	copy: { action: 'Copy', done: 'Copied' }
};

describe('the key this device signs with', () => {
	it('wears the one filled pill, first in its row, and no other row does', async () => {
		const screen = render(KeysBlock, { props: { model } });
		const rows = [...screen.container.querySelectorAll('li.key')];
		const marked = rows.filter((li) => li.querySelector('.pill[data-tone="signs_here"]'));
		expect(marked).toHaveLength(1);
		expect(marked[0].textContent).toContain('Parallel Two');

		const pills = [...marked[0].querySelectorAll('.pill')];
		expect(pills[0].getAttribute('data-tone')).toBe('signs_here');
		expect(pills[0].textContent?.trim()).toBe('Signed in');
		expect(pills[0].querySelector('svg')).not.toBeNull();

		// Filled, where the others are outlined — and the same height as they are.
		const filled = getComputedStyle(pills[0]);
		const outlined = getComputedStyle(pills[1]);
		expect(filled.backgroundColor).not.toBe('rgba(0, 0, 0, 0)');
		expect(outlined.backgroundColor).toBe('rgba(0, 0, 0, 0)');
		expect(filled.fontWeight).not.toBe(outlined.fontWeight);
		expect(pills[0].getBoundingClientRect().height).toBeCloseTo(
			pills[1].getBoundingClientRect().height,
			0
		);
	});
});
