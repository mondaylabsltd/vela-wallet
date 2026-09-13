/**
 * The two contact forms' copy (spec 024), typed once so the phone sheet and
 * the desktop column — which render the SAME form in different containers —
 * cannot drift on which strings they need.
 */
export interface ContactFormCopy {
	title: string;
	nameLabel: string;
	namePlaceholder: string;
	addressLabel: string;
	addressPlaceholder: string;
	save: string;
	cancel: string;
	invalidAddress: string;
}

export interface GroupFormCopy {
	title: string;
	nameLabel: string;
	namePlaceholder: string;
	save: string;
	cancel: string;
}

export interface ContactDraft {
	name: string;
	address: string;
}
