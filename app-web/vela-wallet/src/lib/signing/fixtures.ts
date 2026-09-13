/**
 * Canonical signing fixtures (spec 022, data-model.md §3 — the single canon
 * all four platforms port). Content is verbatim from the 33 CS mocks in
 * `design/explore/`; builders merge it with resolved messages into
 * display-ready view models.
 *
 * The catalogue doubles as the degradation ladder's regression suite: cs23–cs24
 * and cs30–cs32 are the rungs below "verified descriptor", and they are here so
 * that any change to the renderer has to face what a wallet shows when it does
 * NOT know what a transaction does.
 */
import { SITES } from '$lib/explore/fixtures';
import { IDENTITY } from '$lib/wallet/fixtures';
import type { SigningMessages } from './messages';
import type {
	AllowanceChip,
	AmountLine,
	FeeModel,
	KeyValueRow,
	SigningModel,
	SigningStateId,
	TechModel,
	TokenMark,
	Tone
} from './model';

type Identicon = (seed: string) => string;

/** `{{var}}` interpolation — the same one-line fill the wallet fixtures use. */
export function fill(template: string, vars: Record<string, string>): string {
	return Object.entries(vars).reduce(
		(out, [name, value]) => out.replaceAll(`{{${name}}}`, value),
		template
	);
}

// --- Canon ----------------------------------------------------------------

export const ALL_STATES: SigningStateId[] = [
	'cs1',
	'cs2',
	'cs3',
	'cs4',
	'cs5',
	'cs6',
	'cs7',
	'cs8',
	'cs9',
	'cs10',
	'cs11',
	'cs12',
	'cs13',
	'cs14',
	'cs15',
	'cs16',
	'cs17',
	'cs18',
	'cs19',
	'cs20',
	'cs21',
	'cs22',
	'cs23',
	'cs24',
	'cs25',
	'cs26',
	'cs27',
	'cs28',
	'cs29',
	'cs30',
	'cs31',
	'cs32',
	'cs33',
	'cs34',
	'cs35'
];

const NETWORK = { name: 'Ethereum', dot: '#627EEA' };
const FEE_VALUE = '~0.0021 ETH ≈ $5.40';

/** Token marks: brand content, exactly like the wallet's chain colours. */
const T = {
	usdc: { letter: 'U', tint: '#2775CA' },
	eth: { letter: 'E', tint: '#627EEA' },
	weth: { letter: 'W', tint: '#8A92B2' },
	spweth: { letter: 'S', tint: '#4C6FFF' },
	usdt: { letter: 'T', tint: '#26A17B' }
} satisfies Record<string, TokenMark>;

/** dApps as the signing header draws them. `unknown` is the no-name case. */
const D = {
	uniswap: { name: SITES.uniswap.name, host: SITES.uniswap.host, letter: 'U', tint: '#FF007A' },
	oneinch: { name: '1inch', host: 'app.1inch.io', letter: '1', tint: '#C2352D' },
	opensea: { name: SITES.opensea.name, host: SITES.opensea.host, letter: 'O', tint: '#2081E2' },
	morpho: { name: 'Morpho', host: 'app.morpho.org', letter: 'M', tint: '#2E5BFF' },
	safe: { name: 'Safe', host: 'app.safe.global', letter: 'S', tint: '#12FF80' },
	ens: { name: SITES.ens.name, host: SITES.ens.host, letter: 'E', tint: '#5284FF' },
	phish: { name: 'opensae-mint', host: 'opensae-mint.xyz', letter: 'O', tint: '#6E6B62' }
};

const ADDR = {
	alice: '0xaF5e…b3e1',
	aliceFull: '0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1',
	vitalik: '0xd8dA…6045',
	self: IDENTITY.addressDisplay,
	oneinchRouter: '0x1111…0582',
	universalRouter: '0x3fC9…7FAD',
	uniswapV3: '0x68b3…4dC5',
	bayc: '0xBC4C…f13D',
	conduit: '0x1E00…3c71',
	morphoVault: '0x38989B…21eB',
	unknown: '0x4e1dC6…A9C1',
	rewards: '0x067d3D…2ed1',
	usdt: '0xdAC1…1ec7',
	safe: '0x4167…461a',
	deployed: '0x1A2b…9304',
	usdcFull: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48'
};

/** The unknown-site header the blind-signing mocks use. */
function unknownDapp(m: SigningMessages) {
	return { name: m.tagUnverified, host: 'dapp.example.com', letter: 'D', tint: '#6E6B62' };
}

// --- Small builders -------------------------------------------------------

const line = (
	sign: string,
	value: string,
	symbol: string,
	token: TokenMark | undefined,
	tone: Tone,
	fiat?: string,
	caption?: string
): AmountLine => ({ sign, value, symbol, token, tone, fiat, caption });

const row = (label: string, value: string, valueTone?: Tone, mono?: boolean): KeyValueRow => ({
	label,
	value,
	valueTone,
	mono
});

const chip = (id: string, label: string, state: AllowanceChip['state']): AllowanceChip => ({
	id,
	label,
	state
});

const onchainFee = (m: SigningMessages): FeeModel => ({
	kind: 'onchain',
	label: m.feeLabel,
	value: FEE_VALUE
});

/** The technical-details disclosure. Empty is legitimate: it still opens. */
function tech(m: SigningMessages, over: Partial<TechModel> = {}): TechModel {
	return {
		title: m.advancedToggle,
		params: [],
		identities: [],
		copyLabel: m.copyValue,
		explorerLabel: m.viewOnExplorer,
		...over
	};
}

/** cs1/cs29's five-layer panel — the one the founder asked to see in full. */
function transferTech(m: SigningMessages): TechModel {
	return tech(m, {
		fn: { label: m.techFunction, signature: 'transfer(address to, uint256 value)' },
		params: [
			row('to', ADDR.alice, undefined, true),
			row('value', fill(m.techRawUnits, { value: '1000000000', n: '6' }), undefined, true)
		],
		identities: [
			{ role: m.techIdentityToken, name: 'USD Coin', address: ADDR.usdcFull, mark: T.usdc },
			{
				role: m.techIdentityRecipient,
				name: 'Alice Chen',
				address: ADDR.aliceFull,
				mark: { letter: 'A', tint: '#E8572A' }
			}
		],
		simResult: row(m.techSimResult, `−1,000 USDC · ${m.balancesMatchHero}`),
		raw: {
			label: `${m.techRawData} · ${fill(m.byteSize, { n: '68' })}`,
			hex: '0xa9059cbb000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e100000000000000000000000000000000000000000000000000000003b9aca00'
		}
	});
}

// --- The catalogue --------------------------------------------------------

/**
 * One scenario's parts, before messages are woven in. Written as a function of
 * the resolved copy so every string in the sheet comes from the corpus and
 * every number comes from the mock.
 */
type Scenario = (m: SigningMessages, identicon: Identicon) => Omit<SigningModel, 'id'>;

const CATALOGUE: Record<SigningStateId, Scenario> = {
	// -- cs1–cs4: the transfer family ------------------------------------
	cs1: (m) => ({
		dapp: D.uniswap,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSend, tone: 'neutral' },
			{
				kind: 'amount',
				line: line('', '1,000', 'USDC', T.usdc, 'neutral', '≈ $1,000.00')
			},
			{
				kind: 'sentence',
				text: fill(m.summarySend, { amount: '1,000 USDC', to: 'Alice Chen' }),
				tone: 'accent'
			},
			{
				kind: 'party',
				label: m.labelRecipient,
				name: 'Alice Chen',
				address: ADDR.alice,
				badge: { text: m.tagContact, tone: 'neutral' }
			}
		],
		tech: transferTech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmSend, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs2: (m) => ({
		dapp: D.uniswap,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSend, tone: 'neutral' },
			{ kind: 'amount', line: line('', '10', 'ETH', T.eth, 'neutral', '≈ $25,604.00') },
			{
				kind: 'sentence',
				text: fill(m.summarySend, { amount: '10 ETH', to: ADDR.vitalik }),
				tone: 'accent'
			},
			{
				kind: 'party',
				label: m.labelRecipient,
				name: 'vitalik.eth',
				address: ADDR.vitalik,
				badge: { text: m.tagFirstTime, tone: 'caution' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmSend, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs3: (m) => ({
		dapp: D.safe,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSend, tone: 'neutral' },
			{ kind: 'amount', line: line('', '0.5', 'ETH', T.eth, 'neutral', '≈ $1,280.20') },
			{ kind: 'positive', text: m.okSelfTransfer },
			{
				kind: 'party',
				label: m.labelRecipient,
				name: fill(m.selfName, { name: IDENTITY.name }),
				address: ADDR.self,
				badge: { text: m.tagWallet, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmSend, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs4: (m) => ({
		dapp: D.uniswap,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSend, tone: 'neutral' },
			{ kind: 'amount', line: line('', '100', 'USDC', T.usdc, 'neutral', '≈ $100.00') },
			{
				kind: 'sentence',
				text: fill(m.summarySendFrom, { amount: '100 USDC', to: ADDR.vitalik }),
				tone: 'accent'
			},
			{ kind: 'rows', rows: [row(m.labelFrom, ADDR.alice, undefined, true)] },
			{ kind: 'party', label: m.labelRecipient, name: 'vitalik.eth', address: ADDR.vitalik }
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmSend, enabled: true },
		panelTitle: m.panelTitle
	}),

	// -- cs5–cs8: the approval family, under the never-unlimited mandate --
	cs5: (m) => ({
		dapp: D.oneinch,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentApprove, tone: 'danger' },
			{
				kind: 'allowance',
				label: m.labelSpendingCap,
				value: m.valueUnlimited,
				valueTone: 'danger',
				chips: [
					// Permanently disabled, not merely unselected: an unlimited
					// request is the one thing this wallet will not sign as asked.
					chip('requested', m.chipRequested, 'disabled'),
					chip('balance', m.chipBalance, 'idle'),
					chip('custom', m.chipCustom, 'idle'),
					chip('revoke', m.chipRevoke, 'idle')
				],
				note: `${m.unlimitedDisabled}\n${m.choosePrompt}`
			},
			{
				kind: 'party',
				label: m.labelSpender,
				name: '1inch Router',
				address: ADDR.oneinchRouter,
				badge: { text: m.tagVerified, tone: 'success' }
			},
			{ kind: 'warning', tone: 'danger', text: m.warnUnlimited }
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		// Nothing to slide until a finite amount exists.
		confirm: { hint: m.slideToConfirm, action: m.intentApprove, enabled: false },
		panelTitle: m.panelTitle
	}),

	cs6: (m) => ({
		dapp: D.oneinch,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentApprove, tone: 'neutral' },
			{
				kind: 'allowance',
				label: m.labelSpendingCap,
				value: '1,240 USDC',
				valueTone: 'neutral',
				chips: [
					chip('requested', m.chipRequested, 'disabled'),
					chip('balance', m.chipBalance, 'selected'),
					chip('custom', m.chipCustom, 'idle'),
					chip('revoke', m.chipRevoke, 'idle')
				]
			},
			{
				kind: 'sentence',
				text: fill(m.summaryApprove, { spender: '1inch Router', amount: '1,240 USDC' }),
				tone: 'neutral'
			},
			{
				kind: 'party',
				label: m.labelSpender,
				name: '1inch Router',
				address: ADDR.oneinchRouter,
				badge: { text: m.tagVerified, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.intentApprove, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs7: (m) => ({
		dapp: D.uniswap,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentApprove, tone: 'neutral' },
			{
				kind: 'allowance',
				label: m.labelSpendingCap,
				value: '+100 USDC',
				valueTone: 'neutral',
				chips: [
					chip('requested', m.chipRequested, 'selected'),
					chip('balance', m.chipBalance, 'idle'),
					chip('custom', m.chipCustom, 'idle'),
					chip('revoke', m.chipRevoke, 'idle')
				],
				// increaseAllowance is an INCREMENT: the number that matters is
				// the one it lands on, so the sheet does the addition.
				resultingTotal: row(m.labelResultingTotal, '350 USDC')
			},
			{
				kind: 'party',
				label: m.labelSpender,
				name: 'Uniswap Router',
				address: ADDR.universalRouter,
				badge: { text: m.tagVerified, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.intentApprove, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs8: (m) => ({
		dapp: D.oneinch,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentRevoke, tone: 'neutral' },
			{
				kind: 'allowance',
				label: m.labelSpendingCap,
				value: m.valueRevoke,
				valueTone: 'neutral',
				chips: [
					chip('requested', m.chipRequested, 'disabled'),
					chip('balance', m.chipBalance, 'idle'),
					chip('custom', m.chipCustom, 'idle'),
					chip('revoke', m.chipRevoke, 'selected')
				]
			},
			{
				kind: 'sentence',
				text: fill(m.summaryRevoke, { spender: '1inch Router' }),
				tone: 'neutral'
			},
			{
				kind: 'party',
				label: m.labelSpender,
				name: '1inch Router',
				address: ADDR.oneinchRouter,
				badge: { text: m.tagVerified, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.intentRevoke, enabled: true },
		panelTitle: m.panelTitle
	}),

	// -- cs9–cs10: NFTs ---------------------------------------------------
	cs9: (m) => ({
		dapp: D.opensea,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentTransferNft, tone: 'neutral' },
			{ kind: 'nft', id: '#6529', collection: 'Bored Ape Yacht Club' },
			{
				kind: 'sentence',
				text: fill(m.summaryTransferNft, { id: '#6529', to: 'Alice Chen' }),
				tone: 'accent'
			},
			{
				kind: 'party',
				label: m.labelRecipient,
				name: 'Alice Chen',
				address: ADDR.alice,
				badge: { text: m.tagContact, tone: 'neutral' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmPlain, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs10: (m) => ({
		dapp: D.opensea,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentApproveAll, tone: 'danger' },
			{
				kind: 'allowance',
				label: m.labelSpendingCap,
				value: m.valueAllNfts,
				valueTone: 'danger',
				// setApprovalForAll has no finite form to offer — the editor's
				// two chips are the only honest choices.
				chips: [
					chip('revoke', m.chipRevokeAccess, 'idle'),
					chip('grant', m.chipGrantAll, 'selected')
				]
			},
			{
				kind: 'sentence',
				text: fill(m.summaryApproveNft, { operator: 'OpenSea Conduit' }),
				tone: 'accent'
			},
			{
				kind: 'party',
				label: m.labelCollection,
				name: 'Bored Ape Yacht Club',
				address: ADDR.bayc,
				badge: { text: m.tagVerified, tone: 'success' }
			},
			{
				kind: 'party',
				label: m.labelOperator,
				name: 'OpenSea Conduit',
				address: ADDR.conduit
			},
			{ kind: 'warning', tone: 'caution', text: m.warnApproveAll }
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.intentApproveAll, enabled: true },
		panelTitle: m.panelTitle
	}),

	// -- cs11–cs13: swaps -------------------------------------------------
	cs11: (m) => ({
		dapp: D.oneinch,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSwap, tone: 'neutral' },
			{
				kind: 'swap',
				pay: line('−', '1,000', 'USDC', T.usdc, 'neutral', '≈ $1,000.00', m.labelPay),
				receive: line('+', '0.3042', 'WETH', T.weth, 'success', '≈ $778.90', m.labelMinReceived)
			},
			{
				kind: 'sentence',
				text: fill(m.summarySwap, { pay: '1,000 USDC', receive: '0.3042 WETH' }),
				tone: 'accent'
			},
			{
				kind: 'party',
				label: m.labelInteracting,
				name: '1inch Aggregation Router · 1inch Network',
				address: ADDR.oneinchRouter,
				badge: { text: m.tagVerified, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmSwap, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs12: (m) => ({
		dapp: D.uniswap,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSwap, tone: 'neutral' },
			{
				kind: 'swap',
				pay: line('−', '0.5', 'ETH', T.eth, 'neutral', '≈ $1,280.20', m.labelPay),
				receive: line('+', '1,278.11', 'USDC', T.usdc, 'success', '≈ $1,278.11', m.labelMinReceived)
			},
			{
				kind: 'sentence',
				text: fill(m.summarySwap, { pay: '0.5 ETH', receive: '1,278.11 USDC' }),
				tone: 'accent'
			},
			{
				kind: 'party',
				label: m.labelInteracting,
				name: 'Uniswap V3 Router',
				address: ADDR.uniswapV3,
				badge: { text: m.tagVerified, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmSwap, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs13: (m) => ({
		dapp: D.uniswap,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSwap, tone: 'neutral' },
			{
				kind: 'swap',
				pay: line('−', '1,000', 'USDC', T.usdc, 'neutral', undefined, m.labelPay),
				receive: line('+', '0.3042', 'WETH', T.weth, 'success', undefined, m.labelMinReceived)
			},
			{
				kind: 'rows',
				rows: [row(m.labelDeadline, fill(m.expiredValue, { time: '2026-08-14 18:00' }), 'caution')]
			},
			{ kind: 'warning', tone: 'caution', text: m.warnExpired },
			{ kind: 'warning', tone: 'danger', text: m.warnWillFail }
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmSwap, enabled: true },
		panelTitle: m.panelTitle
	}),

	// -- cs14–cs15: ERC-4626 vaults ---------------------------------------
	cs14: (m) => ({
		dapp: D.morpho,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentDeposit, tone: 'neutral' },
			{
				kind: 'swap',
				pay: line('−', '2', 'WETH', T.weth, 'neutral', '≈ $5,120.80', m.labelDepositAsset),
				receive: line(
					'+',
					'1.9631',
					'spWETH',
					T.spweth,
					'success',
					undefined,
					m.labelSharesReceived
				)
			},
			{ kind: 'warning', tone: 'caution', text: m.warnUnverifiedAmount },
			{
				kind: 'party',
				label: m.labelInteracting,
				name: 'Morpho Vault · Morpho Labs',
				address: ADDR.morphoVault,
				badge: { text: m.tagVerified, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmDeposit, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs15: (m) => ({
		dapp: D.morpho,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentWithdraw, tone: 'neutral' },
			{ kind: 'amount', line: line('+', '2', 'WETH', T.weth, 'success', '≈ $5,120.80') },
			{ kind: 'sentence', text: fill(m.summaryReceive, { amount: '2 WETH' }), tone: 'accent' },
			{
				kind: 'party',
				label: m.labelInteracting,
				name: 'Morpho Vault · Morpho Labs',
				address: ADDR.morphoVault,
				badge: { text: m.tagVerified, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmWithdraw, enabled: true },
		panelTitle: m.panelTitle
	}),

	// -- cs16–cs17: off-chain permits -------------------------------------
	cs16: (m) => ({
		dapp: D.uniswap,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentPermit, tone: 'danger' },
			{
				kind: 'sentence',
				text: fill(m.summaryPermitUnlimited, { spender: 'Universal Router', token: 'USDC' }),
				tone: 'danger'
			},
			{
				kind: 'party',
				label: m.labelSpender,
				name: 'Universal Router',
				address: ADDR.universalRouter,
				badge: { text: m.tagVerified, tone: 'success' }
			},
			{
				kind: 'rows',
				rows: [
					row(m.labelSpendingCap, `${m.valueUnlimited} USDC`, 'danger'),
					row(m.labelExpires, '2026-09-14 19:30')
				]
			},
			// The whole reason this is danger and not caution: there is no
			// editor to offer, because a signature cannot be capped here.
			{ kind: 'warning', tone: 'danger', text: m.warnPermitCantCap }
		],
		tech: tech(m),
		techOpen: false,
		fee: { kind: 'offchain', note: m.okNoNetworkFee },
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.signLabel, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs17: (m) => ({
		dapp: D.uniswap,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentPermit, tone: 'neutral' },
			{
				kind: 'sentence',
				text: fill(m.summaryPermit, { spender: 'Universal Router', amount: '1,000 USDC' }),
				tone: 'accent'
			},
			{
				kind: 'party',
				label: m.labelSpender,
				name: 'Universal Router',
				address: ADDR.universalRouter,
				badge: { text: m.tagVerified, tone: 'success' }
			},
			{
				kind: 'rows',
				rows: [row(m.labelSpendingCap, '1,000 USDC'), row(m.labelDeadline, '2030-03-14 08:26')]
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: { kind: 'offchain', note: m.okNoNetworkFee },
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.signLabel, enabled: true },
		panelTitle: m.panelTitle
	}),

	// -- cs18–cs22: messages, from readable down to a raw hash ------------
	cs18: (m) => ({
		dapp: unknownDapp(m),
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentTypedData, tone: 'neutral' },
			{ kind: 'warning', tone: 'caution', text: m.warnBlindTyped },
			{
				kind: 'rows',
				rows: [
					row(m.labelTypedDomain, 'CoolProtocol · v2'),
					row(m.labelType, 'Order'),
					row(m.labelSigningFor, 'dapp.example.com', 'accent')
				]
			},
			{
				kind: 'code',
				lines: [
					'{ "maker": "0x14fB1f…D1eA5c",',
					'  "taker": "0x0000…0000",',
					'  "makerAmount": "1000000000", … }'
				]
			}
		],
		tech: tech(m, { summary: fill(m.byteSize, { n: '412' }) }),
		techOpen: false,
		fee: { kind: 'offchain', note: m.okNoNetworkFee },
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.signLabel, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs19: (m) => ({
		dapp: D.ens,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSignIn, tone: 'neutral' },
			{
				kind: 'rows',
				rows: [
					row(m.labelSiweSite, 'app.ens.domains'),
					row(m.labelSiweStatement, '登录以管理你的 ENS 名称')
				]
			},
			{
				kind: 'code',
				lines: [
					'app.ens.domains wants you to sign in',
					'with your Ethereum account:',
					IDENTITY.addressDisplay
				]
			},
			{ kind: 'positive', text: fill(m.okSiwe, { domain: 'app.ens.domains' }) }
		],
		tech: tech(m),
		techOpen: false,
		fee: { kind: 'offchain', note: m.okNoNetworkFee },
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.signLabel, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs20: (m) => ({
		dapp: D.phish,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSignIn, tone: 'danger' },
			// The mismatch goes ABOVE the facts, not below them: by the time
			// somebody has read a login screen they have already decided.
			{
				kind: 'warning',
				tone: 'danger',
				text: fill(m.warnSiweMismatch, { domain: 'opensea.io', origin: 'opensae-mint.xyz' })
			},
			{
				kind: 'rows',
				rows: [
					row(m.labelSiweSite, 'opensea.io', 'danger'),
					row(m.labelSiweOrigin, 'opensae-mint.xyz', undefined, true),
					row(m.labelSiweStatement, '登录以查看你的 NFT')
				]
			},
			{
				kind: 'code',
				lines: [
					'opensea.io wants you to sign in',
					'with your Ethereum account:',
					IDENTITY.addressDisplay
				]
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: { kind: 'hidden' },
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.signLabel, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs21: (m) => ({
		dapp: unknownDapp(m),
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentMessage, tone: 'neutral' },
			{ kind: 'warning', tone: 'caution', text: m.warnHexMessage },
			{
				kind: 'code',
				lines: [
					'0xdeadbeefcafebabe0102030405',
					'060708091011121314151617181920',
					'2122232425262728293031…'
				],
				note: `(${fill(m.byteSize, { n: '80' })})`
			},
			{ kind: 'rows', rows: [row(m.labelSigningFor, 'dapp.example.com')] }
		],
		tech: tech(m),
		techOpen: false,
		fee: { kind: 'hidden' },
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.signLabel, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs22: (m) => ({
		dapp: unknownDapp(m),
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentBlind, tone: 'danger' },
			{ kind: 'sentence', text: m.bodyEthSign, tone: 'danger' },
			{
				kind: 'code',
				lines: ['0x9c22ff5f21f0b81b113e63f7db6da9', '4fedef11b2119b4088b89664fb9a3c', 'b658']
			},
			{ kind: 'warning', tone: 'danger', text: m.warnEthSign }
		],
		tech: tech(m),
		techOpen: false,
		fee: { kind: 'hidden' },
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmPlain, enabled: true },
		panelTitle: m.panelTitle
	}),

	// -- cs23–cs24: simulation as the protagonist -------------------------
	cs23: (m) => ({
		dapp: unknownDapp(m),
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentContractCall, tone: 'neutral' },
			{ kind: 'warning', tone: 'caution', text: fill(m.warnBlindDecode, { bytes: '196' }) },
			{ kind: 'rows', rows: [row(m.labelAmount, '0.1 ETH ≈ $256.04')] },
			{
				kind: 'party',
				label: m.labelInteracting,
				name: m.tagUnverified,
				address: ADDR.unknown,
				badge: { text: m.tagUnverified, tone: 'caution' }
			},
			{
				kind: 'balances',
				title: m.balancesTitle,
				rows: [{ symbol: 'ETH', delta: '−0.1', tone: 'neutral' }],
				note: m.balancesBlindSimulated
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmPlain, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs24: (m) => ({
		dapp: unknownDapp(m),
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentContractCall, tone: 'danger' },
			{ kind: 'sentence', text: m.summaryDrain, tone: 'danger' },
			{
				kind: 'balances',
				title: m.balancesTitle,
				rows: [
					{ symbol: 'USDC', delta: '−8,450', tone: 'danger' },
					{ symbol: 'ETH', delta: '−0.8', tone: 'danger' }
				],
				note: m.warnDrain,
				noteTone: 'danger'
			},
			{
				kind: 'party',
				label: m.labelInteracting,
				name: m.tagUnverified,
				address: ADDR.unknown,
				badge: { text: m.tagUnverified, tone: 'caution' }
			},
			{ kind: 'warning', tone: 'danger', text: fill(m.warnBlindDecode, { bytes: '4' }) }
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmPlain, enabled: true },
		panelTitle: m.panelTitle
	}),

	// -- cs25–cs28: deploy, batch, Safe, burn -----------------------------
	cs25: (m) => ({
		dapp: D.safe,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentDeploy, tone: 'neutral' },
			{ kind: 'sentence', text: m.summaryDeploy, tone: 'accent' },
			{
				kind: 'rows',
				rows: [
					row(m.labelBytecode, fill(m.byteSize, { n: '246' })),
					row(m.labelPredictedAddress, ADDR.deployed, undefined, true)
				]
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmPlain, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs26: (m) => ({
		dapp: D.oneinch,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentBatch, tone: 'neutral' },
			{ kind: 'sentence', text: fill(m.summaryBatch, { count: '2' }), tone: 'accent' },
			{
				kind: 'card',
				title: fill(m.batchStep, { index: '1', action: m.intentApprove }),
				tone: 'neutral',
				rows: [row(m.labelSpendingCap, '100 USDC'), row(m.labelSpender, '1inch Router')]
			},
			{
				kind: 'card',
				title: fill(m.batchStep, { index: '2', action: m.intentSwap }),
				tone: 'neutral',
				rows: [row(m.labelPay, '−100 USDC'), row(m.labelMinReceived, '+0.0304 WETH')]
			},
			{
				kind: 'balances',
				title: m.balancesTitle,
				rows: [
					{ symbol: 'USDC', delta: '−100', tone: 'neutral' },
					{ symbol: 'WETH', delta: '+0.0304', tone: 'success' }
				],
				note: m.balancesMatchHero
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmPlain, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs27: (m) => ({
		dapp: D.safe,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSafe, tone: 'neutral' },
			{ kind: 'sentence', text: m.summarySafe, tone: 'accent' },
			{
				// Safe's calldata nests, so the sheet decodes the inner call too:
				// a wrapper that only showed the outer call would show nothing.
				kind: 'card',
				title: fill(m.safeInnerCall, { action: m.intentSend }),
				tone: 'neutral',
				rows: [row(m.labelAmount, '250 USDC'), row(m.labelRecipient, 'Alice Chen')]
			},
			{
				kind: 'party',
				label: m.labelInteracting,
				name: 'Safe 1.4.1 · Safe Ecosystem',
				address: ADDR.safe,
				badge: { text: m.tagVerified, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmPlain, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs28: (m) => ({
		dapp: unknownDapp(m),
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentSend, tone: 'danger' },
			{
				kind: 'amount',
				line: line('', '500', 'USDT', T.usdt, 'danger'),
				card: true,
				note: m.sentToTokenContract
			},
			{
				kind: 'party',
				label: m.labelRecipient,
				name: 'Tether USD',
				address: ADDR.usdt,
				badge: { text: m.tagContract, tone: 'danger' }
			},
			{ kind: 'warning', tone: 'danger', text: m.warnTokenToContract }
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmSend, enabled: true },
		panelTitle: m.panelTitle
	}),

	// -- cs29: cs1 with the whole technical panel open --------------------
	cs29: (m, identicon) => ({
		...CATALOGUE.cs1(m, identicon),
		techOpen: true
	}),

	// -- cs30–cs32: the ladder's lower rungs ------------------------------
	cs30: (m) => ({
		dapp: unknownDapp(m),
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentContractCall, tone: 'neutral' },
			{ kind: 'sentence', text: fill(m.summaryBestEffort, { fn: 'execute(…)' }), tone: 'accent' },
			{ kind: 'warning', tone: 'caution', text: m.warnBestEffort },
			{
				kind: 'rows',
				rows: [
					row(m.techFunction, 'execute(bytes,bytes[],uint256)', undefined, true),
					row(fill(m.techParam, { index: '1', name: 'bytes' }), '0x0b00… (2)'),
					row(fill(m.techParam, { index: '2', name: 'bytes[]' }), '2'),
					row(fill(m.techParam, { index: '3', name: 'deadline' }), '2026-08-15 20:00')
				]
			},
			{
				kind: 'party',
				label: m.labelInteracting,
				name: m.tagUnverified,
				address: ADDR.unknown,
				badge: { text: m.tagUnverified, tone: 'caution' }
			},
			{
				kind: 'balances',
				title: m.balancesTitle,
				rows: [
					{ symbol: 'ETH', delta: '−0.1', tone: 'neutral' },
					{ symbol: 'USDC', delta: '+255.8', tone: 'success' }
				],
				note: m.balancesBestEffort
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmPlain, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs31: (m) => ({
		dapp: unknownDapp(m),
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentContractCall, tone: 'neutral' },
			{ kind: 'sentence', text: m.summaryVerifiedAbi, tone: 'neutral' },
			{
				kind: 'rows',
				rows: [
					row('claimRewards · ids', '[128, 129, 130]'),
					row('beneficiary', fill(m.selfName, { name: ADDR.self }), undefined, true),
					row('restake', 'true')
				]
			},
			{
				kind: 'party',
				label: m.labelInteracting,
				name: 'RewardsVault',
				address: ADDR.rewards,
				badge: { text: m.tagContract, tone: 'neutral' }
			},
			{ kind: 'warning', tone: 'caution', text: m.warnVerifiedAbi },
			{
				kind: 'balances',
				title: m.balancesTitle,
				rows: [{ symbol: 'stETH', delta: '+4.21', tone: 'success' }],
				note: m.balancesMatchHero
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmPlain, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs32: (m) => ({
		dapp: unknownDapp(m),
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentContractCall, tone: 'neutral' },
			// The deepest rung: neither decode nor simulation. Both failures are
			// stated plainly, and the amount is still shown — the facts that ARE
			// knowable never get withheld because the rest is not.
			{ kind: 'warning', tone: 'caution', text: fill(m.warnSelectorNotListed, { bytes: '4' }) },
			{ kind: 'warning', tone: 'danger', text: m.warnSimUnavailable },
			{ kind: 'rows', rows: [row(m.labelAmount, '0.25 ETH ≈ $640.10')] },
			{
				kind: 'party',
				label: m.labelInteracting,
				name: m.tagUnverified,
				address: '0x004C22…6819',
				badge: { text: m.tagUnverified, tone: 'caution' }
			},
			{
				kind: 'code',
				lines: [
					'0x8fabe4c2000000000000000000000000',
					'd400866e00b055b20752a826cd5c89b8',
					'11de130b…'
				],
				note: `(${fill(m.byteSize, { n: '132' })})`
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		confirm: { hint: m.slideToConfirm, action: m.confirmPlain, enabled: true },
		panelTitle: m.panelTitle
	}),

	// -- cs33: cs11 with the fee-token selector open ----------------------
	cs33: (m, identicon) => ({
		...CATALOGUE.cs11(m, identicon),
		fee: {
			kind: 'onchain',
			label: m.feeLabel,
			value: FEE_VALUE,
			selector: {
				title: m.feeTokenTitle,
				options: [
					{
						id: 'eth',
						mark: T.eth,
						name: 'ETH',
						balance: `${m.feeBalance} 0.0689`,
						fee: `~0.0021 ETH`,
						selected: true
					},
					{
						id: 'usdc',
						mark: T.usdc,
						name: 'USDC',
						balance: `${m.feeBalance} 1,240.00`,
						fee: `~5.55 USDC`,
						selected: false
					}
				]
			}
		}
	}),

	// -- cs34 / cs35: the cap being TYPED (spec 032 phase 39) --
	//
	// cs5 is where this starts: an unlimited request with its Requested chip
	// dead. These are what the card becomes once somebody picks Custom — the
	// field under the chips, the big number above counting what has been
	// typed, and the slide shut until the core says the amount is finite.
	cs34: (m) => ({
		dapp: D.oneinch,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentApprove, tone: 'neutral' },
			{
				kind: 'allowance',
				label: m.labelSpendingCap,
				value: '500 USDC',
				valueTone: 'neutral',
				chips: [
					chip('requested', m.chipRequested, 'disabled'),
					chip('balance', m.chipBalance, 'idle'),
					chip('custom', m.chipCustom, 'selected'),
					chip('revoke', m.chipRevoke, 'idle')
				],
				custom: { value: '500', symbol: 'USDC', placeholder: '0' }
			},
			{
				kind: 'party',
				label: m.labelSpender,
				name: '1inch Router',
				address: ADDR.oneinchRouter,
				badge: { text: m.tagVerified, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		// A finite cap is a cap: the slide may arm.
		confirm: { hint: m.slideToConfirm, action: m.intentApprove, enabled: true },
		panelTitle: m.panelTitle
	}),

	cs35: (m) => ({
		dapp: D.oneinch,
		network: NETWORK,
		blocks: [
			{ kind: 'intent', text: m.intentApprove, tone: 'neutral' },
			{
				kind: 'allowance',
				label: m.labelSpendingCap,
				// What is still true while the field holds nonsense: the
				// REQUEST is unlimited, and it is drawn as the danger it is.
				value: m.valueUnlimited,
				valueTone: 'danger',
				chips: [
					chip('requested', m.chipRequested, 'disabled'),
					chip('balance', m.chipBalance, 'idle'),
					chip('custom', m.chipCustom, 'selected'),
					chip('revoke', m.chipRevoke, 'idle')
				],
				custom: {
					value: '12.3.4',
					symbol: 'USDC',
					placeholder: '0',
					error: m.invalidAmount
				},
				note: `${m.unlimitedDisabled}\n${m.choosePrompt}`
			},
			{
				kind: 'party',
				label: m.labelSpender,
				name: '1inch Router',
				address: ADDR.oneinchRouter,
				badge: { text: m.tagVerified, tone: 'success' }
			}
		],
		tech: tech(m),
		techOpen: false,
		fee: onchainFee(m),
		signer: signer(m),
		// A cap nobody could parse is not a cap.
		confirm: { hint: m.slideToConfirm, action: m.intentApprove, enabled: false },
		panelTitle: m.panelTitle
	})
};

function signer(m: SigningMessages) {
	// The identicon is threaded in by `buildSigningState`; this keeps the
	// catalogue itself free of the renderer's plumbing.
	return { label: m.signingAccount, name: IDENTITY.name, identiconSvg: '' };
}

/** Build one scenario, ready to render. */
export function buildSigningState(
	state: SigningStateId,
	m: SigningMessages,
	identicon: Identicon
): SigningModel {
	const built = CATALOGUE[state](m, identicon);
	return {
		id: state,
		...built,
		signer: { ...built.signer, identiconSvg: identicon(IDENTITY.addressFull) }
	};
}

/** Gallery chip labels — mock naming, never translated. */
export function stateLabel(state: SigningStateId): string {
	return state.toUpperCase();
}
