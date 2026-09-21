/**
 * The canon settings fixtures (spec 023).
 *
 * One data set feeds every state of every screen, phone and desktop, exactly
 * as spec 015/018 did — so a reviewer comparing two states is looking at the
 * same wallet, and the later "wire real settings" feature swaps this file for
 * a store without touching a component.
 *
 * Numbers, sizes, latencies and URLs are read off `design/settings/*.png`.
 * Where a mock shows a composed string ("200 条 · 1.0 MB") the parts are
 * composed HERE, because composition order is a translation concern and the
 * components must never learn one.
 */
import { fill } from '$lib/wallet/messages';
import type { SettingsMessages } from './messages';
import type {
	AboutModel,
	AccountsSheetModel,
	AddNetworkModel,
	BalanceDetailModel,
	ChainMarkModel,
	CheckItemModel,
	ConfirmSheetModel,
	DesktopSettingsStateId,
	EndpointsModel,
	FeedbackModel,
	IndexDownModel,
	MobileSettingsStateId,
	NetworkDetailModel,
	NetworkRowModel,
	RelayerModel,
	RpcBannerModel,
	RpcFixModel,
	RpcProvidersModel,
	SelectRowModel,
	SelectSheetModel,
	SettingsDesktopModel,
	SettingsHomeModel,
	SettingsNavItemModel,
	SettingsOverlayId,
	SettingsPageId,
	SettingsSectionModel,
	SignerPageModel,
	StorageModel
} from './model';

/** Re-exported so routes can enumerate states without importing `model`. */
export {
	DESKTOP_SETTINGS_STATES as DESKTOP_STATES,
	MOBILE_SETTINGS_STATES as MOBILE_STATES
} from './model';
import { BUILD_COMMIT, BUILD_VERSION } from '$lib/build/info';

/** The signed-in account the mocks draw. Shared with the wallet fixtures. */
export const ACCOUNT_NAME = '大表哥';
export const ADDRESS_FULL = '0x14fB1f4E2b9C7a5D8e3F6a1B4c7D9e2F5a8B1D1eA5c';
const ADDRESS_DISPLAY = '0x14fB...D1eA5c';

/** The other two accounts ST2 / DST1 list. */
const ACCOUNTS = [
	{ name: ACCOUNT_NAME, address: ADDRESS_FULL, display: ADDRESS_DISPLAY, amount: '$3,140.22' },
	{
		name: '旅行基金',
		address: '0x9a01c4E7b2F5a8D3e6C9b1A4d7F0e3B6c9D277C2b',
		display: '0x9a01...77C2b',
		amount: '$122.18'
	},
	{
		name: '试验田',
		address: '0x3Ce4f7A0b3D6e9C2a5F8b1E4d7C0a3F6b9E2A90f1',
		display: '0x3Ce4...A90f1',
		amount: '$0.00'
	}
];

const TOTAL_BALANCE = '$3,262.40';

/** Chain marks — letter over the chain's brand colour, as the mocks draw them. */
/**
 * Chain marks are CONTENT (a chain's brand colour), which is why this file is
 * exempt from the literal audit — and why the live layer imports its marks
 * from here instead of minting colours of its own (spec 024).
 */
export const MARKS: Record<string, ChainMarkModel> = {
	ethereum: { letter: 'E', color: '#627EEA' },
	bnb: { letter: 'B', color: '#F0B90B' },
	polygon: { letter: 'P', color: '#8247E5' },
	arbitrum: { letter: 'A', color: '#28A0F0' },
	base: { letter: 'B', color: '#0052FF' },
	gnosis: { letter: 'G', color: '#2E9E7E' },
	tempo: { letter: 'T', color: '#8C8C8C' },
	xlayer: { letter: 'X', color: '#8C8C8C' },
	zora: { letter: 'Z', color: '#8C8C8C' },
	zircuit: { letter: 'Z', color: '#2E9E7E' }
};

/** The twelve networks, in the order ST9 lists them. */
const NETWORKS = [
	{ id: 'ethereum', name: 'Ethereum', chainId: 1, latency: 45 },
	{ id: 'bnb', name: 'BNB Chain', chainId: 56, latency: 128 },
	{ id: 'polygon', name: 'Polygon', chainId: 137, latency: 45 },
	{ id: 'arbitrum', name: 'Arbitrum', chainId: 42161, latency: 45 },
	{ id: 'base', name: 'Base', chainId: 8453, latency: 45 },
	{ id: 'gnosis', name: 'Gnosis', chainId: 100, latency: 45 },
	{ id: 'tempo', name: 'Tempo', chainId: 4217, latency: 45 },
	{ id: 'xlayer', name: 'X Layer', chainId: 196, latency: 0, custom: true }
];

const NETWORK_COUNT = 12;

/**
 * Language endonyms.
 *
 * NOT corpus strings: a language picker names each language IN that language,
 * so the row reads the same whichever locale the app is in — which is the
 * whole point of showing 日本語 to somebody who cannot read the current UI.
 */
export const LOCALE_ENDONYMS: { id: string; label: string }[] = [
	{ id: 'en', label: 'English' },
	{ id: 'zh', label: '简体中文' },
	{ id: 'zh-TW', label: '繁體中文（台灣）' },
	{ id: 'zh-HK', label: '繁體中文（香港）' },
	{ id: 'ja', label: '日本語' },
	{ id: 'ko', label: '한국어' },
	{ id: 'vi', label: 'Tiếng Việt' },
	{ id: 'id', label: 'Bahasa Indonesia' },
	{ id: 'tr', label: 'Türkçe' },
	{ id: 'es-MX', label: 'Español (México)' },
	{ id: 'pt-BR', label: 'Português (Brasil)' },
	{ id: 'fr', label: 'Français' },
	{ id: 'de', label: 'Deutsch' },
	{ id: 'ru', label: 'Русский' },
	{ id: 'it', label: 'Italiano' }
];

/** Currency rows, with the symbol the ST5 badge shows. */
/** The live money formatter's glyph lookup — content, so it lives here. */
export function currencyGlyph(code: string): string {
	return currencySymbol(code) ?? code + ' ';
}

/**
 * The same lookup, saying out loud when it MISSED (issue 231). `currencyGlyph`
 * folds "no symbol" into a `CODE ` prefix, which is right inside a sentence
 * ("≈ PLN 4.00") and wrong beside a figure being typed, where a code belongs
 * after the number. One catalog behind both, so the send form's hero and the
 * "≈" line beneath it cannot come to disagree about what a currency looks like.
 */
export function currencySymbol(code: string): string | null {
	return CURRENCIES.find((c) => c.id === code)?.glyph ?? null;
}

const CURRENCIES = [
	{ id: 'USD', glyph: '$', caption: '美元' },
	{ id: 'EUR', glyph: '€', caption: '欧元' },
	{ id: 'GBP', glyph: '£', caption: '英镑' },
	{ id: 'CNY', glyph: '¥', caption: '人民币' },
	{ id: 'JPY', glyph: '¥', caption: '日元' },
	{ id: 'KRW', glyph: '₩', caption: '韩元' },
	{ id: 'HKD', glyph: '$', caption: '港元' },
	{ id: 'VND', glyph: '₫', caption: '越南盾' }
];

const NUMBER_SAMPLES = [
	'1,234,567.89',
	'1,234,567.89',
	'1.234.567,89',
	'1 234 567,89',
	'12,34,567.89'
];
const DATE_SAMPLES = [
	'2026/06/13',
	'2026/06/13',
	'06/13/2026',
	'13/06/2026',
	'13.06.2026',
	'2026-06-13'
];
const TIME_SAMPLES = ['13:45', '13:45', '1:45 PM'];

// The build's own, not the mock's — see `$lib/build/info` (spec 064).
const APP_VERSION = BUILD_VERSION;
const APP_COMMIT = BUILD_COMMIT;

/** `45ms` etc. — the latency pill's own text, tone chosen by the number. */
function latencyPill(ms: number, prefix?: string) {
	const tone = ms >= 1000 ? 'warn' : 'ok';
	const label = ms >= 1000 ? `${(ms / 1000).toFixed(1)}s` : `${ms}ms`;
	return { tone, label: prefix === undefined ? label : `${prefix} · ${label}`, dot: true } as const;
}

/** The live layer's mark lookup: builtin marks by id, else a neutral letter. */
export function markFor(id: string, name: string): ChainMarkModel {
	return MARKS[id] ?? { letter: (name.trim()[0] ?? '?').toUpperCase(), color: '#8C8C8C' };
}

export function chainMeta(m: SettingsMessages, chainId: number): string {
	return fill(m.networks.chainId, { chainId });
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

/**
 * ST1/ST1b's row list.
 *
 * `advancedOpen` is the ST1b disclosure. The section is one model either way —
 * a collapsed 高级 still owns its rows, so opening it is a flag rather than a
 * different list, and the desktop (which never collapses) reads the same data.
 */
function sections(m: SettingsMessages, advancedOpen: boolean): SettingsSectionModel[] {
	return [
		{
			rows: [
				{
					id: 'contacts',
					icon: 'users-round',
					title: m.contacts,
					subtitle: m.account.contactsSubtitle,
					trailing: 'chevron'
				},
				{
					id: 'feedback',
					icon: 'message-square-text',
					title: m.feedback.title,
					subtitle: m.feedback.subtitle,
					trailing: 'external'
				}
			]
		},
		{
			label: m.sections.appearance,
			appearanceControls: true,
			rows: [
				{
					id: 'language',
					icon: 'globe',
					title: m.language.title,
					value: `简体中文 · ${m.common.system}`,
					trailing: 'chevron'
				}
			]
		},
		{
			label: m.sections.localization,
			rows: [
				{
					id: 'currency',
					icon: 'coins',
					title: m.localization.currencyTitle,
					value: 'USD · $1,234.56',
					trailing: 'chevron'
				},
				{
					id: 'number-format',
					icon: 'hash',
					title: m.localization.numberTitle,
					value: NUMBER_SAMPLES[0],
					trailing: 'chevron'
				},
				{
					id: 'date-format',
					icon: 'calendar',
					title: m.localization.dateTitle,
					value: DATE_SAMPLES[0],
					trailing: 'chevron'
				},
				{
					id: 'time-format',
					icon: 'clock',
					title: m.localization.timeTitle,
					value: TIME_SAMPLES[0],
					trailing: 'chevron'
				}
			]
		},
		{
			label: m.sections.advanced,
			collapsible: true,
			collapsed: !advancedOpen,
			rows: [
				{
					id: 'networks',
					icon: 'network',
					title: m.advanced.networksTitle,
					subtitle: m.advanced.networksSubtitle,
					value: fill(m.networks.count, { count: NETWORK_COUNT }),
					trailing: 'chevron'
				},
				{
					id: 'rpc-providers',
					icon: 'server',
					title: m.advanced.rpcProvidersTitle,
					subtitle: m.advanced.rpcProvidersSubtitle,
					trailing: 'chevron'
				},
				{
					id: 'add-network',
					icon: 'plus',
					title: m.advanced.addNetworkTitle,
					subtitle: m.advanced.addNetworkSubtitle,
					trailing: 'chevron'
				},
				{
					id: 'endpoints',
					icon: 'zap',
					title: m.advanced.endpointsTitle,
					subtitle: m.advanced.endpointsSubtitle,
					trailing: 'chevron'
				},
				// Spec 068. It sits in 高级 rather than 本地化 because it is not
				// about how a figure READS — it changes what a transaction costs
				// and how fast it lands. The value shown is the committed
				// preference; the fixture draws the factory default.
				// `clock`, not the `zap` above it: two adjacent rows wearing the same
				// lightning bolt read as one group and neither is scannable, and a
				// money-and-speed preference is nothing like a list of endpoints.
				{
					id: 'fee-speed',
					icon: 'clock',
					title: m.advanced.feeSpeedTitle,
					subtitle: m.advanced.feeSpeedSubtitle,
					value: m.feeSpeed.fast,
					trailing: 'chevron'
				},
				// Spec 071, beside the speed: the other thing every signature
				// starts at. The fixture draws the factory default and the
				// official page.
				{
					id: 'sign-with',
					icon: 'pencil',
					title: m.signing.title,
					value: m.signing.methods.auto,
					trailing: 'chevron'
				},
				{
					id: 'clear-signer-page',
					icon: 'link-2',
					title: m.signing.pageTitle,
					value: m.signing.pageOfficial,
					trailing: 'chevron'
				},
				{
					id: 'storage',
					icon: 'hard-drive',
					title: m.storage.title,
					subtitle: m.storage.subtitle,
					trailing: 'chevron'
				}
			]
		},
		{
			rows: [
				{
					id: 'about',
					icon: 'info',
					title: m.about.title,
					value: fill(m.about.subtitleTemplate, { version: APP_VERSION }),
					trailing: 'chevron'
				}
			]
		}
	];
}

function networkRows(m: SettingsMessages, expandedId?: string): NetworkRowModel[] {
	return NETWORKS.map((n) => ({
		id: n.id,
		mark: MARKS[n.id],
		name: n.name,
		meta: chainMeta(m, n.chainId),
		badge: n.custom === true ? undefined : latencyPill(n.latency),
		tag: n.custom === true ? m.networks.custom : undefined,
		removable: n.custom === true,
		expanded: expandedId === n.id
	}));
}

/** ST9b / DST4's expanded editor. `mismatch` adds the red "not saved" callout. */
function networkDetail(m: SettingsMessages, mismatch: boolean): NetworkDetailModel {
	return {
		title: 'Ethereum',
		subtitle: `${chainMeta(m, 1)} · ETH`,
		mark: MARKS.ethereum,
		name: 'Ethereum',
		note: m.networks.builtinNote,
		badge: latencyPill(45, m.networks.online),
		rpc: {
			id: 'rpc',
			label: m.networks.rpcUrl,
			value: 'https://eth.llamarpc.com',
			hint: m.networks.saveHint,
			badge: latencyPill(45),
			tone: mismatch ? 'error' : 'default'
		},
		explorer: { id: 'explorer', label: m.networks.explorer, value: 'https://etherscan.io' },
		callout: mismatch
			? { tone: 'danger', text: fill(m.networks.mismatch, { reported: 56, expected: 1 }) }
			: undefined
	};
}

/** ST10 search, ST10b compatible, ST10c incompatible — one builder, three modes. */
function addNetwork(
	m: SettingsMessages,
	mode: 'search' | 'compatible' | 'incompatible'
): AddNetworkModel {
	const base = {
		title: m.advanced.addNetworkTitle,
		subtitle: m.addNetwork.description,
		searchPlaceholder: m.addNetwork.searchPlaceholder
	};
	if (mode === 'search') {
		return {
			...base,
			results: [
				{ id: 'zora', mark: MARKS.zora, name: 'Zora', meta: chainMeta(m, 7777777) },
				{ id: 'zircuit', mark: MARKS.zircuit, name: 'Zircuit', meta: chainMeta(m, 48900) },
				{
					id: 'zora-sepolia',
					mark: MARKS.zora,
					name: 'Zora Sepolia',
					meta: chainMeta(m, 999999999),
					tag: m.addNetwork.testnet
				}
			]
		};
	}
	// Four rows in both modes: "incompatible" is only legible as an answer if
	// it shows WHICH requirement failed, so the list never shortens.
	const ok = mode === 'compatible';
	const checks: CheckItemModel[] = [
		{ label: m.addNetwork.checkEntryPoint, ok: true },
		{ label: m.addNetwork.checkSafe, ok },
		{ label: m.addNetwork.checkSigner, ok },
		{ label: fill(m.addNetwork.checkRemaining, { count: 8 }), ok }
	];
	if (mode === 'compatible') {
		return {
			...base,
			subtitle: `Zora · ${chainMeta(m, 7777777)}`,
			results: [],
			candidate: {
				mark: MARKS.zora,
				name: 'Zora',
				meta: fill(m.addNetwork.bestRpc, { latencyMs: 182 }),
				badge: { tone: 'ok', label: m.addNetwork.compatible, dot: true }
			},
			checksTitle: m.addNetwork.compatibilityCheck,
			checks,
			customRpc: {
				id: 'custom-rpc',
				label: m.addNetwork.customRpcTitle,
				value: '',
				placeholder: m.addNetwork.customRpcPlaceholder
			},
			primary: m.addNetwork.addNetworkBtn
		};
	}
	return {
		...base,
		subtitle: `Zircuit · ${chainMeta(m, 48900)}`,
		results: [],
		candidate: {
			mark: MARKS.zircuit,
			name: 'Zircuit',
			meta: m.addNetwork.compatibilityCheck,
			badge: { tone: 'error', label: m.addNetwork.incompatible, dot: true }
		},
		checksTitle: m.addNetwork.compatibilityCheck,
		checks,
		callout: { tone: 'warning', text: m.addNetwork.incompatibleHint },
		secondary: m.addNetwork.openChainSetupTool,
		recheck: m.addNetwork.recheckWithRpc
	};
}

/**
 * Where each provider's API key is made. The panel sent every "Get key →" to
 * drpc.org, Alchemy's and Ankr's included.
 */
export const PROVIDER_KEY_URLS = {
	alchemy: 'https://dashboard.alchemy.com/',
	drpc: 'https://drpc.org/',
	ankr: 'https://www.ankr.com/rpc/'
} as const;

function rpcProviders(m: SettingsMessages, withLatency: boolean): RpcProvidersModel {
	const support = fill(m.rpcProviders.supportsCount, { count: 12, total: NETWORK_COUNT });
	return {
		title: m.advanced.rpcProvidersTitle,
		subtitle: m.advanced.rpcProvidersSubtitle,
		description: m.rpcProviders.description,
		providers: [
			{
				id: 'alchemy',
				name: 'Alchemy',
				badge: { tone: 'ok', label: m.rpcProviders.connected, dot: true },
				field: { id: 'alchemy', label: '', value: 'alch_k3y...9fQ2' },
				action: m.rpcProviders.checkKey,
				support: withLatency
					? `${support} · ${fill(m.rpcProviders.avgLatency, { ms: 112 })}`
					: support
			},
			{
				id: 'drpc',
				name: 'dRPC',
				badge: { tone: 'neutral', label: m.rpcProviders.notSet, dot: true },
				field: { id: 'drpc', label: '', value: '', placeholder: m.rpcProviders.notSet },
				action: m.rpcProviders.getKey,
				actionUrl: PROVIDER_KEY_URLS.drpc,
				link: `${m.rpcProviders.getKey} →`,
				linkUrl: PROVIDER_KEY_URLS.drpc
			},
			{
				id: 'ankr',
				name: 'Ankr',
				badge: { tone: 'neutral', label: m.rpcProviders.notSet, dot: true },
				field: { id: 'ankr', label: '', value: '', placeholder: m.rpcProviders.notSet },
				action: m.rpcProviders.getKey,
				actionUrl: PROVIDER_KEY_URLS.ankr,
				support: fill(m.rpcProviders.supportsCount, { count: 8, total: NETWORK_COUNT })
			}
		]
	};
}

function endpoints(m: SettingsMessages, withGuide: boolean): EndpointsModel {
	return {
		title: m.advanced.endpointsTitle,
		description: m.endpoints.description,
		fields: [
			{
				id: 'chain-data',
				label: m.endpoints.chainDataLabel,
				value: 'https://ethereum-data.getvela.app',
				hint: m.endpoints.chainDataHint,
				badge: latencyPill(62)
			},
			{
				id: 'passkey',
				label: m.endpoints.passkeyLabel,
				value: 'https://p256-index-rs.getvela.app',
				hint: m.endpoints.passkeyHint,
				badge: latencyPill(88)
			},
			{
				id: 'relay',
				label: m.endpoints.bundlerLabel,
				value: 'https://vela-relay-cf.getvela.app',
				hint: m.endpoints.bundlerHint,
				badge: latencyPill(104)
			},
			{
				id: 'fiat',
				label: m.endpoints.fiatLabel,
				value: 'https://vela-currency.getvela.app/v2/…',
				hint: m.endpoints.fiatHint,
				badge: latencyPill(1200, m.networks.slow)
			}
		],
		reset: m.endpoints.reset,
		guide: withGuide ? m.endpoints.guide : undefined
	};
}

function storage(m: SettingsMessages): StorageModel {
	return {
		title: m.storage.title,
		subtitle: m.storage.subtitle,
		amount: '2.4',
		unit: 'MB',
		summary: fill(m.storage.summary, { count: 216 }),
		segments: [
			{ id: 'user', label: m.storage.legendUserData, fraction: 0.5, color: '#5A7CF6' },
			{ id: 'cache', label: m.storage.legendCaches, fraction: 0.3, color: '#3DA872' },
			{ id: 'sessions', label: m.storage.legendSessions, fraction: 0.2, color: '#85827A' }
		],
		groups: [
			{
				label: m.storage.userData,
				items: [
					{
						id: 'transactions',
						label: m.storage.itemTransactions,
						meta: `${fill(m.storage.records, { count: 200 })} · 1.0 MB`,
						action: m.storage.clear,
						destructive: true
					},
					{
						id: 'contacts',
						label: m.storage.itemContacts,
						meta: `${fill(m.storage.contactsCount, { count: 18 })} · 42 KB`,
						action: m.storage.clear,
						destructive: true
					},
					{
						id: 'custom',
						label: m.storage.itemCustom,
						meta: `${fill(m.storage.itemsCount, { count: 5 })} · 12 KB`,
						action: m.storage.clear,
						destructive: true
					},
					{
						id: 'browsing',
						label: m.storage.itemBrowsing,
						meta: `${fill(m.storage.records, { count: 31 })} · 58 KB`,
						action: m.storage.clear,
						destructive: true
					}
				]
			},
			{
				label: m.storage.caches,
				action: m.storage.clearAllCaches,
				items: [
					{
						id: 'balances',
						label: m.storage.itemBalances,
						meta: '0.6 MB',
						action: m.storage.clear
					},
					{ id: 'rates', label: m.storage.itemRates, meta: '96 KB', action: m.storage.clear },
					{ id: 'scan', label: m.storage.itemScan, meta: '31 KB', action: m.storage.clear }
				]
			},
			{
				label: m.storage.connections,
				items: [
					{
						id: 'dapps',
						label: m.storage.itemDapps,
						meta: fill(m.storage.sitesCount, { count: 4 }),
						action: m.storage.disconnectAll,
						destructive: true
					}
				]
			}
		]
	};
}

function about(m: SettingsMessages, withLinksHeading: boolean): AboutModel {
	return {
		title: m.about.title,
		tagline: m.about.tagline,
		version: fill(m.about.version, { version: APP_VERSION, commit: APP_COMMIT }),
		sectionTechnical: m.about.sectionTechnical,
		rows: [
			{ label: m.about.techWalletLabel, value: m.about.techWalletValue, mono: true },
			{ label: m.about.techAuthLabel, value: m.about.techAuthValue, mono: true },
			{ label: m.about.techAccountTypeLabel, value: m.about.techAccountTypeValue },
			{ label: m.about.techSignerLabel, value: m.about.techSignerValue },
			{
				id: 'networks',
				label: m.about.techNetworksLabel,
				value: fill(m.about.techNetworksValue, { count: NETWORK_COUNT })
			}
		],
		sectionLinks: withLinksHeading ? m.about.sectionLinks : undefined,
		links: [
			{
				label: m.about.linkWebsite,
				value: 'getvela.app',
				mono: true,
				external: true,
				href: 'https://getvela.app'
			},
			{
				label: m.about.linkGitHub,
				value: 'github.com/mondaylabsltd/vela-wallet',
				mono: true,
				external: true,
				href: 'https://github.com/mondaylabsltd/vela-wallet'
			},
			{
				label: m.about.linkSafeWallet,
				value: 'safe.global',
				mono: true,
				external: true,
				href: 'https://safe.global'
			}
		],
		footer: m.about.footer
	};
}

// ---------------------------------------------------------------------------
// Overlays
// ---------------------------------------------------------------------------

function accountsSheet(
	m: SettingsMessages,
	identicon: (seed: string) => string
): AccountsSheetModel {
	return {
		title: m.accounts.title,
		summary: `${fill(m.accounts.countPrefix, { count: ACCOUNTS.length })}${fill(m.accounts.total, { amount: TOTAL_BALANCE })}`,
		rows: ACCOUNTS.map((a, i) => ({
			name: a.name,
			addressDisplay: a.display,
			addressFull: a.address,
			identiconSvg: identicon(a.address),
			amount: a.amount,
			selected: i === 0
		})),
		primary: m.accounts.createNew,
		secondary: m.accounts.signInExisting
	};
}

function signOutSheet(m: SettingsMessages, warned: boolean): ConfirmSheetModel {
	return {
		title: m.signOut.title,
		body: m.signOut.desc,
		note: m.signOut.keeps,
		callout: warned ? { tone: 'warning', text: m.signOut.warning } : undefined,
		confirm: warned ? m.signOut.anyway : m.signOut.button,
		cancel: m.signOut.cancel,
		tone: 'danger'
	};
}

/**
 * The language picker's rows: 跟随系统 first, with the locale that currently
 * resolves to beside it, then every shipped locale by its endonym. Shared by
 * the phone's sheet and the desktop's dropdown, which offer the same choice.
 */
export function languageRows(m: SettingsMessages, current: string): SelectRowModel[] {
	const currentLabel = LOCALE_ENDONYMS.find((l) => l.id === current)?.label ?? current;
	return [
		{
			id: 'system',
			label: m.language.followSystem,
			note: `${m.common.system} · ${currentLabel}`,
			selected: true
		},
		...LOCALE_ENDONYMS.map((l) => ({ id: l.id, label: l.label }))
	];
}

function languageSheet(m: SettingsMessages, current: string): SelectSheetModel {
	return {
		title: m.language.pickerTitle,
		subtitle: m.language.pickerSubtitle,
		rows: languageRows(m, current),
		footerNote: m.language.contributeNote,
		footerLink: m.language.contributeCta
	};
}

/**
 * The default-speed sheet (spec 068).
 *
 * Three rows, fastest first, each NAMED by its speed and DESCRIBED by what
 * that speed buys — the owner's ruling: the heading asks 交易速度, so every
 * option has to be a speed, and the advantage goes on the second line where it
 * is the reason somebody would pick the slow one. `rapid` is not offered — it
 * is a dead variant nothing constructs and the relay refuses on the wire.
 * The subtitle is load-bearing: it says that this is where every transaction
 * STARTS, and that a single send can still be changed, so nobody reads the
 * send screen's picker as having quietly rewritten this.
 */
function feeSpeedSheet(m: SettingsMessages): SelectSheetModel {
	return {
		title: m.feeSpeed.title,
		subtitle: m.feeSpeed.subtitle,
		rows: [
			{ id: 'fast', label: m.feeSpeed.fast, detail: m.feeSpeed.fastHint, selected: true },
			{ id: 'standard', label: m.feeSpeed.standard, detail: m.feeSpeed.standardHint },
			{ id: 'slow', label: m.feeSpeed.slow, detail: m.feeSpeed.slowHint }
		]
	};
}

/** The official Clear Signer page (`clear_signer::DEFAULT_SIGNER_URL`). */
const OFFICIAL_SIGNER_URL = 'https://sign.getvela.app/';

/**
 * The default "Sign with" (spec 071): the five the core offers, in its order,
 * named as the signing sheet names them. The subtitle is load-bearing, as the
 * speed sheet's is: this is where every signature STARTS, and a single one
 * can still be signed another way.
 */
function signWithSheet(m: SettingsMessages): SelectSheetModel {
	return {
		title: m.signing.title,
		subtitle: m.signing.subtitle,
		rows: [
			{ id: 'auto', label: m.signing.methods.auto, selected: true },
			{ id: 'platform', label: m.signing.methods.platform },
			{ id: 'hybrid', label: m.signing.methods.hybrid },
			{ id: 'security_key', label: m.signing.methods.security_key },
			{
				id: 'clear_signer',
				label: m.signing.methods.clear_signer,
				detail: m.signing.clearSignerBody
			}
		]
	};
}

/** The Clear Signer's page, official — what a device that never chose opens. */
function signerPage(m: SettingsMessages): SignerPageModel {
	return {
		title: m.signing.pageTitle,
		subtitle: m.signing.pageSubtitle,
		field: {
			id: 'signer-url',
			label: m.signing.pageTitle,
			value: OFFICIAL_SIGNER_URL,
			placeholder: OFFICIAL_SIGNER_URL
		},
		save: m.signing.pageSave
	};
}

function currencySheet(m: SettingsMessages): SelectSheetModel {
	return {
		title: m.currency.title,
		searchPlaceholder: m.currency.searchPlaceholder,
		rows: CURRENCIES.map((c, i) => ({
			id: c.id,
			label: c.id,
			glyph: c.glyph,
			caption: c.caption,
			selected: i === 0
		}))
	};
}

/**
 * The three format pickers.
 *
 * Row 0 is always 自动 — it renders the sample the current system would give,
 * with the "自动 · 系统" note; the rest are explicit choices. That shape is the
 * same for numbers, dates and times, so it is one builder with three sample
 * lists rather than three near-identical ones.
 */
function formatSheet(
	m: SettingsMessages,
	title: string,
	subtitle: string | undefined,
	samples: string[],
	notes: (string | undefined)[]
): SelectSheetModel {
	return {
		title,
		subtitle,
		rows: samples.map((sample, i) => ({
			id: `${i}`,
			label: sample,
			mono: true,
			note: i === 0 ? `${m.common.automatic} · ${m.common.system}` : notes[i],
			selected: i === 0
		}))
	};
}

function clearCachesSheet(m: SettingsMessages): ConfirmSheetModel {
	return {
		title: m.storage.clearTitle,
		body: m.storage.clearBody,
		confirm: m.storage.clearConfirm,
		cancel: m.common.cancel,
		tone: 'accent'
	};
}

/** What removing a custom network asks first; the sheet's title is the network's name. */
function removeNetworkSheet(m: SettingsMessages): ConfirmSheetModel {
	return {
		title: m.networks.remove,
		body: m.networks.removeBody,
		confirm: m.networks.removeConfirm,
		cancel: m.networks.removeCancel,
		tone: 'danger'
	};
}

function eraseSheet(m: SettingsMessages): ConfirmSheetModel {
	return {
		title: m.erase.title,
		body: m.erase.desc,
		note: m.erase.keeps,
		callout: { tone: 'danger', text: m.erase.loses },
		confirm: m.erase.confirm,
		cancel: m.erase.cancel,
		tone: 'danger'
	};
}

function feedback(m: SettingsMessages): FeedbackModel {
	return {
		title: m.bugReport.title,
		subtitle: m.bugReport.subtitle,
		placeholder: m.bugReport.whatPlaceholder,
		addSteps: m.bugReport.addSteps,
		previewToggle: m.bugReport.previewToggle,
		// Label AND value on every line: the point of this block is that the
		// person can read what is about to leave their device, and a bare list
		// of values is not readable.
		previewLines: [
			`${m.bugReport.previewVersion}: v${APP_VERSION} (${APP_COMMIT})`,
			`${m.bugReport.previewPlatform}: iOS 26.0`,
			`${m.bugReport.previewLanguage}: zh`,
			`${m.bugReport.previewRpc}: ${m.bugReport.previewNone}`,
			`${m.bugReport.previewFailures}: ${m.bugReport.previewNone}`
		],
		consent: m.bugReport.consent,
		send: m.bugReport.send,
		githubLink: m.bugReport.openGithubForm
	};
}

// ---------------------------------------------------------------------------
// Rescue (SR / DSR)
// ---------------------------------------------------------------------------

function rpcBanner(m: SettingsMessages): RpcBannerModel {
	return {
		text: fill(m.rescue.rpcUnavailableMultiple, { count: 2 }),
		chips: [
			{ id: 'polygon', mark: MARKS.polygon, name: 'Polygon', action: m.rescue.rpcFix },
			{ id: 'gnosis', mark: MARKS.gnosis, name: 'Gnosis', action: m.rescue.rpcFix }
		]
	};
}

/** SR2 (failing) and SR2b (restored) are one model with a flag. */
function rpcFix(m: SettingsMessages, restored: boolean): RpcFixModel {
	return {
		title: m.rescue.rpcFixTitle,
		mark: MARKS.polygon,
		name: 'Polygon',
		meta: `${chainMeta(m, 137)} · POL`,
		badge: restored
			? latencyPill(96, m.networks.online)
			: { tone: 'error', label: m.networks.offline, dot: true },
		callout: restored
			? { tone: 'success', text: m.rescue.rpcFixRestored, icon: 'check' }
			: { tone: 'warning', text: m.rescue.rpcFixWarning },
		field: {
			id: 'rpc',
			label: m.rescue.rpcFixLabel,
			value: 'https://polygon-rpc.com',
			badge: restored ? latencyPill(96) : undefined,
			tone: restored ? 'success' : 'error'
		},
		primary: restored ? m.common.done : m.rescue.rpcFixSaveBtn,
		providersLabel: restored ? undefined : m.rescue.rpcProvidersTitle,
		providers: restored
			? undefined
			: [
					{ label: 'Alchemy', href: 'https://alchemy.com' },
					{ label: 'QuickNode', href: 'https://quicknode.com' },
					{ label: 'dRPC', href: 'https://drpc.org' },
					{ label: 'Chainlist', href: 'https://chainlist.org' }
				],
		report: restored ? undefined : m.rescue.rpcReport
	};
}

function balanceDetail(m: SettingsMessages): BalanceDetailModel {
	return {
		title: m.balanceDetail.title,
		summary: fill(m.balanceDetail.total, { amount: TOTAL_BALANCE }),
		sectionPending: m.balanceDetail.networksLabel,
		pendingNote: m.balanceDetail.networksNote,
		pending: [
			{
				id: 'polygon',
				mark: MARKS.polygon,
				name: 'Polygon',
				status: m.balanceDetail.statusRetrying,
				tone: 'neutral'
			},
			{
				id: 'gnosis',
				mark: MARKS.gnosis,
				name: 'Gnosis',
				status: m.balanceDetail.statusFailed,
				tone: 'error',
				action: m.balanceDetail.retry
			}
		],
		sectionDone: m.balanceDetail.updatedLabel,
		done: [
			{ id: 'ethereum', mark: MARKS.ethereum, name: 'Ethereum', amount: '$2,412.11' },
			{ id: 'bnb', mark: MARKS.bnb, name: 'BNB Chain', amount: '$850.29' }
		],
		sectionUnpriced: 'Some tokens couldn’t be priced.',
		unpriced: []
	};
}

function relayer(m: SettingsMessages): RelayerModel {
	return {
		title: m.relayer.title,
		lead: m.relayer.operatorLead,
		mark: MARKS.gnosis,
		name: 'Gnosis',
		amountHint: fill(m.relayer.amountHint, { amount: '0.01', symbol: 'xDAI' }),
		qrCaption: m.relayer.addressLabel,
		addressDisplay: '0x7Bd0...4E9c',
		copyLabel: m.relayer.copyBtn,
		callout: { tone: 'warning', text: m.relayer.disclaimer },
		primary: m.relayer.retryBtn,
		// Gnosis is a network Vela ships, so the gallery shows the case that
		// leads with telling the operator (spec 060).
		report: {
			label: m.relayer.reportBtn,
			selfFundLabel: m.relayer.selfFundToggle
		}
	};
}

function indexDown(m: SettingsMessages): IndexDownModel {
	return {
		title: m.indexDown.title,
		subtitle: m.indexDown.subtitle,
		callout: { tone: 'warning', text: m.indexDown.warning },
		field: {
			id: 'endpoint',
			label: m.indexDown.endpointLabel,
			value: 'https://p256-index-rs.getvela.app',
			badge: { tone: 'error', label: m.networks.offline, dot: true }
		},
		primary: m.common.tryAgain,
		secondary: m.indexDown.editEndpoint,
		footer: m.indexDown.passkeyHint
	};
}

// ---------------------------------------------------------------------------
// State table
// ---------------------------------------------------------------------------

/** Which page + overlay each mobile mock is. The screens read only this. */
const MOBILE_SHAPE: Record<
	MobileSettingsStateId,
	{
		page: SettingsPageId;
		overlay: SettingsOverlayId;
		tab?: 'wallet' | 'settings';
		backdrop?: 'wallet' | 'send' | 'storage';
	}
> = {
	st1: { page: 'home', overlay: 'none' },
	st1b: { page: 'home', overlay: 'none' },
	st2: { page: 'home', overlay: 'accounts' },
	st3: { page: 'home', overlay: 'sign-out' },
	st3b: { page: 'home', overlay: 'sign-out' },
	st4: { page: 'home', overlay: 'language' },
	st5: { page: 'home', overlay: 'currency' },
	st6: { page: 'home', overlay: 'number-format' },
	st7: { page: 'home', overlay: 'date-format' },
	st8: { page: 'home', overlay: 'time-format' },
	st9: { page: 'networks', overlay: 'none' },
	st9b: { page: 'network-detail', overlay: 'none' },
	st10: { page: 'add-network', overlay: 'none' },
	st10b: { page: 'add-network', overlay: 'none' },
	st10c: { page: 'add-network', overlay: 'none' },
	st11: { page: 'rpc-providers', overlay: 'none' },
	st12: { page: 'endpoints', overlay: 'none' },
	st13: { page: 'storage', overlay: 'none' },
	st13b: { page: 'storage', overlay: 'clear-caches', backdrop: 'storage' },
	st14: { page: 'about', overlay: 'none' },
	st15: { page: 'home', overlay: 'feedback' },
	st16: { page: 'home', overlay: 'erase-device' },
	sr1: { page: 'home', overlay: 'none', tab: 'wallet' },
	sr2: { page: 'home', overlay: 'rpc-fix', tab: 'wallet', backdrop: 'wallet' },
	sr2b: { page: 'home', overlay: 'rpc-fix', tab: 'wallet', backdrop: 'wallet' },
	sr3: { page: 'home', overlay: 'balance-detail', tab: 'wallet', backdrop: 'wallet' },
	sr4: { page: 'home', overlay: 'relayer', tab: 'wallet', backdrop: 'send' },
	sr5: { page: 'home', overlay: 'none', tab: 'wallet' }
};

export function buildMobileState(
	state: MobileSettingsStateId,
	m: SettingsMessages,
	identicon: (seed: string) => string
): SettingsHomeModel {
	const shape = MOBILE_SHAPE[state];
	const addMode = state === 'st10b' ? 'compatible' : state === 'st10c' ? 'incompatible' : 'search';
	const backdropTitle =
		shape.backdrop === 'wallet'
			? m.walletTitle
			: shape.backdrop === 'send'
				? m.sendTitle
				: shape.backdrop === 'storage'
					? m.storage.title
					: m.title;

	return {
		state,
		title: m.title,
		page: shape.page,
		overlay: shape.overlay,
		tab: shape.tab ?? 'settings',
		tabs: m.nav,
		account: {
			name: ACCOUNT_NAME,
			addressDisplay: ADDRESS_DISPLAY,
			addressFull: ADDRESS_FULL,
			identiconSvg: identicon(ADDRESS_FULL),
			action: m.account.switch
		},
		sections: sections(m, state === 'st1b'),
		appearance: {
			theme: {
				label: m.appearance.themeTitle,
				selected: 'dark',
				segments: [
					{ id: 'light', label: m.appearance.themeLight, icon: 'sun' },
					{ id: 'dark', label: m.appearance.themeDark, icon: 'moon' },
					{ id: 'auto', label: m.appearance.themeAuto, icon: 'monitor' }
				]
			},
			avatar: {
				label: m.appearance.avatarTitle,
				selected: 'identicon',
				segments: [
					{ id: 'initials', label: m.appearance.avatarInitials },
					{ id: 'identicon', label: m.appearance.avatarIdenticon }
				]
			},
			// Six stops, standard in the third — `src/constants/text-scale.ts`, which
			// the boards had rounded to seven.
			textScale: { label: m.appearance.textScale, steps: 6, index: 2 }
		},
		signOut: { label: m.signOut.button },
		erase: { title: m.erase.title, subtitle: m.erase.subtitle },
		networks: {
			title: m.advanced.networksTitle,
			subtitle: m.advanced.networksSubtitle,
			rows: networkRows(m),
			addLabel: m.advanced.addNetworkTitle,
			removeLabel: m.networks.remove,
			removeSheet: removeNetworkSheet(m)
		},
		networkDetail: networkDetail(m, state === 'st9b'),
		addNetwork: addNetwork(m, addMode),
		rpcProviders: rpcProviders(m, false),
		endpoints: endpoints(m, false),
		storage: storage(m),
		about: about(m, false),
		accountsSheet: accountsSheet(m, identicon),
		signOutSheet: signOutSheet(m, state === 'st3b'),
		languageSheet: languageSheet(m, 'zh'),
		currencySheet: currencySheet(m),
		feeSpeedSheet: feeSpeedSheet(m),
		signWithSheet: signWithSheet(m),
		signerPage: signerPage(m),
		numberSheet: formatSheet(
			m,
			m.localization.numberTitle,
			m.localization.numberSubtitle,
			NUMBER_SAMPLES,
			[undefined, undefined, undefined, undefined, m.formatNote.indian]
		),
		dateSheet: formatSheet(
			m,
			m.localization.dateTitle,
			m.localization.dateSubtitle,
			DATE_SAMPLES,
			[]
		),
		timeSheet: formatSheet(m, m.localization.timeTitle, m.localization.timeSubtitle, TIME_SAMPLES, [
			undefined,
			m.formatNote.h24,
			m.formatNote.h12
		]),
		clearCachesSheet: clearCachesSheet(m),
		eraseSheet: eraseSheet(m),
		feedback: feedback(m),
		rpcBanner: state === 'sr1' ? rpcBanner(m) : undefined,
		rpcFix: rpcFix(m, state === 'sr2b'),
		balanceDetail: balanceDetail(m),
		relayer: relayer(m),
		indexDown: indexDown(m),
		backdropTitle,
		closeLabel: m.common.close
	};
}

const DESKTOP_PAGE: Record<DesktopSettingsStateId, SettingsPageId> = {
	dst1: 'account',
	dst2: 'appearance',
	dst3: 'localization',
	dst4: 'networks',
	dst4b: 'networks',
	dst5: 'rpc-providers',
	dst6: 'endpoints',
	dst7: 'storage',
	dst8: 'about',
	dsr1: 'account'
};

export function buildDesktopState(
	state: DesktopSettingsStateId,
	m: SettingsMessages,
	identicon: (seed: string) => string
): SettingsDesktopModel {
	const nav: SettingsNavItemModel[] = [
		{ id: 'account', icon: 'users-round', label: m.sections.account },
		{ id: 'appearance', icon: 'sun', label: m.sections.appearance },
		{ id: 'localization', icon: 'coins', label: m.sections.localization },
		{ id: 'networks', icon: 'network', label: m.advanced.networksTitle },
		{ id: 'rpc-providers', icon: 'server', label: m.advanced.rpcProvidersTitle },
		{ id: 'endpoints', icon: 'zap', label: m.advanced.endpointsTitle },
		// Spec 068, in the phone's own order: the 高级 section puts 交易速度
		// between 服务端点 and 存储, and this list mirrors that list.
		{ id: 'fee-speed', icon: 'clock', label: m.advanced.feeSpeedTitle },
		// Spec 071, beside the speed as on the phone.
		{ id: 'signing', icon: 'pencil', label: m.signing.title },
		{ id: 'storage', icon: 'hard-drive', label: m.storage.title },
		{ id: 'about', icon: 'info', label: m.about.title }
	];
	const accounts = accountsSheet(m, identicon);

	return {
		state,
		title: m.title,
		page: DESKTOP_PAGE[state],
		overlay: state === 'dst4b' ? 'add-network' : state === 'dsr1' ? 'rpc-fix' : 'none',
		nav,
		closeLabel: m.common.close,
		account: {
			title: m.sections.account,
			summary: accounts.summary,
			rows: accounts.rows,
			primary: accounts.primary,
			secondary: accounts.secondary,
			signOutLabel: m.signOut.button,
			signOutNote: m.signOut.desc,
			erase: { title: m.erase.title, subtitle: m.erase.subtitle, action: m.erase.confirm }
		},
		appearance: {
			title: m.sections.appearance,
			language: {
				id: 'language',
				label: m.language.title,
				kind: 'dropdown',
				value: `简体中文 · ${m.common.system}`
			},
			textScale: {
				id: 'text-scale',
				label: m.appearance.textScale,
				kind: 'slider',
				scale: { label: m.appearance.textScale, steps: 6, index: 2 }
			},
			theme: {
				id: 'theme',
				label: m.appearance.themeTitle,
				kind: 'segmented',
				segmented: {
					label: m.appearance.themeTitle,
					selected: 'dark',
					segments: [
						{ id: 'light', label: m.appearance.themeLight, icon: 'sun' },
						{ id: 'dark', label: m.appearance.themeDark, icon: 'moon' },
						{ id: 'auto', label: m.appearance.themeAuto, icon: 'monitor' }
					]
				}
			},
			avatar: {
				id: 'avatar',
				label: m.appearance.avatarTitle,
				kind: 'segmented',
				segmented: {
					label: m.appearance.avatarTitle,
					selected: 'identicon',
					segments: [
						{ id: 'initials', label: m.appearance.avatarInitials },
						{ id: 'identicon', label: m.appearance.avatarIdenticon }
					]
				}
			}
		},
		localization: {
			title: m.sections.localization,
			description: m.localization.numberSubtitle,
			rows: [
				{
					id: 'currency',
					label: m.localization.currencyTitle,
					kind: 'dropdown',
					value: 'USD · $1,234.56'
				},
				{
					id: 'number-format',
					label: m.localization.numberTitle,
					kind: 'dropdown',
					value: NUMBER_SAMPLES[0]
				},
				{
					id: 'date-format',
					label: m.localization.dateTitle,
					kind: 'dropdown',
					value: DATE_SAMPLES[0]
				},
				{
					id: 'time-format',
					label: m.localization.timeTitle,
					kind: 'dropdown',
					value: TIME_SAMPLES[0]
				}
			]
		},
		feeSpeed: {
			title: m.feeSpeed.title,
			description: m.feeSpeed.subtitle,
			rows: [
				{
					id: 'fee-speed',
					label: m.advanced.feeSpeedTitle,
					kind: 'dropdown',
					value: m.feeSpeed.fast
				}
			]
		},
		signing: {
			title: m.signing.title,
			description: m.signing.subtitle,
			rows: [
				{
					id: 'sign-with',
					label: m.signing.title,
					kind: 'dropdown',
					value: m.signing.methods.auto
				}
			],
			page: signerPage(m)
		},
		networks: {
			title: m.advanced.networksTitle,
			subtitle: m.advanced.networksSubtitle,
			addLabel: m.advanced.addNetworkTitle,
			removeLabel: m.networks.remove,
			removeSheet: removeNetworkSheet(m),
			// DST4 expands Ethereum in place and drops the built-ins below Base
			// into the custom tail, which is what the mock shows.
			rows: networkRows(m, 'ethereum').filter((r) => !['gnosis', 'tempo'].includes(r.id)),
			detail: networkDetail(m, false)
		},
		rpcProviders: rpcProviders(m, true),
		endpoints: endpoints(m, true),
		storage: storage(m),
		clearCachesSheet: clearCachesSheet(m),
		eraseSheet: eraseSheet(m),
		about: about(m, true),
		addNetwork: addNetwork(m, 'compatible'),
		rpcFix: rpcFix(m, false),
		dropdown:
			state === 'dst3'
				? {
						rowId: 'number-format',
						rows: formatSheet(m, m.localization.numberTitle, undefined, NUMBER_SAMPLES, [
							undefined,
							undefined,
							undefined,
							undefined,
							m.formatNote.indian
						]).rows
					}
				: undefined,
		rpcBanner: state === 'dsr1' ? rpcBanner(m) : undefined,
		backdropTitle: state === 'dsr1' ? m.walletTitle : m.title
	};
}
