//! What a signing sheet must never hide (clear-signing audit cs-10, cs-11):
//! the end of a message below a screenful of empty lines, and characters
//! that draw as nothing or turn the text around. One test per rule.

#![cfg(feature = "crux")]

mod support;

use serde_json::json;
use support::DomainDriver;
use vela_core::app::approval_guard::ApprovalGuard;
use vela_core::app::clear_signing::{
    ClearLocale, ClearMessageLine, ClearShellResult as Res, ClearSignMethod, ClearSigning,
    ClearSurface, ClearTextRun, Event, MESSAGE_BLANK_FOLD,
};
use vela_core::app::sign_confirm::{confirm_state, ConfirmBlock, ConfirmInput, MESSAGE_UNREAD_KEY};
use vela_core::app::sign_request::SignRequest;

type Sut = DomainDriver<ClearSigning>;

fn present(payload: &str, origin: Option<&str>) -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::PersonalSign,
        params: vec![payload.to_owned()],
        request_origin: origin.map(str::to_owned),
    });
    sut
}

fn text(line: &str) -> ClearMessageLine {
    ClearMessageLine::Text {
        runs: if line.is_empty() {
            Vec::new()
        } else {
            vec![ClearTextRun {
                text: line.to_owned(),
                marker: false,
            }]
        },
    }
}

fn hex(s: &str) -> String {
    let mut out = String::from("0x");
    for b in s.as_bytes() {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// cs-10 — a message's end is never a screen of nothing away
// ---------------------------------------------------------------------------

/// The audit's message: a sign-in, sixty empty lines, then the transfer.
/// The sixty fold into one counted line, so the transfer is drawn right
/// under the sign-in.
#[test]
fn a_long_run_of_empty_lines_folds_into_one_counted_line() {
    let payload = format!(
        "Welcome to Explore! Click to sign in.{}I authorise transfer of all my assets to 0xBAdBad.",
        "\n".repeat(61)
    );
    let message = present(&payload, None).view().message.expect("analyzed");
    assert_eq!(
        message.display_lines,
        vec![
            text("Welcome to Explore! Click to sign in."),
            ClearMessageLine::Blank { count: 60 },
            text("I authorise transfer of all my assets to 0xBAdBad."),
        ]
    );
    // The signed bytes are untouched.
    assert_eq!(message.payload, payload);
    assert_eq!(message.decoded_text.as_deref(), Some(payload.as_str()));
}

/// A paragraph break or two is ordinary prose and stays as written; the fold
/// starts at three. Lines of spaces, tabs and CRLF breaks count as empty.
#[test]
fn short_runs_stay_and_white_space_lines_count_as_empty() {
    assert_eq!(MESSAGE_BLANK_FOLD, 3);
    let message = present("a\n\nb\n\n\nc", None)
        .view()
        .message
        .expect("analyzed");
    assert_eq!(
        message.display_lines,
        vec![
            text("a"),
            text(""),
            text("b"),
            text(""),
            text(""),
            text("c"),
        ]
    );
    let message = present("a\r\n \r\n\t\r\n  \r\nb", None)
        .view()
        .message
        .expect("analyzed");
    assert_eq!(
        message.display_lines,
        vec![text("a"), ClearMessageLine::Blank { count: 3 }, text("b")]
    );
    // Trailing empty lines fold too: nothing below them, and the count says so.
    let message = present(&format!("end{}", "\n".repeat(10)), None)
        .view()
        .message
        .expect("analyzed");
    assert_eq!(
        message.display_lines,
        vec![text("end"), ClearMessageLine::Blank { count: 10 }]
    );
}

/// A hex payload is folded by its decoded text, the same as plain text.
#[test]
fn a_hex_encoded_message_folds_by_its_text() {
    let payload = hex(&format!("top{}bottom", "\n".repeat(9)));
    let message = present(&payload, None).view().message.expect("analyzed");
    assert_eq!(
        message.display_lines,
        vec![
            text("top"),
            ClearMessageLine::Blank { count: 8 },
            text("bottom")
        ]
    );
}

/// A line of nothing but hidden characters is not empty: its markers are the
/// line, so it is never folded away.
#[test]
fn a_line_of_hidden_characters_is_never_folded() {
    let message = present("a\n\n\u{200B}\n\nb", None)
        .view()
        .message
        .expect("analyzed");
    assert_eq!(message.display_lines.len(), 5);
    assert_eq!(
        message.display_lines[2],
        ClearMessageLine::Text {
            runs: vec![ClearTextRun {
                text: "⟨U+200B⟩".to_owned(),
                marker: true,
            }],
        }
    );
}

fn message_input(clear_surface: ClearSurface, unread: bool) -> ConfirmInput {
    let mut sign = DomainDriver::<SignRequest>::new().view();
    sign.confirm_gate_open = true;
    sign.confirm_block = None;
    let mut clear = present("hello", None).view();
    clear.surface = clear_surface;
    ConfirmInput {
        sign,
        guard: DomainDriver::<ApprovalGuard>::new().view(),
        clear,
        fee: None,
        speed_tier: None,
        message_unread: unread,
        sim: None,
        refuse_blind: false,
    }
}

/// The confirm of a message whose end has not been in sight is held, with its
/// line; the same message read to its end — or one that fits — signs.
#[test]
fn an_unread_message_holds_the_confirm_until_its_end_was_seen() {
    let held = confirm_state(&message_input(ClearSurface::MessageSign, true));
    assert!(!held.enabled);
    assert_eq!(held.block, Some(ConfirmBlock::MessageUnread));
    assert_eq!(held.key.as_deref(), Some(MESSAGE_UNREAD_KEY));
    assert_eq!(
        MESSAGE_UNREAD_KEY,
        "componentsUi.signing.confirmBlock.messageUnread"
    );

    let read = confirm_state(&message_input(ClearSurface::MessageSign, false));
    assert!(read.enabled);
    assert_eq!(read.block, None);
}

/// The observation counts only for a message, and only once nothing else
/// holds the confirm — any other reason is said first.
#[test]
fn the_unread_gate_is_a_message_s_and_comes_last() {
    // `eth_sign` has its own hard warning; a shell's stray report changes nothing.
    let other = confirm_state(&message_input(ClearSurface::EthSign, true));
    assert!(other.enabled);

    let mut input = message_input(ClearSurface::MessageSign, true);
    input.sign.is_signing = true;
    assert_eq!(confirm_state(&input).block, Some(ConfirmBlock::InFlight));
}

/// A view from a shell that does not observe the scroll reads as "not
/// unread": nothing is held for it.
#[test]
fn a_shell_that_does_not_observe_holds_nothing() {
    let input = message_input(ClearSurface::MessageSign, false);
    let json = vela_core::app::sign_confirm::confirm_state_json(
        &serde_json::to_string(&input.sign).unwrap(),
        &serde_json::to_string(&input.guard).unwrap(),
        &serde_json::to_string(&input.clear).unwrap(),
        None,
        None,
        false,
        None,
        false,
    )
    .expect("reads");
    assert!(json.contains("\"enabled\":true"), "{json}");
}

// ---------------------------------------------------------------------------
// cs-11 — hidden and direction-changing characters are drawn as markers
// ---------------------------------------------------------------------------

/// The audit's message: "Approve transfer to Bob <RLO>gpj.exe<PDF><ZWSP><ZWSP>
/// ok" read "Bob exe.jpg ok". Each hidden character is its own muted marker
/// run, the flag is up, and the bytes signed are the bytes asked for.
#[test]
fn a_bidi_override_in_a_message_is_drawn_as_markers_and_flagged() {
    let payload = "Approve transfer to Bob \u{202E}gpj.exe\u{202C}\u{200B}\u{200B} ok";
    let sut = present(&hex(payload), None);
    let view = sut.view();
    let message = view.message.expect("analyzed");
    assert!(message.has_invisible_chars);
    assert!(view.has_invisible_chars, "the sheet says so too");
    assert_eq!(message.decoded_text.as_deref(), Some(payload));
    let ClearMessageLine::Text { runs } = &message.display_lines[0] else {
        panic!("a text line");
    };
    let drawn: Vec<(&str, bool)> = runs.iter().map(|r| (r.text.as_str(), r.marker)).collect();
    assert_eq!(
        drawn,
        vec![
            ("Approve transfer to Bob ", false),
            ("⟨U+202E⟩", true),
            ("gpj.exe", false),
            ("⟨U+202C⟩", true),
            ("⟨U+200B⟩", true),
            ("⟨U+200B⟩", true),
            (" ok", false),
        ]
    );
}

#[test]
fn an_ordinary_message_raises_no_flag() {
    let view = present("Sign in to Example\n中文 · 👨\u{200D}👩\u{200D}👧", None).view();
    let message = view.message.expect("analyzed");
    assert!(!message.has_invisible_chars);
    assert!(!view.has_invisible_chars);
    assert_eq!(view.surface, ClearSurface::MessageSign);
}

/// A typed-data field a descriptor decodes is drawn display-safe, and the
/// sheet's flag is up; the payload the signer hashes is not the view's.
#[test]
fn a_decoded_typed_data_string_is_display_safe_and_flagged() {
    let encode_type = "Note(address to,string memo)";
    let type_hash = {
        let hash = vela_core::primitives::keccak256(encode_type.as_bytes());
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    let contract = "0x1234567890abcdef1234567890abcdef12345678";
    let mut root = serde_json::Map::new();
    root.insert(
        type_hash,
        json!({
            "display": { "formats": {
                encode_type: {
                    "intent": "Leave a note",
                    "fields": [
                        { "path": "memo", "label": "Memo", "format": "raw" },
                    ],
                },
            } },
        }),
    );
    let typed = json!({
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "chainId", "type": "uint256" },
                { "name": "verifyingContract", "type": "address" },
            ],
            "Note": [
                { "name": "to", "type": "address" },
                { "name": "memo", "type": "string" },
            ],
        },
        "primaryType": "Note",
        "domain": { "name": "Notes", "chainId": 1, "verifyingContract": contract },
        "message": { "to": contract, "memo": "pay Bob \u{202E}gpj.exe" },
    })
    .to_string();

    let mut sut = Sut::new();
    sut.dispatch(Event::ResolveTypedData {
        typed_data_json: typed,
        chain_id: 1,
        locale: ClearLocale::default(),
        from: None,
    });
    sut.resolve(Res::Clock {
        now_ms: 1_754_700_000_000.0,
    });
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/eip712/eip155-1/{contract}.json"),
        json: Some(serde_json::Value::Object(root).to_string()),
    });
    let view = sut.view();
    assert_eq!(view.surface, ClearSurface::ClearSign);
    let memo = view
        .result
        .as_ref()
        .and_then(|r| r.fields.iter().find(|f| f.label == "Memo"))
        .expect("memo field");
    assert_eq!(memo.value, "pay Bob ⟨U+202E⟩gpj.exe");
    assert!(view.has_invisible_chars);
    // The blind projection, held for every typed request, says it too.
    let blind = view.blind_typed.expect("projected");
    assert!(blind.has_invisible_chars);
}

/// The blind rows are display-safe, and the flag reads the WHOLE payload —
/// a hidden character in a key, or in any row, still counts.
#[test]
fn the_blind_typed_projection_is_display_safe_and_reads_the_whole_payload() {
    let mut sut = Sut::new();
    sut.dispatch(Event::ResolveTypedData {
        // Written out, not `json!`: the rows keep the payload's order.
        typed_data_json: concat!(
            r#"{"primaryType":"Order\u200b","#,
            r#""domain":{"name":"Ex\u202echange"},"#,
            r#""message":{"to\u2066":"Bob\u2069","n":1}}"#
        )
        .to_owned(),
        chain_id: 1,
        locale: ClearLocale::default(),
        from: None,
    });
    let blind = sut.view().blind_typed.expect("projected");
    assert!(blind.has_invisible_chars);
    assert_eq!(blind.primary_type.as_deref(), Some("Order⟨U+200B⟩"));
    assert_eq!(blind.domain_name.as_deref(), Some("Ex⟨U+202E⟩change"));
    assert_eq!(blind.fields[0].key, "to⟨U+2066⟩");
    assert_eq!(blind.fields[0].value, "Bob⟨U+2069⟩");

    let mut sut = Sut::new();
    sut.dispatch(Event::ResolveTypedData {
        typed_data_json: json!({
            "message": { "a": 1, "b": 2, "c": 3, "d": 4, "e": 5, "f": "hidden\u{202E}" },
        })
        .to_string(),
        chain_id: 1,
        locale: ClearLocale::default(),
        from: None,
    });
    let blind = sut.view().blind_typed.expect("projected");
    // dapp-9: every row is drawn now, the sixth too, display-safe.
    assert_eq!(blind.fields.len(), 6);
    assert_eq!(blind.fields[5].value, "hidden⟨U+202E⟩");
    assert!(blind.has_invisible_chars, "and it is signed");

    let mut sut = Sut::new();
    sut.dispatch(Event::ResolveTypedData {
        typed_data_json: json!({ "message": { "to": "Bob" } }).to_string(),
        chain_id: 1,
        locale: ClearLocale::default(),
        from: None,
    });
    assert!(
        !sut.view()
            .blind_typed
            .expect("projected")
            .has_invisible_chars
    );
}
