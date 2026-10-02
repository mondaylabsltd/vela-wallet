//! Rules of payment requests, one test per rule: the acknowledge gate, the
//! builder's encode-what-you-display guarantees, and the strict `/pay`
//! grammar that replaces a confirmed crash and a confirmed silent misparse.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::payment_request::{
    Event, Mode, PaymentRequest, PaymentRequestOperation as Op, PaymentRequestShellResult as Res,
};
use vela_core::app::send::{
    Event as SendEvent, Send, SendAccountRef, SendDisplayContext, SendOpenParams,
};

type Sut = DomainDriver<PaymentRequest>;

const ADDR: &str = "0x52908400098527886E0F7030069857D2E4169EE7";
const USDC: &str = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";
const BASE_URL: &str = "https://wallet.getvela.app/pay";

fn started() -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Start {
        account: ADDR.to_owned(),
        recipient: ADDR.to_owned(),
        base_url: BASE_URL.to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::ReadAck {
            account: ADDR.to_owned()
        }]
    );
    sut
}

fn acknowledged() -> Sut {
    let mut sut = started();
    sut.resolve(Res::AckFlag { acknowledged: true });
    sut
}

fn pick_usdc(sut: &mut Sut) {
    sut.dispatch(Event::AssetPicked {
        chain_id: 8453,
        token_address: Some(USDC.to_owned()),
        symbol: "USDC".to_owned(),
        decimals: 6,
        network_name: "Base".to_owned(),
    });
}

fn pay_query(amount: Option<&str>) -> Event {
    Event::LinkOpened {
        to: Some(ADDR.to_owned()),
        chain: Some("8453".to_owned()),
        token: Some(USDC.to_owned()),
        amount: amount.map(str::to_owned),
        sym: Some("USDC".to_owned()),
        dec: Some("6".to_owned()),
        net: Some("Base".to_owned()),
    }
}

// ---------------------------------------------------------------------------
// The acknowledge gate
// ---------------------------------------------------------------------------

/// FR-014 — while the flag loads, the QR stays covered and copy/save stay
/// locked: a first visit must never flash the QR.
#[test]
fn gate_covers_everything_while_loading() {
    let sut = started();
    let view = sut.view();
    assert!(view.gate_loading);
    assert!(!view.acknowledged);
    assert!(!view.can_copy);
    assert!(!view.can_save);
}

/// FR-014 — a read error means "show the gate" (never "skip the warning").
#[test]
fn unread_flag_shows_the_gate() {
    let mut sut = started();
    sut.resolve(Res::AckFlag {
        acknowledged: false,
    });
    let view = sut.view();
    assert!(!view.gate_loading);
    assert!(!view.acknowledged);
    assert!(!view.can_copy);
}

/// Acknowledging lifts the gate immediately (optimistic, as today) and
/// persists best-effort.
#[test]
fn acknowledge_unlocks_and_persists() {
    let mut sut = started();
    sut.resolve(Res::AckFlag {
        acknowledged: false,
    });
    let ops = sut.dispatch(Event::Acknowledge);
    assert_eq!(
        ops,
        vec![Op::WriteAck {
            account: ADDR.to_owned()
        }]
    );
    let view = sut.view();
    assert!(view.acknowledged);
    assert!(view.can_copy);
    assert!(view.can_save);
}

/// Acknowledge is only meaningful from the unacknowledged gate — a stray
/// event while loading or after acknowledgement writes nothing.
#[test]
fn acknowledge_is_idempotent() {
    let mut sut = started();
    assert!(sut.dispatch(Event::Acknowledge).is_empty(), "loading");
    sut.resolve(Res::AckFlag { acknowledged: true });
    assert!(sut.dispatch(Event::Acknowledge).is_empty(), "already done");
}

// ---------------------------------------------------------------------------
// The builder
// ---------------------------------------------------------------------------

/// FR-015 — address mode: QR and copy are the bare address.
#[test]
fn address_mode_exposes_the_bare_address() {
    let sut = acknowledged();
    let view = sut.view();
    assert_eq!(view.mode, Mode::Address);
    assert_eq!(view.qr_value, ADDR);
    assert_eq!(view.copy_payload, ADDR);
}

/// The default asset is native ETH on Ethereum; an open request is the plain
/// `ethereum:{addr}@1` URI.
#[test]
fn open_native_request_builds_the_plain_uri() {
    let mut sut = acknowledged();
    sut.dispatch(Event::ModeChanged {
        mode: Mode::Request,
    });
    let view = sut.view();
    assert_eq!(view.eip681_uri, format!("ethereum:{ADDR}@1"));
    assert_eq!(view.qr_value, view.eip681_uri);
    assert!(!view.has_amount);
}

/// FR-012 — the money line: a 1.5 USDC request encodes exactly 1500000 base
/// units (6 decimals), byte-identical to the TS builder.
#[test]
fn token_request_encodes_exact_base_units() {
    let mut sut = acknowledged();
    sut.dispatch(Event::ModeChanged {
        mode: Mode::Request,
    });
    pick_usdc(&mut sut);
    sut.dispatch(Event::AmountChanged {
        text: "1.5".to_owned(),
    });
    let view = sut.view();
    assert_eq!(
        view.eip681_uri,
        format!("ethereum:{USDC}@8453/transfer?address={ADDR}&uint256=1500000")
    );
    assert!(view.has_amount);
}

/// A native request with an amount uses `?value=` wei.
#[test]
fn native_amount_encodes_wei() {
    let mut sut = acknowledged();
    sut.dispatch(Event::ModeChanged {
        mode: Mode::Request,
    });
    sut.dispatch(Event::AmountChanged {
        text: "0.25".to_owned(),
    });
    let view = sut.view();
    assert_eq!(
        view.eip681_uri,
        format!("ethereum:{ADDR}@1?value=250000000000000000")
    );
}

/// FR-015 — request mode copies the PAY-LINK, never the raw `ethereum:` URI,
/// with today's exact parameter order and encoding.
#[test]
fn request_mode_copies_the_pay_link() {
    let mut sut = acknowledged();
    sut.dispatch(Event::ModeChanged {
        mode: Mode::Request,
    });
    pick_usdc(&mut sut);
    sut.dispatch(Event::AmountChanged {
        text: "1.5".to_owned(),
    });
    let view = sut.view();
    assert_eq!(
        view.copy_payload,
        format!("{BASE_URL}?to={ADDR}&chain=8453&token={USDC}&amount=1.5&sym=USDC&dec=6&net=Base")
    );
}

/// Zero and empty amounts are OPEN requests — no `uint256`, no `amount=`.
#[test]
fn zero_amount_is_an_open_request() {
    let mut sut = acknowledged();
    sut.dispatch(Event::ModeChanged {
        mode: Mode::Request,
    });
    pick_usdc(&mut sut);
    for text in ["", "0", "0.0", "."] {
        sut.dispatch(Event::AmountChanged {
            text: text.to_owned(),
        });
        let view = sut.view();
        assert!(!view.has_amount, "{text:?} is not a positive amount");
        assert!(
            !view.eip681_uri.contains("uint256"),
            "{text:?} must not encode"
        );
        assert!(!view.pay_link.contains("amount="));
    }
}

/// The sanitize table — an exact port of `sanitizeAmount`, quirk included.
#[test]
fn amount_sanitation_matches_the_screen() {
    let mut sut = acknowledged();
    pick_usdc(&mut sut); // 6 decimals
    let cases = [
        ("1a5", "15"),             // strip non-numerics
        ("1.2.3", "1.2."),         // >1 dot: raw input minus its last char
        ("1.1234567", "1.123456"), // clamp to the asset's precision
        ("00.5", "00.5"),          // leading zeros pass through (as today)
    ];
    for (input, expected) in cases {
        sut.dispatch(Event::AmountChanged {
            text: input.to_owned(),
        });
        assert_eq!(sut.view().amount, expected, "input {input:?}");
    }
}

/// FR-012 — switching to a lower-precision asset re-clamps the typed amount,
/// so the encoded value can never silently drop digits later.
#[test]
fn asset_switch_reclamps_precision() {
    let mut sut = acknowledged();
    sut.dispatch(Event::AmountChanged {
        text: "1.123456789012345678".to_owned(),
    }); // fine for ETH's 18
    assert_eq!(sut.view().amount, "1.123456789012345678");
    pick_usdc(&mut sut); // 6 decimals
    assert_eq!(sut.view().amount, "1.123456");
}

// ---------------------------------------------------------------------------
// The /pay validator
// ---------------------------------------------------------------------------

/// A well-formed link validates with exact base units and the same locked
/// parameters the screen passes to Send today.
#[test]
fn valid_pay_link_normalizes() {
    let mut sut = Sut::new();
    sut.dispatch(pay_query(Some("1.5")));
    let view = sut.view();
    assert_eq!(view.pay_valid, Some(true));
    let pay = view.pay.expect("valid request");
    assert_eq!(pay.recipient, ADDR);
    assert_eq!(pay.chain_id, 8453);
    assert_eq!(pay.token_address.as_deref(), Some(USDC));
    assert_eq!(pay.amount.as_deref(), Some("1.5"));
    assert_eq!(pay.amount_base.as_deref(), Some("1500000"));
    assert_eq!(pay.symbol, "USDC");
    assert_eq!(pay.decimals, 6);
    assert_eq!(pay.network_name, "Base");
    assert_eq!(
        pay.eip681_uri,
        format!("ethereum:{USDC}@8453/transfer?address={ADDR}&uint256=1500000")
    );
}

/// An omitted amount is an open request — valid, no base units.
#[test]
fn open_pay_link_is_valid_without_an_amount() {
    let mut sut = Sut::new();
    sut.dispatch(pay_query(None));
    let view = sut.view();
    assert_eq!(view.pay_valid, Some(true));
    let pay = view.pay.expect("valid request");
    assert_eq!(pay.amount, None);
    assert_eq!(pay.amount_base, None);
}

/// research.md D8 — the grammar table. `1e18` crashes today's page mid-render;
/// `0x10` zero-pads into hex and silently prefills ≈7.5×10⁴ tokens; `1,5` and
/// negatives crash; over-precision silently truncates. All are now the
/// invalid surface — never a crash, never a different amount than displayed.
#[test]
fn malformed_amounts_are_invalid_not_crashes() {
    for bad in [
        "1e18",
        "0x10",
        "1,5",
        "-3",
        "+3",
        ".",
        "1.2345678",
        "NaN",
        "١٢",
    ] {
        let mut sut = Sut::new();
        sut.dispatch(pay_query(Some(bad)));
        let view = sut.view();
        assert_eq!(
            view.pay_valid,
            Some(false),
            "amount {bad:?} must be invalid"
        );
        assert!(view.pay.is_none());
    }
}

/// Shapes our own sanitizer can emit — `.5`, `1.` — are in already-shared
/// links and MUST stay valid under the strict grammar.
#[test]
fn builder_emittable_shapes_stay_valid() {
    for (amount, base) in [(".5", "500000"), ("1.", "1000000")] {
        let mut sut = Sut::new();
        sut.dispatch(pay_query(Some(amount)));
        let view = sut.view();
        assert_eq!(view.pay_valid, Some(true), "amount {amount:?}");
        assert_eq!(
            view.pay.expect("valid").amount_base.as_deref(),
            Some(base),
            "amount {amount:?}"
        );
    }
}

/// The recipient and chain follow today's validity rule (`isAddress` +
/// numeric chain); garbage yields the invalid surface exactly as today.
#[test]
fn bad_recipient_or_chain_is_invalid() {
    let cases: [(Option<&str>, Option<&str>); 4] = [
        (None, Some("8453")),
        (Some("0x1234"), Some("8453")),
        (Some(ADDR), None),
        (Some(ADDR), Some("84x3")),
    ];
    for (to, chain) in cases {
        let mut sut = Sut::new();
        sut.dispatch(Event::LinkOpened {
            to: to.map(str::to_owned),
            chain: chain.map(str::to_owned),
            token: None,
            amount: None,
            sym: None,
            dec: None,
            net: None,
        });
        assert_eq!(
            sut.view().pay_valid,
            Some(false),
            "to={to:?} chain={chain:?}"
        );
    }
}

/// Display-hint defaults survive: no `sym` reads "tokens", no `net` reads
/// "Chain {id}", `dec` falls back to 18 — including the `|| 18` quirk where
/// an explicit 0 also falls back (ported faithfully).
#[test]
fn hint_defaults_match_the_screen() {
    let mut sut = Sut::new();
    sut.dispatch(Event::LinkOpened {
        to: Some(ADDR.to_owned()),
        chain: Some("1".to_owned()),
        token: None,
        amount: Some("2".to_owned()),
        sym: None,
        dec: Some("0".to_owned()),
        net: None,
    });
    let view = sut.view();
    let pay = view.pay.expect("valid");
    assert_eq!(pay.symbol, "tokens");
    assert_eq!(pay.network_name, "Chain 1");
    assert_eq!(pay.decimals, 18);
    assert_eq!(pay.amount_base.as_deref(), Some("2000000000000000000"));
}

/// Round-trip: what the builder encodes, the validator accepts and reads back
/// to the same base units (FR-012's "one conversion" guarantee).
#[test]
fn build_then_parse_round_trips() {
    let mut sut = acknowledged();
    sut.dispatch(Event::ModeChanged {
        mode: Mode::Request,
    });
    pick_usdc(&mut sut);
    sut.dispatch(Event::AmountChanged {
        text: "12.000001".to_owned(),
    });
    let built = sut.view();

    let mut pay = Sut::new();
    pay.dispatch(pay_query(Some("12.000001")));
    let parsed = pay.view().pay.expect("valid");
    assert_eq!(parsed.amount_base.as_deref(), Some("12000001"));
    assert!(built
        .eip681_uri
        .ends_with(&format!("uint256={}", parsed.amount_base.unwrap())));
}

// ---------------------------------------------------------------------------
// The network switch (spec 090)
// ---------------------------------------------------------------------------

/// Gnosis' own coin — what a network row asks for.
fn pick_gnosis(sut: &mut Sut) {
    sut.dispatch(Event::AssetPicked {
        chain_id: 100,
        token_address: None,
        symbol: "XDAI".to_owned(),
        decimals: 18,
        network_name: "Gnosis".to_owned(),
    });
}

fn include_network(sut: &mut Sut, include: bool) {
    sut.dispatch(Event::IncludeNetworkChanged { include });
}

/// Off by default: the code is the bare address every wallet can scan, and
/// the switch is offered with nothing under it.
#[test]
fn network_switch_is_offered_off_and_the_code_stays_bare() {
    let mut sut = acknowledged();
    pick_gnosis(&mut sut);
    let view = sut.view();
    assert!(view.network_switch);
    assert!(!view.include_network);
    assert!(!view.network_hint);
    assert_eq!(view.qr_value, ADDR);
}

/// On: the code names the network on screen — `ethereum:<address>@<chain>`,
/// no token, no amount — and the hint rides under the switch. Copy stays the
/// bare address: people paste addresses.
#[test]
fn including_the_network_encodes_the_chain_on_screen() {
    let mut sut = acknowledged();
    pick_gnosis(&mut sut);
    include_network(&mut sut, true);
    let view = sut.view();
    assert_eq!(view.qr_value, format!("ethereum:{ADDR}@100"));
    assert!(view.include_network);
    assert!(view.network_hint);
    assert_eq!(view.copy_payload, ADDR);
}

/// A token's own code names its NETWORK, not the token: a wallet that reads
/// the URI opens a send on that chain and the payer picks the coin.
#[test]
fn a_token_code_with_the_network_names_only_the_chain() {
    let mut sut = acknowledged();
    pick_usdc(&mut sut);
    include_network(&mut sut, true);
    assert_eq!(sut.view().qr_value, format!("ethereum:{ADDR}@8453"));
}

/// The chain follows the pick while the switch stays where the person left
/// it; off again is the bare address with no hint.
#[test]
fn the_switch_follows_the_network_and_turns_back_off() {
    let mut sut = acknowledged();
    pick_gnosis(&mut sut);
    include_network(&mut sut, true);
    pick_usdc(&mut sut);
    assert_eq!(sut.view().qr_value, format!("ethereum:{ADDR}@8453"));
    include_network(&mut sut, false);
    let view = sut.view();
    assert_eq!(view.qr_value, ADDR);
    assert!(!view.network_hint);
    assert_eq!(view.copy_payload, ADDR);
}

/// Session-scoped: opening the receive flow again starts from the code every
/// wallet can read.
#[test]
fn a_new_session_starts_with_the_switch_off() {
    let mut sut = acknowledged();
    include_network(&mut sut, true);
    sut.dispatch(Event::Start {
        account: ADDR.to_owned(),
        recipient: ADDR.to_owned(),
        base_url: BASE_URL.to_owned(),
    });
    let view = sut.view();
    assert!(!view.include_network);
    assert_eq!(view.qr_value, ADDR);
}

/// A request's code always names its network, so request mode offers no
/// switch and draws no hint — whatever address mode left behind.
#[test]
fn request_mode_has_no_network_switch() {
    let mut sut = acknowledged();
    include_network(&mut sut, true);
    sut.dispatch(Event::ModeChanged {
        mode: Mode::Request,
    });
    let view = sut.view();
    assert!(!view.network_switch);
    assert!(!view.network_hint);
    assert_eq!(view.qr_value, format!("ethereum:{ADDR}@1"));
}

/// Before a session there is no address to put in any code.
#[test]
fn no_switch_before_a_recipient() {
    let sut = Sut::new();
    let view = sut.view();
    assert!(!view.network_switch);
    assert!(!view.network_hint);
    assert!(view.qr_value.is_empty());
}

/// Round trip: the code the receive screen draws, read back by Vela's own
/// scanner, opens Send to the same address on the same network (issue #312).
///
/// The tokenizing half is the grammar every shell's `parseEIP681` implements
/// (`ethereum:<target>@<chain>`); each shell's own test runs its real parser on
/// this same core value. What the core decides — that a chain with no token
/// and no amount is "pay me on this network" — runs for real below.
#[test]
fn the_network_code_round_trips_through_the_scanner() {
    let mut sut = acknowledged();
    pick_gnosis(&mut sut);
    include_network(&mut sut, true);
    let code = sut.view().qr_value;

    let (recipient, chain) = code
        .strip_prefix("ethereum:")
        .and_then(|rest| rest.split_once('@'))
        .expect("an EIP-681 URI");
    assert_eq!(recipient, ADDR);
    assert!(chain.bytes().all(|b| b.is_ascii_digit()), "{chain}");

    let mut send = DomainDriver::<Send>::new();
    send.dispatch(SendEvent::Open {
        account: Some(SendAccountRef {
            id: "cred-1".to_owned(),
            address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            name: None,
        }),
        params: SendOpenParams {
            prefilled_recipient: Some(recipient.to_owned()),
            prefilled_chain_id: Some(chain.to_owned()),
            locked: true,
            ..SendOpenParams::default()
        },
        display: SendDisplayContext {
            code: "USD".to_owned(),
            rate: Some(1.0),
            fiat_decimals: 2,
        },
    });
    let view = send.view();
    assert_eq!(view.recipient, ADDR);
    assert_eq!(view.request_chain_id, Some(100));
}
