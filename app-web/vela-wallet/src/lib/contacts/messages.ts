/**
 * Contacts message manifest (spec 018, contracts/i18n-keys.md).
 *
 * Client-safe: names keys and shapes only — resolution happens in
 * `engine.server.ts` at build time, exactly like `wallet/messages.ts`.
 * `shell.*` re-resolves the spec-015 keys the contacts screens need for the
 * reused chrome (tab bar, sidebar, third-column close) so one messages object
 * feeds every builder; `activity.*` reuses the history keys so a contact's
 * 最近往来 rows are literally the wallet's ActivityRow copy.
 */

export interface ContactsMessages {
	title: string;
	searchPlaceholder: string;
	sectionGroups: string;
	sectionContacts: string;
	manage: string;
	allContacts: string;
	/** Template with `{{count}}` — "8 位". */
	countPeople: string;
	/** Template with `{{count}}` — "3 人". */
	groupMembers: string;
	/** Template with `{{count}}` — "3 位成员". */
	membersCount: string;
	groupNew: string;
	groupEdit: string;
	groupRename: string;
	groupDelete: string;
	moveGroup: string;
	addMember: string;
	addContact: string;
	addTitle: string;
	/** The add/edit form (live wiring, spec 024) — corpus keys from 018. */
	editTitle: string;
	nameLabel: string;
	namePlaceholder: string;
	addressPlaceholder: string;
	save: string;
	invalidAddress: string;
	groupNameLabel: string;
	groupNamePlaceholder: string;
	edit: string;
	empty: string;
	emptyHint: string;
	/** Template with `{{query}}`. */
	noResults: string;
	batchSend: string;
	/** Template with `{{count}}`. */
	batchSendHint: string;
	/** Template with `{{count}}`. */
	batchSendHintTitled: string;
	/** Why the group send is disabled: nobody to send to yet. */
	batchSendNeedsMembers: string;
	importFile: string;
	importAll: string;
	importGroup: string;
	exportTitle: string;
	exportAll: string;
	exportGroup: string;
	recentActivity: string;
	viewAllActivity: string;
	addressLabel: string;
	copyAddress: string;
	send: string;
	receive: string;
	actionQr: string;
	deleteContact: string;
	delete: string;
	deleteTitle: string;
	/** Template with `{{name}}`. */
	deleteBody: string;
	cancel: string;
	/** The book as a file (spec 028 US5): the import report and the refusals. */
	importDoneTitle: string;
	/** Template with `{{added}}` and `{{skipped}}`. */
	importDoneBody: string;
	/** Template with `{{invalid}}` — appended when rows had no valid address. */
	importDoneInvalid: string;
	importFailTitle: string;
	importFailBody: string;
	/** The export dialog's one line: choose a format. */
	exportBody: string;
	/** Template with `{{name}}` — deleting a group keeps its contacts, and says so. */
	groupDeleteBody: string;
	/** Members picker heading — the drawn 018 word for the group editor's list. */
	groupMembersLabel: string;
	/** Nothing saved yet, so nothing to pick. */
	groupNoContacts: string;
	/** "Copied" — the address copy's one-word acknowledgement. */
	copied: string;
	/** 最近往来 with nothing in it (the reused history empty state). */
	noActivity: string;
	/** "Done" — the report's dismiss. */
	done: string;
	activity: { sent: string; received: string; today: string; yesterday: string; all: string };
	shell: {
		navWallet: string;
		navContacts: string;
		navExplore: string;
		navSettings: string;
		networksTitle: string;
		allNetworks: string;
		close: string;
	};
}

/** Every corpus key the contacts screens consume (tests iterate this). */
export const CONTACTS_KEYS = [
	'contacts.title',
	'contacts.searchPlaceholder',
	'contacts.sectionGroups',
	'contacts.sectionContacts',
	'contacts.manage',
	'contacts.allContacts',
	'contacts.countPeople',
	'contacts.groupMembers',
	'contacts.membersCount',
	'contacts.groupNew',
	'contacts.groupEdit',
	'contacts.groupRename',
	'contacts.groupDelete',
	'contacts.moveGroup',
	'contacts.addMember',
	'contacts.addContact',
	'contacts.editTitle',
	'contacts.nameLabel',
	'contacts.namePlaceholder',
	'contacts.addressPlaceholder',
	'contacts.save',
	'contacts.invalidAddress',
	'contacts.groupNameLabel',
	'contacts.groupNamePlaceholder',
	'contacts.addTitle',
	'contacts.edit',
	'contacts.empty',
	'contacts.emptyHint',
	'contacts.noResults',
	'contacts.batchSend',
	'contacts.batchSendHint',
	'contacts.batchSendHintTitled',
	'contacts.batchSendNeedsMembers',
	'contacts.importFile',
	'contacts.importAll',
	'contacts.importGroup',
	'contacts.exportTitle',
	'contacts.exportAll',
	'contacts.exportGroup',
	'contacts.recentActivity',
	'contacts.viewAllActivity',
	'contacts.addressLabel',
	'contacts.deleteContact',
	'contacts.delete',
	'contacts.deleteTitle',
	'contacts.deleteBody',
	'contacts.cancel',
	'contacts.actionQr',
	'contacts.importDoneTitle',
	'contacts.importDoneBody',
	'contacts.importDoneInvalid',
	'contacts.importFailTitle',
	'contacts.importFailBody',
	'contacts.exportBody',
	'contacts.groupDeleteBody',
	'contacts.groupMembersLabel',
	'contacts.groupNoContacts',
	'componentsUi.identiconViewer.copied',
	'componentsUi.dayGroup.today',
	'history.emptyTitle',
	'common.done',
	'componentsUi.identiconViewer.copyAddress',
	'componentsUi.dock.send',
	'componentsUi.dock.receive',
	'history.labelSent',
	'history.labelReceived',
	'history.filterAll',
	'componentsUi.dayGroup.yesterday',
	'componentsUi.mainNav.wallet',
	'componentsUi.mainNav.contacts',
	'componentsUi.mainNav.explore',
	'componentsUi.mainNav.settings',
	'settingsModals.network.modalTitle',
	'componentsUi.networkFilter.allNetworks',
	'componentsUi.identiconViewer.close'
] as const;
