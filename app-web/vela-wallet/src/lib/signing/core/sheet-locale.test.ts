/**
 * Spec 097 E: the sheet reads a dApp request in the person's locale and this
 * device's zone — the presets Settings → Region format resolves, and the UTC
 * offset now. The web passed fixed presets (ISO, 24 h, comma-dot) whatever the
 * person chose; the desktop passed none at all, and a swap deadline at 01:56
 * read "10/02/2026, 17:56".
 */
import { afterEach, describe, expect, it, vi } from 'vitest';

const started = vi.hoisted(() => ({ events: [] as unknown[] }));

vi.mock('$lib/core/client', () => ({ loadCore: async () => {} }));
vi.mock('$lib/core/kernels', () => ({ typedDataDocument: () => '{}' }));
vi.mock('./clear-session', () => ({
	createClearSigningSession: () => ({
		start: (event: unknown) => started.events.push(event),
		dispose: () => {}
	})
}));
vi.mock('./guard-session', () => ({
	createApprovalGuardSession: () => ({ start: () => {}, dispatch: () => {}, dispose: () => {} })
}));

import { preferences } from '$lib/services/preferences.svelte';
import { signingSheet } from './sheet.svelte';
import type { SignRequestView } from '$lib/core/generated/SignRequestView';

function request(id: string, kind: string, method: string, params: string): SignRequestView {
	return {
		id,
		kind,
		method,
		params_json: params,
		chain_id: 56,
		origin: 'https://pancakeswap.finance'
	} as unknown as SignRequestView;
}

afterEach(() => {
	signingSheet.dismiss();
	started.events.length = 0;
	preferences.numberFormat = 'auto';
	preferences.dateFormat = 'auto';
	preferences.timeFormat = 'auto';
});

describe("the sheet's locale (spec 097 E)", () => {
	it("is the person's presets and this device's offset, for every reading", async () => {
		preferences.numberFormat = 'dot_comma';
		preferences.dateFormat = 'dmy_dot';
		preferences.timeFormat = 'h12';
		await signingSheet.present(request('r1', 'batch', 'wallet_sendCalls', '[{"calls":[]}]'), null);
		await signingSheet.present(
			request('r2', 'transaction', 'eth_sendTransaction', '[{"to":"0x01"}]'),
			null
		);
		await signingSheet.present(
			request('r3', 'typed_data', 'eth_signTypedData_v4', '["0x01","{}"]'),
			null
		);
		const locales = started.events.map((event) => (event as { locale?: unknown }).locale);
		expect(locales).toHaveLength(3);
		for (const locale of locales) {
			expect(locale).toEqual({
				number_format: 'dot_comma',
				date_format: 'dmy_dot',
				time_format: 'h12',
				tz_offset_minutes: -new Date().getTimezoneOffset()
			});
		}
	});
});
