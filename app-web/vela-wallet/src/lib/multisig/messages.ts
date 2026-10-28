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
		data: string;
		notFound: string;
		opening: string;
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
		addMultisigNote: ''
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
		progress: ''
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
		callWithValue: ''
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
		data: '',
		notFound: '',
		opening: ''
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
		nameInvalid: ''
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
		retry: ''
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
		remove: ''
	},
	add: {
		title: '',
		intro: '',
		placeholder: '',
		add: '',
		notFound: '',
		unavailable: '',
		unsupported: '',
		notMultisig: ''
	}
};
