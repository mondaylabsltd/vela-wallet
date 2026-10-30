/**
 * The multisig screens' copy (`home.multisig` in the vela-core corpus),
 * resolved at build time like every other page's (`engine.server.ts`).
 * Templates keep their `{{name}}` slots; `fillTemplate` fills them.
 */
import type { PluralCopy } from '$lib/i18n/plural';

export interface MultisigMessages {
	/**
	 * 2026-10-29: the hand-off card for a member who approves on a trusted
	 * signing page — the apps' card, in the corpus' words
	 * (`resolveHandoffMessages`). Absent in fixtures that draw no hand-off.
	 */
	handoff?: import('$lib/signing/messages').HandoffMessages;
	switcher: {
		section: string;
		create: string;
		add: string;
		rowName: string;
		readOnly: string;
		notShared: string;
		removeBody: string;
		addAccount: string;
		createAccountNote: string;
		signInNote: string;
		createMultisigNote: string;
		addMultisigNote: string;
		lastMember: string;
	};
	panel: {
		title: string;
		/** r4D-13: the rule of a multisig of one — never "Needs 1 of 1 members". */
		titleSolo: string;
		/** r4D-13, r5reg-10: the header chip, as a requirement ("Needs 2 of 3", "需 3 位中的 2 位"). */
		chip: string;
		/** …and for a multisig of one. */
		chipSolo: string;
		members: string;
		thisDevice: string;
		readOnly: string;
		pending: string;
		stale: string;
		notShared: string;
		invite: string;
		/** r4b-11: the list kept from an earlier page, shown while it is checked (`{{time}}`). */
		lastKnown: string;
		/** r4b-11: reading, nothing to show yet. */
		checking: string;
		/** r4b-2: the history on `{{network}}` could not be read. */
		historyUnreadable: string;
	};
	status: {
		ready: string;
		yourTurn: string;
		waiting: string;
		queued: string;
		progress: string;
		/** Signed here, not in the mailbox yet. */
		unsent: string;
		/** r4b-7: its execution was sent from here and may still land. */
		executing: string;
		/** r4b-6: another proposal waits at its number. */
		conflict: string;
		/** r4-5: a ready one that cannot run now (`{{reason}}`). */
		cannotRun: string;
		/** …because no account on this device can pay the fee. */
		nobodyPays: string;
	};
	action: {
		send: string;
		batch: string;
		cancel: string;
		addOwner: string;
		removeOwner: string;
		swapOwner: string;
		changeThreshold: string;
		unsupported: string;
		to: string;
		network: string;
		number: string;
		/** The number itself, in the locale's form (`{{nonce}}`): "#0", "Nr. 0". */
		numberValue: string;
		call: string;
		callWithValue: string;
		batchCalls: string;
		/** A change of its shared name (`{{name}}`). */
		rename: string;
	};
	/** A member change read in full (the core's `memberChanges`). */
	change: {
		/** `{{member}}`: a name and short address, or the short address. */
		add: string;
		remove: string;
		/** `{{old}}`, `{{new}}`. */
		replace: string;
		threshold: string;
		/** `{{before}}`, `{{after}}`: each "2 of 3" (`create.thresholdValue`). */
		rule: string;
		/** `{{network}}`: where it changes who controls the multisig. */
		caution: string;
		/** The same, short, on its Home row. */
		rowCaution: string;
		/** It does not fit the members it would meet (`{{network}}`): it would revert. */
		doesNotFit: string;
	};
	/** A token's provenance (`token_provenance`): unverified, a lookalike. */
	token: {
		unverified: string;
		/** `{{symbol}}`: the vouched symbol it reads as. */
		lookalike: string;
		contract: string;
	};
	proposal: {
		title: string;
		approvals: string;
		approved: string;
		waiting: string;
		approve: string;
		approveAndExecute: string;
		execute: string;
		as: string;
		/** The safeTxHash's label (vis-11: it was the bare identifier). */
		hash: string;
		feeNote: string;
		invite: string;
		inviteHint: string;
		copyLink: string;
		copied: string;
		share: string;
		reject: string;
		/** r4D-13: the person's own proposal, on its review before its passkey: withdrawn, not declined. */
		withdraw: string;
		rejectHint: string;
		cancelPending: string;
		executing: string;
		executed: string;
		explorer: string;
		refused: string;
		doneNote: string;
		queuedNote: string;
		notMember: string;
		failed: string;
		noExpiry: string;
		cannotPay: string;
		notLanded: string;
		mailboxDown: string;
		notSent: string;
		siteAsks: string;
		siteWaits: string;
		/** How many approvals an `eth_sendTransaction` still waits for (`{{missing}}`). */
		stillNeeded: string;
		/** Its site was told it was proposed (the request's limit came first). */
		siteTold: string;
		data: string;
		notFound: string;
		opening: string;
		changesMembers: string;
		siteFollows: string;
		voided: string;
		/** ms-9: replaced by the cancellation proposed at its number (`{{nonce}}`). */
		voidedNote: string;
		/** ms-9: replaced by another transaction at its number (`{{nonce}}`). */
		voidedByOther: string;
		notInMailbox: string;
		badLink: string;
		/** ms-21: a link's transaction on a device with no member of its multisig. */
		previewNote: string;
		/** ms-21: add the multisig to this device, only to follow it. */
		watch: string;
		/** ms-21: it was added, on the person's tap; the account shown is unchanged. */
		watching: string;
		/** A rename's review: the network it runs on, the one keeping its record (`{{network}}`). */
		renameRuns: string;
		/** A rename where it is not deployed: it cannot be executed there (`{{network}}`). */
		renameUndeployed: string;
		/** The first execution on a network deploys it with the original members, said when others changed. */
		startsOriginal: string;
		/** (`{{m}}`, `{{n}}`, `{{networks}}`: where its members are others now.) */
		startsOriginalNote: string;
		/** Approvals by accounts that are not members on the network now (`{{network}}`, `{{names}}`). */
		notCounted: string;
		/** A proposal on a network this device lacks, its chain index name known (`{{name}}`, `{{chainId}}`). */
		missingNetworkNamed: string;
		missingNetwork: string;
		missingNetworkBody: string;
		/** How many wait there (`{{n}}`: a number, not a plural). */
		missingCount: string;
		addNetwork: string;
		/** A signed approval waiting on a busy service (`{{seconds}}`). */
		outboxBusy: string;
		/** …on a service out of reach (`{{seconds}}`). */
		outboxAway: string;
		/** Not sent after a while: kept, Send again. */
		outboxNotSent: string;
		/** The full hash's copy button. */
		copy: string;
		/** An executed transaction's sender (`{{name}}`). */
		executedBy: string;
		/** The approvals its receipt says counted (`{{approved}}`, `{{threshold}}`). */
		countedThen: string;
		/** Executed, and which approvals counted is not on record here. */
		approvalsSigned: string;
		outboxSendAgain: string;
		/** r4b-1: another member's execution ran first; nothing was sent from here. */
		lostRace: string;
		/** …and this device's attempt landed too, paying its fee. */
		lostRacePaid: string;
		/** r4b-1: another transaction at `{{nonce}}` ran first. */
		replacedFirst: string;
		replacedFirstPaid: string;
		/** This device's execution landed and did not execute it. */
		execFailed: string;
		/** r4b-3: `{{network}}` did not answer. */
		cantCheck: string;
		/** Its number is used; `{{network}}` cannot say by what now. */
		cantCheckUsed: string;
		/** r4b-6: only one of the proposals at `{{nonce}}` can run. */
		conflict: string;
		/** r4a-16: the numbers it waits behind (`{{nonces}}`: "#3, #4"). */
		queuedNoteMany: string;
		/** r4D-19: open the transaction at `{{nonce}}`. */
		openNumber: string;
		/** r4b-12: why it did not go, in the corpus' words. */
		reason: {
			passkeyClosed: string;
			passkeyTimedOut: string;
			passkeyMissing: string;
			passkeyFailed: string;
			serviceKept: string;
			serviceRefused: string;
			offline: string;
			/** `{{network}}` */
			chainUnreachable: string;
			unknown: string;
		};
		/** r4-5: the execution would fail now (`{{reason}}`, one of the `why*`). */
		wouldFail: string;
		/** The multisig holds less than it spends (`{{have}}`, `{{need}}`, with the coin). */
		whyShort: string;
		/** It reverts; the network's reason (`{{reason}}`). */
		whyReverts: string;
		whyRevertsPlain: string;
		/** The fee could not be priced: the reason an execution stopped before its passkey. */
		unpricedReason: string;
		/** Round 5: the look did not answer in time — whether it would run is not known. */
		uncheckedReason: string;
		/** r4-7: the fee before the passkey (`{{fee}}`, `{{name}}`: who pays). */
		feeLine: string;
		feeChecking: string;
		feeUnpriced: string;
		feeUnchecked: string;
		feeRetry: string;
		/** What of the fee is one-time setup (`{{network}}`, `{{amount}}`). */
		feeSetup: string;
		feeSetupPlain: string;
		setupMultisig: string;
		/** A member's account deployed for its approval (`{{name}}`). */
		setupMember: string;
		setupExecutor: string;
		/** r4D-1: who cannot pay, and who here can (`{{name}}`, `{{other}}`). */
		cantPayOther: string;
		/** r4-10: the picker's mark on an account that cannot pay. */
		pickerCantPay: string;
		/** r4-11: approved; only executing could not run (`{{reason}}`). */
		approvedNotExecuted: string;
		/** r4D-6: a member's approval once enough have approved. */
		notNeeded: string;
		sendLink: string;
		/** r4-9: Vela members elsewhere approve from the link (`{{names}}`). */
		approvesInVela: string;
		/** r7: smart contract members still to approve (`{{names}}`): from their own app, not collected yet. */
		contractMembers: string;
		/** r4D-14: the way back from Reject. */
		rejectCancel: string;
		/** r4C-10: no member here, and none can sign through an outside wallet. */
		noOutsideSigner: string;
	};
	send: {
		propose: string;
		/** r4C-10: proposing on a device with no member — nobody here approves. */
		proposeOnly: string;
		invalidAddress: string;
		/** A member's mixed case is not its EIP-55 checksum (send-2). */
		invalidChecksum: string;
	};
	create: {
		title: string;
		intro: string;
		members: string;
		yourAccounts: string;
		byAddress: string;
		addressPlaceholder: string;
		addMember: string;
		order: string;
		threshold: string;
		thresholdValue: string;
		thresholdAll: string;
		thresholdOne: string;
		/** Plural (vis-11: "1 member(s)"): the count is known where it is drawn. */
		thresholdSpare: PluralCopy;
		address: string;
		sameEverywhere: string;
		network: string;
		/** r4D-13: what the first network is for, under its select. */
		networkHint: string;
		creator: string;
		create: string;
		next: string;
		back: string;
		needLocal: string;
		duplicate: string;
		notMember: string;
		created: string;
		createdBody: string;
		/** r5reg-18: a multisig of one has no other members to send the link to. */
		createdBodySolo: string;
		done: string;
		name: string;
		namePlaceholder: string;
		nameHint: string;
		nameInvalid: string;
		fewer: string;
		more: string;
		copyAddress: string;
		/** Every network the person has lacks the Safe contracts a multisig needs. */
		noSafeNetworks: string;
		/** r4a-14: the name's size (`{{bytes}}`, `{{limit}}`). */
		nameBytes: string;
		/** r4a-14: over the registry's limit (`{{bytes}}`, `{{limit}}`). */
		nameTooLong: string;
		/** r4a-14: Next with no name. */
		nameNeeded: string;
		/** r4D-5: a member's own label, typed with its address. */
		memberName: string;
		memberNamePlaceholder: string;
		/** r4D-5: an ENS name's address (`{{name}}`), shown in full under it. */
		ensResolved: string;
		ensNotFound: string;
		ensUnavailable: string;
		ensUnsupported: string;
		/** r4a-14: members from the address book. */
		fromContacts: string;
		/** r4a-14: one member — what a 1 of 1 multisig is. */
		solo: string;
		/** r4a-14: nothing says which; the person picks. */
		networkNeeded: string;
		/** r4a-13: kept on this device, its record on its way. */
		savedSharing: string;
		/** r4a-12: kept on this device, shared once the service is reachable. */
		savedLater: string;
		shared: string;
		/** r5reg-18: …said for a multisig of one. */
		sharedSolo: string;
		/** The service refused the record (`{{reason}}`). */
		refused: string;
		copyInvite: string;
		/** r4D-8: the first steps on the home of a multisig created here. */
		firstRunTitle: string;
		firstRunInvite: string;
		firstRunInviteNote: string;
		firstRunFund: string;
		/** `{{network}}` */
		firstRunFundNote: string;
		/** The first transfer out sets it up there (`{{network}}`). */
		firstRunCost: string;
		firstRunDismiss: string;
	};
	/** r4D-16: the invite sheet. */
	invite: {
		body: string;
		/** The text a share sheet carries with the link. */
		shareText: string;
		qrLabel: string;
		/** Members who sign with another wallet add nothing. */
		outsideHint: string;
	};
	/** r4D-7: what waits on this device, and the rule in force. */
	presence: {
		needsYou: PluralCopy;
		ready: PluralCopy;
		/** r5reg-6: ready, but no member here can pay its fee. */
		waitingPayer: PluralCopy;
		ruleDiffers: string;
		/** The switcher button's dot, in words. */
		othersWaiting: string;
	};
	external: {
		pageTitle: string;
		looking: string;
		noWallet: string;
		noWalletBody: string;
		copyLink: string;
		copied: string;
		chooseWallet: string;
		askConnect: string;
		askNote: string;
		connect: string;
		decline: string;
		keepOpen: string;
		waitingVela: string;
		noSession: string;
		requests: string;
		signTx: string;
		sendTx: string;
		switchChain: string;
		disconnect: string;
		signWithWallet: string;
		sheetTitle: string;
		scanBody: string;
		resumeBody: string;
		waiting: string;
		connecting: string;
		wrongAccount: string;
		switching: string;
		confirming: string;
		declined: string;
		badSignature: string;
		signedBy: string;
		unpair: string;
		ownerWallet: string;
		retry: string;
		pageAway: string;
		connectDeclined: string;
		declinedHere: string;
		otherChain: string;
		/** ms-14: the account the wallet connects this page with (`{{account}}`). */
		connectingAs: string;
		/** ms-14: the members Vela asked for, from the link (`{{accounts}}`). */
		expectedMember: string;
		/** ms-14: the wallet's account is none of them — refused, with what to do. */
		notExpectedMember: string;
		// r4: outside owners — the review, waits, outcomes, the relay, approving from the link.
		connectingVela: string;
		loadingTx: string;
		reviewTitle: string;
		multisigLabel: string;
		doesLabel: string;
		tokenNative: string;
		tokenContract: string;
		queuedTx: string;
		refusedKind: string;
		hashesTitle: string;
		hashesNote: string;
		safeTxHash: string;
		domainHash: string;
		messageHash: string;
		copy: string;
		notSafeTx: string;
		signInWallet: string;
		sendInWallet: string;
		inWallet: string;
		stillWaiting: string;
		stopWaiting: string;
		askAgain: string;
		stoppedWaiting: string;
		signedClose: string;
		signedTypedForm: string;
		signedHashForm: string;
		signedOnChain: string;
		withdrawnInWallet: string;
		typedUnsupported: string;
		ethSignOffer: string;
		signHash: string;
		approveOnChainOffer: string;
		approveOnChain: string;
		velaOffline: string;
		velaNotConnected: string;
		relayFailed: string;
		relayRetrying: string;
		relayUnreachable: string;
		relayRefused: string;
		relayNoToken: string;
		velaRelayFailed: string;
		velaRelayRetrying: string;
		velaRelayUnreachable: string;
		missingTx: string;
		mismatchTx: string;
		txUnavailable: string;
		whatMailbox: string;
		whatChain: string;
		approvalsLabel: string;
		doneTx: string;
		approvedClose: string;
		rejectedHere: string;
		mining: string;
		stepOf: string;
		mailboxBusy: string;
		sendingMailbox: string;
		notSent: string;
		sendAgain: string;
		connectWallet: string;
		notOwnerHere: string;
		alreadyApproved: string;
		executeFee: string;
		executeFeeUnknown: string;
		executeSteps: string;
		executeWith: string;
		approveOnly: string;
		keep: string;
		reverted: string;
		mailboxRefused: string;
		walletDeclined: string;
		walletUnauthorized: string;
		walletUnsupported: string;
		walletGone: string;
		walletFailed: string;
		outcomeDone: string;
		outcomeSignedHash: string;
		outcomeSent: string;
		outcomeSwitched: string;
		outcomeWithdrawn: string;
		outcomeDeclined: string;
		outcomeFailed: string;
		outcomeInWallet: string;
		outcomeWaiting: string;
		pairedWith: string;
		useAnother: string;
		linkLater: string;
		outsideLinkTitle: string;
		outsideLinkHint: string;
		mailboxNoEthSign: string;
		/** r5reg-7: signed, on a page still paired with Vela. */
		signedPaired: string;
		/** r4C-14: the approvals already meet the threshold. */
		enoughApprovals: string;
		/** r5reg-13: `{{fee}}`, `{{balance}}` — the wallet holds less than the fee. */
		executeNeedsGas: string;
		/** r5reg-13: `{{symbol}}` — the wallet holds none of the coin that pays. */
		executeNoGas: string;
		/** r5reg-12: the mailbox's refusals, in plain words. */
		refusedSetupUnchecked: string;
		refusedSetup: string;
		refusedSignature: string;
		refusedFull: string;
		refusedTransaction: string;
		refusedOther: string;
		/** r5reg-12: a refused approval or rejection is kept, and sent again. */
		keptSigned: string;
		/** r4C-12: a request row — `{{number}}` ("#7"), `{{what}}`, `{{to}}`. */
		rowApprove: string;
		rowApproveTo: string;
		rowExecute: string;
		rowReject: string;
		outcomeNotSent: string;
		/** r5reg-23: under the multisig's name in a review. */
		nameFromRecord: string;
		/** Round 6: the link's invite does not read — never "the wallet couldn't sign". */
		linkDamaged: string;
	};
	import: {
		or: string;
		body: string;
		choose: string;
		notExport: string;
		empty: string;
		pick: string;
		undeployed: string;
		importSome: string;
		importing: string;
		imported: string;
		already: string;
		notDeployed: string;
		/** r7: the step that asks each member's kind before the Safes are imported. */
		kindsTitle: string;
		kindsBody: string;
	};
	settings: {
		backupTitle: string;
		backedUp: string;
		notBackedUp: string;
		unavailable: string;
		couldNotCheck: string;
		backupNote: string;
		remove: string;
		/** Its record's name, renamed on chain by a proposal. */
		renameTitle: string;
		/** Who sees the name, and that renaming is one of its transactions (`{{network}}`). */
		renameNote: string;
		renameAction: string;
		/** No network this wallet has holds its record. */
		renameUnavailable: string;
		/** Its record's chain is one it is not deployed on: nothing offered (`{{network}}`). */
		renameNotDeployed: string;
		/** r4a-12: its record is not saved yet — nothing to rename. */
		renameNotRecorded: string;
		originalMembers: string;
		originalNote: string;
		/** `{{network}}` */
		currentOn: string;
		sameAsOriginal: string;
		added: string;
		removed: string;
		/** `{{was}}`, `{{is}}` */
		thresholdChanged: string;
		/** `{{network}}` */
		undeployedOn: string;
		/** The service lacks its setup: shared again with one passkey. */
		shareAgain: string;
		shareAgainNote: string;
	};
	/** What the multisig area says of its service (set in Settings → Service Endpoints). */
	service: {
		/** The multisig area's note while a custom service is in use (`{{host}}`). */
		customNote: string;
		/** Activity's waiting group while the service is away, over the last list. */
		unreachableNote: string;
		retry: string;
		/** r4b-15: the device is offline. */
		offline: string;
		/** …and the list shown was read at `{{time}}`. */
		offlineSince: string;
		/** r4b-3: `{{network}}` did not answer the last read. */
		chainUnreachable: string;
	};
	add: {
		title: string;
		intro: string;
		placeholder: string;
		add: string;
		notFound: string;
		unavailable: string;
		unsupported: string;
		notMultisig: string;
		/** Nothing says which network it is used on: the person picks one. */
		chooseNetwork: string;
		/** r4b-13: no record, and a network did not answer in time — not said as "none". */
		unchecked: string;
		/** r4b-13: the invite link is damaged. */
		badLink: string;
		/** r4b-13: an invite that does not read, said at once. */
		badInvite: string;
		/** r5reg-22: an invite whose address's letter case is not its checksum. */
		badChecksum: string;
		/** `{{networks}}`: where its invite says it is used. */
		usedOn: string;
		/** Its invite's network is not on this device (`{{network}}`). */
		networkMissing: string;
		moreNetworks: string;
		/** r4D-5: the members' labels an invite carries. */
		invitedMembers: string;
		/** r5reg-24: beside the invited member that is this device's own account. */
		thatsYou: string;
		kindPasskey: string;
		kindWallet: string;
		labelsNote: string;
	};
	/**
	 * Who can act for a multisig (§3.5): a genesis the core refuses or cannot
	 * check yet, one line per reason it gives (`{{address}}`, `{{network}}`),
	 * what else can act for a deployed one, and the executor's refusal.
	 */
	authority: {
		refusedTitle: string;
		refusedBody: string;
		unverified: string;
		wrongAddress: string;
		notSetup: string;
		badMembers: string;
		unknownFactory: string;
		unknownSingleton: string;
		module: string;
		setupCode: string;
		handler: string;
		payment: string;
		missingCode: string;
		wrongCode: string;
		riskTitle: string;
		riskBody: string;
		riskModule: string;
		riskGuard: string;
		riskHandler: string;
		riskSingleton: string;
		riskConfirm: string;
		riskFlag: string;
		deployRefused: string;
		deployUnverified: string;
		/** The switcher's flag for a multisig whose genesis a network refused. */
		refusedFlag: string;
		/** A network the re-check refused its genesis on (`{{network}}`): read-only there. */
		refusedOn: string;
		/** Each signature on a high-risk network asks first, naming what was found. */
		signRiskTitle: string;
		signRiskBody: string;
		signRiskConfirm: string;
		signRiskCancel: string;
		/** Who can act for it on `{{network}}` could not be read: nothing is signed yet. */
		signUnchecked: string;
	};
	/** Changing the members and the approvals needed (Settings), one proposal per network. */
	members: MemberMessages;
	/** r4D-17: an executed transaction's story — Created → Approved → Executed. */
	timeline: {
		heading: string;
		/** `{{name}}`: who proposed it. */
		created: string;
		approved: string;
		/** The executor's approval, given by sending it. */
		approvedByExecuting: string;
		executed: string;
		executedBy: string;
		/** An approval that counted when it ran, and one that did not. */
		counted: string;
		notCounted: string;
	};
	/** The one review every member signs from (§3.6): the state-diff guard, who it reaches, the bytes. */
	review: ReviewMessages;
}

/** `home.multisig.review`: what the one review says beside the call's reading (§3.6). */
export interface ReviewMessages {
	/** Running it changes who controls the multisig, undeclared: no signing. */
	stateTitle: string;
	/** `{{network}}` */
	stateBody: string;
	/** `{{before}}`, `{{after}}`: slot 0, the code the proxy runs (Bybit). */
	writeSingleton: string;
	/** `{{address}}` */
	writeModuleOn: string;
	writeModuleOff: string;
	/** `{{after}}` */
	writeGuard: string;
	writeModuleGuard: string;
	writeHandler: string;
	/** `{{address}}` */
	writeAdded: string;
	writeRemoved: string;
	/** An `owners` entry outside the member list (`{{address}}`). */
	writeOwnerEntry: string;
	/** `{{before}}`, `{{after}}` */
	writeThreshold: string;
	/** `{{after}}` */
	writeOwnerCount: string;
	writeNonce: string;
	/** `{{slot}}` */
	writeStorage: string;
	/** `{{network}}`: checked, nothing that decides control changes. */
	stateUnchanged: string;
	/** `{{network}}`: checked, the members change exactly as declared. */
	stateDeclared: string;
	/** `{{network}}`: no state-diff source; only the decoding rules stand. */
	stateUnread: string;
	sourceTrace: string;
	sourceProbe: string;
	/** What moves was read from the probe's balances (no simulation offered). */
	movesProbe: string;
	roleRecipient: string;
	roleContract: string;
	roleMember: string;
	/** `{{n}}`: a batch's call. */
	callNumber: string;
	/** `{{address}}`, `{{label}}`, `{{known}}` */
	lookalike: string;
	/** `{{address}}` */
	toContract: string;
	/** r4a-18: an address the review reaches that is one of this device's own accounts. */
	knownOwn: string;
	/** r4a-18: an address the review reaches that is a member of the multisig. */
	knownMember: string;
	typedData: string;
	techTitle: string;
	hashesTitle: string;
	approveAnyway: string;
	approveExecuteAnyway: string;
	executeAnyway: string;
	proposeAnyway: string;
	/** The gate refused a transaction right before signing. */
	blocked: string;
}

/** `home.multisig.members`: changing who the members are and how many must approve. */
export interface MemberMessages {
	title: string;
	add: string;
	remove: string;
	replace: string;
	/** `{{name}}` */
	removeLabel: string;
	/** `{{name}}` */
	replaceLabel: string;
	lostPasskey: string;
	entryLabel: string;
	entryPlaceholder: string;
	/** `{{name}}` */
	resolving: string;
	/** `{{name}}` */
	nameNotFound: string;
	/** `{{name}}` */
	nameUnavailable: string;
	suggestions: string;
	memberName: string;
	memberNamePlaceholder: string;
	nameNote: string;
	kind: string;
	kindVela: string;
	kindWallet: string;
	/** r7: evidence says a Vela account (code, the passkey index, this device): no choice. */
	kindKnown: string;
	/** r7: no code anywhere and no index record — "Other wallet" only hinted; kept if picked. */
	kindLikely: string;
	/** r7: the wallet can't tell — the person is asked; left unsaid, both ways are offered. */
	kindAsk: string;
	/** r7: code at the address and no index record — a smart contract account. */
	kindContract: string;
	/** r7: …which approves from its own app (EIP-1271), which Vela can't collect yet. */
	kindContractNote: string;
	newAccount: string;
	/** `{{name}}` */
	replaceIntro: string;
	/** `{{name}}` */
	removeIntro: string;
	thresholdAfter: string;
	thresholdTitle: string;
	/** `{{m}}`, `{{n}}` */
	ruleValue: string;
	/** `{{network}}`, `{{before}}`, `{{after}}` */
	onNetwork: string;
	perNetwork: string;
	noneDeployed: string;
	/** `{{network}}` */
	proposing: string;
	/** `{{network}}` */
	proposed: string;
	/** `{{network}}`, `{{reason}}` */
	failedOn: string;
	waitingTitle: string;
	/** `{{name}}` */
	changeAdd: string;
	/** `{{name}}` */
	changeRemove: string;
	/** `{{old}}`, `{{new}}` */
	changeReplace: string;
	/** `{{m}}`, `{{n}}`: passkey-native recovery, said where the rule is chosen. */
	recovery: string;
	/** Every member must approve: a lost passkey cannot be replaced. */
	recoveryAll: string;
	/** Why a change cannot be made on a network: one per refusal of the core's (`MemberRefusal`). */
	refuse: {
		notAnAddress: string;
		noOne: string;
		theMultisig: string;
		alreadyMember: string;
		notMember: string;
		lastMember: string;
		/** `{{threshold}}`, `{{owners}}` */
		thresholdTooHigh: string;
		thresholdZero: string;
		/** `{{owners}}`, `{{threshold}}` */
		belowThreshold: string;
		unchanged: string;
		/** `{{max}}` */
		tooMany: string;
		changeWaiting: string;
	};
}

/** Every key of {@link MultisigMessages}, for the resolver to walk. */
export const MULTISIG_SHAPE: MultisigMessages = {
	switcher: {
		section: '',
		create: '',
		add: '',
		rowName: '',
		readOnly: '',
		notShared: '',
		removeBody: '',
		addAccount: '',
		createAccountNote: '',
		signInNote: '',
		createMultisigNote: '',
		addMultisigNote: '',
		lastMember: ''
	},
	panel: {
		title: '',
		titleSolo: '',
		chip: '',
		chipSolo: '',
		members: '',
		thisDevice: '',
		readOnly: '',
		pending: '',
		stale: '',
		notShared: '',
		invite: '',
		lastKnown: '',
		checking: '',
		historyUnreadable: ''
	},
	status: {
		ready: '',
		yourTurn: '',
		waiting: '',
		queued: '',
		progress: '',
		unsent: '',
		executing: '',
		conflict: '',
		cannotRun: '',
		nobodyPays: ''
	},
	action: {
		send: '',
		batch: '',
		cancel: '',
		addOwner: '',
		removeOwner: '',
		swapOwner: '',
		changeThreshold: '',
		unsupported: '',
		to: '',
		network: '',
		number: '',
		numberValue: '',
		call: '',
		callWithValue: '',
		batchCalls: '',
		rename: ''
	},
	change: {
		add: '',
		remove: '',
		replace: '',
		threshold: '',
		rule: '',
		caution: '',
		rowCaution: '',
		doesNotFit: ''
	},
	token: {
		unverified: '',
		lookalike: '',
		contract: ''
	},
	proposal: {
		title: '',
		approvals: '',
		approved: '',
		waiting: '',
		approve: '',
		approveAndExecute: '',
		execute: '',
		as: '',
		hash: '',
		feeNote: '',
		invite: '',
		inviteHint: '',
		copyLink: '',
		copied: '',
		share: '',
		reject: '',
		withdraw: '',
		rejectHint: '',
		cancelPending: '',
		executing: '',
		executed: '',
		explorer: '',
		refused: '',
		doneNote: '',
		queuedNote: '',
		notMember: '',
		failed: '',
		noExpiry: '',
		cannotPay: '',
		notLanded: '',
		mailboxDown: '',
		notSent: '',
		siteAsks: '',
		siteWaits: '',
		stillNeeded: '',
		siteTold: '',
		data: '',
		notFound: '',
		opening: '',
		changesMembers: '',
		siteFollows: '',
		voided: '',
		voidedNote: '',
		voidedByOther: '',
		notInMailbox: '',
		badLink: '',
		previewNote: '',
		watch: '',
		watching: '',
		renameRuns: '',
		renameUndeployed: '',
		startsOriginal: '',
		startsOriginalNote: '',
		notCounted: '',
		missingNetworkNamed: '',
		missingNetwork: '',
		missingNetworkBody: '',
		missingCount: '',
		addNetwork: '',
		outboxBusy: '',
		outboxAway: '',
		outboxNotSent: '',
		outboxSendAgain: '',
		copy: '',
		executedBy: '',
		countedThen: '',
		approvalsSigned: '',
		lostRace: '',
		lostRacePaid: '',
		replacedFirst: '',
		replacedFirstPaid: '',
		execFailed: '',
		cantCheck: '',
		cantCheckUsed: '',
		conflict: '',
		queuedNoteMany: '',
		openNumber: '',
		reason: {
			passkeyClosed: '',
			passkeyTimedOut: '',
			passkeyMissing: '',
			passkeyFailed: '',
			serviceKept: '',
			serviceRefused: '',
			offline: '',
			chainUnreachable: '',
			unknown: ''
		},
		wouldFail: '',
		whyShort: '',
		whyReverts: '',
		whyRevertsPlain: '',
		unpricedReason: '',
		uncheckedReason: '',
		feeLine: '',
		feeChecking: '',
		feeUnpriced: '',
		feeUnchecked: '',
		feeRetry: '',
		feeSetup: '',
		feeSetupPlain: '',
		setupMultisig: '',
		setupMember: '',
		setupExecutor: '',
		cantPayOther: '',
		pickerCantPay: '',
		approvedNotExecuted: '',
		notNeeded: '',
		sendLink: '',
		approvesInVela: '',
		contractMembers: '',
		rejectCancel: '',
		noOutsideSigner: ''
	},
	send: {
		propose: '',
		proposeOnly: '',
		invalidAddress: '',
		invalidChecksum: ''
	},
	create: {
		title: '',
		intro: '',
		members: '',
		yourAccounts: '',
		byAddress: '',
		addressPlaceholder: '',
		addMember: '',
		order: '',
		threshold: '',
		thresholdValue: '',
		thresholdAll: '',
		thresholdOne: '',
		thresholdSpare: { locale: '', forms: {} },
		address: '',
		sameEverywhere: '',
		network: '',
		networkHint: '',
		creator: '',
		create: '',
		next: '',
		back: '',
		needLocal: '',
		duplicate: '',
		notMember: '',
		created: '',
		createdBody: '',
		createdBodySolo: '',
		done: '',
		name: '',
		namePlaceholder: '',
		nameHint: '',
		nameInvalid: '',
		fewer: '',
		more: '',
		copyAddress: '',
		noSafeNetworks: '',
		nameBytes: '',
		nameTooLong: '',
		nameNeeded: '',
		memberName: '',
		memberNamePlaceholder: '',
		ensResolved: '',
		ensNotFound: '',
		ensUnavailable: '',
		ensUnsupported: '',
		fromContacts: '',
		solo: '',
		networkNeeded: '',
		savedSharing: '',
		savedLater: '',
		shared: '',
		sharedSolo: '',
		refused: '',
		copyInvite: '',
		firstRunTitle: '',
		firstRunInvite: '',
		firstRunInviteNote: '',
		firstRunFund: '',
		firstRunFundNote: '',
		firstRunCost: '',
		firstRunDismiss: ''
	},
	invite: {
		body: '',
		shareText: '',
		qrLabel: '',
		outsideHint: ''
	},
	presence: {
		needsYou: { locale: '', forms: {} },
		ready: { locale: '', forms: {} },
		waitingPayer: { locale: '', forms: {} },
		ruleDiffers: '',
		othersWaiting: ''
	},
	external: {
		pageTitle: '',
		looking: '',
		noWallet: '',
		noWalletBody: '',
		copyLink: '',
		copied: '',
		chooseWallet: '',
		askConnect: '',
		askNote: '',
		connect: '',
		decline: '',
		keepOpen: '',
		waitingVela: '',
		noSession: '',
		requests: '',
		signTx: '',
		sendTx: '',
		switchChain: '',
		disconnect: '',
		signWithWallet: '',
		sheetTitle: '',
		scanBody: '',
		resumeBody: '',
		waiting: '',
		connecting: '',
		wrongAccount: '',
		switching: '',
		confirming: '',
		declined: '',
		badSignature: '',
		signedBy: '',
		unpair: '',
		ownerWallet: '',
		retry: '',
		pageAway: '',
		connectDeclined: '',
		declinedHere: '',
		otherChain: '',
		connectingAs: '',
		expectedMember: '',
		notExpectedMember: '',
		connectingVela: '',
		loadingTx: '',
		reviewTitle: '',
		multisigLabel: '',
		doesLabel: '',
		tokenNative: '',
		tokenContract: '',
		queuedTx: '',
		refusedKind: '',
		hashesTitle: '',
		hashesNote: '',
		safeTxHash: '',
		domainHash: '',
		messageHash: '',
		copy: '',
		notSafeTx: '',
		signInWallet: '',
		sendInWallet: '',
		inWallet: '',
		stillWaiting: '',
		stopWaiting: '',
		askAgain: '',
		stoppedWaiting: '',
		signedClose: '',
		signedTypedForm: '',
		signedHashForm: '',
		signedOnChain: '',
		withdrawnInWallet: '',
		typedUnsupported: '',
		ethSignOffer: '',
		signHash: '',
		approveOnChainOffer: '',
		approveOnChain: '',
		velaOffline: '',
		velaNotConnected: '',
		relayFailed: '',
		relayRetrying: '',
		relayUnreachable: '',
		relayRefused: '',
		relayNoToken: '',
		velaRelayFailed: '',
		velaRelayRetrying: '',
		velaRelayUnreachable: '',
		missingTx: '',
		mismatchTx: '',
		txUnavailable: '',
		whatMailbox: '',
		whatChain: '',
		approvalsLabel: '',
		doneTx: '',
		approvedClose: '',
		rejectedHere: '',
		mining: '',
		stepOf: '',
		mailboxBusy: '',
		sendingMailbox: '',
		notSent: '',
		sendAgain: '',
		connectWallet: '',
		notOwnerHere: '',
		alreadyApproved: '',
		executeFee: '',
		executeFeeUnknown: '',
		executeSteps: '',
		executeWith: '',
		approveOnly: '',
		keep: '',
		reverted: '',
		mailboxRefused: '',
		walletDeclined: '',
		walletUnauthorized: '',
		walletUnsupported: '',
		walletGone: '',
		walletFailed: '',
		outcomeDone: '',
		outcomeSignedHash: '',
		outcomeSent: '',
		outcomeSwitched: '',
		outcomeWithdrawn: '',
		outcomeDeclined: '',
		outcomeFailed: '',
		outcomeInWallet: '',
		outcomeWaiting: '',
		pairedWith: '',
		useAnother: '',
		linkLater: '',
		outsideLinkTitle: '',
		outsideLinkHint: '',
		mailboxNoEthSign: '',
		signedPaired: '',
		enoughApprovals: '',
		executeNeedsGas: '',
		executeNoGas: '',
		refusedSetupUnchecked: '',
		refusedSetup: '',
		refusedSignature: '',
		refusedFull: '',
		refusedTransaction: '',
		refusedOther: '',
		keptSigned: '',
		rowApprove: '',
		rowApproveTo: '',
		rowExecute: '',
		rowReject: '',
		outcomeNotSent: '',
		nameFromRecord: '',
		linkDamaged: ''
	},
	import: {
		or: '',
		body: '',
		choose: '',
		notExport: '',
		empty: '',
		pick: '',
		undeployed: '',
		importSome: '',
		importing: '',
		imported: '',
		already: '',
		notDeployed: '',
		kindsTitle: '',
		kindsBody: ''
	},
	settings: {
		backupTitle: '',
		backedUp: '',
		notBackedUp: '',
		unavailable: '',
		couldNotCheck: '',
		backupNote: '',
		remove: '',
		renameTitle: '',
		renameNote: '',
		renameAction: '',
		renameUnavailable: '',
		renameNotDeployed: '',
		renameNotRecorded: '',
		originalMembers: '',
		originalNote: '',
		currentOn: '',
		sameAsOriginal: '',
		added: '',
		removed: '',
		thresholdChanged: '',
		undeployedOn: '',
		shareAgain: '',
		shareAgainNote: ''
	},
	service: {
		customNote: '',
		unreachableNote: '',
		retry: '',
		offline: '',
		offlineSince: '',
		chainUnreachable: ''
	},
	add: {
		title: '',
		intro: '',
		placeholder: '',
		add: '',
		notFound: '',
		unavailable: '',
		unsupported: '',
		notMultisig: '',
		chooseNetwork: '',
		unchecked: '',
		badLink: '',
		badInvite: '',
		badChecksum: '',
		usedOn: '',
		networkMissing: '',
		moreNetworks: '',
		invitedMembers: '',
		thatsYou: '',
		kindPasskey: '',
		kindWallet: '',
		labelsNote: ''
	},
	authority: {
		refusedTitle: '',
		refusedBody: '',
		unverified: '',
		wrongAddress: '',
		notSetup: '',
		badMembers: '',
		unknownFactory: '',
		unknownSingleton: '',
		module: '',
		setupCode: '',
		handler: '',
		payment: '',
		missingCode: '',
		wrongCode: '',
		riskTitle: '',
		riskBody: '',
		riskModule: '',
		riskGuard: '',
		riskHandler: '',
		riskSingleton: '',
		riskConfirm: '',
		riskFlag: '',
		deployRefused: '',
		deployUnverified: '',
		refusedFlag: '',
		refusedOn: '',
		signRiskTitle: '',
		signRiskBody: '',
		signRiskConfirm: '',
		signRiskCancel: '',
		signUnchecked: ''
	},
	members: {
		title: '',
		add: '',
		remove: '',
		replace: '',
		removeLabel: '',
		replaceLabel: '',
		lostPasskey: '',
		entryLabel: '',
		entryPlaceholder: '',
		resolving: '',
		nameNotFound: '',
		nameUnavailable: '',
		suggestions: '',
		memberName: '',
		memberNamePlaceholder: '',
		nameNote: '',
		kind: '',
		kindVela: '',
		kindWallet: '',
		kindKnown: '',
		kindLikely: '',
		kindAsk: '',
		kindContract: '',
		kindContractNote: '',
		newAccount: '',
		replaceIntro: '',
		removeIntro: '',
		thresholdAfter: '',
		thresholdTitle: '',
		ruleValue: '',
		onNetwork: '',
		perNetwork: '',
		noneDeployed: '',
		proposing: '',
		proposed: '',
		failedOn: '',
		waitingTitle: '',
		changeAdd: '',
		changeRemove: '',
		changeReplace: '',
		recovery: '',
		recoveryAll: '',
		refuse: {
			notAnAddress: '',
			noOne: '',
			theMultisig: '',
			alreadyMember: '',
			notMember: '',
			lastMember: '',
			thresholdTooHigh: '',
			thresholdZero: '',
			belowThreshold: '',
			unchanged: '',
			tooMany: '',
			changeWaiting: ''
		}
	},
	timeline: {
		heading: '',
		created: '',
		approved: '',
		approvedByExecuting: '',
		executed: '',
		executedBy: '',
		counted: '',
		notCounted: ''
	},
	review: {
		stateTitle: '',
		stateBody: '',
		writeSingleton: '',
		writeModuleOn: '',
		writeModuleOff: '',
		writeGuard: '',
		writeModuleGuard: '',
		writeHandler: '',
		writeAdded: '',
		writeRemoved: '',
		writeOwnerEntry: '',
		writeThreshold: '',
		writeOwnerCount: '',
		writeNonce: '',
		writeStorage: '',
		stateUnchanged: '',
		stateDeclared: '',
		stateUnread: '',
		sourceTrace: '',
		sourceProbe: '',
		movesProbe: '',
		roleRecipient: '',
		roleContract: '',
		roleMember: '',
		callNumber: '',
		lookalike: '',
		toContract: '',
		knownOwn: '',
		knownMember: '',
		typedData: '',
		techTitle: '',
		hashesTitle: '',
		approveAnyway: '',
		approveExecuteAnyway: '',
		executeAnyway: '',
		proposeAnyway: '',
		blocked: ''
	}
};
