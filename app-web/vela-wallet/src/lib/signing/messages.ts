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
import type { SettingsMessages } from '$lib/settings/messages';
import type { AgentAdviceMessages } from '$lib/agent/advisor';

/**
 * Every quiet line the core can name for the verdict's place
 * (`sim_outcome::KEY_NO_CHANGE` — `SimVerdict.no_change_key`).
 * `live.test.ts` holds the list to the core's source.
 */
export const SIM_SAID_KEYS = ['componentsUi.signing.simResultNoChange'] as const;

/**
 * The caution the core names for the verdict's place when nothing could be
 * checked (PR 3, `sim_outcome::KEY_UNAVAILABLE`): "Vela couldn't check what
 * this transaction does…". The core hands this key two ways — on the
 * request's own verdict when the node does not offer the simulation or no
 * node answered (`SimVerdict.notice_key`, with `notice_risk: 'caution'`), and
 * on the sign view when the simulation's deadline passed with no verdict at
 * all (`SignView.sim_waited_out_key`). Its words are
 * `SigningMessages.warnSimUnavailable`, resolved from this key.
 * `live.test.ts` holds it to the core's source.
 */
export const SIM_COULD_NOT_CHECK_KEY = 'componentsUi.signing.simUnavailableWarning';

/**
 * The danger the core names for a call the chain says fails
 * (`sim_outcome::KEY_WILL_FAIL_REASON` with the cleaned `{{reason}}`,
 * `KEY_WILL_FAIL` without one) — `SimVerdict.notice_key` under
 * `notice_risk: 'danger'`. Their words are `SigningMessages.warnWillFailReason`
 * and `warnWillFail`. `verdict.test.ts` holds both to the core's source.
 */
export const SIM_WILL_FAIL_REASON_KEY = 'componentsUi.signing.simWillFailReason';
export const SIM_WILL_FAIL_KEY = 'componentsUi.signing.simWillFail';

/**
 * R22 (review 1-2): the cautions the core names for a simulation nobody
 * independent confirmed (`sim_outcome::KEY_UNCONFIRMED`, `KEY_DISAGREED` —
 * `SimVerdict.notice_key` under `kind: 'unconfirmed'`), and the quiet line it
 * names when the person's own node answered alone (`KEY_OWN_NODE`,
 * `SimVerdict.source_key`). `verdict.test.ts` replays the core's vectors.
 */
export const SIM_UNCONFIRMED_KEY = 'componentsUi.signing.simUnconfirmed';
export const SIM_DISAGREED_KEY = 'componentsUi.signing.simDisagreed';
export const SIM_OWN_NODE_KEY = 'componentsUi.signing.simOwnNode';

/**
 * Spec 102 (D4): the hand-off card's words — the gallery's, and live wherever
 * a signature goes to the account's trusted signing page: every signing sheet
 * (`SigningMessages.handoff`, the page everywhere since 2026-10-30) and a
 * multisig member's approval (`MultisigMessages.handoff`). The apps draw the
 * same card from the same keys.
 */
export interface HandoffMessages {
	/** "Review and sign on a trusted signing page". */
	title: string;
	/**
	 * A key ceremony's card title, by the corpus key the core names
	 * (`trusted_signer::ceremony::Ceremony::title_key`): "Create a key on the
	 * signing page", "Sign in on the signing page", "Confirm with the key on
	 * the signing page". Absent where no ceremony is drawn.
	 */
	ceremonies?: Record<string, string>;
	/**
	 * The key row's label, by the corpus key a `KeyLabel` names (`label_key`):
	 * 「确认方式」 / "Confirm with" — and 「新钥匙存在」 / "New key on" while a
	 * ceremony makes the key. The signing page's own row labels.
	 */
	keyLabels: Record<string, string>;
	/** Open — the sheet's "Continue to signing page". */
	open: string;
	/** After Open: "Waiting for the signing page…", where to look, and the way back. */
	waiting: string;
	waitingHint: string;
	reopen: string;
	/** The official page's name. */
	official: string;
	integrity: SettingsMessages['integrity'];
	/**
	 * The three places a key lives, by the corpus key a `KeyLabel` names
	 * (`place_key`) — the key row's value for a key that carries the wallet's
	 * name (D-17).
	 */
	places: Record<string, string>;
	/**
	 * Round 7: why the page did not sign, when the person did not decline
	 * there — it is not answering (r7-2), its answer is not this request's
	 * (r7-7), its own rules refused it.
	 */
	unanswered: string;
	mismatch: string;
	refused: string;
}

export const CERTAIN_KEYS = [
	'readByVela',
	'readByVelaSignature',
	'describedBuiltIn',
	'describedBuiltInPlain',
	'describedFetched',
	'describedFetchedPlain',
	'shapeOnly',
	'selectorName',
	'siteClaim',
	'labelFrom',
	'sourceBuiltIn',
	'sourceFetched',
	'sourceSelectorDb',
	'sourceShape',
	'sourceList',
	'contractNamed',
	'contractUnnamed',
	'contractNoCode',
	'contradiction',
	'noCode',
	'elsewhere',
	'dustMinimum',
	'tokenByContract',
	'thisAccount',
	'allowsUpTo',
	'allowsMore',
	'approveAllYour',
	'signAnyway',
	'chipGrantAll',
	'chooseGrant',
	'labelSource',
	'byVela'
] as const;
export type CertainKey = (typeof CERTAIN_KEYS)[number];

export interface SigningMessages {
	/**
	 * The hand-off card, for an account that reviews and signs on its trusted
	 * page (owner, 2026-10-30: the page, everywhere). Absent in fixtures that
	 * draw no hand-off.
	 */
	handoff?: HandoffMessages;
	panelTitle: string;
	signingAccount: string;
	advancedToggle: string;
	close: string;
	/** The cue to a verdict that landed out of sight: "New check result below ↓". */
	verdictBelow: string;
	verdictAbove: string;
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
	/**
	 * §31 (templates — '{{function}}', '{{safe}}'): the request would have made
	 * this account act for a Safe it owns — a multisig it is a member of —
	 * or whether it owns that Safe could not be read (`…Unchecked`).
	 */
	parentSafeBlocked: string;
	parentSafeUnchecked: string;
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
	/**
	 * cs-11: what is drawn carries hidden or direction-changing characters,
	 * each shown as its `⟨U+…⟩` marker (`ClearSigningView.has_invisible_chars`).
	 */
	warnHiddenChars: string;
	/** cs-10: a folded run of empty lines in a message (`{{lines}}`, how many). */
	messageBlankLines: string;
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
	/**
	 * The simulation could not check — the sentence of
	 * {@link SIM_COULD_NOT_CHECK_KEY}. Drawn live (PR 3) when the core says
	 * so, by that key: the request's verdict is a not-checked answer, or the
	 * simulation's deadline passed with no verdict.
	 */
	warnSimUnavailable: string;
	/**
	 * r10 follow-up: the one line on a moved site's next request (template —
	 * '{{network}}'), `connect.browser.spaceMoved`: a network-space switch
	 * moved the site to another network since it last asked
	 * (`SignRequestView.space_note`).
	 */
	spaceMoved: string;
	warnDrain: string;
	okSelfTransfer: string;
	okNoNetworkFee: string;
	balancesTitle: string;
	/**
	 * What the verdict's place says while the simulation is out — the plain
	 * "Checking…" the phones and the desktop say there
	 * (`componentsUi.funding.checking`).
	 */
	simChecking: string;
	/**
	 * dapp-15: the could-not-check line of a node nobody reached is asked
	 * again — "Check again" on it (`componentsUi.signing.simRecheck`), and
	 * "Checking again…" over it while the new check is out, the old line
	 * marked stale (`componentsUi.signing.simRechecking`).
	 */
	simRecheck: string;
	simRechecking: string;
	/**
	 * R22 (review 1-2): the sheet's simulation nobody independent confirmed
	 * — one node answered (`componentsUi.signing.simUnconfirmed`), or two
	 * independent nodes disagreed (`…simDisagreed`): the core's
	 * `SimVerdict.notice_key` under `kind: 'unconfirmed'`.
	 */
	simUnconfirmed: string;
	simDisagreed: string;
	/**
	 * R22: "Checked by your own node only" — the quiet line under the balance
	 * changes when "Only use my node" answered alone (`SimVerdict.source_key`).
	 */
	simOwnNode: string;
	/**
	 * R22: under an unconfirmed verdict, where a trusted node is set
	 * (`componentsUi.signing.ownNodeHint`), and its action into this network's
	 * RPC settings (`home.nodeFallbackOpen`, "Network settings").
	 */
	ownNodeHint: string;
	ownNodeAction: string;
	/**
	 * R22 (review 1-3): a held confirm — "Hold to confirm" beside it and as its
	 * hint (`componentsUi.signing.holdToConfirm`), and the assistive path's
	 * armed words, "Press again to confirm" (`…confirmAgain`).
	 */
	holdToConfirm: string;
	confirmAgain: string;
	/**
	 * A balance row's label for a token nobody vouches for
	 * (`componentsUi.signing.balanceUnverifiedToken`): the row says this and
	 * which way it moves, and never a figure.
	 */
	balanceUnverifiedToken: string;
	/**
	 * §41 (P2-3): an NFT's row label (`componentsUi.signing.balanceNft`,
	 * `{{id}}`), a grant's (`balanceApproval` / `balanceApprovalAll`,
	 * `{{spender}}`) and a log naming the account that Vela could not read
	 * (`balanceUnread`).
	 */
	balanceNft: string;
	balanceApproval: string;
	balanceApprovalAll: string;
	balanceUnread: string;
	/**
	 * The verdict's quiet line by the corpus key the core names for it

	 * (`SimVerdict.no_change_key` — {@link SIM_SAID_KEYS}): "No asset changes"
	 * when the sheet's simulation was a check and nothing of the person's
	 * moves. The shell never picks the sentence, or the case; it only looks
	 * the core's key up here.
	 */
	simSaid: Readonly<Record<string, string>>;
	balancesMatchHero: string;
	balancesBlindSimulated: string;
	balancesBestEffort: string;
	feeLabel: string;
	/** The fee row while the quote is in flight. */
	feeEstimating: string;
	/** The fee row after the quote failed; tapping it asks again. */
	feeRetry: string;
	/**
	 * PR 2 polish: the fee row after the relay answered that the operation
	 * would fail with the coin in force — a tap opens the fee coins
	 * (`componentsUi.gas.payWithAnotherCoin`, `FeeFailureView.figure_key`).
	 */
	feePayWithAnotherCoin: string;
	feeTokenTitle: string;
	/** Issue 262: the selected coin cannot pay — the send form's issue-211 sentence ({{sym}}). */
	feeShort: string;
	/**
	 * r10-3: the coin in force covers the fee alone, not once the request has
	 * taken what it moves of the same balance (the core's
	 * `short_with_amount`) — the send form's amount + gas sentence ({{sym}}).
	 */
	feeShortWithAmount: string;
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
	 * Spec 099 R7: the line under a shut confirm, by the corpus key the core's
	 * `confirm_state` names (`componentsUi.signing.confirmBlock.*`).
	 */
	confirmBlock: Record<string, string>;
	/**
	 * §37 (review 1-12): the guard's danger line under an approval, by the
	 * corpus key the core names (`GuardView.unlimited_warning_key`) —
	 * "Unlimited — …" or "Over 100× your balance — …".
	 */
	guardWarnings: Record<string, string>;
	/**
	 * §37 (review 1-11): a sign-in the wallet refuses — the title over it,
	 * and its sentence by the core's key (`SignBlockedView.sign_in.key`,
	 * `componentsUi.signing.signIn.*`), filled with the core's named params.
	 */
	signInRefusedTitle: string;
	signIn: Record<string, string>;
	/**
	 * signer-11: the line by the confirm when the last passkey prompt signed
	 * nothing (`SignView.passkey_note`) — closed (or no passkey here), or out
	 * of time.
	 */
	passkeyNote: { cancelled: string; timed_out: string };
	/**
	 * §39 (r22-3 / A1): the line by the confirm when the fee went up since the
	 * press (`SignErrorKind::FeeRose`, `componentsUi.signing.feeRose`): nothing
	 * was signed; the next confirm pays the figure now drawn.
	 */
	feeRose: string;
	/**
	 * Spec 099 R8: how the passkey failed, by the core's reason key
	 * (`componentsUi.browserStatus.reason.signer*`) — the failed status says
	 * it in place of the generic hint.
	 */
	signerReasons: Record<string, string>;
	/**
	 * Spec 102 (P2b-W1): why this account cannot sign here — the core's
	 * `VenueBlock`, the same words Settings draws under "Where you review and
	 * sign". The failed status says it in place of the generic hint.
	 */
	venueBlock: SettingsMessages['venue']['blocked'];
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
	/** The raw data's copy button, its "Copied", its "Show all", its JSON-in-a-string note. */
	techRawCopy: string;
	techRawCopied: string;
	techRawShowAll: string;
	techRawJsonText: string;
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
	/**
	 * What is CERTAIN and what is INTERPRETED (clear-signing audit cs-1…cs-19,
	 * `componentsUi.signing.certain`): the provenance line under the intent,
	 * each row's source, the contract on the card's face, and the danger lines
	 * the certain decode raises against a description.
	 */
	certain: Record<CertainKey, string>;
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
		/**
		 * A refusal told by its reason (correctness batch item 3): the tracker
		 * entry's `refusal_key` → its sentence — the fee words only for a fee
		 * refusal, "another transaction from this account went first" for a
		 * spent nonce, else the plain refusal.
		 */
		refusals: Readonly<Record<string, string>>;
	};
	/**
	 * Spec 079 (F11 — "可信签名器签完后，回到签名提示框，似乎没有任何提示"): what
	 * the sheet says once the person has approved, in the SEND receipt's words,
	 * as Android's signing receipt says them — never a greyed confirm.
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
		/**
		 * Round 7 (S4): `componentsUi.signing.trustedSignerWaiting` — the
		 * account's trusted signing page has the request, and the person signs
		 * there; no passkey prompt is up here ("Waiting for biometric…" said
		 * otherwise).
		 */
		pageSigning: string;
		/** `send.txErrorGeneric` — the submission failed; funds are safe. */
		failedHint: string;
		/** Spec 096 F8: `send.txRetryBtn` — a failure that sent nothing, tried again. */
		retry: string;
		/**
		 * PR 2 polish: `componentsUi.signing.notSentTitle` — "Not sent yet", in
		 * place of "Failed" when the relay turned the operation back because
		 * the account's previous one still holds the nonce
		 * (`SignView.failure_not_sent`). Its sentence is the refusal key's
		 * (`componentsUi.signing.notSentBody`, in `receipt.refusals`).
		 */
		notSentTitle: string;
	};
	viewOnExplorer: string;
	byteSize: string;
	safeInnerCall: string;
	batchStep: string;
	/** 089 S1: the native coin a whole batch moves (`send.splitTotalLabel`). */
	labelTotal: string;
	expiredValue: string;
	sentToTokenContract: string;
	/**
	 * The person's AI agent's opinion, in its own card under Vela's own lines
	 * (spec 103, `vela_advise`): never one of the sheet's words.
	 */
	agentAdvice: AgentAdviceMessages;
}
