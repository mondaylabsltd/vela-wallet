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

import type { SpeedWords } from '$lib/flows/speed-control';

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
	/**
	 * Spec 081: the request would have changed who controls the account, so the
	 * wallet refused it. `…LegBody` names the step inside a batch; `…SafeTx` is
	 * the typed-data case, which names no function.
	 */
	selfCallBlockedTitle: string;
	selfCallBlockedBody: string;
	selfCallBlockedLegBody: string;
	selfCallBlockedSafeTx: string;
	/** What a typed cap that is not a number gets told. */
	invalidAmount: string;
	/**
	 * Spec 096 F7: the sheet while the core is still reading the request — a
	 * neutral "Loading…", never the cap editor's "set a finite amount".
	 */
	loading: string;
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
	/** Spec 082 RJ19: the relay's estimate says it reverts, and why (`{{reason}}`). */
	warnWillFailReason: string;
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
	/** The core's `partial`: the reading is incomplete (spec 097 N1). */
	warnPartial: string;
	warnVerifiedAbi: string;
	/**
	 * Spec 081 FR-008: this description came from the descriptor service and
	 * nobody authenticated it. Optional only until its corpus key lands with
	 * the feature's i18n pass — the resolver hands `undefined` while the
	 * catalogs lack it, and the sheet says one thing less rather than drawing
	 * a raw key path.
	 */
	warnDescriptorFetched: string;
	/**
	 * Spec 096 F5: the call signs an order whose amounts live off chain (a CoW
	 * pre-signature) — the core's `terms_off_chain`.
	 */
	warnOrderTerms: string;
	warnSimUnavailable: string;
	warnDrain: string;
	okSelfTransfer: string;
	okNoNetworkFee: string;
	balancesTitle: string;
	balancesMatchHero: string;
	balancesBlindSimulated: string;
	balancesBestEffort: string;
	feeLabel: string;
	/** The fee row while the quote is in flight. */
	feeEstimating: string;
	/** The fee row after the quote failed; tapping it asks again. */
	feeRetry: string;
	feeTokenTitle: string;
	/** Issue 262: the selected coin cannot pay — the send form's issue-211 sentence ({{sym}}). */
	feeShort: string;
	/**
	 * Spec 096 F2: the coin in force is one the transaction itself may spend
	 * (the core's `spent_by_operation`), so too little may be left for the fee
	 * ({{sym}}).
	 */
	feeCoinSpent: string;
	/**
	 * Issue 408: under a greyed coin, why it cannot pay — `{{need}}` and
	 * `{{have}}` are the core's `FeeShortfall` words, never re-formatted here.
	 */
	feeRowShort: string;
	/**
	 * Issue 408: under the fee, when not one coin on offer can pay it (the
	 * core's `no_coin_pays`) — said instead of naming the coin in force.
	 */
	feeNoCoinPays: string;
	/** Spec 079: the send form's refresh control (`send.feeRefresh`). */
	feeRefresh: string;
	/** Spec 079: the send form's stale note (`send.feeStale`). */
	feeStale: string;
	/**
	 * Spec 082 RJ13: the reason line under a failed fee, by the corpus key the
	 * CORE picks (`feeFailureReasonKey`): the relay out of reach, a rate-limited
	 * chain node, a chain node out of reach (`{{chain}}`). The shell never picks
	 * these words itself; it only looks the core's key up here.
	 */
	feeReasons: Record<string, string>;
	/**
	 * Spec 099 R7: the line under a shut slide, by the corpus key the core's
	 * `confirm_state` names (`componentsUi.signing.confirmBlock.*`).
	 */
	confirmBlock: Record<string, string>;
	/**
	 * Spec 099 R8: how the passkey failed, by the core's reason key
	 * (`componentsUi.browserStatus.reason.signer*`) — the failed status says
	 * it in place of the generic hint.
	 */
	signerReasons: Record<string, string>;
	/**
	 * The speed control under the fee row (spec 069) — the send form's words,
	 * so the two surfaces name a speed identically.
	 */
	speed: SpeedWords;
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
	/**
	 * The words the core names on a clear-signing result (`ClearTerm` → its
	 * word here): descriptor intents, field labels, the threshold's
	 * "Unlimited". A result's text is English; `localizedTerms` in `live.ts`
	 * swaps in these. Keyed by term (`signing/terms.ts` lists them).
	 */
	terms: Record<string, string>;
	/**
	 * Spec 077: the landing a submitted transaction shows, in the SEND
	 * receipt's own words. Borrowed rather than written again so the two
	 * surfaces cannot drift into describing the same moment differently —
	 * which is what the owner asked for ("UI 要保持一致性").
	 *
	 * On `SigningMessages` rather than any one surface's, because the LANDING
	 * belongs to the sheet: every surface that mounts it submits through the
	 * same machine, and the first version — which put it on the request
	 * window's copy alone — left Settings' backup to Ethereum without one.
	 */
	receipt: {
		confirming: string;
		confirmingHint: string;
		submitted: string;
		confirmed: string;
		failed: string;
		failedHint: string;
		opHashLabel: string;
		txHashLabel: string;
		explorer: string;
		done: string;
		/**
		 * Spec 079: an op past its wait window, an op past 24 h, and a
		 * message signed — the words Android's aftercare says, no new keys.
		 */
		stillConfirming: string;
		unknownOutcome: string;
		/** `send.txRelayFunding`: the relay is topping up its gas before it sends. */
		relayFunding: string;
		/** `send.txRelaySending`: the relay has it and has not sent it yet (099 R6). */
		relaySending: string;
		signed: string;
		/**
		 * Spec 082 RA10: the relay's reply was lost — `componentsUi.signing.
		 * maybeSent` ("It may have been sent. Vela keeps checking — don't send
		 * it again."), never "failed — try again".
		 */
		maybeSent: string;
		/** `send.txCloseBackground` — the one way out of a may-have-been-sent op. */
		closeBackground: string;
		/** `send.txErrorGeneric` — not sent: nothing left, funds are safe. */
		notSentHint: string;
		/** Spec 082 G56: `send.txSubmitting` — the title while it may have been sent. */
		submitting: string;
		/**
		 * Spec 082 RJ3: `componentsUi.signing.refused` — the network refused it,
		 * nothing was sent; no Retry words.
		 */
		refused: string;
	};
	/**
	 * Spec 079 (F11 — "可信签名器签完后，回到签名提示框，似乎没有任何提示"): what
	 * the sheet says once the person has approved, in the SEND receipt's words,
	 * as Android's signing receipt says them — never a greyed slide.
	 */
	status: {
		/**
		 * `send.txPreparing` — approved, and the passkey not asked yet: the
		 * funding check, the nonce and the estimate (083 H3, spec 082 RA9).
		 */
		preparing: string;
		/** `send.txSigning` — the passkey prompt is up. */
		signing: string;
		/** `send.txSubmitting` */
		submitting: string;
		/** `send.txBackgroundHint` — closing keeps it running. */
		backgroundHint: string;
		/** `componentsUi.signing.signing` — a message never "submits". */
		messageSigning: string;
		/** `send.txErrorGeneric` — the submission failed; funds are safe. */
		failedHint: string;
		/** Spec 096 F8: `send.txRetryBtn` — a failure that sent nothing, tried again. */
		retry: string;
	};
	viewOnExplorer: string;
	byteSize: string;
	safeInnerCall: string;
	batchStep: string;
	/** 089 S1: the native coin a whole batch moves (`send.splitTotalLabel`). */
	labelTotal: string;
	expiredValue: string;
	sentToTokenContract: string;
}
