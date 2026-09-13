/**
 * The contacts screens' UI-event vocabulary (spec 024, widened in 028 US5) —
 * the contacts sibling of settings' `net-events.ts`. One union, one optional
 * callback: absent = the gallery's pure picture; present = the route
 * translates each variant into core events. No variant decides anything.
 *
 * The 028 additions are the screens' remaining affordances, every one of
 * which was drawn in 018 and did nothing: the detail's three actions, the
 * address copy, the group chips' `+`, "全部 ›", the group screen's CTA and
 * its ⋯, the desktop's right-click menus, and the mobile "+" that opens the
 * drawn C5 sheet rather than the form.
 */

export type ContactsUiEvent =
	| { kind: 'tab'; id: 'wallet' | 'contacts' | 'explore' | 'settings' }
	| { kind: 'query'; value: string }
	| { kind: 'add' }
	| { kind: 'open'; address: string }
	| { kind: 'back' }
	| { kind: 'edit' }
	| { kind: 'delete'; address: string }
	| { kind: 'group-open'; id: string }
	| { kind: 'group-new' }
	| { kind: 'add-member' }
	| { kind: 'empty-primary' }
	| { kind: 'empty-secondary' }
	/** A row in the open action/confirm sheet, by its label (the sheet is data). */
	| { kind: 'sheet-select'; label: string }
	| { kind: 'sheet-close' }
	/**
	 * The detail's pill/card actions and the address copy (spec 028 US5).
	 * `address` names the contact they act on — the detail's, or a
	 * right-clicked row's — so the route never has to guess from its own
	 * selection state.
	 */
	| { kind: 'action'; id: 'send' | 'receive' | 'qr' | 'copy' | 'move-group'; address: string }
	/** 最近往来's "全部 ›" / 查看全部往来. */
	| { kind: 'activity-all' }
	/** 群发转账 for the open group. */
	| { kind: 'batch-send'; id: string }
	/** The group screen's ⋯ (mobile) / a rail row's right-click (desktop). */
	| { kind: 'group-menu'; id: string }
	/** A contact row's right-click (desktop). The row is named; the pick follows as `sheet-select`. */
	| { kind: 'contact-menu'; address: string };

export type OnContactsUiEvent = (event: ContactsUiEvent) => void;
