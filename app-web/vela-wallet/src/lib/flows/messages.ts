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
	'history.emptyFilter',
	'history.labelSent',
	'history.labelReceived',
	'history.txLabelSent',
	'history.txLabelReceived',
	'history.deleteRecord',
	'history.toName',
	'history.fromName',
	'history.viewOnExplorer',
	'componentsTx.receipt.statusConfirmed',
	// A feed row is not settled history: a submitted send is pending until
	// the tracker resolves it, and a refusal is failed (issue 211).
	'componentsTx.detail.statusPending',
	'componentsTx.detail.statusFailed',
	'componentsTx.detail.from',
	'componentsTx.detail.to',
	'componentsTx.detail.labelChain',
	'componentsTx.detail.labelDate',
	'componentsTx.detail.labelHash',
	'componentsTx.detail.sectionTitle',

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
	'addToken.deployContracts',
	'addToken.errorNotCompatible',
	// The live add-token sheet (spec 028 T442): the probe in flight, and a
	// write that failed — both existed in the corpus, neither was on a mock.
	'addToken.searchingNetworks',
	'addToken.errorSaveToken',
	// T3b live: an inconclusive probe is never worded as incompatible (024 invariant ③).
	'settingsModals.addNetwork.unableToVerify',

	// ------------------------------------------------------------------ send
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
	'send.txErrorGeneric',
	'send.scanAria',
	'send.splitTotalLabel',
	'send.continueBtn',
	'componentsUi.gas.networkFee',

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
	// …and the one line under each that says what that speed BUYS. The names
	// are a scale and nothing else now, so without these the cheap tier would
	// read as a defect nobody picks on purpose. `rapid` gets none, for the
	// same reason its name is not requested here.
	'send.gasTierHintFast',
	'send.gasTierHintStandard',
	'send.gasTierHintSlow',
	// …and what each speed BUYS as a number: the effective gas price beside
	// each option (issue 684). Drawn as digits alone — this names them for a
	// screen reader, which would otherwise hear a bare "300 gwei". An existing
	// corpus key, already translated in all 15 locales, so no path is invented
	// for a caption; `gwei` and `wei` are proper nouns and are not translated.
	'send.gasPriceLabel',

	// send · contact picker
	'send.pickContactTitle',
	'send.pickContactSearch',
	'send.scanToFill',
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
	'send.txWaitingConfirm',
	'send.txTypicalTime',
	'send.txRemaining',
	'send.txElapsed',
	'send.txSlowConfirm',
	'componentsTx.receipt.txHash',
	'componentsTx.receipt.done'
] as const;

export type WalletFlowKey = (typeof WALLET_FLOW_KEYS)[number];

/** Resolved templates, keyed by corpus path. */
export type WalletFlowMessages = Readonly<Record<WalletFlowKey, string>>;
