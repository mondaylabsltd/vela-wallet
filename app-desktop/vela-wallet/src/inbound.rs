//! What arrives from outside the app's own fields (N-D3): a link or an
//! address PASTED onto the window (⌘V with no text field taking it), or a
//! file DROPPED onto it — a picture of a QR code, a saved link
//! (`.webloc`, `.url`, `.txt`).
//!
//! The shell only gets the TEXT out (decoding a picture is `executor::qr`'s,
//! reading a link file is here). What the text IS is the core's to say:
//!
//! - a payment link — `getvela.app/pay?…`, `velawallet://pay?…` — read by the
//!   core's `/pay` grammar (`executor::payment_request::pay_link`), an EIP-681
//!   request (`flows::eip681`, the scanner's tokenizer) or a bare address:
//!   the wallet's scan path decides (`WalletPage::scan_resolved`), so a drop
//!   or a paste opens exactly the Send a scan of the same text would — or,
//!   on the Contacts section, a bare address opens "Add contact" on it;
//! - a multisig proposal, invite or connect link — the getvela.app forms
//!   (`…/wallet#ms=…`, `…/wallet#msa=…`, `…/connect#…`) — read by
//!   `vela_core::multisig_link`. Multisig runs on the web wallet until it
//!   lands in this app (AUDIT F0.4), so a link that READS opens there — the
//!   canonical link the core rebuilds from what it read, at the wallet's own
//!   origin, never the text as pasted: a look-alike host in a pasted link
//!   cannot be what the browser opens;
//! - a multisig link that does NOT read is said, in the multisig words the
//!   web says it in (`home.multisig.proposal.badLink`, `…add.badInvite`,
//!   `…add.badChecksum`), never ignored (ms-22).
//!
//! Nothing here moves value: every path ends at a form the person reviews.

use vela_core::multisig_link::{self, AddLinkFault, BrokenLink};

/// What one piece of inbound text is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inbound {
    /// For the scan path: a payment link (read or refused by the core), an
    /// EIP-681 request or a bare address.
    Payment(String),
    /// A multisig link the core read, as the canonical link it rebuilt.
    Multisig { kind: MultisigKind, url: String },
    /// Meant as a multisig link, and it does not read: the corpus key of
    /// what the web says about it.
    Damaged { key: &'static str },
    /// Nothing this wallet acts on.
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultisigKind {
    Proposal,
    Invite,
    Connect,
}

/// The most text a paste or a dropped link file is read for. A link is a
/// few hundred bytes; an invite with every member's label a few thousand.
pub const MAX_TEXT: usize = 16 * 1024;

/// Read one piece of text. `locale` is the app's language, the path the
/// rebuilt link carries (`/<locale>/wallet`).
#[must_use]
pub fn classify(text: &str, locale: &str) -> Inbound {
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_TEXT {
        return Inbound::Nothing;
    }
    if crate::executor::payment_request::pay_link(text).is_some()
        || crate::flows::eip681::parse(text).is_some()
        || crate::flows::eip681::is_hex_address(text)
    {
        return Inbound::Payment(text.to_owned());
    }
    multisig(text, locale).unwrap_or(Inbound::Nothing)
}

/// A getvela.app multisig link, or a bare `#ms=…` / `#msa=…` hash.
fn multisig(text: &str, locale: &str) -> Option<Inbound> {
    let origin = multisig_link::SITE_ORIGIN;
    // A bare hash, as somebody copied it out of an address bar.
    let (path, hash) = if text.starts_with('#') {
        (String::new(), text)
    } else {
        let url = url::Url::parse(text).ok()?;
        let ours = url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| host == "getvela.app" || host.ends_with(".getvela.app"));
        if !ours {
            return None;
        }
        let at = text.find('#')?;
        (url.path().to_owned(), &text[at..])
    };
    let connect = path.trim_end_matches('/').ends_with("/connect");
    if connect {
        let read = multisig_link::read_connect_hash(hash);
        if read.damaged {
            return Some(Inbound::Damaged {
                key: "home.multisig.external.linkDamaged",
            });
        }
        if read.invite.is_none() && read.tx.is_none() {
            return None;
        }
        let page = format!("{origin}/{locale}/connect");
        let url = multisig_link::connect_link(
            &page,
            read.invite.as_deref(),
            &read.members,
            read.tx.as_ref(),
        );
        return Some(Inbound::Multisig {
            kind: MultisigKind::Connect,
            url,
        });
    }
    if !multisig_link::is_multisig_link(hash) {
        return None;
    }
    if let Some(link) = multisig_link::parse_proposal_link(hash) {
        return Some(Inbound::Multisig {
            kind: MultisigKind::Proposal,
            url: multisig_link::proposal_link(origin, locale, &link),
        });
    }
    if let Some(invite) = multisig_link::parse_add_link(hash) {
        let members = invite.members.unwrap_or_default();
        let url = multisig_link::add_link(
            origin,
            locale,
            &invite.multisig,
            &invite.chain_ids,
            &members,
        )
        .ok()?;
        return Some(Inbound::Multisig {
            kind: MultisigKind::Invite,
            url,
        });
    }
    Some(Inbound::Damaged {
        key: match (
            multisig_link::broken_link(hash),
            multisig_link::add_link_fault(hash),
        ) {
            (_, Some(AddLinkFault::Checksum)) => "home.multisig.add.badChecksum",
            (Some(BrokenLink::Add), _) => "home.multisig.add.badInvite",
            _ => "home.multisig.proposal.badLink",
        },
    })
}

/// The link a saved link file holds: a macOS `.webloc` (a property list with
/// one `URL` string), a Windows `.url` (an INI with `URL=`), or plain text.
#[must_use]
pub fn text_of_link_file(name: &str, bytes: &[u8]) -> Option<String> {
    if bytes.len() > MAX_TEXT {
        return None;
    }
    let text = String::from_utf8_lossy(bytes);
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".webloc") {
        let start = text.find("<string>")? + "<string>".len();
        let end = text[start..].find("</string>")? + start;
        return Some(unescape_xml(text[start..end].trim()));
    }
    if lower.ends_with(".url") {
        return text
            .lines()
            .find_map(|line| line.trim().strip_prefix("URL="))
            .map(|url| url.trim().to_owned());
    }
    if lower.ends_with(".txt") {
        return Some(text.trim().to_owned());
    }
    None
}

/// The five XML entities a plist escapes (an `&` in a query string is
/// `&amp;` in a `.webloc`).
fn unescape_xml(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Whether a dropped file is a picture the QR reader should look at.
#[must_use]
pub fn is_image(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".tif", ".tiff",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
    const TX: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";

    #[test]
    fn payments_and_addresses_go_to_the_scan_path() {
        let pay = format!("https://getvela.app/pay?to={SAFE}&chain=100");
        assert_eq!(classify(&pay, "en"), Inbound::Payment(pay.clone()));
        let scheme = format!("velawallet://pay?to={SAFE}&chain=100");
        assert_eq!(classify(&scheme, "en"), Inbound::Payment(scheme.clone()));
        assert_eq!(
            classify(&format!("  {SAFE}\n"), "en"),
            Inbound::Payment(SAFE.to_owned())
        );
        let eip681 = format!("ethereum:{SAFE}@100");
        assert_eq!(classify(&eip681, "en"), Inbound::Payment(eip681.clone()));
        // A refused payment link still goes there: the scan path says why.
        let refused = "https://getvela.app/pay?to=nope&chain=100";
        assert_eq!(
            classify(refused, "en"),
            Inbound::Payment(refused.to_owned())
        );
    }

    #[test]
    fn a_proposal_link_opens_its_canonical_form_at_the_wallets_origin() {
        let pasted = format!("https://wallet.getvela.app/de/wallet#ms=100.{SAFE}.{TX}");
        assert_eq!(
            classify(&pasted, "zh"),
            Inbound::Multisig {
                kind: MultisigKind::Proposal,
                url: format!("https://wallet.getvela.app/zh/wallet#ms=100.{SAFE}.{TX}"),
            }
        );
        // A bare hash reads too; a look-alike host is not ours.
        assert!(matches!(
            classify(&format!("#ms=100.{SAFE}.{TX}"), "en"),
            Inbound::Multisig {
                kind: MultisigKind::Proposal,
                ..
            }
        ));
        assert_eq!(
            classify(
                &format!("https://wallet.getvela.app.evil.example/en/wallet#ms=100.{SAFE}.{TX}"),
                "en"
            ),
            Inbound::Nothing
        );
    }

    #[test]
    fn an_invite_is_rebuilt_by_the_core_and_a_damaged_one_is_said() {
        let invite = format!("https://wallet.getvela.app/en/wallet#msa=100:{SAFE}");
        match classify(&invite, "en") {
            Inbound::Multisig {
                kind: MultisigKind::Invite,
                url,
            } => {
                assert!(
                    url.starts_with("https://wallet.getvela.app/en/wallet#msa="),
                    "{url}"
                );
                assert!(url.contains(SAFE), "{url}");
            }
            other => unreachable!("{other:?}"),
        }
        assert_eq!(
            classify("https://wallet.getvela.app/en/wallet#msa=100:0x88cc", "en"),
            Inbound::Damaged {
                key: "home.multisig.add.badInvite"
            }
        );
        // The checksum's letters changed on the way.
        let swapped = SAFE.replace("cCA0", "Cca0");
        assert_eq!(
            classify(&format!("#msa=100:{swapped}"), "en"),
            Inbound::Damaged {
                key: "home.multisig.add.badChecksum"
            }
        );
        assert_eq!(
            classify("#ms=100.0x88.0x11", "en"),
            Inbound::Damaged {
                key: "home.multisig.proposal.badLink"
            }
        );
    }

    #[test]
    fn a_connect_link_keeps_only_what_reads() {
        let lower = SAFE.to_lowercase();
        let link = format!("https://wallet.getvela.app/en/connect#for={lower}&ms=100.{lower}.{TX}");
        match classify(&link, "ja") {
            Inbound::Multisig {
                kind: MultisigKind::Connect,
                url,
            } => assert_eq!(
                url,
                format!("https://wallet.getvela.app/ja/connect#for={lower}&ms=100.{lower}.{TX}")
            ),
            other => unreachable!("{other:?}"),
        }
        // A connect page with nothing to carry is nothing to open.
        assert_eq!(
            classify("https://wallet.getvela.app/en/connect", "en"),
            Inbound::Nothing
        );
    }

    #[test]
    fn other_text_is_nothing() {
        for text in [
            "",
            "hello",
            "https://example.org/#ms=1",
            "0x1234",
            "velawallet://settings",
        ] {
            assert_eq!(classify(text, "en"), Inbound::Nothing, "{text}");
        }
        assert_eq!(classify(&"a".repeat(MAX_TEXT + 1), "en"), Inbound::Nothing);
    }

    #[test]
    fn what_a_damaged_link_says_is_in_every_language() {
        for key in [
            "home.multisig.proposal.badLink",
            "home.multisig.add.badInvite",
            "home.multisig.add.badChecksum",
            "home.multisig.external.linkDamaged",
        ] {
            for language in ["en", "zh", "ja", "de"] {
                let said = crate::loc::Loc::for_language(language).t(key);
                assert!(!said.is_empty() && said.as_ref() != key, "{language} {key}");
            }
        }
    }

    #[test]
    fn link_files_give_up_their_link() {
        let webloc = br#"<?xml version="1.0"?><plist version="1.0"><dict><key>URL</key><string>https://getvela.app/pay?to=0xabc&amp;chain=100</string></dict></plist>"#;
        assert_eq!(
            text_of_link_file("Pay.webloc", webloc).as_deref(),
            Some("https://getvela.app/pay?to=0xabc&chain=100")
        );
        let url = b"[InternetShortcut]\r\nURL=https://wallet.getvela.app/en/wallet#ms=1\r\n";
        assert_eq!(
            text_of_link_file("x.URL", url).as_deref(),
            Some("https://wallet.getvela.app/en/wallet#ms=1")
        );
        assert_eq!(
            text_of_link_file("a.txt", b" 0xabc \n").as_deref(),
            Some("0xabc")
        );
        assert_eq!(text_of_link_file("a.pdf", b"0xabc"), None);
        assert!(is_image("Card.PNG") && !is_image("notes.txt"));
    }
}
