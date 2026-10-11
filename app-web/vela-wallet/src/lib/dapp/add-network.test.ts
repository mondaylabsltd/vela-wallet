/**
 * Spec 100: the extension's add-network card, in Settings' own words — and
 * the core's one answer table, read over the real wasm.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { dappAddChainAsk, dappAddOutcomeError } from '$lib/core/kernels';
import { resolveRequestMessages } from '$lib/i18n/engine.server';
import type { NetDappAddView } from '$lib/core/generated/NetDappAddView';
import { addNetworkCard } from './add-network';

const m = resolveRequestMessages('en').addNetwork;

function view(over: Partial<NetDappAddView> = {}): NetDappAddView {
	return {
		tab: '7',
		id: '7:n1',
		origin: 'https://app.example',
		host: 'app.example',
		chain_id: 11155111,
		name: 'Ethereum Sepolia',
		native_symbol: 'ETH',
		rpc_host: 'rpc.sepolia.example',
		explorer_host: 'sepolia.etherscan.io',
		from_site: false,
		phase: 'ready',
		reported_chain_id: null,
		compat: {
			chain_id: 11155111,
			compatible: true,
			multi_key_ready: true,
			contracts: [],
			p256_available: true,
			best_rpc_url: 'https://rpc.sepolia.example',
			best_rpc_latency_ms: 20,
			rpc_failure: null,
			blocker: null,
			hint_key: null,
			setup_url: null
		},
		can_add: true,
		...over
	};
}

/**
 * The two refusals as the core's check carries them (`net_blocker`,
 * `NO_P256_HINT` / `MISSING_CONTRACTS_HINT`, `chain_setup_url`): no P-256
 * verifier wins over missing contracts, and only a deployable gap has a link.
 */
const COMPAT = view().compat!;
const NO_P256: NetDappAddView['compat'] = {
	...COMPAT,
	compatible: false,
	multi_key_ready: false,
	contracts: [{ name: 'Safe L2', address: '0x2', deployed: false, multi_key_only: false }],
	p256_available: false,
	blocker: 'no_p256',
	hint_key: 'settingsModals.addNetwork.noP256Hint',
	setup_url: null
};
const MISSING_CONTRACTS: NetDappAddView['compat'] = {
	...COMPAT,
	compatible: false,
	multi_key_ready: false,
	contracts: [{ name: 'Safe L2', address: '0x2', deployed: false, multi_key_only: false }],
	p256_available: true,
	blocker: 'missing_contracts',
	hint_key: 'settingsModals.addNetwork.incompatibleHint',
	setup_url: 'https://getvela.app/chain-setup?chain=11155111'
};

describe('the add-network card', () => {
	it('offers Add for a compatible chain, naming who asks', () => {
		const card = addNetworkCard(view(), m);
		expect(card.title).toBe('Add Network');
		expect(card.lead).toBe('app.example asks to add a network');
		expect(card.add).toBe('Add Network');
		expect(card.pill).toEqual({ tone: 'ok', label: 'Compatible', dot: true });
		expect(card.rows.map((r) => r.label)).toEqual([
			'Name',
			'Chain ID',
			'Native Token',
			'RPC URL',
			'Explorer'
		]);
		expect(card.dismiss).toBe('Cancel');
		expect(card.fromSite).toBeNull();
	});

	it('closes a verdict with Done and never offers Add', () => {
		const incompatible = addNetworkCard(
			view({ phase: 'not_compatible', can_add: false, compat: MISSING_CONTRACTS }),
			m
		);
		expect(incompatible.add).toBeNull();
		expect(incompatible.dismiss).toBe('Done');
		const wrong = addNetworkCard(
			view({ phase: 'wrong_rpc', can_add: false, reported_chain_id: 100, from_site: true }),
			m
		);
		expect(wrong.note).toBe('That RPC serves a different network (chain 100, expected 11155111).');
		expect(wrong.fromSite).toBe('Not in Vela’s network list — the name and coin are the site’s.');
		const none = addNetworkCard(view({ phase: 'no_rpc', can_add: false }), m);
		expect(none.note).toBe('The site gave no usable RPC for this network');
	});

	// A refusal says WHY, and only a gap that can be deployed offers Chain
	// Setup. The card used to say "contracts are missing" and offer the tool
	// for every refusal — over a network with no P-256 verifier that sends a
	// person to deploy nothing, and says nothing about money getting stuck.
	it('missing contracts: says so, and opens Chain Setup on that chain', () => {
		const card = addNetworkCard(
			view({ phase: 'not_compatible', can_add: false, compat: MISSING_CONTRACTS }),
			m
		);
		expect(card.pill).toEqual({ tone: 'error', label: 'Incompatible', dot: true });
		expect(card.note).toBe(
			"Some contracts Vela needs aren't on this network yet. Chain Setup shows which ones and who can deploy them."
		);
		expect(card.setupTool).toEqual({
			label: 'Open Chain Setup Tool',
			url: 'https://getvela.app/chain-setup?chain=11155111'
		});
		// The check list still shows which rows failed: the verifier is there.
		expect(card.checks.at(-1)).toEqual({ label: 'P-256 precompile', ok: true });
	});

	it('no P-256 verifier: says the network cannot run Vela wallets — and offers nothing to deploy', () => {
		const card = addNetworkCard(
			view({ phase: 'not_compatible', can_add: false, compat: NO_P256 }),
			m
		);
		expect(card.pill).toEqual({ tone: 'error', label: 'Incompatible', dot: true });
		expect(card.note).toContain("Vela wallets can't work here");
		expect(card.note).toContain("Don't send money to your Vela address on this network");
		expect(card.note).not.toContain('Chain Setup');
		expect(card.setupTool).toBeNull();
		expect(card.add).toBeNull();
		expect(card.dismiss).toBe('Done');
		expect(card.checks.at(-1)).toEqual({ label: 'P-256 precompile', ok: false });
		// In the person's language too.
		const zh = addNetworkCard(
			view({ phase: 'not_compatible', can_add: false, compat: NO_P256 }),
			resolveRequestMessages('zh').addNetwork
		);
		expect(zh.note).toContain('转进去会被卡住');
		expect(zh.setupTool).toBeNull();
	});

	it('a refusal whose check is not in hand says the verdict and invents no reason', () => {
		const card = addNetworkCard(view({ phase: 'not_compatible', can_add: false, compat: null }), m);
		expect(card.pill?.label).toBe('Incompatible');
		expect(card.note).toBeNull();
		expect(card.setupTool).toBeNull();
	});

	it('offers Retry when nothing answered, and names the chain before its name is known', () => {
		const card = addNetworkCard(
			view({ phase: 'check_failed', can_add: false, name: '', native_symbol: '' }),
			m
		);
		expect(card.retry).toBe('Retry');
		expect(card.rows[0].value).toBe('Chain 11155111');
	});
});

describe('the core reads the ask and names the answer', () => {
	it('reads a typical ask, and refuses what Vela cannot count', () => {
		const read = dappAddChainAsk([
			{
				chainId: '0xaa36a7',
				chainName: 'Sepolia',
				nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
				rpcUrls: ['http://rpc.example', 'https://rpc.example']
			}
		]);
		expect(read).toEqual({
			ok: {
				chain_id: 11155111,
				chain_name: 'Sepolia',
				native_symbol: 'ETH',
				rpc_urls: ['https://rpc.example'],
				refused_rpc_urls: 1,
				explorer_url: null
			}
		});
		expect('error' in dappAddChainAsk([{ chainId: '0x1', nativeCurrency: { decimals: 6 } }])).toBe(
			true
		);
	});

	it('answers each ending the way the in-app browsers do', () => {
		expect(dappAddOutcomeError({ type: 'added', chain_id: 1 }, 1)).toBeNull();
		expect(dappAddOutcomeError({ type: 'declined' }, 1)?.code).toBe(4001);
		expect(dappAddOutcomeError({ type: 'not_compatible' }, 61)?.code).toBe(4902);
		expect(dappAddOutcomeError({ type: 'bad_rpc' }, 61)?.code).toBe(-32602);
		expect(dappAddOutcomeError({ type: 'busy' }, 61)?.code).toBe(-32002);
	});
});
