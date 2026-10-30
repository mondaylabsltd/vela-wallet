//! ENS nodes (EIP-137): the namehash of a name and the reverse node of an
//! address, the two values every name lookup starts from.
//!
//! The native shells computed these themselves from keccak calls into the
//! core, one per label; one rule here, exported to every shell.
//!
//! Pure: no I/O. The name is hashed as given: normalising it (ENSIP-15) is
//! the caller's, before it gets here.

use crate::error::CoreError;
use crate::primitives::{keccak256, parse_address, to_hex};

/// EIP-137 namehash of `name`, as `0x`-prefixed lower-case hex.
/// `namehash("")` is 32 zero bytes; then the labels fold in right to left:
/// `namehash("a.b") = keccak256(namehash("b") ++ keccak256("a"))`.
#[must_use]
pub fn ens_namehash(name: &str) -> String {
    let mut node = vec![0_u8; 32];
    if !name.is_empty() {
        for label in name.rsplit('.') {
            node.extend_from_slice(&keccak256(label.as_bytes()));
            node = keccak256(&node);
        }
    }
    to_hex(&node, true)
}

/// The reverse node of `address`: the namehash of
/// `<address, lower-case hex, no 0x>.addr.reverse`, where an address names
/// itself on ENS (ENSIP-3). An L2 that derives its reverse node from a
/// registrar call (ENSIP-19) asks that registrar instead.
///
/// # Errors
/// `InvalidAddress` when `address` is not one.
pub fn ens_reverse_node(address: &str) -> Result<String, CoreError> {
    let address = parse_address(address.trim())?;
    let bare = to_hex(address.as_slice(), false);
    Ok(ens_namehash(&format!("{bare}.addr.reverse")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namehash_matches_eip_137() {
        assert_eq!(ens_namehash(""), format!("0x{}", "00".repeat(32)));
        assert_eq!(
            ens_namehash("eth"),
            "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae"
        );
        assert_eq!(
            ens_namehash("foo.eth"),
            "0xde9b09fd7c5f901e23a3f19fecc54828e9c848539801e86591bd9801b019f84f"
        );
        assert_eq!(
            ens_namehash("addr.reverse"),
            "0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2"
        );
    }

    #[test]
    fn the_reverse_node_is_the_lower_case_address_under_addr_reverse() {
        let checksummed = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        let node = ens_reverse_node(checksummed).expect("an address");
        assert_eq!(
            node,
            ens_namehash("d8da6bf26964af9d7eed9e03e53415d37aa96045.addr.reverse")
        );
        assert_eq!(ens_reverse_node(&checksummed.to_lowercase()).expect("lower"), node);
        assert!(ens_reverse_node("0x1234").is_err());
    }
}
