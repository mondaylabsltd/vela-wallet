/**
 * Wallet-flow message manifest (spec 021).
 *
 * A flat `dotted key -> resolved template` map, the shape spec 014's onboarding
 * flow and spec 020's intro already use, rather than spec 015's nested
 * manifest. With ~120 strings across four journeys the nested form would be
 * more field declarations than copy, and every one of them would have to be
 * mirrored in `engine.server.ts` by hand.
 *
 * Resolution happens at build time in `engine.server.ts`; `{{var}}` fills
 * happen where the fixture knows the value, through spec 015's `fill`.
 */

import {
	NET_HINT_KEYS,
	NET_RPC_FIELD_KEYS,
	NET_STOP_KEYS,
	VENUE_BLOCK_KEYS
} from '$lib/settings/messages';

/**
 * The corpus keys `fee_policy::failure_reason_key` answers with (spec 082
 * RJ13) — resolved at build time so the fee row (the sheet's and Send's) can
 * look up whichever the core names. A key the core adds later and this list
 * lacks draws no line.
 */
export const FEE_REASON_KEYS = [
	'componentsUi.gas.reasonQuote',
	'componentsUi.gas.reasonFeeToken',
	'componentsUi.gas.reasonSimulation',
	'componentsUi.gas.reasonQuoteHigh',
	'home.balanceDetailStatusRetrying',
	// Issue 483: the fee's own sentences — a chain out of reach ("Can't reach
	// {{chain}} to price this"), and a fault inside Vela. The browser's
	// `explore.chainDown` ("page data may be incomplete") is no longer the fee's.
	'componentsUi.gas.reasonChainDown',
	'componentsUi.gas.reasonInternal'
] as const;

/** Every corpus key the wallet-flow screens consume. Tests iterate this. */
export const WALLET_FLOW_KEYS = [
	// ---------------------------------------------------------------- chrome
	'receive.a11yBack',
	'componentsUi.identiconViewer.close',
	'componentsUi.identiconViewer.copyAddress',
	'componentsUi.networkFilter.pillAll',
	'componentsUi.dayGroup.today',
	'componentsUi.dayGroup.yesterday',
	'componentsUi.dock.send',

	// --------------------------------------------------------------- receive
	'receive.title',
	'receive.networksLine',
	'receive.searchNetworkPlaceholder',
	'receive.searchNetworkEmpty',
	'receive.qrTitleNetwork',
	'receive.qrTitleAsset',
	'receive.tokenContract',
	'receive.warningReminder',
	'receive.copied',
	'receive.request.saveImage',
	'receive.shareCardHeadline',
	'receive.shareCardNetworkNote',
	// Spec 090: the code's opt-in "include network" switch and its hint.
	'receive.includeNetwork',
	'receive.includeNetworkHint',
	// 087 F13: a receive row's button that shows its network's code.
	'componentsUi.funding.showQr',

	// ------------------------------------------------------------------ scan
	'componentsUi.scanner.title',
	'componentsUi.scanner.hint',
	'componentsUi.scanner.gallery',
	'componentsUi.scanner.fromGallery',
	'componentsUi.scanner.torch',
	'componentsUi.scanner.flipCamera',
	// Why there is nothing to look at. A browser refuses in more ways than a
	// phone does, and each refusal is a different thing for a person to do.
	'componentsUi.scanner.permissionText',
	'componentsUi.scanner.noCamera',
	'componentsUi.scanner.insecureOrigin',
	'componentsUi.scanner.cameraUnavailable',
	'componentsUi.scanner.noQrFoundMsg',
	'home.invalidQrTitle',

	// -------------------------------------------------------------- activity
	'history.navTitle',
	'history.loadingText',
	'history.emptyTitle',
	'history.emptyFilter',
	'history.labelSent',
	'history.labelReceived',
	'history.txLabelSent',
	'history.txLabelReceived',
	// Spec 082 RG2: a dApp's transaction from a core that did not describe it.
	'history.txLabelDappTx',
	// Spec 082 RJ16: a dApp call's counterparty that is the contract it went to.
	'componentsUi.signing.interactingLabel',
	'history.deleteRecord',
	'history.toName',
	'history.fromName',
	'history.viewOnExplorer',
	'componentsTx.receipt.statusConfirmed',
	// A feed row is not settled history: a submitted send is pending until
	// the tracker resolves it, and a refusal is failed (issue 211).
	'componentsTx.detail.statusPending',
	'componentsTx.detail.statusFailed',
	// 087 F04: a pending record nothing will settle reads "Unknown" — never
	// "Failed" (it may have been sent), never "Pending" for ever.
	'componentsUi.signing.intentUnknown',
	'componentsTx.detail.from',
	'componentsTx.detail.to',
	'componentsTx.detail.labelChain',
	'componentsTx.detail.labelDate',
	'componentsTx.detail.labelHash',
	// 083 H2: the site a dApp's transaction came from — the connection
	// detail's word for it.
	'connect.detail.labelApp',
	'componentsTx.detail.sectionTitle',
	// Spec 093: a dApp record's detail — the core's facts and its collapsed
	// technical lines, in the words the signing sheet and Connections use.
	'connect.detail.offChainNote',
	'componentsUi.signing.labelSpender',
	'componentsUi.signingApprove.spendingCap',
	'componentsUi.signingApprove.expiresLabel',
	'componentsUi.signingApprove.noExpiry',
	'componentsUi.signing.balanceChangesTitle',
	'componentsUi.signing.balanceUnverifiedToken',
	'componentsUi.signing.advancedToggle',
	'componentsTx.detail.labelOperation',
	'componentsTx.detail.opContractInteraction',
	'componentsUi.signing.batchSubtitle',
	'componentsTx.detail.opSignature',
	'componentsTx.detail.opTypedDataSignature',
	'connect.detail.contentCallData',
	'connect.detail.contentTypedData',
	'connect.detail.contentMessage',
	'connect.detail.contentMissing',
	'componentsUi.signing.typeLabel',
	// Spec 097 N4: why a dApp row failed — the words its request ended with
	// (`send.txErrorGeneric` is listed with the send flow's words).
	'componentsTx.receipt.failedHint',
	'componentsUi.signing.refused',

	// ---------------------------------------------------------------- assets
	'assets.sectionTitle',
	'assets.addToken',
	'assets.searchPlaceholder',
	'assets.addByAddress',
	'assets.emptyTitle',
	'assets.emptySubtext',
	'assets.notShowingTitle',
	'assets.notShowingBody',
	'tokenDetail.send',
	'tokenDetail.receive',
	'tokenDetail.labelPrice',
	'tokenDetail.priceValue',
	'tokenDetail.labelContract',
	'tokenDetail.labelDecimals',
	'tokenDetail.labelTransactions',
	'tokenDetail.viewOnExplorer',

	// ------------------------------------------------------------- add token
	'addToken.navTitle',
	'addToken.tabErc20',
	'addToken.tabNative',
	'addToken.labelNetwork',
	'addToken.tokenAddressLabel',
	'addToken.addToWalletBtn',
	'addToken.tokenAdded',
	'addToken.invalidAddress',
	'addToken.notFoundTitle',
	'addToken.notFoundMessage',
	'addToken.nativeAliasTitle',
	'addToken.nativeAliasMessage',
	'addToken.netSearchLabel',
	'addToken.netSearchPlaceholder',
	'addToken.netPickerEmpty',
	'addToken.netPickerSearchPlaceholder',
	'addToken.labelChainId',
	'addToken.labelNativeToken',
	'addToken.compatible',
	'addToken.notCompatible',
	'addToken.networkAdded',
	'addToken.addNetworkBtn',
	// The live add-token sheet (spec 028 T442): the probe in flight, and a
	// write that failed — both existed in the corpus, neither was on a mock.
	'addToken.searchingNetworks',
	'addToken.errorSaveToken',
	// T3b live. Why a network is refused, and why the wizard stopped, are the
	// core's sentences by key (`NetCompatibility.hint_key`,
	// `NetWizardView.error_key`): the check's two reasons, and the five stops —
	// among them "unable to verify", which is never worded as incompatible
	// (024 invariant ③). The card used to say "Not compatible · Deploy missing
	// contracts ↗" under every one of them, as text that went nowhere.
	...NET_HINT_KEYS,
	...NET_STOP_KEYS,
	'settingsModals.addNetwork.openChainSetupTool',
	// The RPC field under the wizard's result and "Re-check with this RPC" —
	// drawn when the core gives the field (`NetWizardView.rpc_field`), labelled
	// by the key it names. The tab's no-RPC stop said "Enter one, then
	// re-check" and had neither.
	...NET_RPC_FIELD_KEYS,
	'settingsModals.addNetwork.customRpcPlaceholder',
	'settingsModals.addNetwork.recheckWithRpc',

	// ------------------------------------------------------------------ send
	// A payment request this wallet cannot take up as it is (the send core's
	// `lock_error`): a network it does not have — with "Add this network" and
	// what came of it — or a token it cannot describe. In the corpus since the
	// first client, and on no web surface until PR 3's final round (F6/F27).
	'send.lock.netTitle',
	'send.lock.netBody',
	'send.lock.addNetwork',
	'send.lock.netNotFound',
	'send.lock.netNotCompatible',
	'send.lock.netAddError',
	'send.lock.tokenTitle',
	'send.lock.tokenBody',
	'send.selectTokenTitle',
	'send.searchPlaceholder',
	// What an EMPTY list says. Both were in the corpus and neither was on a
	// web surface: a picker that has nothing to offer showed a blank panel,
	// which is where an account holding nothing now lands (issue 209).
	'send.noTokensWithBalance',
	'send.noMatchingTokens',
	'history.filterAll',
	'send.filterStable',
	'send.filterGas',
	'send.filterOther',
	'send.multiSendTitle',
	'send.multiSendSummary',
	'send.multiSendChainNotice',
	'send.multiSendSameRecipient',
	'send.multiSendContinue',
	'send.selectAllValuable',
	'send.sendTitle',
	'send.maxBtn',
	'send.balanceLabel',
	'send.recipientLabel',
	'send.recipientN',
	'send.recipientDuplicate',
	'send.recipientCount_one',
	'send.recipientCount_other',
	// The split row's own prompt: an empty field with nothing in it read as a
	// line somebody forgot to draw.
	'send.recipientPlaceholder',
	'send.addRecipient',
	'send.fromContacts',
	'send.batchImport',
	'send.removeRecipient',
	'send.recipientPickAria',
	// The trust line the `send` core resolves for a recipient (spec 026).
	'componentsUi.signing.firstTimeTag',
	// Spec 096 F12: the recipient is a token's own contract (`send` core).
	'send.recipientTokenContract',
	'send.txErrorGeneric',
	// A submit the relay could not take, said on the confirm with its retry
	// (`SendView.tx_error`): the relay's gas account, and Try again.
	'send.txErrorBundlerFund',
	'send.txRetryBtn',
	// Correctness batch item 3: the account's previous transaction on this
	// network still holds the nonce — the held confirm's one line
	// (`SendView.previous_pending.key`)…
	'componentsUi.signing.confirmBlock.previousPending',
	// …and, PR 2 polish, the submit the relay turned back for it
	// (`tx_error` `previous_pending`): "Not sent yet", calmly, as on the sheet.
	'componentsUi.signing.notSentTitle',
	'componentsUi.signing.notSentBody',
	// …and a refusal told by its reason (`SendReceiptView.refusal_key`): the
	// fee sentence only for a fee refusal, "another transaction went first"
	// for a spent nonce, else the plain refusal (listed with the dApp rows).
	'send.txRejectedFees',
	'componentsUi.signing.wentFirst',
	// Spec 102: why this account cannot sign on the web (`SendView.tx_venue_block`).
	...VENUE_BLOCK_KEYS,
	'send.scanAria',
	'send.splitTotalLabel',
	'send.continueBtn',
	'componentsUi.gas.networkFee',

	// send · a failed fee, in the core's words (PR 2 note 1): the reason
	// under the row (`FeeView.failure.reason_key`), the row's figure when a
	// tap is the one way (`figure_key`), and the line under the held confirm
	// (`footer_key`) — "Retrying…" while the core asks again by itself.
	...FEE_REASON_KEYS,
	'componentsUi.gas.estimateFailed',
	'componentsUi.signing.confirmBlock.feeRetrying',
	'componentsUi.signing.confirmBlock.feeFailed',
	// …and, PR 2 polish, after the relay answered that it would fail: the
	// figure when a tap opens the fee coins, and the line under the confirm.
	'componentsUi.gas.payWithAnotherCoin',
	'componentsUi.signing.confirmBlock.feeWouldFail',

	// send · fee token
	'send.feeTokenLabel',
	'send.feeTokenHint',
	'send.feeTokenEstimate',

	// send · the fee you can refresh, at a speed you can choose (spec 068).
	// `send.gasTier.rapid` is deliberately NOT here: the variant is dead, the
	// relay never reports it and refuses it on the wire, so this shell must
	// never be able to name it.
	'send.feeRefresh',
	'send.feeStale',
	'send.feeSpeedLabel',
	'send.feeSpeedOnce',
	// …and the two things the control says about the network rather than the
	// person's choice (issue 686): that a faster speed is free here, so this
	// send takes it; and that there is only one speed to have.
	'send.feeSpeedFree',
	'send.feeSpeedSingle',
	'send.gasTier.fast',
	'send.gasTier.standard',
	'send.gasTier.slow',
	// No line under each name (2026-10-08): the option's own fee and gas bid
	// say what it buys. The descriptions (`send.gasTierHint*`) belong to the
	// Settings default-speed sheet, which shows neither figure.
	// …and what each speed BUYS as a number: the effective gas price beside
	// each option (issue 684). Drawn as digits alone — this names them for a
	// screen reader, which would otherwise hear a bare "300 gwei". An existing
	// corpus key, already translated in all 15 locales, so no path is invented
	// for a caption; `gwei` and `wei` are proper nouns and are not translated.
	'send.gasPriceLabel',

	// send · contact picker
	'send.pickContactTitle',
	'send.pickContactSearch',
	'contacts.sectionGroups',
	'contacts.title',
	'contacts.groupMembers',

	// send · batch import
	'send.batchTitle',
	'send.batchUnitFiat',
	'send.batchUnitToken',
	'send.batchPastePlaceholder',
	'send.batchImportFile',
	'send.batchTemplate',
	'send.batchRateSection',
	'send.batchRateLabel',
	'send.batchRateHint',
	'send.batchRateFailed',
	'send.batchRateLoading',
	'send.batchRateReset',
	'send.batchParsedCount',
	'send.batchBadAddress',
	'send.batchRejected_one',
	'send.batchRejected_other',
	'send.batchApply_other',
	'send.batchApply_one',
	'send.batchApplyEmpty',
	// The importer's verdicts (issues 204, 205). Every one of these has been in
	// the corpus, in all fifteen locales, since the importer was ported — the
	// core computed the judgement, the words were written, and this shell read
	// neither, so its button went dark and said nothing.
	'send.batchOverBalance',
	'send.batchOverCap',
	'send.batchDup',
	'send.batchNoPrice',
	'send.batchReading',
	'send.batchTemplateSaved',
	'send.batchImportFailedTitle',
	'send.batchImportFailedBody',
	// 087: a file in a legacy code page is refused with how to save it — the
	// contacts import's own sentence.
	'contacts.importFailEncoding',
	// The sentences those issues did NOT find waiting (new with them): what the
	// first choice on the importer is, what token mode does with the sheet's
	// figures, what importing does to the people already on the form and how to
	// choose the other, which row a dark Continue is waiting on, what is left to
	// give out, and one amount for every empty row.
	'send.batchUnitCaption',
	'send.batchTokenHint',
	'send.batchAddsToRows',
	'send.batchReplacesRows',
	'send.batchReplaceInstead',
	'send.batchAddInstead',
	'send.badAmount',
	'send.splitNeedsAddress',
	'send.splitNeedsAmount',
	'send.splitRemaining',
	'send.splitFillEmpty',

	// send · the core's alerts, worded (spec 038 #D4)
	'send.alertEstimateFailedTitle',
	'send.alertEstimateFailedBody',
	// PR 2 note 13: the estimate alert by its cause
	// (`sendEstimateFailureBodyKey`) — the chain out of reach by name; a fault
	// inside Vela is `componentsUi.gas.reasonInternal` (above).
	'send.alertEstimateChainDownBody',
	'send.alertAccountUnavailableBody',
	'send.alertInvalidAddressTitle',
	'send.alertInvalidAddressBody',
	'send.alertInvalidAmountTitle',
	'send.alertInvalidAmountBody',
	'send.alertInsufficientBalanceTitle',
	'send.alertInsufficientBalanceBody',
	'send.alertLoadTokensError',
	// The core's live amount verdicts (`SendAmountWarning`), which this shell
	// used to drop on the floor (issues 211 and 210) — the other three have
	// drawn them since spec 032.
	'send.warnNotEnoughToken',
	'send.warnInsufficientForGas',
	'send.warnInsufficientGas',
	'send.warnNeedGas',
	'send.warnCannotConvert',
	// Why ⇄ is inert: no rate to enter the display currency against. Already in
	// the corpus for the phones; web drew the control and read none of it.
	'send.denomToggleNoRate',
	// The same-asset ceiling (`same_asset_fee_issue`): the coin being sent also
	// pays the fee, and the two together do not fit. The phones and the desktop
	// have said it since spec 032.
	'send.sameFeeTokenBody',
	'send.sameFeeTokenMax',

	// send · confirm
	'send.confirmTitle',
	'send.fromLabel',
	'send.toLabel',
	// Spec 097 F: a name from the public wallet registry is tagged as such
	// beside the address it claims, never drawn as if it were the person's own.
	'send.velaUser',
	'send.estFeeLabel',
	'send.confirmSendBtn',
	'send.confirmTotalLine',
	'componentsTx.receipt.assetsCount',

	// send · receipt
	'send.txSubmitting',
	'send.txPreparingBiometric',
	'send.txBackgroundHint',
	'send.txCloseBackground',
	'send.txSubmittedTitle',
	'send.txConfirmedTitle',
	// Spec 097 F: a sweep's success has no one figure; its coins are listed.
	'componentsTx.detail.sent',
	'send.txWaitingConfirm',
	'send.txTypicalTime',
	'send.txRelaySending',
	'send.txRemaining',
	'send.txElapsed',
	'send.txSlowConfirm',
	'send.txRelayFunding',
	'componentsTx.receipt.txHash',
	'componentsTx.receipt.done',
	// Spec 082 RA10: a lost relay reply ("may have been sent") and a provable
	// "not sent", on the Send receipt as on the signing sheet.
	'componentsUi.signing.maybeSent',
	'componentsTx.receipt.userOpHash',
	'componentsTx.receipt.statusFailed'
] as const;

export type WalletFlowKey = (typeof WALLET_FLOW_KEYS)[number];

/** Resolved templates, keyed by corpus path. */
export type WalletFlowMessages = Readonly<Record<WalletFlowKey, string>>;
