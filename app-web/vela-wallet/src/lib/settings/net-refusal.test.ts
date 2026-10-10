/**
 * Why a network is refused is the core's to say (`NetCompatibility.hint_key`,
 * `.setup_url`), and `netRefusal` is the one place the web reads it — for
 * Settings' wizard and for a dApp's add-network sheet alike.
 *
 * The manifest is held to the core's SOURCE: every hint key
 * `network_admin.rs` names has words in both bags, in all fifteen languages.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { NetCompatibility } from '$lib/core/generated/NetCompatibility';
import { resolveRequestMessages, resolveSettingsMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { NET_HINT_KEYS } from './messages';
import { netRefusal } from './net-refusal';

const m = resolveSettingsMessages('en');

const CHECKED: NetCompatibility = {
	chain_id: 64800,
	compatible: false,
	multi_key_ready: false,
	contracts: [],
	p256_available: true,
	best_rpc_url: 'https://rpc.example',
	best_rpc_latency_ms: 40,
	rpc_failure: null,
	blocker: 'missing_contracts',
	hint_key: 'settingsModals.addNetwork.incompatibleHint',
	setup_url: 'https://getvela.app/chain-setup?chain=64800'
};

describe('netRefusal', () => {
	it('missing contracts: the line, and the link the core gave — on that chain', () => {
		expect(netRefusal(CHECKED, m.addNetwork.hints)).toEqual({
			hint: "Some contracts Vela needs aren't on this network yet. Chain Setup shows which ones and who can deploy them.",
			setupUrl: 'https://getvela.app/chain-setup?chain=64800'
		});
	});

	it('no P-256 verifier: the line, and no link — there is nothing to deploy', () => {
		const refusal = netRefusal(
			{
				...CHECKED,
				p256_available: false,
				blocker: 'no_p256',
				hint_key: 'settingsModals.addNetwork.noP256Hint',
				setup_url: null
			},
			m.addNetwork.hints
		);
		expect(refusal.hint).toContain('no P-256 verifier');
		expect(refusal.hint).toContain("Vela wallets can't work here");
		expect(refusal.hint).toContain('It would be stuck');
		expect(refusal).not.toHaveProperty('setupUrl');
	});

	it('the link is the core’s address, never one built here', () => {
		const elsewhere = netRefusal(
			{ ...CHECKED, setup_url: 'https://getvela.app/chain-setup?chain=1' },
			m.addNetwork.hints
		);
		expect(elsewhere.setupUrl).toBe('https://getvela.app/chain-setup?chain=1');
	});

	it('nothing refused, nothing said: a compatible check, an inconclusive one, none at all', () => {
		const none = { blocker: null, hint_key: null, setup_url: null };
		expect(netRefusal({ ...CHECKED, compatible: true, ...none }, m.addNetwork.hints)).toEqual({});
		expect(
			netRefusal({ ...CHECKED, rpc_failure: 'all_probes_failed', ...none }, m.addNetwork.hints)
		).toEqual({});
		expect(netRefusal(null, m.addNetwork.hints)).toEqual({});
		expect(netRefusal(undefined, m.addNetwork.hints)).toEqual({});
	});

	it('a key this build has no words for draws no line — never a dotted path', () => {
		const refusal = netRefusal(
			{ ...CHECKED, hint_key: 'settingsModals.addNetwork.somethingNew' },
			m.addNetwork.hints
		);
		expect(refusal.hint).toBeUndefined();
		expect(refusal.setupUrl).toBe(CHECKED.setup_url);
	});
});

describe('the hint manifest is the core’s', () => {
	it('names every hint key `network_admin.rs` declares', () => {
		const source = readFileSync('../../rust/crates/vela-core/src/app/network_admin.rs', 'utf8');
		const declared = [
			...source.matchAll(/pub const [A-Z0-9_]+_HINT: &str = "([A-Za-z0-9.]+)";/g)
		].map((match) => match[1]);
		expect(declared.length).toBeGreaterThanOrEqual(2);
		expect([...declared].sort()).toEqual([...NET_HINT_KEYS].sort());
	});

	it('the Chain Setup link the core builds carries the chain', () => {
		const source = readFileSync('../../rust/crates/vela-core/src/app/network_admin.rs', 'utf8');
		// `chain_setup_url`: "{CHAIN_SETUP_URL}?chain={chain_id}".
		expect(source).toContain(
			'pub const CHAIN_SETUP_URL: &str = "https://getvela.app/chain-setup";'
		);
		expect(source).toContain('format!("{CHAIN_SETUP_URL}?chain={chain_id}")');
	});

	it('every language has both lines, in Settings and on the dApp sheet', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const settings = resolveSettingsMessages(locale).addNetwork.hints;
			const sheet = resolveRequestMessages(locale).addNetwork.hints;
			for (const key of NET_HINT_KEYS) {
				expect(settings[key], `${locale} ${key}`).toBeTruthy();
				expect(sheet[key], `${locale} ${key}`).toBe(settings[key]);
			}
			// The two lines are different sentences in every language.
			expect(new Set(Object.values(settings)).size).toBe(NET_HINT_KEYS.length);
		}
	});
});
