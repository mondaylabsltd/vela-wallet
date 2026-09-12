//! The contacts screen's display models, built from what the core decided.
//!
//! The sibling of `fixtures.rs`, never its replacement.
//!
//! ## What the core owns, and what this adds
//!
//! The core owns the book itself: saved entries merged with history-derived
//! suggestions, tombstone-suppressed, sorted favourites-first then most-recent.
//! **That order is not re-sorted here.** This adds only the two things the core
//! declines to decide because they are render rules — which of the three names a
//! row shows, and which letter it files under — and then groups the rows the
//! core already ordered.

use std::collections::HashMap;

use gpui::SharedString;

use vela_core::app::contacts::{Contact, ContactsView};

use crate::contacts::fixtures::ContactDetailModel;
use crate::contacts::model::{ContactRowModel, shorten};

/// The name a row shows: the person's own label, else a resolved identity, else
/// the shortened address.
///
/// The core carries all three and refuses to pick — `name` "wins over
/// `resolved_name` for display" is the only precedence it states, and it states
/// it as a comment about the shell's job rather than a field it computes.
fn display_name(contact: &Contact) -> SharedString {
    contact
        .name
        .as_deref()
        .filter(|value| !value.is_empty())
        .or(contact
            .resolved_name
            .as_deref()
            .filter(|value| !value.is_empty()))
        .map_or_else(
            || shorten(&contact.address),
            |value| SharedString::from(value.to_owned()),
        )
}

/// The set one tap produces: everything currently in it, with the tapped key
/// flipped.
///
/// Both directions of group membership go through this. The core offers ONE
/// event for each — `SetContactGroups` (which groups hold this contact) and
/// `SetGroupMembers` (which contacts this group holds) — and both carry the
/// WHOLE set, so "add" and "remove" are the same call with a different answer.
///
/// Written here rather than inline in the menus because the thing that goes
/// wrong is silent: a toggle that rebuilt the set from the tapped key alone
/// would empty every OTHER membership, and the menu is covering the rows that
/// would have shown it.
#[must_use]
pub fn set_after_toggle(current: &[(String, bool)], tapped: &str) -> Vec<String> {
    current
        .iter()
        .filter(|(key, member)| if key == tapped { !member } else { *member })
        .map(|(key, _)| key.clone())
        .collect()
}

/// One group's members — DC4.
///
/// `None` when the index names no group: the rail can change under an open
/// view, and drawing whichever group slid into that slot would put somebody
/// else's members under this group's name — with a 群发转账 button above them.
#[must_use]
pub fn group_members(
    view: &ContactsView,
    index: usize,
) -> Option<(SharedString, Vec<ContactRowModel>)> {
    let group = view.groups.get(index)?;
    // A group's rows carry the same letter the directory filed each member
    // under — the core's, looked up, not re-derived per list.
    let letters = letters(view);
    Some((
        SharedString::from(group.name.clone()),
        group
            .members
            .iter()
            .map(|contact| {
                let letter = letters
                    .get(&contact.address.to_lowercase())
                    .cloned()
                    .unwrap_or_else(|| SharedString::from(UNFILED));
                row(contact, letter)
            })
            .collect(),
    ))
}

/// One contact, in detail — DC2.
///
/// **The panel used to draw a FIXTURE while its delete and copy acted on the
/// real contact.** So somebody clicking their cousin saw Alice's name, Alice's
/// avatar and Alice's address, and the delete button removed the cousin. A
/// mismatch is worse than a mock: a mock is honestly a picture, and this was a
/// picture with a live weapon attached.
///
/// `None` when the index names nobody — the roster can change under an open
/// panel, and drawing the row that took its place would silently swap who the
/// delete button is pointed at.
#[must_use]
pub fn detail(
    view: &ContactsView,
    index: usize,
    feed: &vela_core::app::activity_feed::FeedView,
    wallet: &crate::wallet::WalletStrings,
    hidden: bool,
) -> Option<ContactDetailModel> {
    let contact = rows(view).into_iter().nth(index)?;
    let address = contact.address_full.to_string();
    let lower = address.to_lowercase();
    Some(ContactDetailModel {
        name: contact.name.clone(),
        // The ADDRESS, not the name: two contacts a person named the same must
        // not draw the same avatar, and the avatar is how somebody checks they
        // are looking at the right one.
        seed: contact.address_full.clone(),
        chips: view
            .groups
            .iter()
            .filter(|group| {
                group
                    .members
                    .iter()
                    .any(|member| member.address.to_lowercase() == lower)
            })
            .map(|group| SharedString::from(group.name.clone()))
            .collect(),
        address_full: contact.address_full,
        // What this person and I have actually exchanged, from the same feed
        // the home draws. Matched on the counterparty, which is the only thing
        // that makes a row "theirs".
        activity: feed
            .rows
            .iter()
            .filter_map(|row| match row {
                vela_core::app::activity_feed::FeedRow::Item { item }
                    if item
                        .counterparty
                        .as_ref()
                        .is_some_and(|other| other.to_lowercase() == lower) =>
                {
                    Some(crate::wallet::live::activity_row(
                        feed, item, wallet, hidden,
                    ))
                }
                _ => None,
            })
            .collect(),
    })
}

/// The group rail: the person's own groups, with how many people are in each.
///
/// The rail drew `fixtures::GROUPS` for a signed-in person too — 家人 / 工作 /
/// … under somebody's real address book. The same shape the home's asset strip
/// had before phase 17, and found the same way: by reading what the surface
/// actually calls rather than trusting that "contacts is live".
///
/// Returns the id alongside, because the row that opens a group's menu has to
/// name WHICH group to the core, and an index into a list that reorders is not
/// a name.
#[must_use]
pub fn groups(view: &ContactsView) -> Vec<(SharedString, SharedString, u32)> {
    view.groups
        .iter()
        .map(|group| {
            (
                SharedString::from(group.id.clone()),
                SharedString::from(group.name.clone()),
                u32::try_from(group.members.len()).unwrap_or(u32::MAX),
            )
        })
        .collect()
}

/// One row per contact, in the core's order.
#[must_use]
pub fn rows(view: &ContactsView) -> Vec<ContactRowModel> {
    let letters = letters(view);
    view.contacts
        .iter()
        .map(|contact| {
            let letter = letters
                .get(&contact.address.to_lowercase())
                .cloned()
                .unwrap_or_else(|| SharedString::from(UNFILED));
            row(contact, letter)
        })
        .collect()
}

/// The letter the CORE filed each address under.
///
/// Spec 028 moved the A–Z rule into `app/contacts_initials.rs` — a per-codepoint
/// pinyin initial table, so 阿豪 files under A rather than the `#` this shell's
/// own ASCII test produced. Two clients disagreeing about which letter one
/// person lives under is the kind of divergence nobody reports and everybody
/// notices.
fn letters(view: &ContactsView) -> HashMap<String, SharedString> {
    let mut out = HashMap::new();
    for section in &view.sections {
        let letter = SharedString::from(section.letter.clone());
        for address in &section.addresses {
            out.insert(address.to_lowercase(), letter.clone());
        }
    }
    out
}

/// A contact the core did not file. Not reachable through the core's own view
/// (it sections every contact it lists), so this is the shape of a
/// disagreement between the two lists, drawn rather than panicked on.
const UNFILED: &str = "#";

/// One contact as a row. The name precedence is `display_name`'s; the letter
/// is the core's, never re-derived here.
fn row(contact: &Contact, letter: SharedString) -> ContactRowModel {
    ContactRowModel {
        section: letter,
        name: display_name(contact),
        address_display: shorten(&contact.address),
        address_full: SharedString::from(contact.address.clone()),
    }
}

/// The roster as the screen draws it — the core's directory, looked up.
///
/// Neither grouping nor sorting is decided here any more. `ContactsView.sections`
/// arrives already lettered (A–Z then `#`) with the book's own order inside each
/// letter, so this only resolves each address back to the contact it names.
/// The run-length grouping this used to do also had a bug the core does not:
/// two non-adjacent contacts under one letter became two sections.
#[must_use]
pub fn sections(view: &ContactsView) -> Vec<(SharedString, Vec<ContactRowModel>)> {
    let by_address: HashMap<String, &Contact> = view
        .contacts
        .iter()
        .map(|contact| (contact.address.to_lowercase(), contact))
        .collect();
    view.sections
        .iter()
        .map(|section| {
            let letter = SharedString::from(section.letter.clone());
            let rows = section
                .addresses
                .iter()
                .filter_map(|address| by_address.get(&address.to_lowercase()).copied())
                .map(|contact| row(contact, letter.clone()))
                .collect();
            (letter, rows)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::contacts::{ContactKind, ContactSource};

    fn contact(address: &str, name: Option<&str>, resolved: Option<&str>) -> Contact {
        Contact {
            address: address.to_owned(),
            name: name.map(str::to_owned),
            resolved_name: resolved.map(str::to_owned),
            resolved_source: None,
            kind: ContactKind::Eoa,
            favorite: false,
            note: None,
            tx_count: 0,
            last_used_ms: 0.0,
            first_seen_ms: 0.0,
            source: ContactSource::Manual,
        }
    }

    /// A view whose directory is built by the CORE's rule, not by an empty
    /// `Vec` — an empty `sections` would file every row under `#` and leave
    /// these tests passing while proving nothing about the letters.
    fn view(contacts: Vec<Contact>) -> ContactsView {
        ContactsView {
            loaded: true,
            sections: vela_core::app::contacts::section_contacts(&contacts),
            contacts,
            groups: Vec::new(),
            last_import: None,
            import_failure: None,
            export: None,
            recipient: None,
        }
    }

    /// Ticking one group leaves the others exactly as they were.
    ///
    /// The menu covers the chips that would show the damage, so a toggle that
    /// dropped the other memberships would be invisible until the person
    /// closed it — and by then the core has already been told.
    #[test]
    fn ticking_one_group_does_not_empty_the_others() {
        let groups = vec![
            ("family".to_owned(), true),
            ("payroll".to_owned(), false),
            ("friends".to_owned(), true),
        ];

        // Joining one: the two it was already in survive.
        let joined = set_after_toggle(&groups, "payroll");
        assert_eq!(joined, vec!["family", "payroll", "friends"]);

        // Leaving one: only that one goes.
        let left = set_after_toggle(&groups, "family");
        assert_eq!(left, vec!["friends"]);

        // Leaving the last one is an empty set, not "leave it alone" — the
        // core reads the set it is given.
        let one = vec![("family".to_owned(), true)];
        assert!(set_after_toggle(&one, "family").is_empty());

        // A group nobody tapped changes nothing.
        assert_eq!(set_after_toggle(&groups, "nope"), vec!["family", "friends"]);
    }

    /// A contact saved without a name shows the identity the core resolved for
    /// it — which is the whole point of asking.
    ///
    /// This precedence has been implemented since 031 and, until spec 034, was
    /// unreachable: nothing in this shell ever dispatched `InspectRecipient`,
    /// so `resolved_name` was never written and the middle branch never ran.
    /// The book showed `0x1234…5678` for an address the network could have
    /// named.
    #[test]
    fn an_unnamed_contact_shows_the_name_the_core_resolved() {
        const ADDR: &str = "0xaaaa000000000000000000000000000000000001";

        // Nothing known: the address, shortened.
        let bare = display_name(&contact(ADDR, None, None));
        assert!(bare.contains('…'), "{bare}");

        // Resolved: the identity, which is what the inspection writes back.
        assert_eq!(
            display_name(&contact(ADDR, None, Some("vitalik.eth"))),
            "vitalik.eth"
        );

        // The person's own label still wins — the core states that precedence
        // and states it as the shell's job.
        assert_eq!(
            display_name(&contact(ADDR, Some("Dad"), Some("vitalik.eth"))),
            "Dad"
        );

        // An empty string is not a name. Both fields can arrive empty rather
        // than absent from a store another client wrote.
        let empty = display_name(&contact(ADDR, Some(""), Some("")));
        assert!(empty.contains('…'), "{empty}");
    }

    /// The letter now comes from spec 028's per-codepoint table, so a Chinese
    /// name files under its pinyin initial. This shell's own rule filed 阿豪
    /// under `#`, which is the same person in a different place on two
    /// clients.
    #[test]
    fn a_chinese_name_files_under_its_pinyin_initial() {
        let view = view(vec![contact(
            "0xaaaa000000000000000000000000000000000001",
            Some("阿豪"),
            None,
        )]);
        assert_eq!(rows(&view)[0].section, SharedString::from("A"));
        assert_eq!(sections(&view)[0].0, SharedString::from("A"));
    }

    /// The run-length grouping this file used to do produced TWO `A` sections
    /// when the book's order interleaved them. The core's directory does not.
    #[test]
    fn one_letter_is_one_section_however_the_book_is_ordered() {
        let view = view(vec![
            contact(
                "0xaaaa000000000000000000000000000000000001",
                Some("Ada"),
                None,
            ),
            contact(
                "0xbbbb000000000000000000000000000000000002",
                Some("Bo"),
                None,
            ),
            contact(
                "0xcccc000000000000000000000000000000000003",
                Some("Amy"),
                None,
            ),
        ]);
        let letters: Vec<_> = sections(&view).into_iter().map(|(l, _)| l).collect();
        assert_eq!(
            letters,
            vec![SharedString::from("A"), SharedString::from("B")]
        );
        assert_eq!(
            sections(&view)[0].1.len(),
            2,
            "Ada and Amy share their letter"
        );
    }

    #[test]
    fn a_saved_name_wins_then_a_resolved_one_then_the_address() {
        let rows = rows(&view(vec![
            contact(
                "0xaaaa000000000000000000000000000000000001",
                Some("Ada"),
                Some("ada.eth"),
            ),
            contact(
                "0xbbbb000000000000000000000000000000000002",
                None,
                Some("bob.eth"),
            ),
            contact("0xcccc000000000000000000000000000000000003", None, None),
        ]));
        assert_eq!(rows[0].name, SharedString::from("Ada"));
        assert_eq!(rows[1].name, SharedString::from("bob.eth"));
        assert_eq!(rows[2].name, SharedString::from("0xcccc…0003"));
    }

    /// The detail is about the contact that was opened, and its avatar is
    /// seeded by the address rather than the name.
    #[test]
    fn the_detail_is_about_the_contact_that_was_opened() {
        use vela_core::app::activity_feed::{
            ActivityFeed, Event as FeedEvent, FeedDirection, FeedItem, FeedRow, FeedView,
        };
        use vela_core::app::contacts::ContactGroupView;

        let mut book = view(vec![
            contact(
                "0xAAA0000000000000000000000000000000000001",
                Some("Alice"),
                None,
            ),
            contact(
                "0xBBB0000000000000000000000000000000000002",
                Some("Cousin"),
                None,
            ),
        ]);
        book.groups = vec![ContactGroupView {
            id: "g1".to_owned(),
            name: "Family".to_owned(),
            color: None,
            members: vec![contact(
                "0xBBB0000000000000000000000000000000000002",
                Some("Cousin"),
                None,
            )],
        }];

        let mut host = crate::core_host::CoreHost::<ActivityFeed>::new();
        let _ = host.dispatch(FeedEvent::AccountSwitched {
            address: "0xme".to_owned(),
        });
        let feed = FeedView {
            rows: vec![FeedRow::Item {
                item: FeedItem {
                    id: "t1".to_owned(),
                    direction: FeedDirection::Out,
                    // Cousin's, in a different case — the match must not care.
                    counterparty: Some("0xbbb0000000000000000000000000000000000002".to_owned()),
                    alias: None,
                    value: Some("2".to_owned()),
                    symbol: "xDAI".to_owned(),
                    decimals: Some(18),
                    usd_value: 2.0,
                    chain_id: 100,
                    timestamp: 1_788_500_000.0,
                    day_start_ms: 0.0,
                    tx_hash: None,
                    batch: None,
                },
            }],
            ..host.view()
        };
        let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());

        // Row 1 is the cousin, and the panel must be about the cousin.
        let cousin =
            detail(&book, 1, &feed, &wallet, false).unwrap_or_else(|| unreachable!("row 1 exists"));
        assert_eq!(cousin.name, "Cousin");
        assert_eq!(
            cousin.address_full,
            "0xBBB0000000000000000000000000000000000002"
        );
        // Seeded by the ADDRESS: two contacts named the same must not share an
        // avatar, and the avatar is how somebody checks they have the right one.
        assert_eq!(cousin.seed, cousin.address_full);
        assert_eq!(cousin.chips, vec![SharedString::from("Family")]);
        // Their own history, matched case-insensitively.
        assert_eq!(cousin.activity.len(), 1);

        // Alice is in no group and has nothing with me.
        let alice =
            detail(&book, 0, &feed, &wallet, false).unwrap_or_else(|| unreachable!("row 0 exists"));
        assert_eq!(alice.name, "Alice");
        assert!(alice.chips.is_empty());
        assert!(alice.activity.is_empty());

        // The roster moved: no panel rather than the wrong one.
        assert!(detail(&book, 9, &feed, &wallet, false).is_none());

        // And a group's members are that group's.
        let (name, members) =
            group_members(&book, 0).unwrap_or_else(|| unreachable!("group 0 exists"));
        assert_eq!(name, "Family");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].name, "Cousin");
        assert!(group_members(&book, 5).is_none());
    }

    /// The rail lists the person's own groups, with the id each row needs.
    #[test]
    fn the_group_rail_is_the_persons_own_and_carries_each_id() {
        use vela_core::app::contacts::ContactGroupView;

        let mut book = view(Vec::new());
        book.groups = vec![
            ContactGroupView {
                id: "g-family".to_owned(),
                name: "Family".to_owned(),
                color: None,
                members: vec![contact("0xaaa", None, None), contact("0xbbb", None, None)],
            },
            ContactGroupView {
                id: "g-empty".to_owned(),
                name: "Work".to_owned(),
                color: None,
                members: Vec::new(),
            },
        ];

        let rows = groups(&book);
        assert_eq!(rows.len(), 2);
        // The ID, not the index: a rail that reorders would otherwise delete
        // whichever group slid into the slot that was clicked.
        assert_eq!(rows[0].0, "g-family");
        assert_eq!(rows[0].1, "Family");
        assert_eq!(rows[0].2, 2);
        // An empty group is still a group — it is a thing the person made, and
        // hiding it would make its delete unreachable.
        assert_eq!(rows[1].0, "g-empty");
        assert_eq!(rows[1].2, 0);

        assert!(groups(&view(Vec::new())).is_empty());
    }

    /// Two rules, and 028 moved the line between them.
    ///
    /// The LETTERS are a directory: A–Z then `#`, so `Ada` heads the list even
    /// though the book puts the Zs first. Inside a letter the book's own order
    /// — favourites first, then most recent — must survive untouched; that is
    /// the core's product rule and re-sorting it here would silently override
    /// it. This test asserted the old shell rule (letters in book order) and
    /// is updated, not deleted: half of it was always right.
    #[test]
    fn the_letters_are_a_directory_and_the_book_orders_within_one() {
        let rows = view(vec![
            contact("0x1", Some("Zoe"), None),
            contact("0x2", Some("Zack"), None),
            contact("0x3", Some("Ada"), None),
        ]);
        let sections = sections(&rows);
        assert_eq!(sections.len(), 2);
        assert_eq!(
            sections[0].0,
            SharedString::from("A"),
            "A–Z, not book order"
        );
        assert_eq!(sections[1].0, SharedString::from("Z"));
        assert_eq!(
            sections[1]
                .1
                .iter()
                .map(|r| r.name.to_string())
                .collect::<Vec<_>>(),
            vec!["Zoe", "Zack"],
            "the core's order inside a letter must survive"
        );
    }

    /// A name that is not a letter files under `#`, as the mocks draw.
    #[test]
    fn a_non_alphabetic_name_files_under_hash() {
        let rows = rows(&view(vec![contact(
            "0xdddd000000000000000000000000000000000004",
            Some("42"),
            None,
        )]));
        assert_eq!(rows[0].section, SharedString::from("#"));
    }
}
