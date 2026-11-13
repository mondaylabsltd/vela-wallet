//! The multisig links on the web's own cases: each vector in
//! `tests/vectors/multisig/links.json` is `{ name, method, args, expect }` —
//! `method` and `args` exactly what `multisig_link::call_json` takes,
//! `expect` the JSON it must answer. The cases are the web's
//! `core/link.test.ts`, `core/invite.test.ts`, `core/kept-link.test.ts`,
//! `external/connect-link.test.ts`, `external/eip55.test.ts`,
//! `external/connect-boot.test.ts`, `core/row-id.test.ts` and
//! `core/units.test.ts`, plus edges beyond them; every one was also
//! replayed against the TypeScript itself, which answers the same.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::Value;
use vela_core::multisig_link::call_json;

fn vectors() -> Vec<Value> {
    let path = format!(
        "{}/tests/vectors/multisig/links.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn every_vector_answers_as_the_web_does() {
    let vectors = vectors();
    assert!(vectors.len() > 200, "the vector file is cut short");
    let mut failures = Vec::new();
    for v in &vectors {
        let name = v["name"].as_str().unwrap();
        let method = v["method"].as_str().unwrap();
        match call_json(method, &v["args"].to_string()) {
            Ok(out) => {
                let got: Value = serde_json::from_str(&out).unwrap();
                if got != v["expect"] {
                    failures.push(format!("{name}\n  got    {got}\n  expect {}", v["expect"]));
                }
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} vectors differ:\n{}",
        failures.len(),
        vectors.len(),
        failures.join("\n")
    );
}

#[test]
fn every_method_is_covered_by_a_vector() {
    let vectors = vectors();
    for method in [
        "shareOrigin",
        "proposalLink",
        "parseProposalLink",
        "brokenLink",
        "carriedLinkHash",
        "isMultisigLink",
        "brokenLinkText",
        "eip55",
        "checksumHolds",
        "memberNameMax",
        "addLink",
        "addLinkFault",
        "parseAddLink",
        "invitedMembers",
        "inviteLink",
        "keepLink",
        "takeKeptLink",
        "connectLink",
        "readConnectHash",
        "parseConnectLink",
        "isExpected",
        "connectBootReadsTx",
        "waitingRowId",
        "executedRowId",
        "transactionOfRow",
        "legacyRowId",
        "formatUnits",
        "parseUnits",
        "plainUnits",
        "progressLine",
    ] {
        assert!(
            vectors.iter().any(|v| v["method"] == method),
            "no vector calls {method}"
        );
    }
}

#[test]
fn vector_names_are_unique() {
    let vectors = vectors();
    let mut names: Vec<&str> = vectors
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len());
}
