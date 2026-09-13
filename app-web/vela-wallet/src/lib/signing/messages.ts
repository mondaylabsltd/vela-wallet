/**
 * Signing message manifest (spec 022 §5).
 *
 * Client-safe: names keys and shapes only — resolution happens in
 * `engine.server.ts` at build time, exactly like `wallet/messages.ts`.
 *
 * Roughly 95% of these keys predate this spec: the shipping React Native
 * signing sheet already had them under `componentsUi.signing` /
 * `componentsUi.signingApprove`, and reusing them is what keeps one wallet
 * saying one thing about a transaction. Only the ladder's deeper rungs (the
 * verified-ABI decode, the 4byte best effort, the un-simulatable case, the
 * drain reveal) needed new copy.
 *
 * Every `{{var}}` template is filled by the fixture layer, which is static —
 * so interpolation happens at prerender, like the wallet's.
 */

export interface SigningMessages {
	panelTitle: string;
	signingAccount: string;
	advancedToggle: string;
	close: string;
	slideToConfirm: string;
	slideConfirmAction: string;
	confirmSend: string;
	confirmSwap: string;
	confirmDeposit: string;
	confirmWithdraw: string;
	confirmPlain: string;
	signLabel: string;
	intentSend: string;
	intentApprove: string;
	intentApproveAll: string;
	intentRevoke: string;
	intentSwap: string;
	intentDeposit: string;
	intentWithdraw: string;
	intentTransferNft: string;
	intentContractCall: string;
	intentBatch: string;
	intentBlind: string;
	intentSignIn: string;
	intentMessage: string;
	intentTypedData: string;
	intentPermit: string;
	intentDeploy: string;
	intentSafe: string;
	labelRecipient: string;
	labelSpender: string;
	labelOperator: string;
	labelCollection: string;
	labelInteracting: string;
	labelFrom: string;
	labelAmount: string;
	labelDeadline: string;
	labelMinReceived: string;
	labelPay: string;
	labelSiweSite: string;
	labelSiweOrigin: string;
	labelSiweStatement: string;
	labelTypedDomain: string;
	labelType: string;
	labelSigningFor: string;
	labelSpendingCap: string;
	labelExpires: string;
	labelResultingTotal: string;
	labelBytecode: string;
	labelPredictedAddress: string;
	labelDepositAsset: string;
	labelSharesReceived: string;
	tagContact: string;
	tagWallet: string;
	tagContract: string;
	tagVerified: string;
	tagUnverified: string;
	tagFirstTime: string;
	tagExpired: string;
	selfName: string;
	chipRequested: string;
	chipBalance: string;
	chipCustom: string;
	chipRevoke: string;
	chipRevokeAccess: string;
	chipGrantAll: string;
	valueRevoke: string;
	valueUnlimited: string;
	valueAllNfts: string;
	unlimitedDisabled: string;
	/** What a typed cap that is not a number gets told. */
	invalidAmount: string;
	choosePrompt: string;
	summarySend: string;
	summarySendFrom: string;
	summarySwap: string;
	summaryReceive: string;
	summaryApprove: string;
	summaryApproveUnlimited: string;
	summaryRevoke: string;
	summaryTransferNft: string;
	summaryApproveNft: string;
	summaryPermit: string;
	summaryPermitUnlimited: string;
	summaryDeploy: string;
	summaryBatch: string;
	summarySafe: string;
	summaryBestEffort: string;
	summaryVerifiedAbi: string;
	summaryDrain: string;
	warnUnlimited: string;
	warnBlindDecode: string;
	warnSelectorNotListed: string;
	warnExpired: string;
	warnWillFail: string;
	warnHexMessage: string;
	warnBlindTyped: string;
	warnEthSign: string;
	bodyEthSign: string;
	warnSiweMismatch: string;
	okSiwe: string;
	warnTokenToContract: string;
	warnUnverifiedAmount: string;
	warnApproveAll: string;
	warnPermitCantCap: string;
	warnBestEffort: string;
	warnVerifiedAbi: string;
	warnSimUnavailable: string;
	warnDrain: string;
	okSelfTransfer: string;
	okNoNetworkFee: string;
	balancesTitle: string;
	balancesMatchHero: string;
	balancesBlindSimulated: string;
	balancesBestEffort: string;
	feeLabel: string;
	feeTokenTitle: string;
	feeEstimated: string;
	feeBalance: string;
	techFunction: string;
	techParam: string;
	techRawUnits: string;
	techRawData: string;
	techSimResult: string;
	techIdentityToken: string;
	techIdentityRecipient: string;
	copyValue: string;
	viewOnExplorer: string;
	byteSize: string;
	safeInnerCall: string;
	batchStep: string;
	expiredValue: string;
	sentToTokenContract: string;
}
