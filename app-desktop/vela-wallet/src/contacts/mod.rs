//! Contacts (spec 018): fixture-driven list-management vocabulary.
//!
//! `fixtures` is the canonical data (specs/018-contacts-ui/data-model.md,
//! verbatim), `components` the reusable visuals; the page state and
//! interaction live in `wallet::page` — the desktop reuses `WalletPage` as
//! the three-column shell with a `Section` switch (research.md D1).

pub mod components;
pub mod fixtures;
pub mod live;
pub mod model;

use gpui::SharedString;

use crate::loc::Loc;

/// Every contacts string the desktop renders, resolved once per locale
/// (contracts/i18n-keys.md — new `contacts.*` keys plus the reused-key map).
pub struct ContactsStrings {
    pub title: SharedString,
    pub search_placeholder: SharedString,
    pub add_contact: SharedString,
    pub section_contacts: SharedString,
    pub section_groups: SharedString,
    pub all_contacts: SharedString,
    pub group_new: SharedString,
    pub add_member: SharedString,
    pub batch_send: SharedString,
    /// Template carrying `{{count}}` (dc4 caption, titled variant).
    pub batch_send_hint_titled: String,
    /// Template carrying `{{count}}` (家人 3 位成员).
    pub members_count: String,
    pub empty: SharedString,
    pub empty_hint: SharedString,
    pub import_file: SharedString,
    pub import_all: SharedString,
    /// What an import actually did. Same four keys the RN screen alerts with —
    /// an import that reports nothing is a feature that looks broken.
    pub import_done_title: SharedString,
    pub import_done_body: String,
    pub import_fail_title: SharedString,
    pub import_fail_body: SharedString,
    /// The add/edit sheet. 030 called this "blocked on drawn UI that does not
    /// exist" — the WORDS existed all along, and by 031 the app had a dialog
    /// idiom and a text field to put them in.
    pub add_title: SharedString,
    pub edit_title: SharedString,
    pub name_label: SharedString,
    pub name_placeholder: SharedString,
    pub address_placeholder: SharedString,
    pub save: SharedString,
    pub cancel: SharedString,
    pub group_name_label: SharedString,
    pub group_name_placeholder: SharedString,
    pub export_all: SharedString,
    pub import_group: SharedString,
    pub export_group: SharedString,
    pub group_rename: SharedString,
    pub group_delete: SharedString,
    pub move_group: SharedString,
    pub recent_activity: SharedString,
    pub view_all_activity: SharedString,
    pub delete_contact: SharedString,
    pub delete: SharedString,
    pub edit: SharedString,
    pub action_qr: SharedString,
    pub address_label: SharedString,
    /// Template carrying `{{query}}` (search-empty board variant).
    pub no_results: String,
    // reused keys (spec 015 map — no corpus change)
    pub action_send: SharedString,
    pub action_receive: SharedString,
    pub copy_address: SharedString,
    pub label_sent: SharedString,
    pub label_received: SharedString,
    pub yesterday: SharedString,
}

impl ContactsStrings {
    pub fn resolve(loc: &Loc) -> Self {
        let s = |key: &str| loc.t(key);
        let raw = |key: &str| loc.t(key).to_string();
        Self {
            title: s("contacts.title"),
            search_placeholder: s("contacts.searchPlaceholder"),
            add_contact: s("contacts.addContact"),
            section_contacts: s("contacts.sectionContacts"),
            section_groups: s("contacts.sectionGroups"),
            all_contacts: s("contacts.allContacts"),
            group_new: s("contacts.groupNew"),
            add_member: s("contacts.addMember"),
            batch_send: s("contacts.batchSend"),
            batch_send_hint_titled: raw("contacts.batchSendHintTitled"),
            members_count: raw("contacts.membersCount"),
            empty: s("contacts.empty"),
            empty_hint: s("contacts.emptyHint"),
            import_file: s("contacts.importFile"),
            import_all: s("contacts.importAll"),
            import_done_title: s("contacts.importDoneTitle"),
            import_done_body: raw("contacts.importDoneBody"),
            import_fail_title: s("contacts.importFailTitle"),
            import_fail_body: s("contacts.importFailBody"),
            add_title: s("contacts.addTitle"),
            edit_title: s("contacts.editTitle"),
            name_label: s("contacts.nameLabel"),
            name_placeholder: s("contacts.namePlaceholder"),
            address_placeholder: s("contacts.addressPlaceholder"),
            save: s("contacts.save"),
            cancel: s("contacts.cancel"),
            group_name_label: s("contacts.groupNameLabel"),
            group_name_placeholder: s("contacts.groupNamePlaceholder"),
            export_all: s("contacts.exportAll"),
            import_group: s("contacts.importGroup"),
            export_group: s("contacts.exportGroup"),
            group_rename: s("contacts.groupRename"),
            group_delete: s("contacts.groupDelete"),
            move_group: s("contacts.moveGroup"),
            recent_activity: s("contacts.recentActivity"),
            view_all_activity: s("contacts.viewAllActivity"),
            delete_contact: s("contacts.deleteContact"),
            delete: s("contacts.delete"),
            edit: s("contacts.edit"),
            action_qr: s("contacts.actionQr"),
            address_label: s("contacts.addressLabel"),
            no_results: raw("contacts.noResults"),
            action_send: s("componentsUi.dock.send"),
            action_receive: s("componentsUi.dock.receive"),
            copy_address: s("componentsUi.identiconViewer.copyAddress"),
            label_sent: s("history.labelSent"),
            label_received: s("history.labelReceived"),
            yesterday: s("componentsUi.dayGroup.yesterday"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SC discipline from specs 007/015: none of the contacts keys may echo
    /// in the locale the visual pass uses.
    #[test]
    fn contacts_strings_resolve_without_echo() {
        let loc = Loc::from_env();
        let s = ContactsStrings::resolve(&loc);
        for (value, key) in [
            (s.title.as_ref(), "contacts.title"),
            (s.search_placeholder.as_ref(), "contacts.searchPlaceholder"),
            (s.section_contacts.as_ref(), "contacts.sectionContacts"),
            (s.all_contacts.as_ref(), "contacts.allContacts"),
            (s.batch_send.as_ref(), "contacts.batchSend"),
            (s.import_all.as_ref(), "contacts.importAll"),
            (s.export_all.as_ref(), "contacts.exportAll"),
            (s.group_rename.as_ref(), "contacts.groupRename"),
            (s.recent_activity.as_ref(), "contacts.recentActivity"),
            (s.view_all_activity.as_ref(), "contacts.viewAllActivity"),
            (s.delete_contact.as_ref(), "contacts.deleteContact"),
            (s.action_qr.as_ref(), "contacts.actionQr"),
            (s.import_done_title.as_ref(), "contacts.importDoneTitle"),
            (s.import_done_body.as_str(), "contacts.importDoneBody"),
            (s.import_fail_title.as_ref(), "contacts.importFailTitle"),
            (s.import_fail_body.as_ref(), "contacts.importFailBody"),
            (s.add_title.as_ref(), "contacts.addTitle"),
            (s.edit_title.as_ref(), "contacts.editTitle"),
            (s.name_label.as_ref(), "contacts.nameLabel"),
            (s.save.as_ref(), "contacts.save"),
            (s.cancel.as_ref(), "contacts.cancel"),
            (s.group_name_label.as_ref(), "contacts.groupNameLabel"),
        ] {
            assert_ne!(value, key, "`{key}` echoed the key");
            assert!(!value.is_empty(), "`{key}` resolved empty");
        }
        assert!(
            s.members_count.contains("{{count}}"),
            "membersCount must be a template"
        );
        assert!(
            s.batch_send_hint_titled.contains("{{count}}"),
            "batchSendHintTitled must be a template"
        );
        assert!(
            s.no_results.contains("{{query}}"),
            "noResults must be a template"
        );
    }
}
