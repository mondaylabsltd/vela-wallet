/**
 * perf-10: settings and the address book NOTE the account (`account_noted`)
 * rather than set it — the REAL `balance_dashboard` core, over the JSON seam
 * the resident uses. Opened directly, those pages read 24 networks for a
 * figure nobody saw; now the cache is read and no network is, until the
 * wallet home (or the account switcher) shows the balance.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { BalanceDashboardCore } from '$lib/core/client';
import type { BalanceEvent } from '$lib/core/generated/BalanceEvent';

const ADDRESS = '0x14fb1fb21751e29f7ec48dc450017552e3d1ea5c';

type Out = { effects: { id: number; operation: { type: string } }[] };

function asked(core: BalanceDashboardCore, event: BalanceEvent): string[] {
	const out = JSON.parse(core.dispatch(JSON.stringify(event))) as Out;
	return out.effects.map((effect) => effect.operation.type);
}

describe('a noted account', () => {
	it('reads its cached total and asks no network', () => {
		const core = new BalanceDashboardCore();
		expect(asked(core, { type: 'account_noted', address: ADDRESS })).toEqual([
			'read_balance_cache'
		]);
		expect(asked(core, { type: 'app_focused' })).toEqual([]);
	});

	it('is read when the wallet home sets the same account', () => {
		const core = new BalanceDashboardCore();
		asked(core, { type: 'account_noted', address: ADDRESS });
		expect(asked(core, { type: 'account_changed', address: ADDRESS })).toEqual(['fetch_tokens']);
	});

	it('is read when the account switcher opens', () => {
		const core = new BalanceDashboardCore();
		asked(core, { type: 'account_noted', address: ADDRESS });
		expect(asked(core, { type: 'switcher_opened', addresses: [ADDRESS] })).toContain(
			'fetch_tokens'
		);
	});
});
