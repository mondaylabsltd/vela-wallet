/**
 * Nobody is on a developer's multisig service without being told: the
 * multisig area says "Using a custom multisig service (host)" while the
 * person's own is in use, and nothing with the default. And the waiting
 * group's note carries its Retry while the service is away.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import GroupNote from '$lib/wallet/ui/GroupNote.svelte';
import type { MultisigMessages } from '../messages';
import { saveServiceEndpoints } from '$lib/onboarding/core/storage';
import CustomServiceNote from './CustomServiceNote.svelte';

const m = {
	service: { customNote: 'Using a custom multisig service ({{host}})' }
} as unknown as MultisigMessages;

afterEach(() => localStorage.removeItem('vela.serviceEndpoints'));

describe('the custom-service note', () => {
	it('names the host of the person’s own service, and goes with the default', async () => {
		saveServiceEndpoints({ genesisServiceURL: 'http://127.0.0.1:8788' });
		const screen = render(CustomServiceNote, { props: { m } });
		await tick();
		expect(screen.container.textContent?.trim()).toBe(
			'Using a custom multisig service (127.0.0.1:8788)'
		);
		// Settings → Service Endpoints writes the record: the note follows.
		saveServiceEndpoints({ genesisServiceURL: 'https://safe-genesis.getvela.app' });
		await tick();
		expect(screen.container.querySelector('[data-testid="custom-genesis-service"]')).toBeNull();
	});
});

describe("an activity group's note", () => {
	it('carries its action, and says which was pressed', async () => {
		const pressed: string[] = [];
		const screen = render(GroupNote, {
			props: {
				group: {
					label: 'Waiting',
					note: 'Multisig service unreachable',
					action: { id: 'multisig-retry', label: 'Retry' },
					rows: []
				},
				onaction: (id: string) => pressed.push(id)
			}
		});
		await tick();
		expect(screen.container.querySelector('[role="status"]')?.textContent).toBe(
			'Multisig service unreachable'
		);
		screen.container.querySelector('button')?.click();
		expect(pressed).toEqual(['multisig-retry']);
	});

	it('is only the note without an action (or in the gallery)', async () => {
		const screen = render(GroupNote, {
			props: { group: { label: 'Waiting', note: 'stale', rows: [] } }
		});
		await tick();
		expect(screen.container.textContent?.trim()).toBe('stale');
		expect(screen.container.querySelector('button')).toBeNull();
	});
});
