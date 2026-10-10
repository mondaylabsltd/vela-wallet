/**
 * Every path a network is added by says why it stopped (PR 3 notes 5, 10, 18).
 *
 * The add-network wizard stops five ways, and the web had words for two: a
 * network already added, a chain whose document was not found and one with no
 * RPC endpoint listed all read "Incompatible" — and on Settings' page a stop
 * with no network to show drew nothing at all, because the callout was only
 * drawn beside a candidate. The core now names the sentence (`error_key`) and
 * keeps the check beside a refusal on the path that saves without a confirm
 * step, so a refusal there can say WHY.
 *
 * This is the REAL `network_admin` core, answered by hand the way the executor
 * answers it, read by the two builders that draw a stopped wizard on the web:
 * Settings' page (`liveAddNetwork`) and the add-token sheet's network tab
 * (`liveAddToken`). The sentences are the real corpus's.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { NetworkAdminCore } from '$lib/core/client';
import type { MtokView } from '$lib/core/generated/MtokView';
import type { NetEvent } from '$lib/core/generated/NetEvent';
import type { NetOperation } from '$lib/core/generated/NetOperation';
import type { NetRawChainData } from '$lib/core/generated/NetRawChainData';
import type { NetShellResult } from '$lib/core/generated/NetShellResult';
import type { NetView } from '$lib/core/generated/NetView';
import type { NetWizardView } from '$lib/core/generated/NetWizardView';
import { buildFlowState } from '$lib/flows/fixtures';
import { liveAddToken } from '$lib/flows/live';
import { resolveSettingsMessages, resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { liveAddNetwork } from './live';
import { NET_HINT_KEYS, NET_STOP_KEYS } from './messages';
import { netStopLine, stopIsRefusal } from './net-refusal';

const m = resolveSettingsMessages('en');
const fm = resolveWalletFlowMessages('en');

const CHAIN = 7_777_777;
const RPC = 'https://rpc.sample.test';
const P256 = '0x0000000000000000000000000000000000000100';
const ONE = '0x' + '0'.repeat(63) + '1';

type Out = { view: NetView; effects: { id: number; operation: NetOperation }[] };

/** What the chain under test answers the check with. */
interface Chain {
	/** Its registry document; `null` = the registry has none. */
	doc: NetRawChainData | null;
	/** The RPC answers at all. */
	up: boolean;
	/** Every required contract has code. */
	contracts: boolean;
	/** The P-256 verifier is there. */
	p256: boolean;
}

const DOC: NetRawChainData = {
	chain_id: CHAIN,
	name: 'Sample',
	short_name: 'smpl',
	native_currency_name: 'Sample Coin',
	native_currency_symbol: 'SMP',
	native_currency_decimals: 18,
	rpc: [RPC],
	explorers: [],
	testnet: false
};

const HEALTHY: Chain = { doc: DOC, up: true, contracts: true, p256: true };

/** The executor's answers, by operation — a chain that behaves as `chain` says. */
function answer(operation: NetOperation, chain: Chain): NetShellResult | undefined {
	switch (operation.type) {
		case 'read_store':
			return {
				type: 'store_loaded',
				custom_networks: [],
				network_configs: [],
				endpoints: {
					ethereum_data_url: null,
					passkey_index_url: null,
					bundler_service_url: null,
					fiat_rates_url: null
				},
				provider_keys: { alchemy: null, drpc: null, ankr: null }
			};
		case 'fetch_chain_info':
			return { type: 'chain_info', chain_id: operation.chain_id, data: chain.doc };
		case 'probe_rpc':
			return {
				type: 'probed',
				url: operation.url,
				reported_chain_id: chain.up ? CHAIN : null,
				latency_ms: 40
			};
		case 'rpc_get_code': {
			const verifier = operation.address.toLowerCase() === P256;
			const has = verifier ? chain.p256 : chain.contracts;
			return {
				type: 'code',
				url: operation.url,
				address: operation.address,
				code: chain.up ? (has ? '0x6001' : '0x') : null
			};
		}
		case 'rpc_call_p256':
			return {
				type: 'p256_call',
				url: operation.url,
				result: chain.up ? (chain.p256 ? ONE : '0x') : null
			};
		case 'probe_reachable':
			return { type: 'reachable', url: operation.url, ok: true, latency_ms: 30 };
		case 'fetch_search_index':
			return { type: 'search_index', chains: [] };
		case 'start_search_debounce':
			return { type: 'debounce_elapsed' };
		case 'write_custom_networks':
		case 'write_network_configs':
		case 'write_service_endpoints':
		case 'write_rpc_providers':
			return { type: 'written' };
		case 'invalidate_pools':
			return { type: 'invalidated' };
		case 'clear_bundler_cache':
			return { type: 'bundler_cache_cleared' };
		default:
			// Service-health probes and the dApp sheet are not this test's.
			return undefined;
	}
}

/**
 * Boot the machine, raise `event`, answer everything, and hand back the
 * wizard. `typed`: what the person put in the search before the event.
 */
function wizardAfter(event: NetEvent, chain: Chain = HEALTHY, typed?: string): NetWizardView {
	const core = new NetworkAdminCore();
	let view: NetView | undefined;
	const settle = (first: Out) => {
		const queue = [...first.effects];
		view = first.view;
		for (let guard = 0; queue.length > 0 && guard < 500; guard += 1) {
			const effect = queue.shift()!;
			const result = answer(effect.operation, chain);
			if (result === undefined) continue;
			const out = JSON.parse(core.resolve_effect(BigInt(effect.id), JSON.stringify(result))) as Out;
			view = out.view;
			queue.push(...out.effects);
		}
	};
	settle(JSON.parse(core.dispatch(JSON.stringify({ type: 'started' }))) as Out);
	if (typed !== undefined) {
		const search: NetEvent = { type: 'search_input', query: typed };
		settle(JSON.parse(core.dispatch(JSON.stringify(search))) as Out);
	}
	settle(JSON.parse(core.dispatch(JSON.stringify(event))) as Out);
	core.free();
	if (view === undefined) throw new Error('the core committed no view');
	expect(view.loaded).toBe(true);
	return view.wizard;
}

const pick = (chain_id: number): NetEvent => ({
	type: 'chain_selected',
	chain_id,
	keep_custom_rpc: false
});
/** The path that saves without a confirm step (a pay link's "add this network"). */
const auto = (chain_id: number): NetEvent => ({
	type: 'add_by_chain_id_requested',
	chain_id,
	now_iso: '2026-10-10T00:00:00.000Z'
});

const EMPTY_TOKENS: MtokView = {
	input_address: '',
	address_valid: false,
	detecting: false,
	found: [],
	saving: false,
	custom_tokens: [],
	not_found: false,
	native_alias: false,
	save_error: false
};

/** The add-token sheet's network tab over `wizard`, with `query` typed. */
function tab(wizard: NetWizardView, query = 'sample') {
	const sheet = buildFlowState('t3', fm, () => '').sheet;
	if (sheet?.kind !== 'add-token') throw new Error('not the add-token sheet');
	return liveAddToken(sheet.model, {
		view: EMPTY_TOKENS,
		m: fm,
		tab: 'native',
		native: { query, wizard, addedChainId: null }
	}).result;
}

describe('the three stops that had no words (PR 3 note 18)', () => {
	it('already added: said as that, under the search — never "Incompatible"', () => {
		const wizard = wizardAfter(pick(1));
		expect(wizard).toMatchObject({
			phase: 'error',
			error: { type: 'already_added', chain_id: 1 },
			error_key: 'addToken.errorAlreadyAdded',
			chain_info: null
		});

		const page = liveAddNetwork(wizard, m);
		expect(page.callout).toEqual({ tone: 'warning', text: 'This network is already added' });
		// No network to show: the search stays, with the sentence under it.
		expect(page.candidate).toBeUndefined();
		expect(page.secondary).toBeUndefined();
		expect(page.recheck).toBeUndefined();
		expect(JSON.stringify(page)).not.toContain(m.addNetwork.incompatible);

		// The add-token tab names the network it is (data) and says the same sentence.
		const card = tab(wizard, 'ethereum');
		expect(card).toMatchObject({
			kind: 'network',
			name: 'Ethereum',
			note: 'This network is already added'
		});
		expect(card).not.toHaveProperty('chip');
		expect(card).not.toHaveProperty('setup');
	});

	it('chain info not found: said as that', () => {
		const wizard = wizardAfter(pick(CHAIN), { ...HEALTHY, doc: null });
		expect(wizard).toMatchObject({
			phase: 'error',
			error: { type: 'not_found', chain_id: CHAIN },
			error_key: 'addToken.errorChainNotFound'
		});

		const page = liveAddNetwork(wizard, m);
		expect(page.callout).toEqual({ tone: 'warning', text: 'Chain info not found' });
		expect(page.candidate).toBeUndefined();
		expect(JSON.stringify(page)).not.toContain(m.addNetwork.incompatible);

		expect(tab(wizard)).toEqual({ kind: 'not-found', text: 'Chain info not found' });
	});

	it('a stop under the search leaves what was searched in the field', () => {
		// "Chain info not found" comes back from the resolving candidate, where
		// the field is not drawn: re-drawn, it was EMPTY, and the sentence
		// answered a placeholder. The core kept the query all along.
		const lost = liveAddNetwork(wizardAfter(pick(CHAIN), { ...HEALTHY, doc: null }, '424242'), m);
		expect(lost.candidate).toBeUndefined();
		expect(lost.query).toBe('424242');
		expect(lost.callout?.text).toBe('Chain info not found');

		const held = liveAddNetwork(wizardAfter(pick(1), HEALTHY, 'ethereum'), m);
		expect(held.candidate).toBeUndefined();
		expect(held.query).toBe('ethereum');
		expect(held.callout?.text).toBe('This network is already added');
	});

	it('no RPC endpoint listed: the sentence, the field it points at, and the re-check', () => {
		const wizard = wizardAfter(pick(CHAIN), { ...HEALTHY, doc: { ...DOC, rpc: [] } });
		expect(wizard).toMatchObject({
			phase: 'error',
			error: { type: 'no_rpc_endpoint' },
			error_key: 'settingsModals.addNetwork.noRpcEndpoint'
		});
		expect(wizard.chain_info?.name).toBe('Sample');

		const page = liveAddNetwork(wizard, m);
		expect(page.callout).toEqual({
			tone: 'warning',
			text: 'No RPC endpoint is listed for this network. Enter one, then re-check.'
		});
		// "Enter one, then re-check": both are there to do it with.
		expect(page.candidate).toMatchObject({ name: 'Sample' });
		expect(page.candidate?.badge).toBeUndefined();
		// …and the field is not called optional under a sentence that asks for it.
		expect(page.customRpc).toMatchObject({ id: 'custom-rpc', label: 'RPC URL' });
		expect(m.addNetwork.customRpcTitle).toContain('optional');
		expect(page.recheck).toBe(m.addNetwork.recheckWithRpc);
		// Not a verdict: no check list, nothing to deploy, nothing to add.
		expect(page.checks).toBeUndefined();
		expect(page.secondary).toBeUndefined();
		expect(page.primary).toBeUndefined();
		expect(JSON.stringify(page)).not.toContain(m.addNetwork.incompatible);

		const card = tab(wizard);
		expect(card).toMatchObject({
			kind: 'network',
			name: 'Sample',
			note: 'No RPC endpoint is listed for this network. Enter one, then re-check.'
		});
		// No verdict was reached, so none is drawn — above all not "Not compatible".
		expect(card).not.toHaveProperty('chip');
		expect(JSON.stringify(card)).not.toContain(fm['addToken.notCompatible']);
	});
});

describe('a refusal on the path with no confirm step says why (PR 3 notes 5 and 10)', () => {
	it('no P-256 verifier: the network cannot run Vela wallets — and nothing to deploy', () => {
		const wizard = wizardAfter(auto(CHAIN), { ...HEALTHY, p256: false });
		expect(wizard).toMatchObject({
			phase: 'error',
			error: { type: 'not_compatible', chain_id: CHAIN },
			error_key: 'settingsModals.addNetwork.noP256Hint'
		});
		// The check is kept beside the stop: this is what the reason is read from.
		expect(wizard.compat).toMatchObject({ blocker: 'no_p256', setup_url: null });
		expect(stopIsRefusal(wizard.error_key, wizard.compat)).toBe(true);

		const page = liveAddNetwork(wizard, m);
		expect(page.callout?.text).toContain("Vela wallets can't work here");
		expect(page.callout?.text).toContain('It would be stuck');
		expect(page.candidate?.badge).toEqual({
			tone: 'error',
			label: m.addNetwork.incompatible,
			dot: true
		});
		expect(page.checks?.at(-1)).toEqual({ label: m.addNetwork.checkSigner, ok: false });
		expect(page.secondary).toBeUndefined();
		expect(page.primary).toBeUndefined();

		const card = tab(wizard);
		expect(card).toMatchObject({
			kind: 'network',
			chip: { text: fm['addToken.notCompatible'], tone: 'error' }
		});
		expect(card.kind === 'network' && card.note).toContain("Vela wallets can't work here");
		expect(card).not.toHaveProperty('setup');
	});

	it('missing contracts: the reason, and Chain Setup opened on this chain', () => {
		const wizard = wizardAfter(auto(CHAIN), { ...HEALTHY, contracts: false });
		expect(wizard).toMatchObject({
			phase: 'error',
			error: { type: 'not_compatible', chain_id: CHAIN },
			error_key: 'settingsModals.addNetwork.incompatibleHint'
		});
		expect(wizard.compat).toMatchObject({ blocker: 'missing_contracts' });
		const setupUrl = `https://getvela.app/chain-setup?chain=${CHAIN}`;
		expect(wizard.compat?.setup_url).toBe(setupUrl);

		const page = liveAddNetwork(wizard, m);
		expect(page.callout?.text).toBe(
			m.addNetwork.hints['settingsModals.addNetwork.incompatibleHint']
		);
		expect(page.secondary).toEqual({ label: m.addNetwork.openChainSetupTool, href: setupUrl });
		expect(page.checks?.some((check) => !check.ok)).toBe(true);
		expect(page.primary).toBeUndefined();

		const card = tab(wizard);
		expect(card).toMatchObject({
			kind: 'network',
			chip: { text: fm['addToken.notCompatible'], tone: 'error' },
			note: fm['settingsModals.addNetwork.incompatibleHint'],
			setup: { label: fm['settingsModals.addNetwork.openChainSetupTool'], href: setupUrl }
		});
	});

	it('a check that could not be made is "unable to verify" — no reason, no link, never a refusal', () => {
		const wizard = wizardAfter(auto(CHAIN), { ...HEALTHY, up: false });
		expect(wizard).toMatchObject({
			phase: 'error',
			error: { type: 'check_failed', chain_id: CHAIN },
			error_key: 'settingsModals.addNetwork.unableToVerify'
		});
		expect(stopIsRefusal(wizard.error_key, wizard.compat)).toBe(false);

		const page = liveAddNetwork(wizard, m);
		expect(page.callout).toEqual({ tone: 'warning', text: m.addNetwork.unableToVerify });
		expect(page.candidate?.badge).toBeUndefined();
		expect(page.checks).toBeUndefined();
		expect(page.secondary).toBeUndefined();
		expect(page.recheck).toBe(m.addNetwork.recheckWithRpc);
		// Invariant ③: inconclusive is never worded as incompatible.
		expect(JSON.stringify(page)).not.toContain(m.addNetwork.incompatible);

		const card = tab(wizard);
		expect(card).toMatchObject({ kind: 'network', note: m.addNetwork.unableToVerify });
		expect(card).not.toHaveProperty('setup');
		expect(JSON.stringify(card)).not.toContain(fm['addToken.notCompatible']);
	});
});

describe('the wizard’s own verdict is unchanged: a refused check still says why, with the tab’s card', () => {
	it('checked and refused for no verifier: the reason, no link — where it said "Deploy missing contracts"', () => {
		const wizard = wizardAfter(pick(CHAIN), { ...HEALTHY, p256: false });
		expect(wizard.phase).toBe('checked');
		expect(wizard.error_key).toBeNull();
		const card = tab(wizard);
		expect(card.kind === 'network' && card.note).toContain("Vela wallets can't work here");
		expect(card).not.toHaveProperty('setup');
	});

	it('checked and refused for missing contracts: Chain Setup is a real link, on this chain', () => {
		const wizard = wizardAfter(pick(CHAIN), { ...HEALTHY, contracts: false });
		expect(wizard.phase).toBe('checked');
		expect(tab(wizard)).toMatchObject({
			kind: 'network',
			setup: { href: `https://getvela.app/chain-setup?chain=${CHAIN}` }
		});
	});
});

describe('the sentence is the core’s, and only the core’s', () => {
	const words = { stops: m.addNetwork.stops, hints: m.addNetwork.hints };

	it('a key this build has no words for draws no line — never a dotted path, never a guess', () => {
		expect(netStopLine('settingsModals.addNetwork.somethingNew', words)).toBeUndefined();
		expect(netStopLine('constructor', words)).toBeUndefined();
		expect(netStopLine(null, words)).toBeUndefined();
		const page = liveAddNetwork(
			{
				...wizardAfter(pick(1)),
				error_key: 'settingsModals.addNetwork.somethingNew'
			},
			m
		);
		expect(page.callout).toBeUndefined();
	});

	it('an inconclusive check is never dressed as a refusal, whatever the check beside it holds', () => {
		// A contradictory view on purpose: the stop says "could not verify" and
		// the check beside it carries a refusal's reason and link.
		const refused = wizardAfter(auto(CHAIN), { ...HEALTHY, contracts: false });
		const page = liveAddNetwork(
			{
				...refused,
				error: { type: 'check_failed', chain_id: CHAIN },
				error_key: 'settingsModals.addNetwork.unableToVerify'
			},
			m
		);
		expect(page.callout).toEqual({ tone: 'warning', text: m.addNetwork.unableToVerify });
		expect(page.secondary).toBeUndefined();
		expect(page.checks).toBeUndefined();
	});

	it('names every sentence `network_admin.rs` can answer with, in every language', () => {
		const source = readFileSync('../../rust/crates/vela-core/src/app/network_admin.rs', 'utf8');
		const stops = [
			...source.matchAll(/pub const WIZARD_[A-Z0-9_]+: &str = "([A-Za-z0-9.]+)";/g)
		].map((match) => match[1]);
		expect(stops.length).toBeGreaterThanOrEqual(5);
		expect([...stops].sort()).toEqual([...NET_STOP_KEYS].sort());
		for (const locale of SUPPORTED_LOCALES) {
			const settings = resolveSettingsMessages(locale).addNetwork;
			const flow = resolveWalletFlowMessages(locale);
			for (const key of NET_STOP_KEYS) {
				expect(settings.stops[key], `${locale} ${key}`).toBeTruthy();
				expect(flow[key], `${locale} ${key}`).toBe(settings.stops[key]);
			}
			for (const key of NET_HINT_KEYS) {
				expect(flow[key], `${locale} ${key}`).toBe(settings.hints[key]);
			}
			// Five different sentences in every language.
			expect(new Set(Object.values(settings.stops)).size).toBe(NET_STOP_KEYS.length);
		}
	});
});
