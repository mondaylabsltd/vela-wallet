/**
 * The live settings builders (spec 024 T019): NetView → display models,
 * against hand-written core-view fixtures. Sibling of fixtures.test.ts.
 */
import { describe, expect, it } from 'vitest';
import type { NetNetworkRow } from '$lib/core/generated/NetNetworkRow';
import type { NetView } from '$lib/core/generated/NetView';
import type { NetWizardView } from '$lib/core/generated/NetWizardView';
import { MARKS } from './fixtures';
import {
	liveAddNetwork,
	liveEndpoints,
	liveNetworkDetail,
	liveNetworkRows,
	liveRelayer,
	liveRelayReport,
	liveRpcProviders,
	pickRescueMessages,
	liveUnreachable,
	withEraseFailure,
	withLiveFeeSpeed,
	withLiveFeeSpeedDesktop,
	withLiveNetworks,
	withLiveNetworksDesktop
} from './live';
import { buildDesktopState, buildMobileState } from './fixtures';
import { fill } from '$lib/wallet/messages';
import { FeeTierPrefCore } from '$lib/core/client';
import type { FeeTierPrefView } from '$lib/core/generated/FeeTierPrefView';
import type { SendTreasuryStatus } from '$lib/core/generated/SendTreasuryStatus';
import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { UnreachableNetwork } from '$lib/core/generated/UnreachableNetwork';

const m = resolveSettingsMessages('en');

const ROW: NetNetworkRow = {
	id: 'ethereum',
	chain_id: 1,
	display_name: 'Ethereum',
	native_symbol: 'ETH',
	is_custom: false,
	rpc_url: 'https://eth.llamarpc.com',
	explorer_url: 'https://etherscan.io',
	bundler_url: '',
	rpc_health: { type: 'ok', latency_ms: 45 },
	explorer_health: null,
	rpc_chain_mismatch: null,
	rpc_save_deferred: false
};

const CUSTOM: NetNetworkRow = {
	...ROW,
	id: 'xlayer',
	chain_id: 196,
	display_name: 'X Layer',
	native_symbol: 'OKB',
	is_custom: true,
	rpc_health: null
};

const WIZARD_IDLE: NetWizardView = {
	phase: 'idle',
	query: '',
	custom_rpc: '',
	suggestions: [],
	chain_info: null,
	compat: null,
	error: null,
	can_add: false
};

const VIEW: NetView = {
	loaded: true,
	networks: [ROW, CUSTOM],
	wizard: WIZARD_IDLE,
	endpoints: [
		{
			field: 'passkey_index',
			value: 'https://idx.example',
			default_value: 'https://p256-index-v2.getvela.app',
			health: { type: 'ok', latency_ms: 88, rate_count: null }
		},
		{
			field: 'fiat_rates',
			value: '',
			default_value: 'https://vela-currency.getvela.app/v2/rates?base=USD',
			health: { type: 'unreachable', http_status: null, latency_ms: null }
		}
	],
	providers: [
		{ provider: 'alchemy', key: 'alch-key', has_key: true, test: null },
		{ provider: 'drpc', key: '', has_key: false, test: null }
	],
	last_added_chain_id: null,
	dapp_add: null
};

describe('liveNetworkRows', () => {
	it('builtin rows wear their brand mark and latency; customs wear the tag', () => {
		const rows = liveNetworkRows(VIEW, m, 'xlayer');
		expect(rows[0]).toMatchObject({
			id: 'ethereum',
			mark: MARKS.ethereum,
			badge: { tone: 'ok', label: '45ms' },
			removable: false,
			expanded: false
		});
		expect(rows[1]).toMatchObject({
			id: 'xlayer',
			tag: m.networks.custom,
			removable: true,
			expanded: true
		});
		// A custom network's mark is its initial on the neutral colour.
		expect(rows[1].mark.letter).toBe('X');
	});
});

describe('liveNetworkDetail', () => {
	it('a refused save renders the mismatch callout with both chain ids', () => {
		const detail = liveNetworkDetail(
			{ ...ROW, rpc_chain_mismatch: { expected_chain_id: 1, reported_chain_id: 56 } },
			m
		);
		expect(detail.rpc.tone).toBe('error');
		expect(detail.callout?.tone).toBe('danger');
		expect(detail.callout?.text).toContain('56');
		expect(detail.callout?.text).toContain('1');
	});
});

describe('liveAddNetwork', () => {
	const info = {
		chain_id: 7777777,
		name: 'Zora',
		short_name: 'zora',
		native_name: 'Ether',
		native_symbol: 'ETH',
		native_decimals: 18,
		rpc_url: 'https://rpc.zora.energy',
		rpc_urls: ['https://rpc.zora.energy'],
		explorer_url: '',
		logo_url: '',
		is_testnet: false
	};

	it('a compatible verdict offers the add action', () => {
		const model = liveAddNetwork(
			{
				...WIZARD_IDLE,
				phase: 'checked',
				chain_info: info,
				compat: {
					chain_id: 7777777,
					compatible: true,
					multi_key_ready: true,
					contracts: [
						{ name: 'EntryPoint v0.7', address: '0x1', deployed: true, multi_key_only: false }
					],
					p256_available: true,
					best_rpc_url: 'https://rpc.zora.energy',
					best_rpc_latency_ms: 182,
					rpc_failure: null
				},
				can_add: true
			},
			m
		);
		expect(model.primary).toBe(m.addNetwork.addNetworkBtn);
		expect(model.candidate?.badge.label).toBe(m.addNetwork.compatible);
		expect(model.checks?.every((c) => c.ok)).toBe(true);
	});

	it('says so when a compatible chain cannot hold a wallet with several keys', () => {
		// Spec 081 FR-009: the badge stays green — a one-key wallet does work
		// here — and the callout carries what the two crossed rows mean.
		const model = liveAddNetwork(
			{
				...WIZARD_IDLE,
				phase: 'checked',
				chain_info: info,
				compat: {
					chain_id: 7777777,
					compatible: true,
					multi_key_ready: false,
					contracts: [
						{ name: 'EntryPoint v0.7', address: '0x1', deployed: true, multi_key_only: false },
						{
							name: 'Safe Passkey Signer Factory',
							address: '0x3',
							deployed: false,
							multi_key_only: true
						}
					],
					p256_available: true,
					best_rpc_url: 'https://rpc.zora.energy',
					best_rpc_latency_ms: 182,
					rpc_failure: null
				},
				can_add: true
			},
			m
		);
		expect(model.candidate?.badge.label).toBe(m.addNetwork.compatible);
		expect(model.callout).toEqual({ tone: 'warning', text: m.addNetwork.singleKeyOnly });
		expect(model.primary).toBe(m.addNetwork.addNetworkBtn);
	});

	it('an unanswered probe is worded unable-to-verify, NEVER incompatible (invariant ③)', () => {
		const model = liveAddNetwork(
			{
				...WIZARD_IDLE,
				phase: 'checked',
				chain_info: info,
				compat: {
					chain_id: 7777777,
					compatible: false,
					multi_key_ready: false,
					contracts: [],
					p256_available: null,
					best_rpc_url: null,
					best_rpc_latency_ms: null,
					rpc_failure: 'all_probes_failed'
				},
				can_add: false
			},
			m
		);
		expect(model.candidate?.badge.label).toBe(m.addNetwork.unableToVerify);
		expect(model.candidate?.badge.label).not.toBe(m.addNetwork.incompatible);
		expect(model.primary).toBe(m.addNetwork.retry);
	});

	it('a true incompatibility keeps the full check list and the setup-tool exit', () => {
		const model = liveAddNetwork(
			{
				...WIZARD_IDLE,
				phase: 'checked',
				chain_info: info,
				compat: {
					chain_id: 7777777,
					compatible: false,
					multi_key_ready: false,
					contracts: [{ name: 'Safe L2', address: '0x2', deployed: false, multi_key_only: false }],
					p256_available: true,
					best_rpc_url: 'https://rpc.zora.energy',
					best_rpc_latency_ms: 90,
					rpc_failure: null
				},
				can_add: false
			},
			m
		);
		expect(model.candidate?.badge.label).toBe(m.addNetwork.incompatible);
		expect(model.checks?.some((c) => !c.ok)).toBe(true);
		expect(model.secondary).toBe(m.addNetwork.openChainSetupTool);
	});
});

describe('liveEndpoints', () => {
	it('maps field ids to their corpus labels and words every health state', () => {
		const model = liveEndpoints(VIEW, m);
		expect(model.fields[0]).toMatchObject({
			id: 'passkey_index',
			label: m.endpoints.passkeyLabel,
			value: 'https://idx.example',
			badge: { tone: 'ok', label: '88ms' }
		});
		expect(model.fields[1].badge).toMatchObject({ tone: 'error', label: m.networks.offline });
		// The default rides as the placeholder — an empty override shows it.
		expect(model.fields[1].placeholder).toContain('vela-currency');
	});
});

describe('liveRpcProviders', () => {
	it('a set key reads connected with the check action; an unset one invites', () => {
		const model = liveRpcProviders(VIEW, m);
		expect(model.providers[0]).toMatchObject({
			id: 'alchemy',
			badge: { tone: 'ok', label: m.rpcProviders.connected },
			action: m.rpcProviders.checkKey
		});
		expect(model.providers[1]).toMatchObject({
			id: 'drpc',
			badge: { tone: 'neutral', label: m.rpcProviders.notSet },
			action: m.rpcProviders.getKey
		});
	});

	it('an unset provider links to its OWN site, never another provider', () => {
		const model = liveRpcProviders(VIEW, m);
		const drpc = model.providers.find((p) => p.id === 'drpc');
		expect(drpc?.linkUrl).toBe('https://drpc.org/');
		const unset = VIEW.providers.map((p) => ({ ...p, has_key: false, key: '' }));
		const all = liveRpcProviders({ ...VIEW, providers: unset }, m).providers;
		expect(all.find((p) => p.id === 'alchemy')?.linkUrl).toBe('https://dashboard.alchemy.com/');
		expect(model.providers.find((p) => p.id === 'alchemy')?.linkUrl).toBeUndefined();
	});

	it('"Get key" opens the key page; only a key is tested (spec 072)', () => {
		// The in-field "Get key" ran a key TEST on an empty field. Without a key
		// the action is a link to where one is made; with one it is the test,
		// and says so.
		const model = liveRpcProviders(VIEW, m);
		const drpc = model.providers.find((p) => p.id === 'drpc');
		expect(drpc).toMatchObject({ action: m.rpcProviders.getKey, actionUrl: 'https://drpc.org/' });
		const alchemy = model.providers.find((p) => p.id === 'alchemy');
		expect(alchemy?.action).toBe(m.rpcProviders.checkKey);
		expect(alchemy?.actionUrl).toBeUndefined();
	});
});

describe('the network count (spec 072)', () => {
	const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;
	const aboutCount = (rows: { id?: string; value: string }[]) =>
		rows.find((row) => row.id === 'networks')?.value;
	const homeCount = (model: ReturnType<typeof buildMobileState>) =>
		model.sections.flatMap((section) => section.rows).find((row) => row.id === 'networks')?.value;

	it('is the live list’s, on the Networks row and in About — never the drawn 12', () => {
		const home = withLiveNetworks(buildMobileState('st1b', m, IDENTICON), VIEW, m);
		expect(homeCount(home)).toBe(m.networks.count.replace('{{count}}', '2'));
		expect(aboutCount(home.about.rows)).toBe(m.about.techNetworksValue.replace('{{count}}', '2'));

		const desktop = withLiveNetworksDesktop(buildDesktopState('dst8', m, IDENTICON), VIEW, m);
		expect(aboutCount(desktop.about.rows)).toBe(
			m.about.techNetworksValue.replace('{{count}}', '2')
		);
	});

	it('says nothing until the ledger has been read', () => {
		const unread = { ...VIEW, loaded: false, networks: [] };
		const home = withLiveNetworks(buildMobileState('st1b', m, IDENTICON), unread, m);
		expect(homeCount(home)).toBeUndefined();
		expect(aboutCount(home.about.rows)).toBe('');
	});
});

describe('withEraseFailure', () => {
	it('says a failed erase in the sheet, on either layout', () => {
		const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;
		const desktop = withEraseFailure(buildDesktopState('dst1', m, IDENTICON), m, true);
		expect(desktop.eraseSheet.callout).toEqual({ tone: 'danger', text: m.erase.failed });
		const home = withEraseFailure(buildMobileState('st16', m, IDENTICON), m, false);
		expect(home.eraseSheet.callout?.text).toBe(m.erase.loses);
	});
});

describe('liveAccountsSheet', () => {
	const rows = [
		{
			index: 0,
			account: {
				id: 'a',
				name: 'First',
				address: '0xAAAA000000000000000000000000000000000001',
				public_key_hex: '04',
				created_at_iso: '2026-01-01T00:00:00.000Z',
				keys: [],
				signing_domain: 'getvela.app',
				signing_venue: { type: 'in_vela' as const }
			}
		},
		{
			index: 1,
			account: {
				id: 'b',
				name: 'Second',
				address: '0xBBBB000000000000000000000000000000000002',
				public_key_hex: '04',
				created_at_iso: '2026-01-02T00:00:00.000Z',
				keys: [],
				signing_domain: 'getvela.app',
				signing_venue: { type: 'in_vela' as const }
			}
		}
	];
	const usd = { code: 'USD', rate: 1, committed: true };

	it('is the whole sheet from the session and the balance core alone', async () => {
		const { liveAccountsSheet } = await import('./live');
		const balances = new Map<string, number>([
			[rows[0]!.account.address.toLowerCase(), 100],
			[rows[1]!.account.address.toLowerCase(), 20.5]
		]);
		const sheet = liveAccountsSheet(
			{
				rows,
				activeIndex: 1,
				balances,
				currency: usd,
				identicon: (a) => `<svg data-seed="${a}"/>`
			},
			m.accounts
		);
		expect(sheet.title).toBe(m.accounts.title);
		expect(sheet.primary).toBe(m.accounts.createNew);
		expect(sheet.secondary).toBe(m.accounts.signInExisting);
		expect(sheet.rows.map((r) => r.selected)).toEqual([false, true]);
		// The viewer's seed rides on every row, verbatim — never the short form.
		expect(sheet.rows.map((r) => r.addressFull)).toEqual(rows.map((r) => r.account.address));
		expect(sheet.rows[0]!.identiconSvg).toContain(rows[0]!.account.address);
		expect(sheet.rows[0]!.amount).toContain('100');
		expect(sheet.summary).toContain('2');
		expect(sheet.summary).toContain('120');
	});

	it('leaves an unpriced row blank rather than inventing a figure', async () => {
		const { liveAccountsSheet } = await import('./live');
		const sheet = liveAccountsSheet(
			{ rows, activeIndex: 0, balances: new Map(), currency: usd, identicon: () => '' },
			m.accounts
		);
		expect(sheet.rows.map((r) => r.amount)).toEqual(['', '']);
	});
});

// The out-of-gas relayer sheet has to answer a question before it asks for
// anything: who can actually fix this (spec 060)? Getting it wrong is either a
// shrug at a person who could have fixed it themselves, or a request for money
// that was never theirs to pay.
describe('the relayer bootstrap sheet', () => {
	const status = (operator_served: boolean): SendTreasuryStatus => ({
		chain_id: operator_served ? 1 : 5042002,
		address: '0x3e59292e18417f814112f731e7163534c6d2fe3c',
		asset: 'native',
		balance: '0',
		floor: '100000000000000',
		bootstrap_needed: true,
		operator_served,
		coin: {
			symbol: operator_served ? 'ETH' : 'USDC',
			balance: '0',
			floor: '0.0001',
			suggested: '0.0001'
		}
	});

	it('leads with telling the operator on a network Vela ships', () => {
		const panel = liveRelayer(status(true), m);
		expect(panel.report).toBeDefined();
		expect(panel.lead).toBe(m.relayer.operatorLead);
		expect(panel.report?.label).toBe(m.relayer.reportBtn);
		// The funding half stays behind a disclosure: it is the operator's bill.
		expect(panel.report?.selfFundLabel).toBe(m.relayer.selfFundToggle);
	});

	// Issue 466: "Report this" opens the in-app report in the same sheet. The
	// wallet route carries only the rescues' slice of the corpus, so the slice
	// holds the report's words — the same ones Settings → Send feedback reads.
	it('opens the in-app report, with this device’s lines and the sheet’s own words', () => {
		const rm = pickRescueMessages(m);
		const facts = {
			version: '0.9.7',
			client: 'web' as const,
			os: 'Chrome 151 on macOS',
			commit: 'abc1234',
			platform: 'Web · Chrome 151 on macOS',
			language: 'en',
			unreachable: [],
			failures: []
		};
		const panel = liveRelayReport(rm, facts);
		expect(panel.send).toBe(m.bugReport.send);
		expect(panel.consent).toBe(m.bugReport.consent);
		expect(panel.fallback.retry).toBe(m.common.tryAgain);
		expect(panel.previewLines).toEqual([
			`${m.bugReport.previewVersion}: v0.9.7 (abc1234)`,
			`${m.bugReport.previewPlatform}: Web · Chrome 151 on macOS`,
			`${m.bugReport.previewLanguage}: en`,
			`${m.bugReport.previewRpc}: ${m.bugReport.previewNone}`,
			`${m.bugReport.previewFailures}: ${m.bugReport.previewNone}`
		]);
		// No address among the device's own lines: the treasury rides in the
		// report's words, which the person reads in the box above.
		expect(panel.previewLines.join('\n')).not.toMatch(/0x[0-9a-f]{40}/i);
	});

	it('asks nobody to report a network only the person can reach', () => {
		const panel = liveRelayer(status(false), m);
		expect(panel.report).toBeUndefined();
		expect(panel.lead).toBe(m.relayer.customLead);
	});

	// Issue 422: the coin and every figure in it are the core's. This sheet
	// used to name the coin itself and ask for at least a flat 0.01 of it —
	// tens of dollars of ETH, a tenth of a cent of POL — while the phones
	// asked for the relay's shortfall. One stop now says one thing.
	it("asks for the core's contribution, in the core's coin", () => {
		const panel = liveRelayer(status(true), m);
		expect(panel.amountHint).toBe(fill(m.relayer.amountHint, { amount: '0.0001', symbol: 'ETH' }));
		expect(panel.balanceLine).toBe(
			fill(m.relayer.balanceLine, { balance: '0', floor: '0.0001', symbol: 'ETH' })
		);
	});

	it("names the stop's own chain's coin — POL on Polygon, never ETH", () => {
		const polygon: SendTreasuryStatus = {
			...status(true),
			chain_id: 137,
			coin: { symbol: 'POL', balance: '0.00004', floor: '0.0001', suggested: '0.00006' }
		};
		const panel = liveRelayer(polygon, m);
		expect(panel.amountHint).toBe(fill(m.relayer.amountHint, { amount: '0.00006', symbol: 'POL' }));
		expect(panel.amountHint).not.toContain('ETH');
		expect(panel.balanceLine).toContain('POL');
	});

	it('keeps every digit the core gives — a small contribution is not "0"', () => {
		const tiny: SendTreasuryStatus = {
			...status(true),
			coin: { symbol: 'ETH', balance: '0.0000996', floor: '0.0001', suggested: '0.0000004' }
		};
		expect(liveRelayer(tiny, m).amountHint).toContain('0.0000004');
	});

	it('names no amount when the core could read no figures', () => {
		const panel = liveRelayer({ ...status(true), coin: null }, m);
		expect(panel.amountHint).toBe('');
		expect(panel.balanceLine).toBeUndefined();
		expect(panel.address).toBe('0x3e59292e18417f814112f731e7163534c6d2fe3c');
	});

	it('keeps the non-refundable warning and the treasury address in both cases', () => {
		for (const served of [true, false]) {
			const panel = liveRelayer(status(served), m);
			expect(panel.callout.text).toBe(m.relayer.disclaimer);
			expect(panel.callout.tone).toBe('warning');
			expect(panel.address).toBe('0x3e59292e18417f814112f731e7163534c6d2fe3c');
			expect(panel.primary).toBe(m.relayer.retryBtn);
		}
	});
});

// ---------------------------------------------------------------------------
// Spec 092 — every network the wallet cannot reach, in one place
// ---------------------------------------------------------------------------

describe('the unreachable-networks list (spec 092)', () => {
	const USD: CurrencyView = { code: 'USD', rate: 1, committed: true };
	const CNY: CurrencyView = { code: 'CNY', rate: 7, committed: true };
	const row = (
		chain_id: number,
		line_key: string,
		last_seen_usd: number | null = null
	): UnreachableNetwork => ({
		chain_id,
		last_known: last_seen_usd === null ? 'not_read' : 'held',
		last_seen_usd,
		line_key
	});
	const view = (
		networks: UnreachableNetwork[],
		key: string | null,
		hidden = false
	): BalanceView => ({
		address: '0xabc',
		display_total_usd: hidden ? null : 10,
		balance_unknown: false,
		balance_partial: networks.length > 0,
		unreachable: false,
		notice: null,
		hidden,
		refreshing: false,
		last_refreshed_at_ms: 1,
		tokens: [],
		unpriced_tokens: [],
		failed_chain_ids: networks.map((n) => n.chain_id),
		rate_limited_chain_ids: [],
		unreachable_networks: networks,
		unreachable_key: key,
		holdings_loading: false,
		cached_total_usd: null,
		switcher: { open: false, loading: false, balances: [] }
	});
	const three = [
		row(1, 'assets.lastSeen', 4500),
		row(56, 'assets.lastSeenEmpty'),
		row(137, 'assets.notReadYet')
	];

	it('lists every network in the core’s order, each with what was last read there', () => {
		const panel = liveUnreachable(view(three, 'assets.unreachableMany'), USD, m);
		expect(panel.title).toBe("Can't reach 3 networks right now");
		expect(panel.summary).toBe(m.rescue.unreachableBody);
		expect(panel.rows.map((r) => [r.name, r.line, r.action])).toEqual([
			['Ethereum', 'Last seen $4,500.00', m.rescue.rpcFix],
			['BNB Chain', 'Held nothing when last read', m.rescue.rpcFix],
			['Polygon', 'Not read yet', m.rescue.rpcFix]
		]);
		expect(panel.rows.map((r) => r.chainId)).toEqual([1, 56, 137]);
	});

	it('names the one network, and writes the worth in the display currency', () => {
		const panel = liveUnreachable(
			view([row(1, 'assets.lastSeen', 100)], 'assets.unreachableOne'),
			CNY,
			m
		);
		expect(panel.title).toBe("Can't reach Ethereum right now");
		expect(panel.rows[0]?.line).toMatch(/700/);
	});

	it('masks the worth while privacy hides it, and keeps the network', () => {
		const panel = liveUnreachable(
			view([row(1, 'assets.lastSeen', null)], 'assets.unreachableOne', true),
			USD,
			m
		);
		expect(panel.rows).toHaveLength(1);
		expect(panel.rows[0]?.line).toBe('Last seen ••••');
	});

	it('says so once every network has come back', () => {
		const panel = liveUnreachable(view([], null), USD, m);
		expect(panel.title).toBe(m.rescue.unreachableNone);
		expect(panel.summary).toBeUndefined();
		expect(panel.rows).toEqual([]);
	});
});

// ---------------------------------------------------------------------------
// Spec 068 — the stored default transaction speed
// ---------------------------------------------------------------------------

describe('the default transaction speed, live (spec 068)', () => {
	const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;
	const view = (tier: FeeTierPrefView['tier'], committed: boolean): FeeTierPrefView => ({
		tier,
		committed,
		offered: ['fast', 'standard', 'slow']
	});
	const row = (model: ReturnType<typeof buildMobileState>) =>
		model.sections.flatMap((section) => section.rows).find((r) => r.id === 'fee-speed');

	it('shows the committed tier on the row and ticks it in the sheet', () => {
		const model = withLiveFeeSpeed(buildMobileState('st1', m, IDENTICON), view('slow', true));
		expect(row(model)?.value).toBe(m.feeSpeed.slow);
		expect(model.feeSpeedSheet.rows.filter((r) => r.selected).map((r) => r.id)).toEqual(['slow']);
	});

	// A device that never chose still has to read as something, and the
	// something is the factory default — the core's own view before any read,
	// `standard` since the Ethereum fee fix (it was `fast`).
	it('reads as the factory default when nothing was ever chosen', () => {
		const core = new FeeTierPrefCore();
		const factory = JSON.parse(core.view()) as FeeTierPrefView;
		core.free();
		expect(factory).toMatchObject({ tier: 'standard', committed: false });
		const model = withLiveFeeSpeed(buildMobileState('st1', m, IDENTICON), factory);
		expect(row(model)?.value).toBe(m.feeSpeed.standard);
		expect(model.feeSpeedSheet.rows.filter((r) => r.selected).map((r) => r.id)).toEqual([
			'standard'
		]);
	});

	it('never offers the dead `rapid` tier', () => {
		const model = withLiveFeeSpeed(buildMobileState('st1', m, IDENTICON), view('standard', true));
		expect(model.feeSpeedSheet.rows.map((r) => r.id)).toEqual(['fast', 'standard', 'slow']);
	});

	// One list of tiers for both layouts: the desktop dropdown is filled from
	// the phone sheet's own rows, so the two surfaces cannot drift apart.
	it('fills the desktop dropdown from the same rows, with the same tick', () => {
		const phone = withLiveFeeSpeed(buildMobileState('st1', m, IDENTICON), view('standard', true));
		const desktop = withLiveFeeSpeedDesktop(
			buildDesktopState('dst1', m, IDENTICON),
			view('standard', true),
			phone.feeSpeedSheet
		);
		expect(desktop.feeSpeed.rows[0].value).toBe(m.feeSpeed.standard);
		expect(desktop.feeSpeed.rows[0].options?.filter((r) => r.selected).map((r) => r.id)).toEqual([
			'standard'
		]);
	});
});
