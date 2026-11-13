/**
 * An agent's request on its approval sheet (spec 103): what it names, in the
 * person's words — never a raw id — and the sentence in the locale's own
 * punctuation.
 */
import { describe, expect, it } from 'vitest';
import type { IntentAction } from '$lib/core/generated/IntentAction';
import { approvalDetail, approvalMessage, type DetailContext } from './detail';
import { resolveAgentMessages } from '$lib/i18n/engine.server';

const SAFE = '0x2222222222222222222222222222222222222222';

function context(locale: 'en' | 'zh' = 'en', over: Partial<DetailContext> = {}): DetailContext {
	return {
		messages: resolveAgentMessages(locale),
		locale,
		contacts: [{ address: '0x00000000219ab540356cbb839cbe05303d7705fa', name: 'Alice' }],
		groups: [{ id: 'g1', name: 'Work' }],
		networks: [{ chain_id: 100, display_name: 'Gnosis' }],
		checking: null,
		records: [
			{
				id: 'r1',
				user_op_hash: '0xop',
				value: '5',
				symbol: 'xDAI',
				to: '0x1111111111111111111111111111111111111111',
				to_name: null
			}
		],
		space: { mainnet: 'Mainnet', testnet: 'Testnet' },
		core: (action) =>
			action.type === 'switch_account'
				? `Main (0xaaaa…aaaa) → Team (${SAFE.slice(0, 6)}…${SAFE.slice(-4)})`
				: null,
		...over
	};
}

const detail = (action: IntentAction, ctx = context()) => approvalDetail(action, ctx);

describe('an approval’s detail, in the person’s words', () => {
	it('says a setting as Settings does — never "analytics: false"', () => {
		const en = resolveAgentMessages('en');
		expect(
			detail({ type: 'set_preference', preference: { kind: 'analytics', value: false } })
		).toBe(`${en.preferences.analytics}: ${en.off}`);
		expect(detail({ type: 'set_preference', preference: { kind: 'theme', value: 'dark' } })).toBe(
			`${en.preferences.theme}: ${en.values['theme.dark']}`
		);
		expect(
			detail({ type: 'set_preference', preference: { kind: 'fee_speed', value: 'fast' } })
		).toBe(`${en.preferences.fee_speed}: ${en.values['fee_speed.fast']}`);
		expect(
			detail({ type: 'set_preference', preference: { kind: 'date_format', value: 'iso' } })
		).toContain('2026-12-31');
		expect(
			detail({ type: 'set_preference', preference: { kind: 'text_scale', value: 'comfortable' } })
		).toContain('110');
	});

	it('uses the locale’s colon, and its words, in Chinese', () => {
		const zh = resolveAgentMessages('zh');
		const said = detail(
			{ type: 'set_preference', preference: { kind: 'analytics', value: false } },
			context('zh')
		);
		expect(said).toBe(`${zh.preferences.analytics}：${zh.off}`);
		expect(said).not.toMatch(/analytics|false/);
	});

	it('names stored data, services and a network’s fields by their labels', () => {
		const en = resolveAgentMessages('en');
		expect(detail({ type: 'clear_storage_item', item: 'rates' })).toBe(en.storage.rates);
		expect(
			detail({ type: 'set_service_endpoint', field: 'fiat_rates', url: 'https://x.example' })
		).toBe(`${en.services.fiat_rates} → https://x.example`);
		expect(
			detail({
				type: 'set_network_endpoint',
				chain_id: 100,
				field: 'rpc',
				url: 'https://rpc.example'
			})
		).toBe(`Gnosis · ${en.fields.rpc} → https://rpc.example`);
	});

	it('says "Only use my node" either way, as the network’s switch names it (round 3)', () => {
		const en = resolveAgentMessages('en');
		expect(detail({ type: 'set_only_my_node', chain_id: 100, on: false })).toBe(
			`Gnosis · ${en.onlyMine}: ${en.off}`
		);
		expect(detail({ type: 'set_only_my_node', chain_id: 100, on: true })).toBe(
			`Gnosis · ${en.onlyMine}: ${en.on}`
		);
		expect(en.actions.set_only_my_node).toBeTruthy();
		expect(en.actions.set_only_my_node).not.toContain('settings.agent');
		const zh = resolveAgentMessages('zh');
		expect(detail({ type: 'set_only_my_node', chain_id: 100, on: true }, context('zh'))).toBe(
			`Gnosis · ${zh.onlyMine}：${zh.on}`
		);
	});

	it('names a network by its name, or as "Network 59144" — never "chain 59144"', () => {
		expect(detail({ type: 'remove_network', chain_id: 100 })).toBe('Gnosis');
		expect(detail({ type: 'add_network', chain_id: 59144, rpc_url: null })).toBe('Network 59144');
		expect(
			detail(
				{ type: 'add_network', chain_id: 59144, rpc_url: null },
				context('en', { checking: { chain_id: 59144, name: 'Linea' } })
			)
		).toBe('Linea');
	});

	it('names a record by what it moved, a contact and group by name', () => {
		expect(detail({ type: 'delete_activity', id: 'r1' })).toBe('5 xDAI · 0x1111…1111');
		expect(
			detail({ type: 'delete_contact', address: '0x00000000219AB540356cBB839Cbe05303d7705Fa' })
		).toBe('Alice');
		expect(detail({ type: 'delete_group', id: 'g1' })).toBe('Work');
	});

	it('leaves the wallets a switch moves between to the core, by their names', () => {
		expect(detail({ type: 'switch_account', address: SAFE })).toContain('Team');
	});
});

describe('the sheet’s sentence', () => {
	it('is the locale’s: an em dash and a period in English, a full-width stop in Chinese', () => {
		const en = resolveAgentMessages('en');
		expect(approvalMessage('Delete a contact', 'Alice', en)).toBe(
			`Delete a contact — Alice. ${en.approveHint}`
		);
		expect(approvalMessage('Clear caches', null, en)).toBe(`Clear caches. ${en.approveHint}`);
		const zh = resolveAgentMessages('zh');
		const said = approvalMessage('删除联系人', 'Alice', zh);
		expect(said).toContain('。');
		expect(said).not.toContain('. ');
	});
});
