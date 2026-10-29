/**
 * The multisig screens' copy (`home.multisig` in the vela-core corpus),
 * resolved at build time like every other page's (`engine.server.ts`).
 * Templates keep their `{{name}}` slots; `fillTemplate` fills them.
 */
import type { PluralCopy } from '$lib/i18n/plural';

export interface MultisigMessages {
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
		members: string;
		thisDevice: string;
		readOnly: string;
		pending: string;
		stale: string;
		notShared: string;
		invite: string;
	};
	status: {
		ready: string;
		yourTurn: string;
		waiting: string;
		queued: string;
		progress: string;
		/** Signed here, not in the mailbox yet. */
		unsent: string;
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
		call: string;
		callWithValue: string;
		batchCalls: string;
		/** A rename of its record in the registry (`{{name}}`). */
		rename: string;
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
		outboxSendAgain: string;
	};
	send: {
		propose: string;
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
		creator: string;
		create: string;
		next: string;
		back: string;
		needTwo: string;
		needLocal: string;
		duplicate: string;
		notMember: string;
		created: string;
		createdBody: string;
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
	};
	/** Changing the members and the approvals needed (Settings), one proposal per network. */
	members: MemberMessages;
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
		members: '',
		thisDevice: '',
		readOnly: '',
		pending: '',
		stale: '',
		notShared: '',
		invite: ''
	},
	status: {
		ready: '',
		yourTurn: '',
		waiting: '',
		queued: '',
		progress: '',
		unsent: ''
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
		call: '',
		callWithValue: '',
		batchCalls: '',
		rename: ''
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
		outboxSendAgain: ''
	},
	send: {
		propose: '',
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
		creator: '',
		create: '',
		next: '',
		back: '',
		needTwo: '',
		needLocal: '',
		duplicate: '',
		notMember: '',
		created: '',
		createdBody: '',
		done: '',
		name: '',
		namePlaceholder: '',
		nameHint: '',
		nameInvalid: '',
		fewer: '',
		more: '',
		copyAddress: '',
		noSafeNetworks: ''
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
		notExpectedMember: ''
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
		notDeployed: ''
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
		retry: ''
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
		chooseNetwork: ''
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
	}
};
