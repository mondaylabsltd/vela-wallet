/**
 * Live overlays for the pushed flow screens (spec 025 Phase 3 — the assets
 * screen; Phase 4 adds receive/activity). Siblings of `fixtures.ts`: the
 * gallery states keep their canon, the live route swaps the money.
 *
 * The assets list is the SAME holdings the home shows, so it is the same
 * builder (`liveAssetRow`) — one rule for what a held token looks like. The
 * empty body's copy comes from the fixture layer's own empty state (T4): the
 * words are the corpus's either way, and borrowing the built block avoids a
 * second builder for four strings.
 */

import { feedPositionOf } from '$lib/wallet/live-detail';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { MtokView } from '$lib/core/generated/MtokView';
import type { NetWizardView } from '$lib/core/generated/NetWizardView';
import type { WalletFlowMessages } from './messages';
import {
	chainName,
	explorerAddressURL,
	getAllNetworksSync,
	nativeSymbol
} from '$lib/services/networks';
import { chainLogoURL } from '$lib/services/tokens-model';
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import { balanceTokenMark, chainMark, tokenMarkFor } from './marks';
import { chainColor, MASK } from '$lib/wallet/fixtures';
import type {
	AddTokenTab,
	FactRowModel,
	FlowHeaderModel,
	ShareCardModel,
	StatusChipModel,
	TokenDetailModel,
	TokenMarkModel
} from './model';
import type { WalletIdentity } from '$lib/wallet/identity';
import { shortenAddress } from '$lib/wallet/identity';
import { fill } from '$lib/wallet/messages';
import { encodeQr } from '$lib/wallet/qr';
import {
	liveActivityGroups,
	liveActivityRow,
	liveAssetRow,
	moneyText,
	narrowedFeed,
	tokenExplorerURL,
	trimBalance
} from '$lib/wallet/live';
import { addressLines } from './fixtures';
import { liveBatchImport, type BatchLiveInputs } from './live-batch';
import { liveContactPick, type ContactPickLiveInputs } from './live-contact-pick';
import {
	liveFeeTokenPick,
	liveNetworkPill,
	liveSendConfirm,
	liveSendForm,
	liveSendPick,
	liveSendReceipt,
	type SendLiveInputs
} from './live-send';
import type { WalletMessages } from '$lib/wallet/messages';
import type {
	AddTokenModel,
	AssetsModel,
	DesktopFlowModel,
	FlowScreenModel,
	HistoryModel,
	ReceiveListModel,
	ReceiveQrModel
} from './model';

export interface FlowsLiveInputs {
	balance: BalanceView;
	currency: CurrencyView;
	m: WalletMessages;
	/** The fixture T4 body, for the empty state's corpus copy. */
	emptyCopy: AssetsModel['empty'];
	/** Phase 4: the feed (history screen) and the person (receive card). */
	feed?: FeedView | null;
	identity?: WalletIdentity;
	/**
	 * The live send flow (spec 026). Present only while a send session exists —
	 * absent, every send screen stays the picture 021 drew, which is what the
	 * gallery renders.
	 */
	send?: SendLiveInputs;
	/** The batch importer, while its sheet is open (spec 026 US3). */
	batch?: BatchLiveInputs;
	/**
	 * The address book, for the recipient picker (spec 028 US5). Present while
	 * a send is open; absent, the picker stays the gallery's picture.
	 */
	contactPick?: ContactPickLiveInputs;
	/**
	 * The `manage_tokens` core's view while the add-token sheet is open (spec
	 * 028 US4), with the flat flow corpus beside it — the same shape `batch`
	 * rides in, because this file's own `m` is the wallet home's nested map.
	 */
	addToken?: AddTokenLiveInputs;
	/**
	 * The flat flow corpus (spec 028 Phase 9): the receive and token screens
	 * re-word their own titles and facts from it. Absent in older callers and
	 * tests, where the drawn words stand.
	 */
	fm?: WalletFlowMessages;
	/**
	 * The network whose code the receive screen shows (T482): the tapped row
	 * or the sidebar's filter. `null`/absent — the list's first network, which
	 * is what the drawn R2 shows.
	 */
	receiveChainId?: number | null;
	/** The sidebar's filter, when one is set — the home's `chainFilter`. */
	chainFilter?: number | null;
	/** The held token a detail screen is about (the page's `selectedAssetId`). */
	selectedToken?: BalanceToken;
}

/** The networks the receive list walks, in the order the page indexes them. */
export function receiveNetworks(): ReturnType<typeof getAllNetworksSync> {
	return getAllNetworksSync();
}

export { balanceTokenMark, chainMark } from './marks';

/**
 * The held tokens a pushed screen lists — the home's rule (`liveSections`):
 * narrowed to the sidebar's / the pill's chain when one is chosen. The page
 * indexes the assets screen through this same list (`visibleBalanceTokens`),
 * so a tapped row names the token that was tapped.
 */
export function visibleBalanceTokens(
	tokens: BalanceToken[],
	chainFilter: number | null | undefined
): BalanceToken[] {
	const filter = chainFilter ?? null;
	return filter === null ? tokens : tokens.filter((t) => t.chain_id === filter);
}

/** The pill on A1 / T1 / SD1, about the chain the screen is narrowed to. */
function livePill(model: { header: FlowHeaderModel }, inputs: FlowsLiveInputs): FlowHeaderModel {
	const pillAll =
		inputs.fm?.['componentsUi.networkFilter.pillAll'] ?? inputs.m.networkFilter.pillAll;
	return { ...model.header, pill: liveNetworkPill(inputs.chainFilter, pillAll, model.header.pill) };
}

function liveAssets(model: AssetsModel, inputs: FlowsLiveInputs): AssetsModel {
	const { balance: view, currency, m } = inputs;
	const tokens = visibleBalanceTokens(view.tokens, inputs.chainFilter);
	const rows = tokens.map((t) => liveAssetRow(t, currency, m, view.hidden));
	// Empty only once the core has actually looked (never while unknown/loading)
	// — or when the chosen chain holds nothing while others do (spec 028 Phase
	// 10: the pill narrows this screen as the sidebar narrows the home).
	const settledEmpty = rows.length === 0 && !view.balance_unknown && !view.holdings_loading;
	const filteredEmpty = rows.length === 0 && view.tokens.length > 0;
	return {
		...model,
		header: livePill(model, inputs),
		rows,
		empty: settledEmpty || filteredEmpty ? inputs.emptyCopy : undefined
	};
}

function liveHistory(model: HistoryModel, inputs: FlowsLiveInputs): HistoryModel {
	const { balance: view, m } = inputs;
	const header = livePill(model, inputs);
	if (!inputs.feed) return { ...model, header, mode: 'loading', groups: [] };
	// The same narrowing the home applies (`narrowedFeed`), so the pushed
	// list and the page's row index walk one feed.
	const feed = narrowedFeed(inputs.feed, inputs.chainFilter ?? null);
	const groups = liveActivityGroups(feed, m, view.hidden);
	return {
		...model,
		header,
		mode: groups.length > 0 ? 'rows' : view.balance_unknown ? 'loading' : 'empty',
		groups
	};
}

/**
 * The receive list: every network the wallet knows, all with THE address —
 * and the count in the subtitle is that list's, not the drawn eight (spec 028
 * Phase 9, T481: "同一地址，通用于全部 8 个网络" over twelve rows).
 */
function liveReceiveList(model: ReceiveListModel, inputs: FlowsLiveInputs): ReceiveListModel {
	const identity = inputs.identity;
	if (identity === undefined) return model;
	const template = model.rows[0];
	const networks = receiveNetworks();
	return {
		...model,
		subtitle:
			inputs.fm === undefined
				? model.subtitle
				: fill(inputs.fm['receive.networksLine'], { count: networks.length }),
		rows: networks.map((n) => ({
			name: n.displayName,
			code: nativeSymbol(n.chainId),
			badgeColor: chainColor(n.chainId),
			chainId: n.chainId,
			logoUrl: chainLogoURL(n.chainId) || undefined,
			addressDisplay: shortenAddress(identity.address),
			addressFull: identity.address,
			copyLabel: template?.copyLabel ?? '',
			qrLabel: template?.qrLabel ?? ''
		}))
	};
}

/**
 * The QR screen, about the network that was actually chosen (spec 028 Phase 9,
 * T482). Until this phase the tapped row's index fell off the flow stack and
 * every code said "Ethereum" with an ETH mark. R3 — the asset variant, told
 * apart by its contract line — is about the held token instead: its logo in
 * the centre, its contract under the title, its chain in the words.
 */
function liveReceiveQr(model: ReceiveQrModel, inputs: FlowsLiveInputs): ReceiveQrModel {
	const identity = inputs.identity;
	if (identity === undefined) return model;
	const token = model.contract === undefined ? undefined : inputs.selectedToken;
	const chainId = token?.chain_id ?? inputs.receiveChainId ?? receiveNetworks()[0]?.chainId ?? 1;
	const network = chainName(chainId);
	const fm = inputs.fm;
	const title =
		fm === undefined
			? model.title
			: token === undefined
				? fill(fm['receive.qrTitleNetwork'], { network })
				: fill(fm['receive.qrTitleAsset'], { symbol: token.symbol, network });
	return {
		...model,
		title,
		// The code is the ADDRESS, encoded (spec 028 T411). Until now this screen
		// drew 021's placeholder pattern, which never encoded anything — a person
		// showed it to a friend and no money arrived.
		code: encodeQr(identity.address),
		contract:
			model.contract === undefined || token === undefined
				? model.contract
				: {
						...model.contract,
						value:
							token.token_address === null
								? inputs.m.assetDetail.nativeToken
								: shortenAddress(token.token_address),
						copyValue: token.token_address ?? undefined
					},
		account: {
			...model.account,
			name: identity.name,
			identiconSvg: identity.identiconSvg,
			lines: addressLines(identity.address)
		},
		centre: token === undefined ? chainMark(chainId) : balanceTokenMark(token),
		explorerUrl: explorerAddressURL(chainId, identity.address),
		// What 保存图片 produces: R4, worded and marked for THIS network or token.
		share:
			fm === undefined
				? undefined
				: {
						headline: fm['receive.shareCardHeadline'],
						code: encodeQr(identity.address),
						name: identity.name,
						lines: addressLines(identity.address),
						networkNote: fill(fm['receive.shareCardNetworkNote'], { network }),
						networkMark: token === undefined ? chainMark(chainId) : balanceTokenMark(token),
						identiconSvg: identity.identiconSvg,
						wordmark: 'Vela Wallet'
					}
	};
}

/**
 * T2 — the phone's token screen, about the held token the row named (spec 028
 * Phase 9, T483). Until this phase it was the drawn fixture: a USDT the person
 * did not hold, whichever row they tapped. The facts are the desktop's
 * `liveAssetDetail` facts in T2's shape, worded from the flow corpus.
 */
function liveTokenDetail(model: TokenDetailModel, inputs: FlowsLiveInputs): TokenDetailModel {
	const token = inputs.selectedToken;
	const fm = inputs.fm;
	if (token === undefined || fm === undefined) return model;
	const { balance: view, currency, m } = inputs;
	const hidden = view.hidden;
	const held = parseFloat(token.balance) || 0;
	const fiat =
		hidden || token.price_usd === null
			? m.balance.noPrice
			: moneyText(held * token.price_usd, currency);
	const items = (inputs.feed?.rows ?? [])
		.flatMap((row) => (row.type === 'item' ? [row.item] : []))
		.filter((item) => item.chain_id === token.chain_id && item.symbol === token.symbol);
	const rows = items.map((item) => liveActivityRow(item, m, hidden));
	// Each row can open its own detail — through the history's own index,
	// so it is the same screen, reached the same way (spec 038 #E2).
	const rowTargets = items.map((item) => feedPositionOf(inputs.feed, item.id));
	const facts: FactRowModel[] = [
		{
			label: fm['tokenDetail.labelPrice'],
			value:
				token.price_usd === null
					? m.balance.noPrice
					: fill(fm['tokenDetail.priceValue'], {
							symbol: token.symbol,
							value: moneyText(token.price_usd, currency)
						})
		},
		{
			label: fm['tokenDetail.labelContract'],
			value:
				token.token_address === null
					? m.assetDetail.nativeToken
					: shortenAddress(token.token_address),
			mono: token.token_address !== null,
			copy:
				token.token_address === null ? undefined : fm['componentsUi.identiconViewer.copyAddress'],
			copyValue: token.token_address ?? undefined
		},
		{ label: fm['tokenDetail.labelDecimals'], value: String(token.decimals) },
		{ label: fm['addToken.labelNetwork'], value: chainName(token.chain_id) }
	];
	return {
		...model,
		mark: balanceTokenMark(token),
		symbol: token.symbol,
		chain: chainName(token.chain_id),
		balance: hidden ? MASK : `${trimBalance(token.balance)} ${token.symbol}`,
		fiat,
		facts,
		rows,
		rowTargets,
		explorerUrl: tokenExplorerURL(token, view.address)
	};
}

/**
 * The share card (spec 028 T413) — the image someone hands to a stranger.
 *
 * Three things ride together on purpose, and the third is the reason: the
 * address in readable text, so a person can check it without a scanner; the
 * code, so a camera can; and the account's own identicon, which is DERIVED from
 * the address. A card someone doctored to swap the address carries artwork that
 * no longer matches it — the mismatch is the tell.
 */
function liveShareCard(model: ShareCardModel, inputs: FlowsLiveInputs): ShareCardModel {
	const identity = inputs.identity;
	if (identity === undefined) return model;
	return {
		...model,
		code: encodeQr(identity.address),
		name: identity.name,
		lines: addressLines(identity.address),
		identiconSvg: identity.identiconSvg
	};
}

/**
 * T3 — adding a token by contract address (spec 028 T442).
 *
 * The drawn sheet has one field, one result card and one CTA, and every state
 * T5 draws is a variant of those. Here each is the `manage_tokens` core's
 * projection: validity is `address_valid`, the probe is `detecting`, the card
 * is the first chain that answered with an identity (`found`, registry order),
 * and "added" is the core's own dedupe verdict against what is stored. The
 * shell words it and nothing more.
 *
 * Two things the mock draws are not here yet, and are recorded rather than
 * improvised: the network picker (the core probes EVERY known chain at once
 * and the card names the one that answered, so a picker would be choosing
 * what the core already found), and the native-token tab (that is
 * `network_admin`'s, and it lives in Settings).
 */
export interface AddTokenLiveInputs {
	view: MtokView;
	m: WalletFlowMessages;
	/**
	 * Which tab is showing (spec 028 Phase 10). The drawn toggle switched
	 * nothing until this phase: the ERC-20 half is `manage_tokens`' and the
	 * native half is `network_admin`'s add-network wizard, driven through the
	 * app-resident session the settings screen already uses.
	 */
	tab?: AddTokenTab;
	native?: AddNetworkTabInputs;
}

/** T3b's inputs: what was typed, the wizard's state, and the chain just added. */
export interface AddNetworkTabInputs {
	query: string;
	wizard: NetWizardView;
	/** The chain this sheet added, so the card can say so and the CTA can rest. */
	addedChainId: number | null;
}

/** A chain's mark for the native tab: its logo when the index has one, its coin's letters otherwise. */
function wizardMark(chainId: number, symbol: string, hasLogo: boolean): TokenMarkModel {
	return {
		ticker: symbol,
		badgeColor: chainColor(chainId),
		logoUrls: hasLogo ? [chainLogoURL(chainId)] : undefined,
		badgeHidden: true
	};
}

/**
 * T3b / T5b — adding a network by name or chain ID (spec 028 Phase 10).
 *
 * Every state the drawings show is the wizard's phase, worded here: the
 * index's matches while typing, one card with a neutral chip while the
 * chain is probed, a verdict once it has answered — and an inconclusive
 * probe is worded as "unable to verify", never as incompatible (024's
 * invariant ③, kept where the words are chosen).
 */
function liveAddNetworkTab(
	model: AddTokenModel,
	m: WalletFlowMessages,
	native: AddNetworkTabInputs
): AddTokenModel {
	const { wizard, query, addedChainId } = native;
	const info = wizard.chain_info;
	const facts = (chainId: number, symbol: string): FactRowModel[] => [
		{ label: m['addToken.labelChainId'], value: String(chainId) },
		{ label: m['addToken.labelNativeToken'], value: symbol }
	];
	const notFound = (): AddTokenModel['result'] => ({
		kind: 'not-found',
		text: fill(m['addToken.netPickerEmpty'], { query })
	});
	const card = (
		chip: StatusChipModel,
		link?: string,
		chainId = info?.chain_id,
		name = info?.name,
		symbol = info?.native_symbol
	): AddTokenModel['result'] => ({
		kind: 'network',
		mark:
			chainId === undefined
				? { ticker: symbol ?? '', badgeColor: chainColor(0), badgeHidden: true }
				: wizardMark(chainId, symbol ?? nativeSymbol(chainId), true),
		name: name ?? query,
		chip,
		link,
		facts: chainId === undefined ? [] : facts(chainId, symbol ?? nativeSymbol(chainId))
	});

	let result: AddTokenModel['result'];
	let canAdd = false;
	if (addedChainId !== null) {
		// The network is in the registry now: its own name and coin.
		result = card(
			{ text: m['addToken.networkAdded'], tone: 'success' },
			undefined,
			addedChainId,
			chainName(addedChainId),
			nativeSymbol(addedChainId)
		);
	} else if (query.trim() === '') {
		result = { kind: 'none' };
	} else if (wizard.phase === 'searching') {
		result = { kind: 'searching', text: m['addToken.searchingNetworks'] };
	} else if (wizard.phase === 'idle' || wizard.phase === 'suggested') {
		result =
			wizard.suggestions.length === 0
				? notFound()
				: {
						kind: 'suggestions',
						rows: wizard.suggestions.map((s) => ({
							id: String(s.chain_id),
							mark: wizardMark(s.chain_id, s.native_currency_symbol, s.has_logo),
							name: s.name,
							meta: `${m['addToken.labelChainId']} ${s.chain_id}`
						}))
					};
	} else if (wizard.phase === 'resolving' || wizard.phase === 'checking') {
		result = card({ text: m['addToken.searchingNetworks'], tone: 'info' });
	} else if (wizard.phase === 'error') {
		const error = wizard.error;
		if (error === null || error.type === 'not_found') result = notFound();
		else if (error.type === 'already_added') {
			result = card(
				{ text: m['addToken.networkAdded'], tone: 'success' },
				undefined,
				error.chain_id,
				info?.name ?? chainName(error.chain_id),
				info?.native_symbol ?? nativeSymbol(error.chain_id)
			);
		} else {
			result = card(
				{ text: m['addToken.notCompatible'], tone: 'error' },
				`${m['addToken.errorNotCompatible']} · ${m['addToken.deployContracts']}`
			);
		}
	} else {
		// Checked: the verdict.
		const compat = wizard.compat;
		if (compat === null || compat.rpc_failure !== null) {
			result = card({ text: m['settingsModals.addNetwork.unableToVerify'], tone: 'warning' });
		} else if (compat.compatible) {
			result = card({ text: m['addToken.compatible'], tone: 'success' });
			canAdd = wizard.can_add;
		} else {
			result = card(
				{ text: m['addToken.notCompatible'], tone: 'error' },
				`${m['addToken.errorNotCompatible']} · ${m['addToken.deployContracts']}`
			);
		}
	}

	return {
		...model,
		tab: 'native',
		network: undefined,
		fieldLabel: m['addToken.netSearchLabel'],
		fieldValue: query,
		fieldPlaceholder: m['addToken.netSearchPlaceholder'],
		fieldError: undefined,
		result,
		cta: m['addToken.addNetworkBtn'],
		ctaDisabled: !canAdd
	};
}

export function liveAddToken(model: AddTokenModel, inputs: AddTokenLiveInputs): AddTokenModel {
	const { view, m } = inputs;
	if (inputs.tab === 'native' && inputs.native !== undefined) {
		return liveAddNetworkTab(model, m, inputs.native);
	}
	const first = view.found[0];
	const typed = view.input_address.trim() !== '';
	const result: AddTokenModel['result'] = view.detecting
		? { kind: 'searching', text: m['addToken.searchingNetworks'] }
		: first !== undefined
			? {
					kind: 'token',
					// The found token's own logo, by its contract (T492).
					mark: tokenMarkFor(first.chain_id, first.symbol, view.input_address.trim() || null),
					name: first.name,
					detail: `${first.symbol} · ${m['tokenDetail.labelDecimals']} ${first.decimals} · ${first.network_name}`,
					chip: first.added ? { text: m['addToken.tokenAdded'], tone: 'success' } : undefined
				}
			: view.not_found
				? {
						kind: 'not-found',
						text: `${m['addToken.notFoundTitle']} — ${m['addToken.notFoundMessage']}`
					}
				: { kind: 'none' };
	return {
		...model,
		tab: 'erc20',
		network: undefined,
		fieldValue: view.input_address,
		fieldError: view.save_error
			? m['addToken.errorSaveToken']
			: typed && !view.address_valid
				? m['addToken.invalidAddress']
				: undefined,
		result,
		cta: m['addToken.addToWalletBtn'],
		ctaDisabled: first === undefined || first.added || view.saving
	};
}

/** A mobile flow screen with its live bodies swapped in; others untouched. */
export function withLiveFlow(model: FlowScreenModel, inputs: FlowsLiveInputs): FlowScreenModel {
	let next = model;
	if (model.base.kind === 'assets') {
		next = { ...next, base: { kind: 'assets', model: liveAssets(model.base.model, inputs) } };
	} else if (model.base.kind === 'history') {
		next = { ...next, base: { kind: 'history', model: liveHistory(model.base.model, inputs) } };
	} else if (model.base.kind === 'share-card') {
		next = {
			...next,
			base: { kind: 'share-card', model: liveShareCard(model.base.model, inputs) }
		};
	} else if (model.base.kind === 'receive-list') {
		next = {
			...next,
			base: { kind: 'receive-list', model: liveReceiveList(model.base.model, inputs) }
		};
	}
	if (model.sheet?.kind === 'receive-qr') {
		next = {
			...next,
			sheet: { kind: 'receive-qr', model: liveReceiveQr(model.sheet.model, inputs) }
		};
	}
	const send = inputs.send;
	if (send) {
		if (model.base.kind === 'send-pick') {
			next = { ...next, base: { kind: 'send-pick', model: liveSendPick(model.base.model, send) } };
		} else if (model.base.kind === 'send-form') {
			next = { ...next, base: { kind: 'send-form', model: liveSendForm(model.base.model, send) } };
		} else if (model.base.kind === 'send-confirm') {
			next = {
				...next,
				base: { kind: 'send-confirm', model: liveSendConfirm(model.base.model, send) }
			};
		} else if (model.base.kind === 'send-receipt') {
			next = {
				...next,
				base: { kind: 'send-receipt', model: liveSendReceipt(model.base.model, send) }
			};
		}
		if (model.sheet?.kind === 'fee-token') {
			next = {
				...next,
				sheet: { kind: 'fee-token', model: liveFeeTokenPick(model.sheet.model, send) }
			};
		}
	}
	if (inputs.batch && model.sheet?.kind === 'batch-import') {
		next = {
			...next,
			sheet: { kind: 'batch-import', model: liveBatchImport(model.sheet.model, inputs.batch) }
		};
	}
	if (model.sheet?.kind === 'token-detail') {
		next = {
			...next,
			sheet: { kind: 'token-detail', model: liveTokenDetail(model.sheet.model, inputs) }
		};
	}
	if (inputs.contactPick && model.sheet?.kind === 'contact-pick') {
		next = {
			...next,
			sheet: {
				kind: 'contact-pick',
				model: liveContactPick(model.sheet.model, inputs.contactPick)
			}
		};
	}
	if (inputs.addToken && model.sheet?.kind === 'add-token') {
		next = {
			...next,
			sheet: { kind: 'add-token', model: liveAddToken(model.sheet.model, inputs.addToken) }
		};
	}
	return next;
}

/**
 * The desktop third-column shape of the same overlay. The column's title is
 * the live body's own header title whenever the body went live (spec 028
 * Phase 9, T484): the panel used to keep the fixture's "发送 USDT" over a
 * live ETH form, because the phone reads `header.title` and the column
 * read `model.title`.
 */
export function withLiveDesktopFlow(
	model: DesktopFlowModel,
	inputs: FlowsLiveInputs
): DesktopFlowModel {
	const next = withLiveDesktopBody(model, inputs);
	if (next.body === model.body) return next;
	const header = (next.body.model as { header?: FlowHeaderModel }).header;
	return header === undefined ? next : { ...next, title: header.title };
}

function withLiveDesktopBody(model: DesktopFlowModel, inputs: FlowsLiveInputs): DesktopFlowModel {
	switch (model.body.kind) {
		case 'assets':
			return { ...model, body: { kind: 'assets', model: liveAssets(model.body.model, inputs) } };
		case 'history':
			return { ...model, body: { kind: 'history', model: liveHistory(model.body.model, inputs) } };
		case 'receive-list':
			return {
				...model,
				body: { kind: 'receive-list', model: liveReceiveList(model.body.model, inputs) }
			};
		case 'receive-qr':
			return {
				...model,
				body: { kind: 'receive-qr', model: liveReceiveQr(model.body.model, inputs) }
			};
		case 'send-pick':
			return inputs.send
				? {
						...model,
						body: { kind: 'send-pick', model: liveSendPick(model.body.model, inputs.send) }
					}
				: model;
		case 'send-form':
			return inputs.send
				? {
						...model,
						body: { kind: 'send-form', model: liveSendForm(model.body.model, inputs.send) }
					}
				: model;
		case 'send-confirm':
			return inputs.send
				? {
						...model,
						body: { kind: 'send-confirm', model: liveSendConfirm(model.body.model, inputs.send) }
					}
				: model;
		case 'send-receipt':
			return inputs.send
				? {
						...model,
						body: { kind: 'send-receipt', model: liveSendReceipt(model.body.model, inputs.send) }
					}
				: model;
		case 'fee-token':
			return inputs.send
				? {
						...model,
						body: { kind: 'fee-token', model: liveFeeTokenPick(model.body.model, inputs.send) }
					}
				: model;
		case 'add-token':
			return inputs.addToken
				? {
						...model,
						body: { kind: 'add-token', model: liveAddToken(model.body.model, inputs.addToken) }
					}
				: model;
		// The importer is a SHEET on the phone and a column BODY here; the sheet
		// arm below the phone's switch never reached this shape, so the desktop
		// drew the fixture over a live session — tabs that would not switch, a
		// rate nobody could edit (spec 038 #E6).
		case 'batch-import':
			return inputs.batch
				? {
						...model,
						body: { kind: 'batch-import', model: liveBatchImport(model.body.model, inputs.batch) }
					}
				: model;
		case 'contact-pick':
			return inputs.contactPick
				? {
						...model,
						body: {
							kind: 'contact-pick',
							model: liveContactPick(model.body.model, inputs.contactPick)
						}
					}
				: model;
		default:
			return model;
	}
}
