/**
 * The multisig screens' copy (`home.multisig` in the vela-core corpus),
 * resolved at build time like every other page's (`engine.server.ts`).
 * Templates keep their `{{name}}` slots; `fillTemplate` fills them.
 */
export interface MultisigMessages {
	switcher: {
		section: string;
		create: string;
		add: string;
		rowName: string;
		readOnly: string;
		removeBody: string;
	};
	panel: {
		title: string;
		members: string;
		thisDevice: string;
		external: string;
		readOnly: string;
		pending: string;
		nothingPending: string;
		stale: string;
		notShared: string;
		propose: string;
		invite: string;
	};
	status: {
		ready: string;
		yourTurn: string;
		waiting: string;
		queued: string;
		done: string;
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
		notFound: string;
		opening: string;
	};
	send: {
		title: string;
		asset: string;
		amount: string;
		max: string;
		balance: string;
		recipient: string;
		recipientPlaceholder: string;
		propose: string;
		hint: string;
		noAssets: string;
		invalidAddress: string;
		invalidAmount: string;
		proposed: string;
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
		thresholdSpare: string;
		review: string;
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
		created: string;
		createdBody: string;
		done: string;
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
	receiveNote: string;
}

/** Every key of {@link MultisigMessages}, for the resolver to walk. */
export const MULTISIG_SHAPE: MultisigMessages = {
	switcher: {
		section: '',
		create: '',
		add: '',
		rowName: '',
		readOnly: '',
		removeBody: '',
	},
	panel: {
		title: '',
		members: '',
		thisDevice: '',
		external: '',
		readOnly: '',
		pending: '',
		nothingPending: '',
		stale: '',
		notShared: '',
		propose: '',
		invite: '',
	},
	status: {
		ready: '',
		yourTurn: '',
		waiting: '',
		queued: '',
		done: '',
		progress: '',
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
		notFound: '',
		opening: '',
	},
	send: {
		title: '',
		asset: '',
		amount: '',
		max: '',
		balance: '',
		recipient: '',
		recipientPlaceholder: '',
		propose: '',
		hint: '',
		noAssets: '',
		invalidAddress: '',
		invalidAmount: '',
		proposed: '',
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
		thresholdSpare: '',
		review: '',
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
		created: '',
		createdBody: '',
		done: '',
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
	},
	receiveNote: '',
};
