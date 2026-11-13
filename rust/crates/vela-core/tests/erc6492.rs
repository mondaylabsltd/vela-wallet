//! §37 (review 1-11): a message signature from an account that is not
//! deployed yet goes out in ERC-6492's envelope — `abi.encode(factory,
//! factoryCalldata, signature) ‖ 0x6492…6492` — the factory call being the
//! account's own deployment; from a deployed account it goes out as it is.
//! The envelope is verified against a real verifier on the fork
//! (`erc6492_fork.rs`); these are its shape and its refusals.

#![cfg(feature = "dev-fixtures")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use vela_core::dev_fixtures as fx;
use vela_core::primitives;
use vela_core::user_op::{
    build_init_code_for_keys, erc6492_wrap, message_signature, WalletKey, ERC6492_MAGIC,
};

fn keys(indices: &[usize]) -> Vec<WalletKey> {
    indices
        .iter()
        .map(|i| {
            let a = fx::account(*i).unwrap();
            WalletKey {
                credential_id: a.credential_id_hex.clone(),
                public_key_hex: a.public_key_hex.clone(),
            }
        })
        .collect()
}

fn word(bytes: &[u8], at: usize) -> usize {
    let w = &bytes[at..at + 32];
    assert!(w[..24].iter().all(|b| *b == 0));
    usize::from_be_bytes(w[24..].try_into().unwrap())
}

#[test]
fn an_undeployed_accounts_signature_is_wrapped_with_its_own_deployment() {
    let ks = keys(&[0]);
    let account = fx::account(0).unwrap().address;
    let signature = vec![0xab; 100];
    let wrapped = erc6492_wrap(&signature, &ks, &account).unwrap();
    assert_eq!(&wrapped[wrapped.len() - 32..], &ERC6492_MAGIC);
    let body = &wrapped[..wrapped.len() - 32];
    let init = build_init_code_for_keys(&[ks[0].public_key_hex.clone()]).unwrap();
    assert_eq!(&body[12..32], &init[..20], "the factory");
    let (calldata_at, sig_at) = (word(body, 32), word(body, 64));
    let calldata_len = word(body, calldata_at);
    assert_eq!(
        &body[calldata_at + 32..calldata_at + 32 + calldata_len],
        &init[20..],
        "the account's own createProxyWithNonce"
    );
    let sig_len = word(body, sig_at);
    assert_eq!(&body[sig_at + 32..sig_at + 32 + sig_len], &signature[..]);
}

#[test]
fn a_multi_key_account_is_wrapped_too() {
    let all: Vec<usize> = (0..fx::count()).collect();
    let ks = keys(&all);
    let account = fx::multi_address().unwrap();
    let wrapped = erc6492_wrap(&[1, 2, 3], &ks, &account).unwrap();
    assert!(wrapped.ends_with(&ERC6492_MAGIC));
}

/// A wrapper never deploys anything but the account that signed.
#[test]
fn keys_that_do_not_make_the_account_are_refused() {
    let other = fx::account(1).unwrap().address;
    assert!(erc6492_wrap(&[1], &keys(&[0]), &other).is_err());
    assert!(erc6492_wrap(&[1], &[], &other).is_err());
}

#[test]
fn deployed_is_plain_and_undeployed_or_unread_is_wrapped() {
    let ks = keys(&[0]);
    let account = fx::account(0).unwrap().address;
    let signature = vec![7u8; 65];
    let runtime = format!("0x{}", primitives::to_hex(&[0x60, 0x80, 0x60, 0x40], false));
    assert_eq!(
        message_signature(&signature, &ks, &account, Some(&runtime)).unwrap(),
        signature,
        "deployed: the EIP-1271 signature as it is"
    );
    for code in [Some("0x"), Some("0x0000"), None] {
        let out = message_signature(&signature, &ks, &account, code).unwrap();
        assert!(out.ends_with(&ERC6492_MAGIC), "{code:?}");
    }
}
