/**
 * Canonical wallet-flow fixtures (spec 021 — the single canon all four
 * platforms port). Content is verbatim from `docs/design/wallet-2/`; builders merge
 * it with resolved messages into display-ready view models.
 *
 * Pure data plus assembly. Nothing here fetches, signs, formats a number or
 * decides a business rule — spec 021 is the UI, and the later "real data"
 * feature replaces this file and nothing else.
 *
 * Where a mock invented content that the product already has a canon for, the
 * canon wins: the contact picker uses spec 018's roster, and every address is
 * spec 015's or spec 018's, so identicon artwork matches across features and
 * across clients.
 */
import { CHAIN_COLORS, IDENTITY, NETWORK_COUNT } from '$lib/wallet/fixtures';
import { CONTACTS, GROUPS } from '$lib/contacts/fixtures';
import { fill } from '$lib/wallet/messages';
import type { WalletFlowMessages } from './messages';
import type {
	ActivityGroupModel,
	AddTokenModel,
	AssetRowModel,
	AssetsModel,
	BatchImportModel,
	ContactPickModel,
	DesktopFlowModel,
	DesktopFlowStateId,
	FactRowModel,
	FeeTokenPickModel,
	FlowScreenModel,
	FlowStateId,
	HistoryModel,
	NetworkRowModel,
	ReceiveListModel,
	ReceiveQrModel,
	ScanModel,
	SendConfirmModel,
	SendFormModel,
	SendPickModel,
	SendReceiptModel,
	ShareCardModel,
	TokenDetailModel,
	TxDetailModel
} from './model';

type Identicon = (seed: string) => string;

// --- Canon ----------------------------------------------------------------

/** The eight supported networks, in the order R1 lists them. */
export const NETWORKS = [
	{ name: 'Ethereum', code: 'ETH', color: CHAIN_COLORS.ethereum, chainId: '1' },
	{ name: 'BNB Chain', code: 'BNB', color: CHAIN_COLORS.bnb, chainId: '56' },
	{ name: 'Polygon', code: 'POL', color: CHAIN_COLORS.polygon, chainId: '137' },
	{ name: 'Arbitrum', code: 'ARB', color: CHAIN_COLORS.arbitrum, chainId: '42161' },
	{ name: 'Optimism', code: 'OP', color: CHAIN_COLORS.optimism, chainId: '10' },
	{ name: 'Base', code: 'BASE', color: CHAIN_COLORS.base, chainId: '8453' },
	{ name: 'Avalanche', code: 'AVAX', color: CHAIN_COLORS.avalanche, chainId: '43114' },
	{ name: 'Gnosis', code: 'GNO', color: CHAIN_COLORS.gnosis, chainId: '100' }
] as const;

/** USDT on Ethereum — the real contract, as the mocks print it. */
export const USDT_CONTRACT = '0xdAC17F958D2ee523a2206206994597C13D831ec7';
export const USDT_CONTRACT_SHORT = '0xdAC1…1ec7';

/** The counterparty every send mock addresses. Spec 018's Alice. */
const ALICE = CONTACTS[0];

const TX_HASH_RECEIVED = '0x8f3a…c21d';
const TX_HASH_SENT = '0x3c2d…8e1f';

/**
 * Split a 0x address into the two lines the mocks wrap it into.
 * 42 characters, so 21 and 21 — an even break rather than one that leaves a
 * stub on the second line.
 */
export function addressLines(address: string): [string, string] {
	const half = Math.ceil(address.length / 2);
	return [address.slice(0, half), address.slice(half)];
}

export const MOBILE_FLOW_STATES: FlowStateId[] = [
	'r1',
	'r2',
	'r2x',
	'r3',
	'r4',
	's1',
	'a1',
	'a2',
	'a3',
	't1',
	't2',
	't3',
	't3b',
	't4',
	't5',
	't5b',
	'sd1',
	'sd1b',
	'sd2',
	'sd2b',
	'sd2c',
	'sd2d',
	'sd2e',
	'sd2f',
	'sd3',
	'sd3b',
	'sd3c',
	'sd4a',
	'sd4b',
	'sd4c'
];

export const DESKTOP_FLOW_STATES: DesktopFlowStateId[] = [
	'dr1',
	'dr2',
	'dr3',
	'ds1',
	'da1',
	'da2',
	'da3',
	'dt1',
	'dt3',
	'dt3b',
	'dt4',
	'dsd1',
	'dsd2',
	'dsd2b',
	'dsd3',
	'dsd4',
	'dsd2c',
	'dsd2e',
	'dsd2f'
];

/** The assets T1 lists, verbatim from the mock. */
const ASSETS: { ticker: string; chain: string; color: string; balance: string; fiat: string }[] = [
	{
		ticker: 'BNB',
		chain: 'BNB Chain',
		color: CHAIN_COLORS.bnb,
		balance: '0.8533',
		fiat: '$496.46'
	},
	{
		ticker: 'ETH',
		chain: 'Arbitrum',
		color: CHAIN_COLORS.arbitrum,
		balance: '0.2253',
		fiat: '$422.62'
	},
	{
		ticker: 'ETH',
		chain: 'Ethereum',
		color: CHAIN_COLORS.ethereum,
		balance: '0.0689',
		fiat: '$129.25'
	},
	{
		ticker: 'XDAI',
		chain: 'Gnosis',
		color: CHAIN_COLORS.gnosis,
		balance: '74.3965',
		fiat: '$74.38'
	},
	{
		ticker: 'USDT',
		chain: 'Ethereum',
		color: CHAIN_COLORS.ethereum,
		balance: '53.4836',
		fiat: '$53.48'
	},
	{
		ticker: 'USDC',
		chain: 'Polygon',
		color: CHAIN_COLORS.polygon,
		balance: '12.04',
		fiat: '$12.04'
	}
];

/** SD1's order differs from T1's: the send picker leads with what you'd send. */
const SEND_ASSETS = [
	ASSETS[4],
	ASSETS[2],
	{
		...ASSETS[5],
		chain: 'Ethereum',
		color: CHAIN_COLORS.ethereum,
		balance: '18.20',
		fiat: '$18.20'
	},
	ASSETS[0],
	ASSETS[3]
];

function assetRow(a: (typeof ASSETS)[number]): AssetRowModel {
	return {
		ticker: a.ticker,
		chain: a.chain,
		badgeColor: a.color,
		balance: a.balance,
		fiat: { kind: 'value', text: a.fiat },
		masked: false
	};
}

// --- Assembly -------------------------------------------------------------

function networkRows(m: WalletFlowMessages): NetworkRowModel[] {
	return NETWORKS.map((n) => ({
		name: n.name,
		code: n.code,
		badgeColor: n.color,
		addressDisplay: IDENTITY.addressDisplay,
		copyLabel: m['componentsUi.identiconViewer.copyAddress'],
		qrLabel: m['componentsUi.scanner.title']
	}));
}

function receiveList(m: WalletFlowMessages): ReceiveListModel {
	return {
		header: {
			title: m['receive.title'],
			backLabel: m['receive.a11yBack']
		},
		subtitle: fill(m['receive.networksLine'], { count: NETWORK_COUNT }),
		searchPlaceholder: m['receive.searchNetworkPlaceholder'],
		emptyText: m['receive.searchNetworkEmpty'],
		rows: networkRows(m)
	};
}

function receiveQr(m: WalletFlowMessages, identicon: Identicon, asset: boolean): ReceiveQrModel {
	const network = NETWORKS[0];
	return {
		title: asset
			? fill(m['receive.qrTitleAsset'], { network: network.name, symbol: 'USDT' })
			: fill(m['receive.qrTitleNetwork'], { network: network.name }),
		closeLabel: m['componentsUi.identiconViewer.close'],
		contract: asset
			? {
					label: m['receive.tokenContract'],
					value: USDT_CONTRACT_SHORT,
					copyLabel: m['componentsUi.identiconViewer.copyAddress']
				}
			: undefined,
		account: {
			name: IDENTITY.name,
			identiconSvg: identicon(IDENTITY.addressFull),
			lines: addressLines(IDENTITY.addressFull),
			copyLabel: m['componentsUi.identiconViewer.copyAddress']
		},
		centre: asset
			? { ticker: 'USDT', badgeColor: CHAIN_COLORS.gnosis }
			: { ticker: network.code, badgeColor: network.color },
		warning: m['receive.warningReminder'],
		saveImage: m['receive.request.saveImage'],
		viewOnExplorer: m['history.viewOnExplorer']
	};
}

function shareCard(m: WalletFlowMessages, identicon: Identicon): ShareCardModel {
	const network = NETWORKS[0];
	return {
		headline: m['receive.shareCardHeadline'],
		name: IDENTITY.name,
		lines: addressLines(IDENTITY.addressFull),
		networkNote: fill(m['receive.shareCardNetworkNote'], { network: network.name }),
		networkMark: { ticker: network.code, badgeColor: network.color },
		identiconSvg: identicon(IDENTITY.addressFull),
		wordmark: 'Vela Wallet'
	};
}

function scan(m: WalletFlowMessages, desktop: boolean): ScanModel {
	return {
		title: m['componentsUi.scanner.title'],
		hint: m['componentsUi.scanner.hint'],
		closeLabel: m['componentsUi.identiconViewer.close'],
		// A desktop webcam has no torch, so the modal offers two tools where
		// the phone offers three.
		tools: desktop
			? [
					{ id: 'gallery', label: m['componentsUi.scanner.fromGallery'] },
					{ id: 'flip', label: m['componentsUi.scanner.flipCamera'] }
				]
			: [
					{ id: 'gallery', label: m['componentsUi.scanner.gallery'] },
					{ id: 'torch', label: m['componentsUi.scanner.torch'] },
					{ id: 'flip', label: m['componentsUi.scanner.flipCamera'] }
				]
	};
}

function historyGroups(m: WalletFlowMessages): ActivityGroupModel[] {
	const sent = m['history.labelSent'];
	const received = m['history.labelReceived'];
	const to = (name: string, clock: string) => `${fill(m['history.toName'], { name })} · ${clock}`;
	const from = (name: string, clock: string) =>
		`${fill(m['history.fromName'], { name })} · ${clock}`;

	return [
		{
			label: m['componentsUi.dayGroup.today'],
			rows: [
				{
					kind: 'sent',
					title: sent,
					subtitle: to('hold on', '14:02'),
					amount: '−2',
					unit: 'POL',
					positive: false,
					masked: false,
					badgeColor: CHAIN_COLORS.polygon
				},
				{
					kind: 'received',
					title: received,
					subtitle: from(ALICE.addressDisplay, '11:20'),
					amount: '+120',
					unit: 'USDT',
					positive: true,
					masked: false,
					badgeColor: CHAIN_COLORS.ethereum
				}
			]
		},
		{
			label: m['componentsUi.dayGroup.yesterday'],
			rows: [
				{
					kind: 'received',
					title: received,
					subtitle: from('Alice', '20:15'),
					amount: '+50',
					unit: 'USDC',
					positive: true,
					masked: false,
					badgeColor: CHAIN_COLORS.base
				},
				{
					kind: 'sent',
					title: sent,
					subtitle: to('Bob', '09:12'),
					amount: '−0.4',
					unit: 'XDAI',
					positive: false,
					masked: false,
					badgeColor: CHAIN_COLORS.gnosis
				}
			]
		},
		{
			// A literal date once the run of named days ends — the mock's 8月12日.
			label: '8/12',
			rows: [
				{
					kind: 'received',
					title: received,
					subtitle: from('0x21aE…9F3c', '08:44'),
					amount: '+0.9',
					unit: 'BNB',
					positive: true,
					masked: false,
					badgeColor: CHAIN_COLORS.bnb
				}
			]
		}
	];
}

function history(m: WalletFlowMessages): HistoryModel {
	return {
		header: {
			title: m['history.navTitle'],
			backLabel: m['receive.a11yBack'],
			pill: {
				dots: [CHAIN_COLORS.ethereum, CHAIN_COLORS.polygon, CHAIN_COLORS.bnb],
				label: m['componentsUi.networkFilter.pillAll']
			}
		},
		mode: 'rows',
		emptyText: m['history.emptyFilter'],
		groups: historyGroups(m)
	};
}

function txDetail(
	m: WalletFlowMessages,
	identicon: Identicon,
	kind: 'received' | 'sent'
): TxDetailModel {
	const network = kind === 'received' ? NETWORKS[0] : NETWORKS[2];
	const facts: FactRowModel[] = [
		{
			label: kind === 'received' ? m['componentsTx.detail.from'] : m['componentsTx.detail.to'],
			value: kind === 'received' ? ALICE.addressDisplay : 'hold on',
			lead: {
				kind: 'identicon',
				svg: identicon(kind === 'received' ? ALICE.addressFull : CONTACTS[6].addressFull),
				address: kind === 'received' ? ALICE.addressFull : CONTACTS[6].addressFull
			},
			mono: kind === 'received',
			copy: m['componentsUi.identiconViewer.copyAddress']
		},
		{
			label: m['componentsTx.detail.labelChain'],
			value: network.name,
			lead: { kind: 'token', mark: { ticker: network.code, badgeColor: network.color } }
		}
	];

	// Only an ERC-20 transfer has a contract. A3's native coin does not, and
	// printing an empty row there would invite the question "which contract?".
	if (kind === 'received') {
		facts.push({
			// `receive.tokenContract` (代币合约), NOT `tokenDetail.labelContract`
			// (合约). The corpus carries both and the mocks use both: the token
			// sheet is already about a token, so it says "contract"; a
			// transaction row has to say WHICH contract.
			label: m['receive.tokenContract'],
			value: USDT_CONTRACT_SHORT,
			mono: true,
			copy: m['componentsUi.identiconViewer.copyAddress']
		});
	}

	facts.push(
		{
			label: m['componentsTx.detail.labelDate'],
			value: `${m['componentsUi.dayGroup.today']} ${kind === 'received' ? '11:20' : '14:02'}`
		},
		{
			label: m['componentsTx.detail.labelHash'],
			value: kind === 'received' ? TX_HASH_RECEIVED : TX_HASH_SENT,
			mono: true,
			copy: m['componentsUi.identiconViewer.copyAddress']
		}
	);

	return {
		title:
			kind === 'received'
				? fill(m['history.txLabelReceived'], { symbol: 'USDT' })
				: fill(m['history.txLabelSent'], { symbol: 'POL' }),
		status: { text: m['componentsTx.receipt.statusConfirmed'], tone: 'success' },
		closeLabel: m['componentsUi.identiconViewer.close'],
		amount: kind === 'received' ? '+120 USDT' : '−2 POL',
		fiat: kind === 'received' ? '≈ $120.00' : '≈ $0.98',
		positive: kind === 'received',
		facts,
		viewOnExplorer: m['history.viewOnExplorer']
	};
}

function assets(m: WalletFlowMessages, empty: boolean): AssetsModel {
	return {
		header: {
			title: m['assets.sectionTitle'],
			backLabel: m['receive.a11yBack'],
			action: m['assets.addToken'],
			pill: {
				dots: [CHAIN_COLORS.ethereum, CHAIN_COLORS.polygon, CHAIN_COLORS.bnb],
				label: m['componentsUi.networkFilter.pillAll']
			}
		},
		searchPlaceholder: m['assets.searchPlaceholder'],
		rows: empty ? [] : ASSETS.map(assetRow),
		addByAddress: m['assets.addByAddress'],
		empty: empty
			? {
					title: m['assets.emptyTitle'],
					caption: m['assets.emptySubtext'],
					cta: m['addToken.navTitle'],
					hintTitle: m['assets.notShowingTitle'],
					hintBody: m['assets.notShowingBody']
				}
			: undefined
	};
}

function tokenDetail(m: WalletFlowMessages): TokenDetailModel {
	return {
		mark: { ticker: 'USDT', badgeColor: CHAIN_COLORS.ethereum },
		symbol: 'USDT',
		chain: 'Ethereum',
		closeLabel: m['componentsUi.identiconViewer.close'],
		balance: '53.4836 USDT',
		fiat: '$53.48',
		receive: m['tokenDetail.receive'],
		send: m['tokenDetail.send'],
		facts: [
			{
				label: m['tokenDetail.labelPrice'],
				value: fill(m['tokenDetail.priceValue'], { symbol: 'USDT', value: '$1.00' })
			},
			{
				label: m['tokenDetail.labelContract'],
				value: USDT_CONTRACT_SHORT,
				mono: true,
				copy: m['componentsUi.identiconViewer.copyAddress']
			},
			{ label: m['tokenDetail.labelDecimals'], value: '6' },
			{ label: m['addToken.labelNetwork'], value: 'Ethereum' }
		],
		transactionsTitle: m['tokenDetail.labelTransactions'],
		rows: [
			{
				kind: 'received',
				title: m['history.labelReceived'],
				subtitle: `${fill(m['history.fromName'], { name: ALICE.addressDisplay })} · ${m['componentsUi.dayGroup.today']}`,
				amount: '+120',
				unit: 'USDT',
				positive: true,
				masked: false,
				badgeColor: CHAIN_COLORS.polygon
			},
			{
				kind: 'sent',
				title: m['history.labelSent'],
				subtitle: `${fill(m['history.toName'], { name: 'Alice' })} · 8/10`,
				amount: '−30',
				unit: 'USDT',
				positive: false,
				masked: false,
				badgeColor: CHAIN_COLORS.polygon
			}
		],
		viewOnExplorer: m['tokenDetail.viewOnExplorer']
	};
}

/** T3 / T3b and their T5 / T5b failure variants. */
type AddTokenVariant =
	| 'erc20'
	| 'native'
	| 'erc20-invalid'
	| 'erc20-not-found'
	| 'erc20-added'
	| 'native-not-found'
	| 'native-incompatible'
	| 'native-added';

function addToken(m: WalletFlowMessages, variant: AddTokenVariant): AddTokenModel {
	const tabs = { erc20: m['addToken.tabErc20'], native: m['addToken.tabNative'] };
	const native = variant.startsWith('native');
	const avax = NETWORKS[6];

	const base = {
		title: m['addToken.navTitle'],
		closeLabel: m['componentsUi.identiconViewer.close'],
		tab: (native ? 'native' : 'erc20') as 'native' | 'erc20',
		tabs
	};

	if (native) {
		const found =
			variant === 'native-incompatible' || variant === 'native-added' || variant === 'native';
		return {
			...base,
			fieldLabel: m['addToken.netSearchLabel'],
			fieldValue: variant === 'native-not-found' ? 'fantom' : found ? 'Avalanche' : '',
			fieldPlaceholder: m['addToken.netSearchPlaceholder'],
			result:
				variant === 'native-not-found'
					? { kind: 'not-found', text: fill(m['addToken.netPickerEmpty'], { query: 'fantom' }) }
					: {
							kind: 'network',
							mark: { ticker: avax.code, badgeColor: avax.color },
							name: avax.name,
							chip:
								variant === 'native-incompatible'
									? { text: m['addToken.notCompatible'], tone: 'error' }
									: variant === 'native-added'
										? { text: m['addToken.networkAdded'], tone: 'success' }
										: { text: m['addToken.compatible'], tone: 'success' },
							link:
								variant === 'native-incompatible'
									? `${m['addToken.errorNotCompatible']} · ${m['addToken.deployContracts']}`
									: undefined,
							facts: [
								{ label: m['addToken.labelChainId'], value: avax.chainId },
								{ label: m['addToken.labelNativeToken'], value: avax.code }
							]
						},
			cta: m['addToken.addNetworkBtn'],
			ctaDisabled: variant !== 'native'
		};
	}

	return {
		...base,
		network: {
			mark: { ticker: NETWORKS[0].code, badgeColor: NETWORKS[0].color },
			name: NETWORKS[0].name,
			pickLabel: m['addToken.netPickerSearchPlaceholder']
		},
		fieldLabel: m['addToken.tokenAddressLabel'],
		fieldValue:
			variant === 'erc20-invalid'
				? USDT_CONTRACT.slice(0, -4)
				: variant === 'erc20-not-found'
					? '0x1234…abcd'
					: USDT_CONTRACT,
		fieldPlaceholder: USDT_CONTRACT,
		fieldError: variant === 'erc20-invalid' ? m['addToken.invalidAddress'] : undefined,
		result:
			variant === 'erc20-invalid'
				? { kind: 'none' }
				: variant === 'erc20-not-found'
					? {
							kind: 'not-found',
							text: `${m['addToken.notFoundTitle']} — ${m['addToken.notFoundMessage']}`
						}
					: {
							kind: 'token',
							mark: { ticker: 'USDT', badgeColor: CHAIN_COLORS.ethereum },
							name: 'Tether USD',
							detail: `USDT · ${m['tokenDetail.labelDecimals']} 6 · Ethereum`,
							chip:
								variant === 'erc20-added'
									? { text: m['addToken.tokenAdded'], tone: 'success' }
									: undefined
						},
		cta: m['addToken.addToWalletBtn'],
		ctaDisabled: variant !== 'erc20'
	};
}

function sendPick(m: WalletFlowMessages, multi: boolean): SendPickModel {
	const rows = SEND_ASSETS.map(assetRow);
	const filters = [
		{ id: 'all', label: m['history.filterAll'], selected: true },
		{ id: 'stable', label: m['send.filterStable'], selected: false },
		{ id: 'gas', label: m['send.filterGas'], selected: false },
		{ id: 'other', label: m['send.filterOther'], selected: false }
	];

	return {
		header: {
			title: multi ? m['send.multiSendTitle'] : m['send.selectTokenTitle'],
			backLabel: m['receive.a11yBack'],
			pill: {
				dots: [CHAIN_COLORS.ethereum, CHAIN_COLORS.polygon, CHAIN_COLORS.bnb],
				label: m['componentsUi.networkFilter.pillAll']
			}
		},
		searchPlaceholder: m['send.searchPlaceholder'],
		filters,
		notice: multi
			? {
					mark: { ticker: NETWORKS[0].code, badgeColor: NETWORKS[0].color },
					text: fill(m['send.multiSendChainNotice'], { network: NETWORKS[0].name })
				}
			: undefined,
		rows,
		selection: multi
			? {
					// The first three rows are on Ethereum; the last two are not,
					// which is exactly what the greying is there to explain.
					selected: [true, true, true, false, false],
					dimmed: [false, false, false, true, true],
					selectAll: m['send.selectAllValuable']
				}
			: undefined,
		cta: multi
			? {
					label: fill(m['send.multiSendContinue'], { n: 3, chain: NETWORKS[0].name }),
					accent: true
				}
			: { label: m['send.multiSendTitle'], accent: false }
	};
}

function sendForm(
	m: WalletFlowMessages,
	identicon: Identicon,
	mode: 'single' | 'split' | 'sweep'
): SendFormModel {
	const fee = {
		label: m['componentsUi.gas.networkFee'],
		mark: { ticker: 'ETH', badgeColor: CHAIN_COLORS.ethereum },
		value:
			mode === 'single'
				? '0.0021 ETH · ≈$0.55'
				: mode === 'split'
					? '0.0034 ETH · ≈$0.89'
					: '0.0041 ETH · ≈$1.07',
		openLabel: m['send.feeTokenLabel']
	};

	const header = {
		title:
			mode === 'sweep' ? m['send.multiSendTitle'] : fill(m['send.sendTitle'], { symbol: 'USDT' }),
		backLabel: m['receive.a11yBack']
	};

	if (mode === 'sweep') {
		return {
			header,
			mode,
			sweepSummary: fill(m['send.multiSendSummary'], { n: 3, chain: NETWORKS[0].name }),
			sweepRows: [
				{
					mark: { ticker: 'USDT', badgeColor: CHAIN_COLORS.ethereum },
					symbol: 'USDT',
					balanceLabel: fill(m['send.balanceLabel'], { amount: '53.4836' }),
					amount: '53.4836',
					max: m['send.maxBtn']
				},
				{
					mark: { ticker: 'ETH', badgeColor: CHAIN_COLORS.ethereum },
					symbol: 'ETH',
					balanceLabel: fill(m['send.balanceLabel'], { amount: '0.0689' }),
					amount: '0.05',
					max: m['send.maxBtn']
				},
				{
					mark: { ticker: 'USDC', badgeColor: CHAIN_COLORS.ethereum },
					symbol: 'USDC',
					balanceLabel: fill(m['send.balanceLabel'], { amount: '18.20' }),
					amount: '18.20',
					max: m['send.maxBtn']
				}
			],
			recipient: {
				label: m['send.recipientLabel'],
				lines: [ALICE.addressDisplay, ''],
				address: ALICE.addressFull,
				identiconSvg: identicon(ALICE.addressFull),
				pickLabel: m['send.recipientPickAria'],
				scanLabel: m['send.scanAria'],
				note: m['send.multiSendSameRecipient']
			},
			fee,
			cta: m['send.continueBtn']
		};
	}

	const token = {
		mark: { ticker: 'USDT', badgeColor: CHAIN_COLORS.ethereum },
		symbol: 'USDT',
		detail: `Ethereum · ${fill(m['send.balanceLabel'], { amount: '53.4836' })}`,
		max: mode === 'single' ? m['send.maxBtn'] : undefined
	};

	if (mode === 'split') {
		return {
			header,
			mode,
			token,
			recipients: [
				{
					ordinal: fill(m['send.recipientN'], { n: 1 }),
					name: ALICE.addressDisplay,
					identiconSvg: identicon(ALICE.addressFull),
					address: ALICE.addressFull,
					amount: '50',
					removeLabel: m['send.removeRecipient']
				},
				{
					ordinal: fill(m['send.recipientN'], { n: 2 }),
					name: 'Alice',
					addressShort: CONTACTS[1].addressDisplay,
					identiconSvg: identicon(CONTACTS[1].addressFull),
					address: CONTACTS[1].addressFull,
					amount: '30',
					removeLabel: m['send.removeRecipient']
				},
				{
					ordinal: fill(m['send.recipientN'], { n: 3 }),
					name: 'hold on',
					addressShort: CONTACTS[6].addressDisplay,
					identiconSvg: identicon(CONTACTS[6].addressFull),
					address: CONTACTS[6].addressFull,
					amount: '40',
					removeLabel: m['send.removeRecipient']
				}
			],
			recipientActions: [
				{ id: 'add', label: m['send.addRecipient'] },
				{ id: 'contacts', label: m['send.fromContacts'] },
				{ id: 'import', label: m['send.batchImport'] }
			],
			summary: {
				label: `${m['send.splitTotalLabel']} · ${fill(m['send.recipientCount_other'], { count: 3 })}`,
				value: '120 USDT',
				detail: '≈ $120.00'
			},
			fee,
			cta: m['send.continueBtn']
		};
	}

	return {
		header,
		mode,
		token,
		amount: {
			value: '120',
			fiat: '≈ $120.00',
			denomLabel: m['send.feeTokenLabel'],
			// Drawn live: SD2's token is priced, which is the condition the core
			// shows the ⇄ row on.
			denomToggle: { enabled: true }
		},
		recipient: {
			label: m['send.recipientLabel'],
			lines: addressLines(ALICE.addressFull),
			address: ALICE.addressFull,
			identiconSvg: identicon(ALICE.addressFull),
			pickLabel: m['send.recipientPickAria']
		},
		addRecipient: m['send.addRecipient'],
		fee,
		cta: m['send.continueBtn']
	};
}

function contactPick(m: WalletFlowMessages, identicon: Identicon): ContactPickModel {
	return {
		title: m['send.pickContactTitle'],
		closeLabel: m['componentsUi.identiconViewer.close'],
		searchPlaceholder: m['send.pickContactSearch'],
		scanRow: m['send.scanToFill'],
		groupsTitle: m['contacts.sectionGroups'],
		groups: GROUPS.slice(0, 2).map((g, i) => ({
			name: g.name,
			count: fill(m['contacts.groupMembers'], { count: g.count }),
			colors: (i === 0
				? [CHAIN_COLORS.polygon, CHAIN_COLORS.bnb]
				: [CHAIN_COLORS.gnosis, CHAIN_COLORS.arbitrum]) as [string, string]
		})),
		contactsTitle: m['contacts.title'],
		contacts: CONTACTS.slice(0, 3).map((c) => ({
			name: c.name,
			group: c.groups[0],
			addressDisplay: c.addressDisplay,
			addressFull: c.addressFull,
			identiconSvg: identicon(c.addressFull)
		}))
	};
}

function feeTokenPick(m: WalletFlowMessages): FeeTokenPickModel {
	return {
		title: m['send.feeTokenLabel'],
		closeLabel: m['componentsUi.identiconViewer.close'],
		hint: m['send.feeTokenHint'],
		estimateLabel: m['send.feeTokenEstimate'],
		rows: [
			{
				mark: { ticker: 'ETH', badgeColor: CHAIN_COLORS.ethereum },
				symbol: 'ETH',
				balanceLabel: fill(m['send.balanceLabel'], { amount: '0.0689' }),
				fee: '~0.0021 ETH',
				selected: true
			},
			{
				mark: { ticker: 'USDC', badgeColor: CHAIN_COLORS.ethereum },
				symbol: 'USDC',
				balanceLabel: fill(m['send.balanceLabel'], { amount: '18.20' }),
				fee: '~0.55 USDC',
				selected: false
			},
			{
				mark: { ticker: 'USDT', badgeColor: CHAIN_COLORS.ethereum },
				symbol: 'USDT',
				balanceLabel: fill(m['send.balanceLabel'], { amount: '53.4836' }),
				fee: '~0.55 USDT',
				selected: false
			}
		]
	};
}

function batchImport(m: WalletFlowMessages, identicon: Identicon): BatchImportModel {
	const bob = CONTACTS[3];
	return {
		title: m['send.batchTitle'],
		closeLabel: m['componentsUi.identiconViewer.close'],
		unitCaption: m['send.batchUnitCaption'],
		units: {
			fiat: fill(m['send.batchUnitFiat'], { code: 'CNY' }),
			token: fill(m['send.batchUnitToken'], { sym: 'USDT' })
		},
		unit: 'fiat',
		pasteValue:
			'Alice, 0x9F3c…21aE, 5000\nBob, 0x44Aa…9C21, 8000\nAlice, 0x9F3c…21aE, 5000\nMallory, 0x12zz, 10',
		pastePlaceholder: m['send.batchPastePlaceholder'],
		tools: {
			file: { label: m['send.batchImportFile'], busy: false },
			template: { label: m['send.batchTemplate'], saved: false },
			formats: 'xlsx · csv · txt'
		},
		rate: {
			section: m['send.batchRateSection'],
			lead: '1 USDT',
			sign: '≈',
			value: '7.25',
			code: 'CNY',
			editable: false,
			edited: false,
			reset: m['send.batchRateReset'],
			hint: fill(m['send.batchRateHint'], { code: 'CNY', sym: 'USDT' }),
			hintTone: 'plain'
		},
		preview: {
			label: fill(m['send.batchParsedCount'], { n: 4 }),
			rows: [
				{
					kind: 'row',
					ok: true,
					name: ALICE.name,
					address: ALICE.addressDisplay,
					addressFull: ALICE.addressFull,
					identiconSvg: identicon(ALICE.addressFull),
					amount: '689.655172 USDT',
					source: '5,000 CNY'
				},
				{
					kind: 'row',
					ok: true,
					name: bob.name,
					address: bob.addressDisplay,
					addressFull: bob.addressFull,
					identiconSvg: identicon(bob.addressFull),
					amount: '1,103.448276 USDT',
					source: '8,000 CNY'
				},
				// The same person twice is what a sheet actually gets wrong: the
				// first line keeps the payment and this one is skipped, by name.
				{
					kind: 'row',
					ok: false,
					name: ALICE.name,
					address: ALICE.addressDisplay,
					addressFull: ALICE.addressFull,
					identiconSvg: identicon(ALICE.addressFull),
					amount: '—',
					note: m['send.batchDup']
				},
				// And a line that never became a row: the text to find it by in the
				// sheet, and the reason. These were only ever counted.
				{ kind: 'refused', text: 'Mallory , 0x12zz , 10', note: m['send.batchBadAddress'] }
			]
		},
		notices: [fill(m['send.batchRejected_other'], { count: 2 })],
		total: {
			label: `${m['send.splitTotalLabel']} · ${fill(m['send.recipientCount_other'], { count: 2 })}`,
			value: '1,793.103448 USDT',
			detail: '13,000 CNY',
			balance: fill(m['send.balanceLabel'], { amount: '53.4836 USDT' }),
			// The drawn account holds 53 USDT and the sheet asks for 1,793: the
			// picture is of the refusal, because that is the state with the most
			// to say and the one that used to say nothing.
			over: true,
			overText: fill(m['send.batchOverBalance'], { sym: 'USDT' })
		},
		// Two of three rows parsed, so the button offers two — never three.
		cta: fill(m['send.batchApply_other'], { count: 2 }),
		ctaDisabled: true
	};
}

function sendConfirm(
	m: WalletFlowMessages,
	identicon: Identicon,
	variant: 'single' | 'split' | 'sweep'
): SendConfirmModel {
	const facts: FactRowModel[] = [
		{
			label: m['send.fromLabel'],
			value: IDENTITY.name,
			lead: {
				kind: 'identicon',
				svg: identicon(IDENTITY.addressFull),
				address: IDENTITY.addressFull
			}
		},
		{
			label: m['send.toLabel'],
			value:
				variant === 'split'
					? fill(m['send.recipientCount_other'], { count: 3 })
					: ALICE.addressDisplay,
			lead:
				variant === 'split'
					? undefined
					: { kind: 'identicon', svg: identicon(ALICE.addressFull), address: ALICE.addressFull },
			mono: variant !== 'split'
		},
		{
			label: m['componentsTx.detail.labelChain'],
			value: NETWORKS[0].name,
			lead: { kind: 'token', mark: { ticker: NETWORKS[0].code, badgeColor: NETWORKS[0].color } }
		},
		{
			label: m['send.estFeeLabel'],
			value:
				variant === 'single'
					? '~0.0021 ETH · ≈$0.55'
					: variant === 'split'
						? '~0.0034 ETH · ≈$0.89'
						: '~0.0041 ETH · ≈$1.07'
		}
	];

	const header = { title: m['send.confirmTitle'], backLabel: m['receive.a11yBack'] };

	if (variant === 'sweep') {
		return {
			header,
			amount: fill(m['componentsTx.receipt.assetsCount'], { n: 3 }),
			subline: fill(m['send.confirmTotalLine'], { fiat: '$200.90', network: NETWORKS[0].name }),
			facts,
			breakdown: [
				{
					lead: { ticker: 'USDT', badgeColor: CHAIN_COLORS.ethereum },
					label: 'USDT',
					value: '53.4836 USDT · ≈$53.48'
				},
				{
					lead: { ticker: 'ETH', badgeColor: CHAIN_COLORS.ethereum },
					label: 'ETH',
					value: '0.05 ETH · ≈$93.79'
				},
				{
					lead: { ticker: 'USDC', badgeColor: CHAIN_COLORS.ethereum },
					label: 'USDC',
					value: '18.20 USDC · ≈$18.20'
				}
			],
			cta: m['send.confirmSendBtn']
		};
	}

	return {
		header,
		mark: { ticker: 'USDT', badgeColor: CHAIN_COLORS.ethereum },
		amount: '120 USDT',
		subline: '≈ $120.00',
		facts,
		breakdown:
			variant === 'split'
				? [
						{
							identiconSvg: identicon(ALICE.addressFull),
							address: ALICE.addressFull,
							label: ALICE.addressDisplay,
							mono: true,
							value: '50 USDT'
						},
						{
							identiconSvg: identicon(CONTACTS[1].addressFull),
							address: CONTACTS[1].addressFull,
							label: 'Alice',
							// On the page that signs, a name never stands alone.
							detail: CONTACTS[1].addressDisplay,
							value: '30 USDT'
						},
						{
							identiconSvg: identicon(CONTACTS[6].addressFull),
							address: CONTACTS[6].addressFull,
							label: 'hold on',
							detail: CONTACTS[6].addressDisplay,
							value: '40 USDT'
						}
					]
				: undefined,
		cta: m['send.confirmSendBtn']
	};
}

function sendReceipt(
	m: WalletFlowMessages,
	stage: 'submitting' | 'submitted' | 'confirmed'
): SendReceiptModel {
	const header = {
		title: fill(m['send.sendTitle'], { symbol: 'USDT' }),
		backLabel: m['receive.a11yBack']
	};

	if (stage === 'submitting') {
		return {
			header,
			stage,
			title: m['send.txSubmitting'],
			captions: [m['send.txPreparingBiometric'], m['send.txBackgroundHint']],
			cta: m['send.txCloseBackground'],
			ctaAccent: false
		};
	}

	if (stage === 'submitted') {
		return {
			header,
			stage,
			title: m['send.txSubmittedTitle'],
			captions: [
				m['send.txWaitingConfirm'],
				fill(m['send.txTypicalTime'], { chainName: NETWORKS[0].name, estSecs: 12 })
			],
			cta: m['send.txCloseBackground'],
			ctaAccent: false
		};
	}

	return {
		header,
		stage,
		title: fill(m['send.txConfirmedTitle'], { amount: '120', symbol: 'USDT' }),
		captions: [
			`${fill(m['history.toName'], { name: ALICE.addressDisplay })} · ${NETWORKS[0].name}`
		],
		hash: {
			label: m['componentsTx.receipt.txHash'],
			value: TX_HASH_RECEIVED,
			copyLabel: m['componentsUi.identiconViewer.copyAddress']
		},
		viewOnExplorer: m['history.viewOnExplorer'],
		cta: m['componentsTx.receipt.done'],
		ctaAccent: true
	};
}

// --- Builders -------------------------------------------------------------

/** Build one mobile state (spec.md's state matrix). */
export function buildFlowState(
	state: FlowStateId,
	m: WalletFlowMessages,
	identicon: Identicon
): FlowScreenModel {
	const scale = state === 'r2x' ? 1.35 : 1;

	switch (state) {
		case 'r1':
			return { state, base: { kind: 'receive-list', model: receiveList(m) }, textScale: scale };
		case 'r2':
		case 'r2x':
			return {
				state,
				base: { kind: 'receive-list', model: receiveList(m) },
				sheet: { kind: 'receive-qr', model: receiveQr(m, identicon, false) },
				textScale: scale
			};
		case 'r3':
			return {
				state,
				base: { kind: 'receive-list', model: receiveList(m) },
				sheet: { kind: 'receive-qr', model: receiveQr(m, identicon, true) },
				textScale: scale
			};
		case 'r4':
			return {
				state,
				base: { kind: 'share-card', model: shareCard(m, identicon) },
				textScale: scale
			};
		case 's1':
			return { state, base: { kind: 'scan', model: scan(m, false) }, textScale: scale };
		case 'a1':
			return { state, base: { kind: 'history', model: history(m) }, textScale: scale };
		case 'a2':
			return {
				state,
				base: { kind: 'history', model: history(m) },
				sheet: { kind: 'tx-detail', model: txDetail(m, identicon, 'received') },
				textScale: scale
			};
		case 'a3':
			return {
				state,
				base: { kind: 'history', model: history(m) },
				sheet: { kind: 'tx-detail', model: txDetail(m, identicon, 'sent') },
				textScale: scale
			};
		case 't1':
			return { state, base: { kind: 'assets', model: assets(m, false) }, textScale: scale };
		case 't2':
			return {
				state,
				base: { kind: 'assets', model: assets(m, false) },
				sheet: { kind: 'token-detail', model: tokenDetail(m) },
				textScale: scale
			};
		case 't3':
			return {
				state,
				base: { kind: 'assets', model: assets(m, false) },
				sheet: { kind: 'add-token', model: addToken(m, 'erc20') },
				textScale: scale
			};
		case 't3b':
			return {
				state,
				base: { kind: 'assets', model: assets(m, false) },
				sheet: { kind: 'add-token', model: addToken(m, 'native') },
				textScale: scale
			};
		case 't4':
			return { state, base: { kind: 'assets', model: assets(m, true) }, textScale: scale };
		case 't5':
			return {
				state,
				base: { kind: 'assets', model: assets(m, false) },
				sheet: { kind: 'add-token', model: addToken(m, 'erc20-invalid') },
				textScale: scale
			};
		case 't5b':
			return {
				state,
				base: { kind: 'assets', model: assets(m, false) },
				sheet: { kind: 'add-token', model: addToken(m, 'native-incompatible') },
				textScale: scale
			};
		case 'sd1':
			return { state, base: { kind: 'send-pick', model: sendPick(m, false) }, textScale: scale };
		case 'sd1b':
			return { state, base: { kind: 'send-pick', model: sendPick(m, true) }, textScale: scale };
		case 'sd2':
			return {
				state,
				base: { kind: 'send-form', model: sendForm(m, identicon, 'single') },
				textScale: scale
			};
		case 'sd2b':
			return {
				state,
				base: { kind: 'send-form', model: sendForm(m, identicon, 'split') },
				textScale: scale
			};
		case 'sd2d':
			return {
				state,
				base: { kind: 'send-form', model: sendForm(m, identicon, 'sweep') },
				textScale: scale
			};
		case 'sd2c':
			return {
				state,
				base: { kind: 'send-form', model: sendForm(m, identicon, 'split') },
				sheet: { kind: 'batch-import', model: batchImport(m, identicon) },
				textScale: scale
			};
		case 'sd2e':
			return {
				state,
				base: { kind: 'send-form', model: sendForm(m, identicon, 'single') },
				sheet: { kind: 'contact-pick', model: contactPick(m, identicon) },
				textScale: scale
			};
		case 'sd2f':
			return {
				state,
				base: { kind: 'send-form', model: sendForm(m, identicon, 'single') },
				sheet: { kind: 'fee-token', model: feeTokenPick(m) },
				textScale: scale
			};
		case 'sd3':
			return {
				state,
				base: { kind: 'send-confirm', model: sendConfirm(m, identicon, 'single') },
				textScale: scale
			};
		case 'sd3b':
			return {
				state,
				base: { kind: 'send-confirm', model: sendConfirm(m, identicon, 'split') },
				textScale: scale
			};
		case 'sd3c':
			return {
				state,
				base: { kind: 'send-confirm', model: sendConfirm(m, identicon, 'sweep') },
				textScale: scale
			};
		case 'sd4a':
			return {
				state,
				base: { kind: 'send-receipt', model: sendReceipt(m, 'submitting') },
				textScale: scale
			};
		case 'sd4b':
			return {
				state,
				base: { kind: 'send-receipt', model: sendReceipt(m, 'submitted') },
				textScale: scale
			};
		case 'sd4c':
			return {
				state,
				base: { kind: 'send-receipt', model: sendReceipt(m, 'confirmed') },
				textScale: scale
			};
	}
}

/** Build one desktop state — the same content, in the third column. */
export function buildDesktopFlowState(
	state: DesktopFlowStateId,
	m: WalletFlowMessages,
	identicon: Identicon
): DesktopFlowModel {
	const close = m['componentsUi.identiconViewer.close'];
	const back = m['receive.a11yBack'];

	switch (state) {
		case 'dr1':
			return {
				state,
				title: m['receive.title'],
				closeLabel: close,
				body: { kind: 'receive-list', model: receiveList(m) }
			};
		case 'dr2':
			return {
				state,
				title: m['receive.title'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'receive-qr', model: receiveQr(m, identicon, false) }
			};
		case 'dr3':
			return {
				state,
				title: m['receive.title'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'receive-qr', model: receiveQr(m, identicon, true) }
			};
		case 'ds1':
			// The scanner is a centred modal on the desktop, not a panel — see
			// `FlowsDesktop`, which routes this one state away from the column.
			return {
				state,
				title: m['componentsUi.scanner.title'],
				closeLabel: close,
				body: { kind: 'history', model: history(m) }
			};
		case 'da1':
			return {
				state,
				title: m['history.navTitle'],
				closeLabel: close,
				body: { kind: 'history', model: history(m) }
			};
		case 'da2':
			return {
				state,
				title: m['componentsTx.detail.sectionTitle'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'tx-detail', model: txDetail(m, identicon, 'received') }
			};
		case 'da3':
			return {
				state,
				title: m['history.navTitle'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'tx-detail', model: txDetail(m, identicon, 'sent') }
			};
		case 'dt1':
			return {
				state,
				title: m['assets.sectionTitle'],
				closeLabel: close,
				body: { kind: 'assets', model: assets(m, false) }
			};
		case 'dt3':
			return {
				state,
				title: m['addToken.navTitle'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'add-token', model: addToken(m, 'erc20') }
			};
		case 'dt3b':
			return {
				state,
				title: m['addToken.navTitle'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'add-token', model: addToken(m, 'native') }
			};
		case 'dt4':
			return {
				state,
				title: m['assets.sectionTitle'],
				closeLabel: close,
				body: { kind: 'assets', model: assets(m, true) }
			};
		case 'dsd1':
			return {
				state,
				title: m['send.selectTokenTitle'],
				closeLabel: close,
				body: { kind: 'send-pick', model: sendPick(m, false) }
			};
		case 'dsd2':
			return {
				state,
				title: fill(m['send.sendTitle'], { symbol: 'USDT' }),
				backLabel: back,
				closeLabel: close,
				body: { kind: 'send-form', model: sendForm(m, identicon, 'single') }
			};
		case 'dsd2b':
			return {
				state,
				title: fill(m['send.sendTitle'], { symbol: 'USDT' }),
				backLabel: back,
				closeLabel: close,
				body: { kind: 'send-form', model: sendForm(m, identicon, 'split') }
			};
		case 'dsd3':
			return {
				state,
				title: m['send.confirmTitle'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'send-confirm', model: sendConfirm(m, identicon, 'single') }
			};
		case 'dsd2e':
			return {
				state,
				title: m['send.pickContactTitle'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'contact-pick', model: contactPick(m, identicon) }
			};
		case 'dsd2f':
			return {
				state,
				title: m['send.feeTokenLabel'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'fee-token', model: feeTokenPick(m) }
			};
		case 'dsd2c':
			return {
				state,
				title: m['send.batchTitle'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'batch-import', model: batchImport(m, identicon) }
			};
		case 'dsd4':
			return {
				state,
				title: m['componentsUi.dock.send'],
				backLabel: back,
				closeLabel: close,
				body: { kind: 'send-receipt', model: sendReceipt(m, 'submitted') }
			};
	}
}

/** The desktop scanner's own model — `ds1` renders it over a dimmed window. */
export function buildDesktopScan(m: WalletFlowMessages): ScanModel {
	return scan(m, true);
}
