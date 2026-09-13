/**
 * Canonical wallet-home fixtures (spec 015, data-model.md — the single canon
 * all four platforms port). Content is verbatim from `docs/design/wallet/` mocks;
 * builders merge it with resolved messages into display-ready view models.
 * Pure data + assembly: no fetching, no formatting rules, no business state.
 */
import { fill, type WalletMessages } from './messages';
import type {
	ActivityGroupModel,
	ActivityRowModel,
	AssetRowModel,
	BalanceModel,
	ChainRowModel,
	DesktopStateId,
	AssetDetailPanelModel,
	MobileStateId,
	NetworkPillModel,
	ReceivePanelModel,
	SectionModel,
	WalletDesktopModel,
	WalletHeaderModel,
	WalletHomeModel
} from './model';

// --- Canon ----------------------------------------------------------------

export const CHAIN_COLORS = {
	bnb: '#F0B90B',
	ethereum: '#627EEA',
	arbitrum: '#28A0F0',
	gnosis: '#21BCA5',
	base: '#0052FF',
	polygon: '#8247E5',
	// Spec 021: the receive list is the first screen to draw all eight
	// supported networks, not just the six the home screen holds balances on.
	optimism: '#FF0420',
	avalanche: '#E84142'
} as const;

export const PILL_DOTS = [CHAIN_COLORS.ethereum, CHAIN_COLORS.polygon, CHAIN_COLORS.bnb];

export const IDENTITY = {
	name: '大表哥',
	longName: '这是一个非常长',
	addressDisplay: '0x14fB1f…D1eA5c',
	addressFull: '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c'
} as const;

/** Identicon-board seeds (US3): cross-platform eyeball parity set. */
export const IDENTICON_BOARD_SEEDS = [
	IDENTITY.addressFull,
	'0xd8da6bf26964af9d7eed9e03e53415d37aa96045',
	'alice',
	'bob',
	'0x9F3c00000000000000000000000000000000021aE',
	''
] as const;

/** The privacy masks — exported so the live layer masks with the same glyphs. */
export const MASK = '••••';
export const BALANCE_MASK = '••••••';

/**
 * The live layer's chain colour, by chain id: the canon brand colour for a
 * built-in chain, neutral for anything else. Lives here because chain
 * colours are CONTENT (this file's audit exemption), not tokens.
 */
export function chainColor(chainId: number): string {
	const id = CHAINS_BY_ID[chainId];
	return (
		(id !== undefined ? CHAIN_COLORS[id as keyof typeof CHAIN_COLORS] : undefined) ?? '#8C8C8C'
	);
}

const CHAINS_BY_ID: Record<number, string> = {
	1: 'ethereum',
	56: 'bnb',
	137: 'polygon',
	42161: 'arbitrum',
	8453: 'base',
	100: 'gnosis'
};

type Identicon = (seed: string) => string;

interface ActivityFixture {
	kind: 'sent' | 'received' | 'dapp';
	/** subtitle direction: to/from get localized templates; plain stays verbatim. */
	direction: { to: string } | { from: string } | { plain: string };
	day: 'today' | 'yesterday' | { literal: string };
	clock?: string;
	amount: string;
	unit: string;
	positive: boolean;
	badgeColor: string;
}

const DEFAULT_ACTIVITY: ActivityFixture[] = [
	{
		kind: 'sent',
		direction: { to: 'hold on' },
		day: 'today',
		clock: '14:02',
		amount: '−2',
		unit: 'POL',
		positive: false,
		badgeColor: CHAIN_COLORS.polygon
	},
	{
		kind: 'received',
		direction: { from: '0x9F3c…21aE' },
		day: 'today',
		clock: '11:20',
		amount: '+120',
		unit: 'USDT',
		positive: true,
		badgeColor: CHAIN_COLORS.ethereum
	},
	{
		kind: 'dapp',
		direction: { plain: 'PancakeSwap · BNB Chain' },
		day: 'today',
		clock: '09:41',
		amount: '−0.05',
		unit: 'BNB',
		positive: false,
		badgeColor: CHAIN_COLORS.bnb
	},
	{
		kind: 'received',
		direction: { from: 'Alice' },
		day: 'yesterday',
		clock: '20:15',
		amount: '+50',
		unit: 'USDC',
		positive: true,
		badgeColor: CHAIN_COLORS.base
	}
];

const EXTREME_ACTIVITY: ActivityFixture[] = [
	{
		kind: 'sent',
		direction: { to: 'Alexandra' },
		day: 'today',
		amount: '−1234.5678',
		unit: 'POL',
		positive: false,
		badgeColor: CHAIN_COLORS.polygon
	},
	{
		kind: 'dapp',
		direction: { plain: 'app.uniswap.org · BNB' },
		day: 'today',
		amount: '−0.0000001',
		unit: 'BNB',
		positive: false,
		badgeColor: CHAIN_COLORS.bnb
	}
];

interface AssetFixture {
	ticker: string;
	chain: string;
	badgeColor: string;
	balance: string;
	fiat: string | null;
}

const DEFAULT_ASSETS: AssetFixture[] = [
	{
		ticker: 'BNB',
		chain: 'BNB Chain',
		badgeColor: CHAIN_COLORS.bnb,
		balance: '0.8533',
		fiat: '$496.46'
	},
	{
		ticker: 'ETH',
		chain: 'Arbitrum',
		badgeColor: CHAIN_COLORS.arbitrum,
		balance: '0.2253',
		fiat: '$422.62'
	},
	{
		ticker: 'ETH',
		chain: 'Ethereum',
		badgeColor: CHAIN_COLORS.ethereum,
		balance: '0.0689',
		fiat: '$129.25'
	},
	{
		ticker: 'XDAI',
		chain: 'Gnosis',
		badgeColor: CHAIN_COLORS.gnosis,
		balance: '74.3965',
		fiat: '$74.38'
	},
	{
		ticker: 'USDT',
		chain: 'Ethereum',
		badgeColor: CHAIN_COLORS.ethereum,
		balance: '53.4836',
		fiat: '$53.48'
	},
	{
		ticker: 'USDC',
		chain: 'Polygon',
		badgeColor: CHAIN_COLORS.polygon,
		balance: '12.04',
		fiat: '$12.04'
	}
];

const PARTIAL_PRICE_ASSETS: AssetFixture[] = [
	DEFAULT_ASSETS[0],
	DEFAULT_ASSETS[1],
	{ ticker: 'CAKE', chain: 'BNB Chain', badgeColor: CHAIN_COLORS.bnb, balance: '18.20', fiat: null }
];

const EXTREME_ASSETS: AssetFixture[] = [
	{
		ticker: 'WBTC',
		chain: '以太坊主网 Ethereum',
		badgeColor: CHAIN_COLORS.ethereum,
		balance: '0.00000042',
		fiat: '$0.03'
	},
	{
		ticker: 'USDT',
		chain: 'Ethereum',
		badgeColor: CHAIN_COLORS.ethereum,
		balance: '1,234,567.8901',
		fiat: '$1,234,567.89'
	}
];

interface ChainFixture {
	name: string;
	dot: string;
	count: number;
}

const CHAINS: ChainFixture[] = [
	{ name: 'BNB Chain', dot: CHAIN_COLORS.bnb, count: 1 },
	{ name: 'Ethereum', dot: CHAIN_COLORS.ethereum, count: 3 },
	{ name: 'Arbitrum', dot: CHAIN_COLORS.arbitrum, count: 1 },
	{ name: 'Gnosis', dot: CHAIN_COLORS.gnosis, count: 1 },
	{ name: 'Base', dot: CHAIN_COLORS.base, count: 1 },
	{ name: 'Polygon', dot: CHAIN_COLORS.polygon, count: 1 }
];

export const NETWORK_COUNT = 8;

export const MOBILE_STATES: MobileStateId[] = [
	'h1',
	'h1s',
	'h2',
	'h3',
	'h4',
	'h5',
	'h6',
	'h7',
	'h7x',
	'h8'
];

export const DESKTOP_STATES: DesktopStateId[] = ['d1', 'd2', 'd3'];

// --- Assembly -------------------------------------------------------------

function subtitle(m: WalletMessages, f: ActivityFixture, withTime: boolean): string {
	const direction =
		'to' in f.direction
			? fill(m.activity.toName, { name: f.direction.to })
			: 'from' in f.direction
				? fill(m.activity.fromName, { name: f.direction.from })
				: f.direction.plain;
	if (!withTime) return direction;
	const day =
		f.day === 'today'
			? m.activity.today
			: f.day === 'yesterday'
				? m.activity.yesterday
				: f.day.literal;
	const time = f.clock === undefined ? day : `${day} ${f.clock}`;
	// Desktop dApp rows drop the chain suffix in favor of the timestamp (D1 mock).
	const head = 'plain' in f.direction ? f.direction.plain.split(' · ')[0] : direction;
	return `${head} · ${time}`;
}

function activityRow(
	m: WalletMessages,
	f: ActivityFixture,
	opts: { masked?: boolean; withTime?: boolean } = {}
): ActivityRowModel {
	const title =
		f.kind === 'sent'
			? m.activity.sent
			: f.kind === 'received'
				? m.activity.received
				: m.activity.dapp;
	return {
		kind: f.kind,
		title,
		subtitle: subtitle(m, f, opts.withTime ?? false),
		amount: opts.masked === true ? MASK : f.amount,
		unit: f.unit,
		positive: f.positive,
		masked: opts.masked ?? false,
		badgeColor: f.badgeColor
	};
}

function groupByDay(
	m: WalletMessages,
	fixtures: ActivityFixture[],
	opts: { masked?: boolean; withTime?: boolean } = {}
): ActivityGroupModel[] {
	const groups: ActivityGroupModel[] = [];
	for (const f of fixtures) {
		const label =
			f.day === 'today'
				? m.activity.today
				: f.day === 'yesterday'
					? m.activity.yesterday
					: f.day.literal;
		const last = groups.at(-1);
		const row = activityRow(m, f, opts);
		if (last !== undefined && last.label === label) last.rows.push(row);
		else groups.push({ label, rows: [row] });
	}
	return groups;
}

function assetRow(m: WalletMessages, f: AssetFixture, masked = false): AssetRowModel {
	return {
		ticker: f.ticker,
		chain: f.chain,
		badgeColor: f.badgeColor,
		balance: masked ? MASK : f.balance,
		fiat: masked
			? { kind: 'masked' }
			: f.fiat === null
				? { kind: 'no-price', text: m.balance.noPrice }
				: { kind: 'value', text: f.fiat },
		masked
	};
}

/**
 * The sidebar/sheet network list. Exported so the spec-018 contacts desktop
 * shell feeds the SAME rows into the reused Sidebar without a second copy of
 * the chain canon (SC-006 is about components; this keeps the data single too).
 */
export function chainRowsFor(allNetworksLabel: string): ChainRowModel[] {
	return [
		{ name: allNetworksLabel, dot: 'all', count: NETWORK_COUNT, selected: true },
		...CHAINS.map((c) => ({ name: c.name, dot: c.dot, count: c.count, selected: false }))
	];
}

function chainRows(m: WalletMessages): ChainRowModel[] {
	return chainRowsFor(m.networkFilter.allNetworks);
}

function balance(
	m: WalletMessages,
	state: BalanceModel['state'],
	amount?: { integer: string; decimals: string },
	status?: BalanceModel['status']
): BalanceModel {
	return {
		label: m.balance.totalLabel,
		currency: 'USD',
		state,
		integer: state === 'hidden' ? BALANCE_MASK : amount?.integer,
		decimals: state === 'hidden' ? undefined : amount?.decimals,
		liveText: state === 'zero-live' ? m.balance.liveIndicator : undefined,
		status,
		a11yHide: m.balance.a11yHide,
		a11yShow: m.balance.a11yShow
	};
}

function header(m: WalletMessages, identicon: Identicon, long = false): WalletHeaderModel {
	void m;
	return {
		name: long ? IDENTITY.longName : IDENTITY.name,
		addressDisplay: IDENTITY.addressDisplay,
		addressFull: IDENTITY.addressFull,
		identiconSvg: identicon(IDENTITY.addressFull)
	};
}

const DEFAULT_BALANCE = { integer: '$1,383', decimals: '28' };

/** Assemble the mobile home view model for one H-state. */
export function buildMobileState(
	state: MobileStateId,
	m: WalletMessages,
	identicon: Identicon
): WalletHomeModel {
	const pillAll: NetworkPillModel = {
		kind: 'all',
		dots: [...PILL_DOTS],
		label: m.networkFilter.pillAll
	};
	const sections = (mode: SectionModel['mode']): [SectionModel, SectionModel] => [
		{
			title: m.sections.activity,
			action: m.sections.all,
			mode,
			empty: { title: m.activity.emptyTitle, caption: m.activity.emptyCaption }
		},
		{
			title: m.sections.assets,
			action: m.sections.all,
			mode,
			empty: { title: m.assets.emptyTitle, caption: m.assets.emptyCaption }
		}
	];

	const base: WalletHomeModel = {
		state,
		header: header(m, identicon, state === 'h7' || state === 'h7x'),
		pill:
			state === 'h7' || state === 'h7x'
				? { kind: 'single', dot: CHAIN_COLORS.bnb, label: 'BNB Chain' }
				: pillAll,
		balance: balance(m, 'normal', DEFAULT_BALANCE),
		actions: { receive: m.actions.receive, send: m.actions.send, scan: m.actions.scan },
		activitySection: sections('rows')[0],
		activityGroups: groupByDay(m, DEFAULT_ACTIVITY),
		assetsSection: sections('rows')[1],
		assetRows: DEFAULT_ASSETS.map((f) => assetRow(m, f)),
		tabs: { ...m.nav },
		textScale: state === 'h7x' ? 1.35 : 1
	};

	switch (state) {
		case 'h1':
			// First screen: two activity rows, three asset rows visible (scroll shows the rest).
			return base;
		case 'h1s':
			return base;
		case 'h2': {
			const [activitySection, assetsSection] = sections('empty');
			return {
				...base,
				balance: balance(m, 'zero-live', { integer: '$0', decimals: '00' }),
				activitySection,
				activityGroups: [],
				assetsSection,
				assetRows: []
			};
		}
		case 'h3': {
			const [activitySection, assetsSection] = sections('loading');
			return {
				...base,
				balance: balance(m, 'loading'),
				activitySection,
				activityGroups: [],
				assetsSection,
				assetRows: []
			};
		}
		case 'h4':
			return {
				...base,
				balance: balance(
					m,
					'normal',
					{ integer: '$1,383', decimals: '46' },
					{
						kind: 'warning',
						text: m.balance.unpriced
					}
				),
				activityGroups: groupByDay(m, DEFAULT_ACTIVITY.slice(0, 2)),
				assetRows: PARTIAL_PRICE_ASSETS.map((f) => assetRow(m, f))
			};
		case 'h5':
			return {
				...base,
				balance: balance(m, 'hidden'),
				activityGroups: groupByDay(m, DEFAULT_ACTIVITY, { masked: true }),
				assetRows: DEFAULT_ASSETS.map((f) => assetRow(m, f, true))
			};
		case 'h6':
			return {
				...base,
				balance: balance(m, 'normal', DEFAULT_BALANCE, {
					kind: 'refreshing',
					text: m.balance.stale
				})
			};
		case 'h7':
		case 'h7x':
			return {
				...base,
				balance: balance(m, 'normal', { integer: '$1,234,567', decimals: '89' }),
				activityGroups: groupByDay(m, EXTREME_ACTIVITY),
				assetRows: EXTREME_ASSETS.map((f) => assetRow(m, f))
			};
		case 'h8':
			return { ...base, sheet: { title: m.networkFilter.sheetTitle, rows: chainRows(m) } };
	}
}

/** Assemble the desktop view model for one D-state. */
export function buildDesktopState(
	state: DesktopStateId,
	m: WalletMessages,
	identicon: Identicon
): WalletDesktopModel {
	const receivePanel: ReceivePanelModel = {
		kind: 'receive',
		title: m.receive.title,
		token: {
			ticker: 'BNB',
			badgeColor: CHAIN_COLORS.bnb,
			detail: fill(m.receive.networkDetail, { name: 'BNB Chain', id: 56 })
		},
		qrCaption: m.receive.qrCaption,
		addressLabel: m.receive.addressLabel,
		addressFull: IDENTITY.addressFull,
		copyAddress: m.receive.copyAddress,
		warningTitle: m.receive.warningTitle,
		warningReminder: m.receive.warningReminder,
		networksLine: fill(m.receive.networksLine, { count: NETWORK_COUNT })
	};
	const assetDetailPanel: AssetDetailPanelModel = {
		kind: 'asset-detail',
		title: 'BNB',
		token: {
			ticker: 'BNB',
			badgeColor: CHAIN_COLORS.bnb,
			balance: '0.8533 BNB',
			fiatLine: '$496.46 · BNB Chain'
		},
		send: m.assetDetail.send,
		receive: m.assetDetail.receive,
		facts: [
			{ label: m.assetDetail.labelName, value: 'BNB' },
			{
				label: m.assetDetail.labelPrice,
				value: fill(m.assetDetail.priceValue, { symbol: 'BNB', value: '$581.85' })
			},
			{ label: m.assetDetail.labelContract, value: m.assetDetail.nativeToken },
			{ label: m.assetDetail.labelDecimals, value: '18' }
		],
		viewOnExplorer: m.assetDetail.viewOnExplorer,
		transactionsTitle: m.assetDetail.labelTransactions,
		rows: [
			activityRow(m, DEFAULT_ACTIVITY[2], { withTime: true }),
			activityRow(
				m,
				{
					kind: 'received',
					direction: { from: '0x21aE…9F3c' },
					day: { literal: '8月1日' },
					amount: '+0.9',
					unit: 'BNB',
					positive: true,
					badgeColor: CHAIN_COLORS.bnb
				},
				{ withTime: true }
			)
		]
	};

	return {
		state,
		sidebar: {
			header: header(m, identicon),
			nav: [
				{ id: 'wallet', label: m.nav.wallet, selected: true },
				{ id: 'contacts', label: m.nav.contacts, selected: false },
				{ id: 'explore', label: m.nav.explore, selected: false },
				{ id: 'settings', label: m.nav.settings, selected: false }
			],
			networksTitle: m.sidebar.networks,
			networks: chainRows(m)
		},
		balance: balance(m, 'normal', DEFAULT_BALANCE),
		actions: { receive: m.actions.receive, send: m.actions.send, scan: m.actions.scan },
		activitySection: {
			title: m.sections.activity,
			action: m.sections.all,
			mode: 'rows'
		},
		activityGroups: groupByDay(m, DEFAULT_ACTIVITY, { withTime: true }),
		assetsSection: { title: m.sections.assets, action: m.sections.add, mode: 'rows' },
		assetRows: DEFAULT_ASSETS.map((f) => assetRow(m, f)),
		panels: { receive: receivePanel, assetDetail: assetDetailPanel },
		initialPanel: state === 'd2' ? 'receive' : state === 'd3' ? 'asset-detail' : 'none',
		closeLabel: m.close
	};
}
