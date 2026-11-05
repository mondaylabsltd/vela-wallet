//! Multisig accounts: a standard Safe (v1.4.1, M-of-N) whose owners are Vela
//! accounts (and possibly other addresses). See
//! `docs/multisig-vault-design.md`.
//!
//! Everything here is a pure rule — the shell fetches, stores and signs:
//!
//! - **Genesis** ([`multisig_genesis`]): a multisig is created without being
//!   deployed. Its address is fixed by its genesis — owners IN THE ORDER GIVEN,
//!   threshold, a salt nonce — on every chain; the first execution on a chain
//!   deploys it. The genesis is recorded in the Safe Genesis Registry, so
//!   another device needs only the address.
//! - **Approvals** ([`approval_challenge`], [`approval_from_assertion`],
//!   [`verify_approval`]): an owner that is a Vela account approves a SafeTx
//!   through EIP-1271. A v1.4.1 Safe calls the owner's legacy
//!   `isValidSignature(txHashData, sig)`; the owner's handler (Safe4337Module
//!   inherits `CompatibilityFallbackHandler`) checks
//!   `SafeMessage(keccak256(txHashData))` — i.e. `SafeMessage(safeTxHash)` —
//!   under the OWNER's domain, which is what the owner's passkey signs. NOT
//!   [`crate::user_op::compute_safe_message_hash`], whose extra keccak is the
//!   dApp path (`isValidSignature(bytes32, …)`): a dApp signature can never
//!   pass for an approval.
//! - **Execution** ([`execution_calls`]): the executor — an owner that is a
//!   Vela account on this device — sends ONE user operation: deploy the
//!   multisig and any approving owner that has no code there yet (both are
//!   permissionless, from their geneses), then `execTransaction`. The
//!   executor counts as an approval of its own (`v = 1`, `msg.sender`).
//! - **Review** ([`review`]): what a SafeTx does, decoded by the core, and
//!   whether Vela signs it at all. Vela signs only shapes it can read in full,
//!   with no delegatecall (except to MultiSendCallOnly) and no gas refund.

use std::collections::BTreeMap;

use alloy_primitives::{keccak256, Address, B256, U256};
use serde::{Deserialize, Serialize};
#[cfg(feature = "bindings")]
use ts_rs::TS;

use crate::error::CoreError;
use crate::primitives;
use crate::safe;
use crate::user_op::{self, WalletKey};
use crate::P256PublicKey;

/// Safe v1.4.1 `CompatibilityFallbackHandler`: what a multisig's EIP-1271
/// answers come from.
pub const COMPATIBILITY_FALLBACK_HANDLER: &str = "0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99";
/// Safe v1.4.1 `MultiSendCallOnly`: the one delegatecall target Vela signs.
pub const MULTI_SEND_CALL_ONLY: &str = "0x9641d764fc13c8B624c04430C7356C1C7C8102e2";
/// The most owners a multisig Vela creates may have.
pub const MAX_OWNERS: usize = 20;
/// Safe's owner linked-list sentinel.
const SENTINEL: Address =
    Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

const SAFE_TX_TYPEHASH_PREIMAGE: &str = "SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)";

fn err(message: impl Into<String>) -> CoreError {
    CoreError::Multisig(message.into())
}

fn addr(s: &str) -> Result<Address, CoreError> {
    s.parse::<Address>()
        .map_err(|_| err(format!("not an address: {s}")))
}

fn word_u256(v: U256) -> [u8; 32] {
    v.to_be_bytes::<32>()
}

fn word_addr(a: Address) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(a.as_slice());
    w
}

fn word_usize(n: usize) -> [u8; 32] {
    word_u256(U256::from(n))
}

fn pad32(len: usize) -> usize {
    (32 - len % 32) % 32
}

/// `abi.encode(bytes)` tail: length word, data, zero padding.
fn bytes_tail(data: &[u8]) -> Vec<u8> {
    let mut out = word_usize(data.len()).to_vec();
    out.extend_from_slice(data);
    out.extend(std::iter::repeat_n(0u8, pad32(data.len())));
    out
}

fn selector(signature: &str) -> [u8; 4] {
    let h = keccak256(signature.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

fn hex(data: &[u8]) -> String {
    primitives::to_hex(data, true)
}

fn unhex(s: &str) -> Result<Vec<u8>, CoreError> {
    primitives::from_hex(s)
}

fn quantity(s: &str) -> Result<U256, CoreError> {
    let clean = s.trim();
    if let Some(h) = clean.strip_prefix("0x") {
        if h.is_empty() {
            return Ok(U256::ZERO);
        }
        U256::from_str_radix(h, 16).map_err(|_| err(format!("not a quantity: {s}")))
    } else {
        U256::from_str_radix(clean, 10).map_err(|_| err(format!("not a quantity: {s}")))
    }
}

fn hex_quantity(v: U256) -> String {
    format!("{v:#x}")
}

// ---------------------------------------------------------------------------
// Wire types (JSON, the same shapes the genesis service speaks)
// ---------------------------------------------------------------------------

/// A Safe's creation parameters, as the registry stores them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Genesis {
    pub factory: String,
    pub singleton: String,
    /// Hex quantity.
    pub salt_nonce: String,
    pub initializer: String,
    /// The record's name: a label, not part of the address, not unique.
    /// The registry requires one (1 to 64 bytes): whoever writes the record
    /// first chooses it, and the Safe itself may replace it (`rename`). A
    /// genesis read from chain history has none until one is given
    /// ([`named_genesis`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A multisig created here: its genesis and the address it gives.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultisigGenesis {
    pub address: String,
    pub genesis: Genesis,
    pub owners: Vec<String>,
    pub threshold: u64,
}

/// A Safe transaction, as `execTransaction` takes it. Quantities are hex.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeTx {
    pub to: String,
    pub value: String,
    pub data: String,
    pub operation: u8,
    pub safe_tx_gas: String,
    pub base_gas: String,
    pub gas_price: String,
    pub gas_token: String,
    pub refund_receiver: String,
    pub nonce: String,
}

/// One owner's approval of a safeTxHash, by one of its passkeys — the
/// genesis service's `PasskeyApproval`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasskeyApproval {
    pub owner: String,
    pub owner_genesis: Genesis,
    pub signer: String,
    pub authenticator_data: String,
    pub client_data_fields: String,
    /// Hex quantities.
    pub r: String,
    pub s: String,
}

/// An owner that is a plain account (an EOA in MetaMask, Rabby, …): its
/// EIP-712 signature of the SafeTx (`eth_signTypedData_v4`), `r ‖ s ‖ v`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EcdsaApproval {
    pub owner: String,
    pub signature: String,
}

/// An owner that approved on chain: it called the Safe's
/// `approveHash(safeTxHash)` itself (`approvedHashes(owner, hash) != 0`), so
/// its entry is `r = owner, v = 1`, as the executor's. Never the mailbox's:
/// the wallet reads it from the chain, and the Safe checks it there again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnChainApproval {
    pub owner: String,
    /// Always `true`: what tells this kind apart.
    pub on_chain: bool,
}

/// One owner's approval, any kind — the genesis service's `Approval` (passkey
/// or plain account), or one the chain holds; told apart by their fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Approval {
    Passkey(Box<PasskeyApproval>),
    Ecdsa(EcdsaApproval),
    OnChain(OnChainApproval),
}

impl Approval {
    pub fn owner(&self) -> &str {
        match self {
            Self::Passkey(a) => &a.owner,
            Self::Ecdsa(a) => &a.owner,
            Self::OnChain(a) => &a.owner,
        }
    }
}

/// One call of the executor's user operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Call {
    pub to: String,
    /// Hex quantity.
    pub value: String,
    pub data: String,
}

// ---------------------------------------------------------------------------
// Genesis
// ---------------------------------------------------------------------------

/// The registry's `MAX_NAME_LENGTH`, in bytes.
pub const MAX_NAME_BYTES: usize = 64;

/// A name a multisig can be created with: displayable (the bar a Vela
/// account's name is held to — no control characters, no U+FFFD), no space
/// around it, and within the registry's 64 bytes. The name is permanent and
/// every member reads it, so nothing in it may be invisible or reorder what
/// is drawn: no bidirectional controls (U+202E draws "dnuf maeT" as "Team
/// fund") and no zero-width characters — a zero-width joiner only between
/// two pictographs, where it is part of an emoji (👨‍👩‍👧).
pub fn valid_name(name: &str) -> bool {
    let chars: Vec<char> = name.chars().collect();
    let joins_emoji = |i: usize| {
        i > 0
            && pictograph_or_variation(chars[i - 1])
            && chars.get(i + 1).is_some_and(|&next| pictograph(next))
    };
    !name.is_empty()
        && name.trim() == name
        && name.len() <= MAX_NAME_BYTES
        && chars.iter().enumerate().all(|(i, &c)| {
            !(c.is_control() || c == '\u{FFFD}')
                && (!hidden(c) || (c == '\u{200D}' && joins_emoji(i)))
        })
}

/// A character that draws nothing or moves what is drawn around it: the
/// bidirectional controls and marks, and the zero-width and invisible
/// formatting characters.
fn hidden(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
    )
}

/// What a zero-width joiner joins into one emoji.
fn pictograph(c: char) -> bool {
    matches!(c, '\u{2600}'..='\u{27BF}' | '\u{2B00}'..='\u{2BFF}' | '\u{1F000}'..='\u{1FAFF}')
}

/// A pictograph, or the emoji presentation selector that may follow one.
fn pictograph_or_variation(c: char) -> bool {
    pictograph(c) || c == '\u{FE0F}'
}

/// Whether `address` can be a member: an address, and neither the zero
/// address nor the Safe's owner-list sentinel (`0x…01`), which `setup`
/// refuses.
pub fn valid_owner(address: &str) -> bool {
    addr(address).is_ok_and(|a| a != Address::ZERO && a != SENTINEL)
}

/// `setup(owners, threshold, 0, "", handler, 0, 0, 0)`.
fn encode_setup(owners: &[Address], threshold: u64, fallback_handler: Address) -> Vec<u8> {
    encode_setup_full(&SetupArgs {
        owners: owners.to_vec(),
        threshold: U256::from(threshold),
        to: Address::ZERO,
        data: Vec::new(),
        fallback_handler,
        payment_token: Address::ZERO,
        payment: U256::ZERO,
        payment_receiver: Address::ZERO,
    })
}

/// `setup(owners, threshold, to, data, fallbackHandler, paymentToken, payment, paymentReceiver)`.
#[derive(Clone)]
struct SetupArgs {
    owners: Vec<Address>,
    threshold: U256,
    to: Address,
    data: Vec<u8>,
    fallback_handler: Address,
    payment_token: Address,
    payment: U256,
    payment_receiver: Address,
}

fn encode_setup_full(a: &SetupArgs) -> Vec<u8> {
    let mut out =
        selector("setup(address[],uint256,address,bytes,address,address,uint256,address)").to_vec();
    out.extend_from_slice(&word_usize(0x100)); // owners offset (8 head words)
    out.extend_from_slice(&word_u256(a.threshold));
    out.extend_from_slice(&word_addr(a.to));
    out.extend_from_slice(&word_usize(0x100 + 32 + a.owners.len() * 32)); // data offset
    out.extend_from_slice(&word_addr(a.fallback_handler));
    out.extend_from_slice(&word_addr(a.payment_token));
    out.extend_from_slice(&word_u256(a.payment));
    out.extend_from_slice(&word_addr(a.payment_receiver));
    out.extend_from_slice(&word_usize(a.owners.len()));
    for o in &a.owners {
        out.extend_from_slice(&word_addr(*o));
    }
    out.extend(bytes_tail(&a.data));
    out
}

/// The genesis of a Safe that Safe{Wallet} keeps undeployed (an export's
/// `undeployedSafes[chain][address].props`): its `setup` built as Safe{Wallet}
/// deploys it, believed only if it derives `address`.
pub fn safe_wallet_props_genesis(
    props: &serde_json::Value,
    address: &str,
) -> Result<Genesis, CoreError> {
    let field =
        |o: &serde_json::Value, k: &str| o.get(k).and_then(|v| v.as_str()).map(str::to_owned);
    let opt_addr = |o: &serde_json::Value, k: &str| -> Result<Address, CoreError> {
        field(o, k)
            .filter(|s| !s.is_empty())
            .map(|s| addr(&s))
            .unwrap_or(Ok(Address::ZERO))
    };
    let config = props
        .get("safeAccountConfig")
        .ok_or_else(|| err("no safeAccountConfig"))?;
    let owners = config
        .get("owners")
        .and_then(|v| v.as_array())
        .ok_or_else(|| err("no owners"))?
        .iter()
        .map(|o| {
            o.as_str()
                .ok_or_else(|| err("an owner is not an address"))
                .and_then(addr)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let threshold = config
        .get("threshold")
        .and_then(|t| t.as_u64())
        .ok_or_else(|| err("no threshold"))?;
    let payment = match config.get("payment") {
        None | Some(serde_json::Value::Null) => U256::ZERO,
        Some(serde_json::Value::Number(n)) => {
            U256::from(n.as_u64().ok_or_else(|| err("bad payment"))?)
        }
        Some(serde_json::Value::String(s)) => quantity(s)?,
        Some(_) => return Err(err("bad payment")),
    };
    let initializer = encode_setup_full(&SetupArgs {
        owners,
        threshold: U256::from(threshold),
        to: opt_addr(config, "to")?,
        data: field(config, "data")
            .map(|d| unhex(&d))
            .transpose()?
            .unwrap_or_default(),
        fallback_handler: opt_addr(config, "fallbackHandler")?,
        payment_token: opt_addr(config, "paymentToken")?,
        payment,
        payment_receiver: opt_addr(config, "paymentReceiver")?,
    });
    let salt_nonce = field(props, "saltNonce").ok_or_else(|| err("no saltNonce"))?;
    let genesis = Genesis {
        factory: addr(&field(props, "factoryAddress").ok_or_else(|| err("no factoryAddress"))?)?
            .to_checksum(None),
        singleton: addr(&field(props, "masterCopy").ok_or_else(|| err("no masterCopy"))?)?
            .to_checksum(None),
        salt_nonce: hex_quantity(quantity(&salt_nonce)?),
        initializer: hex(&initializer),
        name: None,
    };
    if genesis_address(&genesis)? != addr(address)?.to_checksum(None) {
        return Err(err("these creation parameters give another address"));
    }
    Ok(genesis)
}

/// Safe's v1.3.0 proxy factories' `proxyCreationCode()` (canonical and
/// eip155 share it); v1.4.1's is [`safe::PROXY_CREATION_CODE`].
const PROXY_CREATION_CODE_130: &str = "608060405234801561001057600080fd5b506040516101e63803806101e68339818101604052602081101561003357600080fd5b8101908080519060200190929190505050600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614156100ca576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260228152602001806101c46022913960400191505060405180910390fd5b806000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505060ab806101196000396000f3fe608060405273ffffffffffffffffffffffffffffffffffffffff600054167fa619486e0000000000000000000000000000000000000000000000000000000060003514156050578060005260206000f35b3660008037600080366000845af43d6000803e60008114156070573d6000fd5b3d6000f3fea2646970667358221220d1429297349653a4918076d650332de1a1068c5f3e07c5c82360c277770b955264736f6c63430007060033496e76616c69642073696e676c65746f6e20616464726573732070726f7669646564";

/// Safe's v1.5.0 proxy factory's `proxyCreationCode()`.
const PROXY_CREATION_CODE_150: &str = "608060405234801561001057600080fd5b506040516101b63803806101b68339818101604052602081101561003357600080fd5b8101908080519060200190929190505050600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614156100ca576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260228152602001806101946022913960400191505060405180910390fd5b806000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff16021790555050607b806101196000396000f3fe608060405260005463a619486e60003560e01c14156024578060601b606c5260206060f35b3660008037600080366000845af43d6000803e806040573d6000fd5b3d6000f3fea2646970667358221220e61834ebd2d8cd909d362bf67c47ef58fd665df38e6dd036ce65611101d072e964736f6c63430007060033496e76616c69642073696e676c65746f6e20616464726573732070726f7669646564";

/// The proxy factories a genesis may name, and each one's proxy creation code.
const KNOWN_FACTORIES: [(&str, &str); 4] = [
    (
        "0xa6B71E26C5e0845f74c812102Ca7114b6A896AB2",
        PROXY_CREATION_CODE_130,
    ), // 1.3.0
    (
        "0xC22834581EbC8527d974F8a1c97E1bEA4EF910BC",
        PROXY_CREATION_CODE_130,
    ), // 1.3.0 eip155
    (safe::SAFE_PROXY_FACTORY, safe::PROXY_CREATION_CODE), // 1.4.1
    (
        "0x14F2982D601c9458F93bD70B218933A6f8165e7b",
        PROXY_CREATION_CODE_150,
    ), // 1.5.0
];

/// The Safe singletons (v1.3.0 – v1.5.0, L1 and L2) a genesis may name: the
/// versions whose EIP-712 domain carries the chain id, which every hash here
/// assumes.
const KNOWN_SINGLETONS: [&str; 8] = [
    "0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552", // 1.3.0
    "0x3E5c63644E683549055b9Be8653de26E0B4CD36E", // 1.3.0 L2
    "0x69f4D1788e39c87893C980c06EdF4b7f686e2938", // 1.3.0 eip155
    "0xfb1bffC9d739B8D520DaF37dF666da4C687191EA", // 1.3.0 L2 eip155
    "0x41675C099F32341bf84BFc5382aF534df5C7461a", // 1.4.1
    safe::SAFE_SINGLETON,                         // 1.4.1 L2
    "0xFf51A5898e281Db6DfC7855790607438dF2ca44b", // 1.5.0
    "0xEdd160fEBBD92E350D4D398fb636302fccd67C7e", // 1.5.0 L2
];

fn proxy_init_code_hash_for(factory: Address, singleton: Address) -> Result<B256, CoreError> {
    let code = KNOWN_FACTORIES
        .iter()
        .find(|(f, _)| f.parse::<Address>().ok() == Some(factory))
        .map(|(_, c)| *c)
        .ok_or_else(|| {
            err(format!(
                "{factory} is not a Safe proxy factory this wallet knows"
            ))
        })?;
    if singleton == Address::ZERO
        || !KNOWN_SINGLETONS
            .iter()
            .any(|s| s.parse::<Address>().ok() == Some(singleton))
    {
        return Err(err(format!(
            "{singleton} is not a Safe singleton this wallet supports (v1.3.0 – v1.5.0)"
        )));
    }
    let mut code = unhex(code)?;
    code.extend_from_slice(&word_addr(singleton));
    Ok(keccak256(code))
}

/// The address a genesis deploys to: Safe v1.3.0 – v1.5.0, through a proxy
/// factory this wallet knows.
pub fn genesis_address(g: &Genesis) -> Result<String, CoreError> {
    let factory = addr(&g.factory)?;
    let init_code_hash = proxy_init_code_hash_for(factory, addr(&g.singleton)?)?;
    let init = unhex(&g.initializer)?;
    let mut salt = keccak256(&init).to_vec();
    salt.extend_from_slice(&word_u256(quantity(&g.salt_nonce)?));
    let salt = keccak256(salt);
    let a = factory.create2(salt, init_code_hash);
    Ok(a.to_checksum(None))
}

/// A new multisig's genesis: Safe L2 v1.4.1 — the singleton every Vela account
/// uses, so the address is the same on every chain — with
/// `CompatibilityFallbackHandler`, no setup hook, no module. `owners` keep the
/// order given: it is part of the address.
/// A multisig named `name`, at a random `salt_nonce`. The name is written
/// with its record; it takes no part in the address.
pub fn multisig_genesis(
    owners: &[String],
    threshold: u64,
    name: &str,
    salt_nonce: &str,
) -> Result<MultisigGenesis, CoreError> {
    if !valid_name(name) {
        return Err(err("not a name a multisig can have"));
    }
    let mut ms = genesis_with_salt(owners, threshold, quantity(salt_nonce)?)?;
    ms.genesis.name = Some(name.to_owned());
    Ok(ms)
}

/// The genesis these owners and threshold give with this salt nonce (no name).
fn genesis_with_salt(
    owners: &[String],
    threshold: u64,
    salt_nonce: U256,
) -> Result<MultisigGenesis, CoreError> {
    if owners.is_empty() {
        return Err(err("a multisig needs at least one owner"));
    }
    if owners.len() > MAX_OWNERS {
        return Err(err(format!("at most {MAX_OWNERS} owners")));
    }
    let parsed = owners
        .iter()
        .map(|o| addr(o))
        .collect::<Result<Vec<_>, _>>()?;
    for (i, o) in parsed.iter().enumerate() {
        if *o == Address::ZERO || *o == SENTINEL {
            return Err(err(format!("{o} cannot be an owner")));
        }
        if parsed[..i].contains(o) {
            return Err(err(format!("{o} is listed twice")));
        }
    }
    if threshold == 0 || threshold as usize > parsed.len() {
        return Err(err(
            "the threshold must be between 1 and the number of owners",
        ));
    }
    let genesis = Genesis {
        factory: addr(safe::SAFE_PROXY_FACTORY)?.to_checksum(None),
        singleton: addr(safe::SAFE_SINGLETON)?.to_checksum(None),
        salt_nonce: hex_quantity(salt_nonce),
        initializer: hex(&encode_setup(
            &parsed,
            threshold,
            addr(COMPATIBILITY_FALLBACK_HANDLER)?,
        )),
        name: None,
    };
    Ok(MultisigGenesis {
        address: genesis_address(&genesis)?,
        genesis,
        owners: parsed.iter().map(|o| o.to_checksum(None)).collect(),
        threshold,
    })
}

/// §16: a genesis read back as a multisig Vela creates ([`multisig_genesis`],
/// any name): its members, threshold and address — or an error for anything
/// else. The members and threshold are read from the setup, the genesis is
/// written again from them with its salt nonce, and the factory, singleton
/// and setup must be byte for byte the ones given: Safe's v1.4.1 factory,
/// the L2 singleton, the CompatibilityFallbackHandler, no setup hook, no
/// module, no payment, members a multisig can have and a threshold between
/// one and their number. The trusted page reads a member's owner proof by
/// this rule (`lib/multisig.js` `ownerProof`), case for case.
pub fn vela_multisig_of(g: &Genesis) -> Result<MultisigGenesis, CoreError> {
    let (owners, threshold) = genesis_members(g)?;
    let mut rebuilt = genesis_with_salt(&owners, threshold, quantity(&g.salt_nonce)?)?;
    let same_address = |a: &str, b: &str| addr(a).ok() == addr(b).ok();
    if !same_address(&rebuilt.genesis.factory, &g.factory)
        || !same_address(&rebuilt.genesis.singleton, &g.singleton)
        || unhex(&rebuilt.genesis.initializer)? != unhex(&g.initializer)?
    {
        return Err(err("not a multisig as Vela creates one"));
    }
    rebuilt.genesis.name.clone_from(&g.name);
    Ok(rebuilt)
}

/// A Vela account's own genesis, from its founding keys (`keys[0]` first, as
/// the account stores them).
pub fn vela_account_genesis(keys: &[WalletKey]) -> Result<(String, Genesis), CoreError> {
    let parsed = keys
        .iter()
        .map(|k| safe::parse_public_key(&k.public_key_hex))
        .collect::<Result<Vec<_>, _>>()?;
    let info = safe::compute_safe_address_multi(&parsed)?;
    Ok((
        info.address.clone(),
        Genesis {
            factory: addr(safe::SAFE_PROXY_FACTORY)?.to_checksum(None),
            singleton: addr(safe::SAFE_SINGLETON)?.to_checksum(None),
            salt_nonce: hex_quantity(U256::from_be_slice(&info.salt_nonce)),
            initializer: hex(&info.setup_data),
            name: None,
        },
    ))
}

/// The genesis inside a creating transaction's calldata: every
/// `createProxyWithNonce` / `createProxyWithNonceL2` / `createProxyWithCallback`
/// call found in it — called directly, through a batch, or in a 4337
/// `handleOps` init code — read with each factory this wallet knows, and the
/// one that derives `safe` kept (a callback folded into the salt nonce, as
/// the factory does). `None` when nothing in it makes `safe`.
pub fn genesis_from_calldata(input: &str, safe: &str) -> Result<Option<Genesis>, CoreError> {
    let input = unhex(input)?;
    let safe = addr(safe)?.to_checksum(None);
    let nonce = selector("createProxyWithNonce(address,bytes,uint256)");
    let nonce_l2 = selector("createProxyWithNonceL2(address,bytes,uint256)");
    let callback = selector("createProxyWithCallback(address,bytes,uint256,address)");
    let word = |at: usize| input.get(at..at + 32);
    for i in 0..input.len().saturating_sub(4 + 96) {
        let sel = &input[i..i + 4];
        let with_callback = sel == callback;
        if sel != nonce && sel != nonce_l2 && !with_callback {
            continue;
        }
        let args = i + 4;
        let (Some(w0), Some(w1), Some(w2)) = (word(args), word(args + 32), word(args + 64)) else {
            continue;
        };
        if w0[..12].iter().any(|b| *b != 0) {
            continue;
        }
        let singleton = Address::from_slice(&w0[12..]);
        let Ok(offset) = usize::try_from(U256::from_be_slice(w1)) else {
            continue;
        };
        let Some(len_word) = word(args + offset) else {
            continue;
        };
        let Ok(len) = usize::try_from(U256::from_be_slice(len_word)) else {
            continue;
        };
        let start = args + offset + 32;
        let Some(initializer) = input.get(start..start.saturating_add(len)) else {
            continue;
        };
        let mut salt = U256::from_be_slice(w2);
        if with_callback {
            let Some(w3) = word(args + 96) else { continue };
            let mut packed = word_u256(salt).to_vec();
            packed.extend_from_slice(&w3[12..]);
            salt = U256::from_be_bytes(keccak256(packed).0);
        }
        for (factory, _) in KNOWN_FACTORIES {
            let g = Genesis {
                factory: addr(factory)?.to_checksum(None),
                singleton: singleton.to_checksum(None),
                salt_nonce: hex_quantity(salt),
                initializer: hex(initializer),
                name: None,
            };
            if genesis_address(&g).ok().as_deref() == Some(safe.as_str()) {
                return Ok(Some(g));
            }
        }
    }
    Ok(None)
}

/// The Safe Genesis Registry (v4: every record named, a Safe renames itself),
/// deployed through the deterministic deployer with the salt
/// `keccak256("safe-genesis-registry/v4")`: the same address on every chain
/// it is on. Earlier versions (v1 `0xdA88…fb89`, v2 `0x70b3…4802`, v3
/// `0x535f…801E`) are retired: the service reads only this one.
pub const GENESIS_REGISTRY: &str = "0xF412388bF908E510066d3Be5c704bd9A327f945d";

/// Whether the registry takes `name` with a record or a rename: 1 to
/// [`MAX_NAME_BYTES`] bytes — its `EmptyName` and `NameTooLong` checks, and
/// nothing more (it does not read UTF-8). [`valid_name`] is the stricter rule
/// for a name this wallet writes or shows; every name it allows, the registry
/// takes.
pub fn recordable_name(name: &str) -> bool {
    (1..=MAX_NAME_BYTES).contains(&name.len())
}

const NAME_ADJECTIVES: [&str; 32] = [
    "Amber", "Brave", "Bright", "Calm", "Clever", "Cosmic", "Crimson", "Daring", "Eager", "Gentle",
    "Golden", "Hidden", "Humble", "Jolly", "Kind", "Lucky", "Mellow", "Misty", "Noble", "Proud",
    "Quiet", "Rapid", "Silent", "Silver", "Steady", "Sunny", "Swift", "Tidy", "Velvet", "Vivid",
    "Wild", "Wise",
];
const NAME_NOUNS: [&str; 32] = [
    "Badger", "Beacon", "Brook", "Cedar", "Comet", "Falcon", "Fern", "Fox", "Glacier", "Harbor",
    "Heron", "Island", "Lantern", "Maple", "Meadow", "Otter", "Owl", "Pebble", "Pine", "Raven",
    "Reef", "River", "Robin", "Sparrow", "Summit", "Thistle", "Tiger", "Valley", "Willow", "Wolf",
    "Wren", "Zephyr",
];

/// The name the genesis service records a Safe under when its genesis came
/// with none (genesis-rules' `default_name`, "Quiet Harbor"): two words drawn
/// from `keccak256(address)`, so every device and every service instance
/// gives the same Safe the same one. Nothing private goes into it — an
/// imported Safe's address-book name stays on this device.
pub fn default_name(safe: &str) -> Result<String, CoreError> {
    let h = keccak256(addr(safe)?.as_slice());
    Ok(format!(
        "{} {}",
        NAME_ADJECTIVES[h[0] as usize % 32],
        NAME_NOUNS[h[1] as usize % 32]
    ))
}

/// `g` as the registry and its service take it — with a name. Its own name
/// when the registry would take it, else [`default_name`] of the address it
/// makes: what a record of it holds, whoever writes it. The service ignores a
/// genesis sent without one (a proposal's) and names a record itself, so the
/// wallet always sends this.
pub fn named_genesis(g: &Genesis) -> Result<Genesis, CoreError> {
    if g.name.as_deref().is_some_and(recordable_name) {
        return Ok(g.clone());
    }
    Ok(Genesis {
        name: Some(default_name(&genesis_address(g)?)?),
        ..g.clone()
    })
}

const RECORD_SIGNATURE: &str = "record(address,bytes,address,bytes,uint256,string)";
const RENAME_SIGNATURE: &str = "rename(string)";

/// The registry call that records `g` on any chain —
/// `record(factory, proxyCreationCode, singleton, initializer, saltNonce, name)`,
/// with the name of [`named_genesis`] and its factory's proxy creation code
/// (the registry re-derives the address from it). Anyone may send it; a
/// record that exists already stays as it is, and keeps its name.
pub fn record_call(g: &Genesis) -> Result<Call, CoreError> {
    let g = named_genesis(g)?;
    let factory = addr(&g.factory)?;
    let code = KNOWN_FACTORIES
        .iter()
        .find(|(f, _)| f.parse::<Address>().ok() == Some(factory))
        .map(|(_, c)| unhex(c))
        .transpose()?
        .ok_or_else(|| err("not a proxy factory this wallet knows"))?;
    let init = unhex(&g.initializer)?;
    let name = g.name.unwrap_or_default();
    let mut data = selector(RECORD_SIGNATURE).to_vec();
    let code_tail = bytes_tail(&code);
    let init_tail = bytes_tail(&init);
    data.extend_from_slice(&word_addr(factory));
    data.extend_from_slice(&word_usize(6 * 32)); // proxyCreationCode offset
    data.extend_from_slice(&word_addr(addr(&g.singleton)?));
    data.extend_from_slice(&word_usize(6 * 32 + code_tail.len())); // initializer offset
    data.extend_from_slice(&word_u256(quantity(&g.salt_nonce)?));
    data.extend_from_slice(&word_usize(6 * 32 + code_tail.len() + init_tail.len())); // name offset
    data.extend(code_tail);
    data.extend(init_tail);
    data.extend(bytes_tail(name.as_bytes()));
    Ok(Call {
        to: addr(GENESIS_REGISTRY)?.to_checksum(None),
        value: "0x0".into(),
        data: hex(&data),
    })
}

/// `rename(name)` to the registry: what a Safe calls to give its own record
/// this name. Only a name this wallet can show in full ([`valid_name`]); the
/// registry's limits are within it.
pub fn rename_call(name: &str) -> Result<Call, CoreError> {
    if !valid_name(name) {
        return Err(err("not a name a multisig can have"));
    }
    let mut data = selector(RENAME_SIGNATURE).to_vec();
    data.extend_from_slice(&word_usize(0x20));
    data.extend(bytes_tail(name.as_bytes()));
    Ok(Call {
        to: addr(GENESIS_REGISTRY)?.to_checksum(None),
        value: "0x0".into(),
        data: hex(&data),
    })
}

/// The multisig's transaction that renames its record in the registry on the
/// chain it runs on (a plain CALL, no coin). It needs the multisig deployed
/// there — its executor deploys it first when it is not — and that chain's
/// registry to hold its record (else the call reverts, `NotRecorded`).
pub fn rename_tx(name: &str, nonce: &str) -> Result<SafeTx, CoreError> {
    let call = rename_call(name)?;
    Ok(plain_tx(
        addr(&call.to)?,
        U256::ZERO,
        unhex(&call.data)?,
        0,
        quantity(nonce)?,
    ))
}

/// The name in a `rename(string)` call, if `data` is exactly one (canonical
/// ABI: one offset word, the length, the bytes, zero padding — nothing may
/// hide in the rest) and the name is text. Whether Vela shows it is
/// [`valid_name`]'s to say.
pub fn read_rename(data: &[u8]) -> Option<String> {
    if data.get(..4)? != selector(RENAME_SIGNATURE)
        || U256::from_be_slice(data.get(4..36)?) != U256::from(0x20)
    {
        return None;
    }
    let len: usize = U256::from_be_slice(data.get(36..68)?).try_into().ok()?;
    let bytes = data.get(68..68usize.checked_add(len)?)?;
    if data.len() != 68 + len + pad32(len) || data[68 + len..].iter().any(|b| *b != 0) {
        return None;
    }
    String::from_utf8(bytes.to_vec()).ok()
}

/// A call to the registry, as a multisig's transaction makes it. A rename is
/// read as one — only with a name Vela can show in full, since every member
/// reads it and anyone who looks the multisig up sees it; anything else to
/// the registry (a `record`) is an ordinary contract call. No coin: nothing
/// in the registry takes any.
fn read_registry_call(to: Address, value: U256, data: &[u8]) -> Result<Action, &'static str> {
    if !value.is_zero() {
        return Err("sends coins to the genesis registry");
    }
    if data.get(..4) == Some(&selector(RENAME_SIGNATURE)[..]) {
        return match read_rename(data) {
            Some(name) if valid_name(&name) => Ok(Action::Rename { name }),
            _ => Err("renames the multisig to a name Vela cannot show in full"),
        };
    }
    Ok(Action::Call {
        to: to.to_checksum(None),
        value: value.to_string(),
        data: hex(data),
    })
}

/// What a `record(...)` call to the registry files: the genesis (with the
/// name it carries) and the address the registry files it under — derived
/// here the registry's way, from the proxy creation code IN the call, so the
/// address is the one the record will hold whatever factory it names. `None`
/// for anything that is not such a call.
pub fn read_record_call(data: &[u8]) -> Option<(String, Genesis)> {
    if data.get(..4)? != selector(RECORD_SIGNATURE) {
        return None;
    }
    let body = &data[4..];
    let word = |at: usize| body.get(at..at.checked_add(32)?).map(U256::from_be_slice);
    let address = |i: usize| {
        let w = word(i * 32)?;
        (w >> 160 == U256::ZERO).then(|| Address::from_word(B256::from(w)))
    };
    let dynamic = |i: usize| -> Option<&[u8]> {
        let at: usize = word(i * 32)?.try_into().ok()?;
        let len: usize = word(at)?.try_into().ok()?;
        let from = at.checked_add(32)?;
        body.get(from..from.checked_add(len)?)
    };
    let factory = address(0)?;
    let code = dynamic(1)?;
    let singleton = address(2)?;
    let initializer = dynamic(3)?;
    let salt_nonce = word(4 * 32)?;
    let name = String::from_utf8(dynamic(5)?.to_vec()).ok()?;
    let mut salt = keccak256(initializer).to_vec();
    salt.extend_from_slice(&word_u256(salt_nonce));
    let mut init_code = code.to_vec();
    init_code.extend_from_slice(&word_addr(singleton));
    let safe = factory.create2(keccak256(salt), keccak256(init_code));
    Some((
        safe.to_checksum(None),
        Genesis {
            factory: factory.to_checksum(None),
            singleton: singleton.to_checksum(None),
            salt_nonce: hex_quantity(salt_nonce),
            initializer: hex(initializer),
            name: Some(name),
        },
    ))
}

/// `genesisOf(safe)` calldata, for an `eth_call` to [`GENESIS_REGISTRY`].
pub fn genesis_of_call(safe: &str) -> Result<String, CoreError> {
    let mut data = selector("genesisOf(address)").to_vec();
    data.extend_from_slice(&word_addr(addr(safe)?));
    Ok(hex(&data))
}

/// What one chain's registry holds for a Safe (v4's `Genesis`), as
/// [`read_genesis_of`] reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryRecord {
    /// The genesis, with the record's name on this chain.
    pub genesis: Genesis,
    pub proxy_creation_code_hash: String,
    /// Who wrote the record. Informational only.
    pub recorder: String,
    pub recorded_at: u64,
    /// Its position in this registry's write order.
    pub index: u32,
    /// Block of the Safe's last `rename` here, 0 if it never renamed itself.
    pub renamed_at: u64,
    /// The name is the Safe's own: it renamed itself, or wrote the record.
    /// Otherwise whoever wrote the record first chose it, and it proves nothing.
    pub name_by_safe: bool,
}

/// Read a `genesisOf(safe)` answer (`(bool found, Genesis genesis)`): `None`
/// when the registry holds nothing for `safe`. The genesis is re-derived
/// here, as one from anywhere is, and must make `safe`: a registry files a
/// record only under the address its own derivation gives, so an answer that
/// does not is no registry's.
pub fn read_genesis_of(safe: &str, ret: &str) -> Result<Option<RegistryRecord>, CoreError> {
    let me = addr(safe)?;
    let ret = unhex(ret)?;
    let bad = || err("not a genesisOf answer");
    let word = |at: usize| -> Result<U256, CoreError> {
        ret.get(at..at.checked_add(32).ok_or_else(bad)?)
            .map(U256::from_be_slice)
            .ok_or_else(bad)
    };
    let small = |at: usize| -> Result<usize, CoreError> { word(at)?.try_into().map_err(|_| bad()) };
    let address = |at: usize| -> Result<Address, CoreError> {
        let w = word(at)?;
        if w >> 160 != U256::ZERO {
            return Err(bad());
        }
        Ok(Address::from_word(B256::from(w)))
    };
    let found = word(0)?;
    if found > U256::from(1) {
        return Err(bad());
    }
    if found.is_zero() {
        return Ok(None);
    }
    // The tuple's head: factory, singleton, renamedAt, saltNonce,
    // →initializer, proxyCreationCodeHash, recorder, recordedAt, index, →name.
    let t = small(32)?;
    let field = |i: usize| t.checked_add(i * 32).ok_or_else(bad);
    let tail = |i: usize| -> Result<Vec<u8>, CoreError> {
        let at = t.checked_add(small(field(i)?)?).ok_or_else(bad)?;
        let len = small(at)?;
        let from = at.checked_add(32).ok_or_else(bad)?;
        ret.get(from..from.checked_add(len).ok_or_else(bad)?)
            .map(<[u8]>::to_vec)
            .ok_or_else(bad)
    };
    let number =
        |i: usize| -> Result<u64, CoreError> { word(field(i)?)?.try_into().map_err(|_| bad()) };
    let name = String::from_utf8(tail(9)?).map_err(|_| err("the record's name is not text"))?;
    let recorder = address(field(6)?)?;
    let renamed_at = number(2)?;
    let record = RegistryRecord {
        genesis: Genesis {
            factory: address(field(0)?)?.to_checksum(None),
            singleton: address(field(1)?)?.to_checksum(None),
            salt_nonce: hex_quantity(word(field(3)?)?),
            initializer: hex(&tail(4)?),
            name: (!name.is_empty()).then_some(name),
        },
        proxy_creation_code_hash: hex(&word(field(5)?)?.to_be_bytes::<32>()),
        recorder: recorder.to_checksum(None),
        recorded_at: number(7)?,
        index: u32::try_from(number(8)?).map_err(|_| bad())?,
        renamed_at,
        name_by_safe: renamed_at > 0 || recorder == me,
    };
    if genesis_address(&record.genesis)? != me.to_checksum(None) {
        return Err(err("the registry's record does not make this address"));
    }
    Ok(Some(record))
}

/// `createProxyWithNonce(singleton, initializer, saltNonce)`: deploys a genesis.
pub fn deploy_call(g: &Genesis) -> Result<Call, CoreError> {
    let init = unhex(&g.initializer)?;
    let mut data = selector("createProxyWithNonce(address,bytes,uint256)").to_vec();
    data.extend_from_slice(&word_addr(addr(&g.singleton)?));
    data.extend_from_slice(&word_usize(0x60));
    data.extend_from_slice(&word_u256(quantity(&g.salt_nonce)?));
    data.extend(bytes_tail(&init));
    Ok(Call {
        to: addr(&g.factory)?.to_checksum(None),
        value: "0x0".into(),
        data: hex(&data),
    })
}

/// The owners and threshold a genesis creates (only for geneses this module
/// writes or Vela accounts' — a plain `setup` call).
pub fn genesis_members(g: &Genesis) -> Result<(Vec<String>, u64), CoreError> {
    let init = unhex(&g.initializer)?;
    if init.len() < 4 + 9 * 32
        || init[..4]
            != selector("setup(address[],uint256,address,bytes,address,address,uint256,address)")
    {
        return Err(err("not a Safe setup call"));
    }
    let body = &init[4..];
    let word = |i: usize| -> Result<U256, CoreError> {
        body.get(i..i + 32)
            .map(U256::from_be_slice)
            .ok_or_else(|| err("truncated setup"))
    };
    let offset: usize = word(0)?.try_into().map_err(|_| err("bad owners offset"))?;
    let threshold: u64 = word(32)?.try_into().map_err(|_| err("bad threshold"))?;
    let count: usize = word(offset)?
        .try_into()
        .map_err(|_| err("bad owners length"))?;
    if count > 64 {
        return Err(err("too many owners"));
    }
    let mut owners = Vec::with_capacity(count);
    for i in 0..count {
        let w = word(offset + 32 + i * 32)?;
        owners.push(Address::from_word(B256::from(w)).to_checksum(None));
    }
    Ok((owners, threshold))
}

// ---------------------------------------------------------------------------
// Hashes
// ---------------------------------------------------------------------------

fn domain_separator(chain_id: u64, safe: Address) -> B256 {
    let mut d = keccak256("EIP712Domain(uint256 chainId,address verifyingContract)").to_vec();
    d.extend_from_slice(&word_u256(U256::from(chain_id)));
    d.extend_from_slice(&word_addr(safe));
    keccak256(d)
}

fn eip712(domain: B256, struct_hash: B256) -> B256 {
    let mut p = vec![0x19, 0x01];
    p.extend_from_slice(domain.as_slice());
    p.extend_from_slice(struct_hash.as_slice());
    keccak256(p)
}

struct ParsedTx {
    to: Address,
    value: U256,
    data: Vec<u8>,
    operation: u8,
    safe_tx_gas: U256,
    base_gas: U256,
    gas_price: U256,
    gas_token: Address,
    refund_receiver: Address,
    nonce: U256,
}

fn parse_tx(tx: &SafeTx) -> Result<ParsedTx, CoreError> {
    Ok(ParsedTx {
        to: addr(&tx.to)?,
        value: quantity(&tx.value)?,
        data: unhex(&tx.data)?,
        operation: tx.operation,
        safe_tx_gas: quantity(&tx.safe_tx_gas)?,
        base_gas: quantity(&tx.base_gas)?,
        gas_price: quantity(&tx.gas_price)?,
        gas_token: addr(&tx.gas_token)?,
        refund_receiver: addr(&tx.refund_receiver)?,
        nonce: quantity(&tx.nonce)?,
    })
}

/// The `SafeTx` struct hash of `tx` (EIP-712's `hashStruct`).
fn safe_tx_struct_hash(tx: &SafeTx) -> Result<B256, CoreError> {
    let t = parse_tx(tx)?;
    let mut s = keccak256(SAFE_TX_TYPEHASH_PREIMAGE).to_vec();
    s.extend_from_slice(&word_addr(t.to));
    s.extend_from_slice(&word_u256(t.value));
    s.extend_from_slice(keccak256(&t.data).as_slice());
    s.extend_from_slice(&word_u256(U256::from(t.operation)));
    s.extend_from_slice(&word_u256(t.safe_tx_gas));
    s.extend_from_slice(&word_u256(t.base_gas));
    s.extend_from_slice(&word_u256(t.gas_price));
    s.extend_from_slice(&word_addr(t.gas_token));
    s.extend_from_slice(&word_addr(t.refund_receiver));
    s.extend_from_slice(&word_u256(t.nonce));
    Ok(keccak256(s))
}

/// The safeTxHash of `tx` for `safe` on `chain_id`.
pub fn safe_tx_hash(chain_id: u64, safe: &str, tx: &SafeTx) -> Result<String, CoreError> {
    Ok(hex(eip712(
        domain_separator(chain_id, addr(safe)?),
        safe_tx_struct_hash(tx)?,
    )
    .as_slice()))
}

/// The hashes a hardware wallet shows for a SafeTx's typed data — the
/// domain hash (`EIP712Domain{chainId, verifyingContract}`), the message hash
/// (the `SafeTx` struct hash) and the safeTxHash they make — so the owner can
/// match the device's screen with the review's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeTxHashes {
    pub domain_hash: String,
    pub message_hash: String,
    pub safe_tx_hash: String,
    /// What `personal_sign` signs in the `eth_sign` form: the EIP-191 hash of
    /// the safeTxHash.
    pub eth_sign_hash: String,
}

/// `tx`'s hashes for `safe` on `chain_id` ([`SafeTxHashes`]).
pub fn safe_tx_hashes(chain_id: u64, safe: &str, tx: &SafeTx) -> Result<SafeTxHashes, CoreError> {
    let domain = domain_separator(chain_id, addr(safe)?);
    let message = safe_tx_struct_hash(tx)?;
    let hash = eip712(domain, message);
    Ok(SafeTxHashes {
        domain_hash: hex(domain.as_slice()),
        message_hash: hex(message.as_slice()),
        safe_tx_hash: hex(hash.as_slice()),
        eth_sign_hash: hex(eth_signed_hash(hash).as_slice()),
    })
}

/// What an owner's passkey signs to approve `safe_tx_hash`: the hash the
/// owner's compatibility handler checks for a v1.4.1 Safe asking it,
/// `SafeMessage(safeTxHash)` under the OWNER's domain.
pub fn approval_challenge(
    chain_id: u64,
    owner: &str,
    safe_tx_hash: &str,
) -> Result<String, CoreError> {
    let h = unhex(safe_tx_hash)?;
    if h.len() != 32 {
        return Err(err("safeTxHash must be 32 bytes"));
    }
    let mut s = keccak256("SafeMessage(bytes message)").to_vec();
    s.extend_from_slice(&h);
    Ok(hex(eip712(
        domain_separator(chain_id, addr(owner)?),
        keccak256(s),
    )
    .as_slice()))
}

// ---------------------------------------------------------------------------
// Approvals
// ---------------------------------------------------------------------------

/// A passkey assertion over [`approval_challenge`], as an approval the genesis
/// service and the multisig accept. `keys` are the owner account's founding
/// keys (`keys[0]` first); `credential_id` the key that signed.
pub fn approval_from_assertion(
    keys: &[WalletKey],
    credential_id: &str,
    authenticator_data_hex: &str,
    client_data_json_hex: &str,
    signature_der_hex: &str,
) -> Result<PasskeyApproval, CoreError> {
    let authenticator_data = unhex(authenticator_data_hex)?;
    let client_data_json = unhex(client_data_json_hex)?;
    crate::webauthn::validate_client_data(
        crate::ClientDataKind::Get,
        &client_data_json,
        &authenticator_data,
    )?;
    let raw = crate::webauthn::der_signature_to_raw_low_s(&unhex(signature_der_hex)?)?;
    let (owner, owner_genesis) = vela_account_genesis(keys)?;
    let signer = user_op::signer_address_for(keys, Some(credential_id))?;
    Ok(PasskeyApproval {
        owner: addr(&owner)?.to_checksum(None),
        owner_genesis,
        signer: addr(&signer)?.to_checksum(None),
        authenticator_data: hex(&authenticator_data),
        client_data_fields: user_op::extract_client_data_fields(&client_data_json),
        r: hex_quantity(U256::from_be_slice(&raw[..32])),
        s: hex_quantity(U256::from_be_slice(&raw[32..])),
    })
}

/// The owner keys a Vela account's setup carries: the shared signer's
/// `configure`d key first, then each `createSigner` key in setup order.
fn vela_setup_keys(initializer: &[u8]) -> Result<Vec<P256PublicKey>, CoreError> {
    let configure = selector("configure((uint256,uint256,uint176))");
    let create_signer = selector("createSigner(uint256,uint256,uint176)");
    let mut keys = Vec::new();
    // Scan for the two calls the Vela setup MultiSend carries; the result is
    // trusted only after the keys rebuild the very same initializer (below).
    let mut i = 0;
    while i + 4 + 64 <= initializer.len() {
        let sel = &initializer[i..i + 4];
        if sel == configure || sel == create_signer {
            keys.push(P256PublicKey {
                x: initializer[i + 4..i + 36].to_vec(),
                y: initializer[i + 36..i + 68].to_vec(),
            });
            i += 68;
        } else {
            i += 1;
        }
    }
    if keys.is_empty() {
        return Err(err("not a Vela account setup"));
    }
    Ok(keys)
}

/// Check an approval the way the chain will: the owner genesis is a Vela
/// account's (its keys rebuild exactly that initializer, and it deploys to
/// `owner`), `signer` is one of its keys' signers, the assertion is
/// user-verified and its signature is that key's over the approval challenge.
pub fn verify_approval(
    chain_id: u64,
    safe_tx_hash: &str,
    a: &PasskeyApproval,
) -> Result<(), CoreError> {
    let initializer = unhex(&a.owner_genesis.initializer)?;
    let keys = vela_setup_keys(&initializer)?;
    let info = safe::compute_safe_address_multi(&keys)?;
    if info.setup_data != initializer
        || U256::from_be_slice(&info.salt_nonce) != quantity(&a.owner_genesis.salt_nonce)?
        || addr(&info.address)? != addr(&a.owner)?
        || genesis_address(&a.owner_genesis)? != addr(&a.owner)?.to_checksum(None)
    {
        return Err(err("the owner genesis is not this owner's Vela account"));
    }
    let signer = addr(&a.signer)?;
    let key = keys
        .iter()
        .enumerate()
        .find(|(i, k)| {
            let s = if *i == 0 {
                safe::WEBAUTHN_SIGNER.to_owned()
            } else {
                safe::compute_webauthn_signer_address(&k.x, &k.y).unwrap_or_default()
            };
            s.parse::<Address>().ok() == Some(signer)
        })
        .map(|(_, k)| k)
        .ok_or_else(|| err("the signer is not one of the owner's keys"))?;
    let auth = unhex(&a.authenticator_data)?;
    if auth.len() < 37 || auth[32] & 0x04 == 0 {
        return Err(err("the approval is not user-verified"));
    }
    let challenge = unhex(&approval_challenge(chain_id, &a.owner, safe_tx_hash)?)?;
    let json = format!(
        "{{\"type\":\"webauthn.get\",\"challenge\":\"{}\",{}}}",
        primitives::to_base64url(&challenge),
        a.client_data_fields
    );
    let digest = crate::webauthn::webauthn_signing_hash(&auth, json.as_bytes());
    let mut rs = word_u256(quantity(&a.r)?).to_vec();
    rs.extend_from_slice(&word_u256(quantity(&a.s)?));
    let sig = p256::ecdsa::Signature::from_slice(&rs).map_err(|_| err("unreadable signature"))?;
    let mut point = vec![0x04];
    point.extend_from_slice(&key.x);
    point.extend_from_slice(&key.y);
    let vk =
        p256::ecdsa::VerifyingKey::from_sec1_bytes(&point).map_err(|_| err("bad owner key"))?;
    use p256::ecdsa::signature::hazmat::PrehashVerifier;
    let sig = sig.normalize_s();
    vk.verify_prehash(&digest, &sig)
        .map_err(|_| err("the approval signature does not verify"))
}

/// How a plain account signed a safeTxHash, as the Safe tells them apart by
/// `v`: its EIP-712 typed data (`v` 27/28, or 0/1 as some wallets answer), or
/// Safe's `eth_sign` form (`v` 31/32) — `personal_sign` over the safeTxHash,
/// for a wallet that cannot sign typed data, which the Safe checks against
/// the EIP-191 hash of the safeTxHash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EcdsaForm {
    TypedData,
    EthSign,
}

/// The form of a 65-byte `r ‖ s ‖ v` by its `v`; an error for any other.
pub fn ecdsa_form(signature: &str) -> Result<EcdsaForm, CoreError> {
    let sig = unhex(signature)?;
    if sig.len() != 65 {
        return Err(err("not a 65-byte signature"));
    }
    match sig[64] {
        0 | 1 | 27 | 28 => Ok(EcdsaForm::TypedData),
        31 | 32 => Ok(EcdsaForm::EthSign),
        v => Err(err(format!("v {v} is not a plain account's approval"))),
    }
}

/// What `personal_sign` signs for a 32-byte message: the EIP-191 hash
/// (`"\x19Ethereum Signed Message:\n32" ‖ hash`), as the Safe recomputes it
/// for an `eth_sign` approval.
fn eth_signed_hash(hash: B256) -> B256 {
    let mut p = b"\x19Ethereum Signed Message:\n32".to_vec();
    p.extend_from_slice(hash.as_slice());
    keccak256(p)
}

/// `ecrecover(hash, v, r, s)` over a 65-byte `r ‖ s ‖ v` (`v` 27/28, or 0/1).
/// The `eth_sign` form (`v` 31/32) recovers over the EIP-191 hash with
/// `v − 4`, as the Safe does.
pub(crate) fn recover(hash: B256, signature: &[u8]) -> Result<Address, CoreError> {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    if signature.len() != 65 {
        return Err(err("not a 65-byte signature"));
    }
    let (parity, hash) = match signature[64] {
        0 | 27 => (false, hash),
        1 | 28 => (true, hash),
        31 => (false, eth_signed_hash(hash)),
        32 => (true, eth_signed_hash(hash)),
        v => return Err(err(format!("v {v} is not a plain account's approval"))),
    };
    let sig = Signature::from_slice(&signature[..64]).map_err(|_| err("unreadable signature"))?;
    let key =
        VerifyingKey::recover_from_prehash(hash.as_slice(), &sig, RecoveryId::new(parity, false))
            .map_err(|_| err("the signature recovers no key"))?;
    let point = key.to_sec1_point(false);
    Ok(Address::from_slice(
        &keccak256(&point.as_bytes()[1..])[12..],
    ))
}

/// Check a plain account's approval as the Safe will (`ecrecover` over the
/// safeTxHash is the owner — or, in the `eth_sign` form, over its EIP-191
/// hash). Returns it with `v` as the Safe reads it (27/28, or 31/32).
pub fn verify_ecdsa_approval(
    safe_tx_hash: &str,
    a: &EcdsaApproval,
) -> Result<EcdsaApproval, CoreError> {
    let mut sig = unhex(&a.signature)?;
    let hash: B256 = safe_tx_hash.parse().map_err(|_| err("not a safeTxHash"))?;
    if recover(hash, &sig)? != addr(&a.owner)? {
        return Err(err(
            "the signature is not this owner's for this transaction",
        ));
    }
    if sig[64] < 27 {
        sig[64] += 27;
    }
    Ok(EcdsaApproval {
        owner: addr(&a.owner)?.to_checksum(None),
        signature: hex(&sig),
    })
}

/// The SafeTx as `eth_signTypedData_v4` takes it: Safe's EIP-712 domain
/// (`chainId`, `verifyingContract` — Safe v1.3.0 and later) and its `SafeTx`
/// type. Its hash is [`safe_tx_hash`]; a wallet shows the fields.
pub fn safe_tx_typed_data(
    chain_id: u64,
    safe: &str,
    tx: &SafeTx,
) -> Result<serde_json::Value, CoreError> {
    let t = parse_tx(tx)?;
    Ok(serde_json::json!({
        "types": {
            "EIP712Domain": [
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"}
            ],
            "SafeTx": [
                {"name": "to", "type": "address"},
                {"name": "value", "type": "uint256"},
                {"name": "data", "type": "bytes"},
                {"name": "operation", "type": "uint8"},
                {"name": "safeTxGas", "type": "uint256"},
                {"name": "baseGas", "type": "uint256"},
                {"name": "gasPrice", "type": "uint256"},
                {"name": "gasToken", "type": "address"},
                {"name": "refundReceiver", "type": "address"},
                {"name": "nonce", "type": "uint256"}
            ]
        },
        "primaryType": "SafeTx",
        "domain": {"chainId": chain_id, "verifyingContract": addr(safe)?.to_checksum(None)},
        "message": {
            "to": t.to.to_checksum(None),
            "value": t.value.to_string(),
            "data": hex(&t.data),
            "operation": t.operation,
            "safeTxGas": t.safe_tx_gas.to_string(),
            "baseGas": t.base_gas.to_string(),
            "gasPrice": t.gas_price.to_string(),
            "gasToken": t.gas_token.to_checksum(None),
            "refundReceiver": t.refund_receiver.to_checksum(None),
            "nonce": t.nonce.to_string()
        }
    }))
}

/// The signature bytes `execTransaction` takes: one entry per approval,
/// ordered by owner address as Safe requires — a contract signature
/// (`r = owner, s = offset, v = 0`, the owner's own EIP-1271 envelope in the
/// tail) per passkey approval, a plain account's own 65 bytes (`v` 27/28)
/// per EOA approval (`v` 27/28, or 31/32 in the `eth_sign` form), and
/// `r = owner, v = 1` for the executor, whose `msg.sender` approves on its
/// own, and for an owner that approved on chain (`approveHash`). An executor
/// that also sent an approval counts once.
pub fn assemble_signatures(
    approvals: &[Approval],
    executor: Option<&str>,
) -> Result<String, CoreError> {
    enum Entry {
        Contract(Vec<u8>),
        Ecdsa(Vec<u8>),
        Sender,
    }
    let executor = executor.map(addr).transpose()?;
    let mut entries: Vec<(Address, Entry)> = Vec::new();
    for approval in approvals {
        let owner = addr(approval.owner())?;
        if Some(owner) == executor || entries.iter().any(|(o, _)| *o == owner) {
            continue;
        }
        let entry = match approval {
            Approval::Passkey(a) => Entry::Contract(user_op::build_eip1271_signature(
                &unhex(&a.authenticator_data)?,
                &a.client_data_fields,
                &word_u256(quantity(&a.r)?),
                &word_u256(quantity(&a.s)?),
                &a.signer,
            )?),
            Approval::Ecdsa(a) => {
                let mut sig = unhex(&a.signature)?;
                if sig.len() != 65 {
                    return Err(err("a plain account's signature is 65 bytes"));
                }
                if sig[64] < 27 {
                    sig[64] += 27;
                }
                Entry::Ecdsa(sig)
            }
            Approval::OnChain(_) => Entry::Sender,
        };
        entries.push((owner, entry));
    }
    if let Some(e) = executor {
        entries.push((e, Entry::Sender));
    }
    entries.sort_by_key(|(o, _)| *o);
    let head_len = entries.len() * 65;
    let mut head = Vec::with_capacity(head_len);
    let mut tail = Vec::new();
    for (owner, entry) in &entries {
        head.extend_from_slice(&word_addr(*owner));
        match entry {
            Entry::Contract(inner) => {
                head.extend_from_slice(&word_usize(head_len + tail.len()));
                head.push(0);
                tail.extend_from_slice(&word_usize(inner.len()));
                tail.extend_from_slice(inner);
            }
            Entry::Ecdsa(sig) => {
                // Not the owner word: the signature itself, r ‖ s ‖ v.
                head.truncate(head.len() - 32);
                head.extend_from_slice(sig);
            }
            Entry::Sender => {
                head.extend_from_slice(&[0u8; 32]);
                head.push(1);
            }
        }
    }
    head.extend(tail);
    Ok(hex(&head))
}

/// `execTransaction(...)` calldata.
pub fn exec_transaction_data(tx: &SafeTx, signatures: &str) -> Result<String, CoreError> {
    let t = parse_tx(tx)?;
    let sigs = unhex(signatures)?;
    let mut out = selector(
        "execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)",
    )
    .to_vec();
    let data_tail = bytes_tail(&t.data);
    out.extend_from_slice(&word_addr(t.to));
    out.extend_from_slice(&word_u256(t.value));
    out.extend_from_slice(&word_usize(10 * 32)); // data offset
    out.extend_from_slice(&word_u256(U256::from(t.operation)));
    out.extend_from_slice(&word_u256(t.safe_tx_gas));
    out.extend_from_slice(&word_u256(t.base_gas));
    out.extend_from_slice(&word_u256(t.gas_price));
    out.extend_from_slice(&word_addr(t.gas_token));
    out.extend_from_slice(&word_addr(t.refund_receiver));
    out.extend_from_slice(&word_usize(10 * 32 + data_tail.len())); // signatures offset
    out.extend(data_tail);
    out.extend(bytes_tail(&sigs));
    Ok(hex(&out))
}

/// What a multisig's execution needs deployed first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlan {
    pub multisig: String,
    /// The multisig's genesis, when it has no code on the chain yet.
    pub deploy_multisig: Option<Genesis>,
    /// Approving owners with no code on the chain yet: their checks need it.
    pub deploy_owners: Vec<Genesis>,
    pub tx: SafeTx,
    pub approvals: Vec<Approval>,
    /// The executor, when it is an owner: it sends this as its own user
    /// operation and approves as `msg.sender`. `None` for a sender that is not
    /// an owner (any Vela account, or an external wallet): the approvals alone
    /// must meet the threshold.
    #[serde(default)]
    pub executor: Option<String>,
    /// The code of what `deploy_multisig` depends on, read on the chain just
    /// now ([`counterfactual_reads`]): the multisig is deployed only if its
    /// genesis is Verified there ([`counterfactual_authority`]).
    #[serde(default)]
    pub deploy_facts: Option<ChainFacts>,
}

/// Whether `tx` is the multisig renaming its own record (`rename(name)` to
/// [`GENESIS_REGISTRY`]).
fn is_rename(tx: &SafeTx) -> bool {
    let (Ok(to), Ok(data)) = (addr(&tx.to), unhex(&tx.data)) else {
        return false;
    };
    to == addr(GENESIS_REGISTRY).unwrap_or_default()
        && data.get(..4) == Some(&selector(RENAME_SIGNATURE)[..])
}

/// The executor's calls: deploy what has no code yet, then `execTransaction`.
///
/// A rename never deploys (owner, 2026-10-29): deploying a multisig on the
/// registry's chain to rename its record would create it there with its
/// ORIGINAL members, whoever controls it on its other networks now. A rename
/// runs only where the multisig is deployed already.
///
/// The multisig is deployed only from a genesis Verified on this chain — with
/// the chain's code in `deploy_facts` — never one that would hand it to
/// anything but its members.
pub fn execution_calls(plan: &ExecutionPlan) -> Result<Vec<Call>, CoreError> {
    let mut calls = Vec::new();
    if let Some(g) = &plan.deploy_multisig {
        if is_rename(&plan.tx) {
            return Err(err(
                "a rename runs only where the multisig is deployed: it never deploys it",
            ));
        }
        if genesis_address(g)? != addr(&plan.multisig)?.to_checksum(None) {
            return Err(err("the genesis does not deploy this multisig"));
        }
        deploy_verified(plan, g)?;
        calls.push(deploy_call(g)?);
    }
    for g in &plan.deploy_owners {
        calls.push(deploy_call(g)?);
    }
    let signatures = assemble_signatures(&plan.approvals, plan.executor.as_deref())?;
    calls.push(Call {
        to: addr(&plan.multisig)?.to_checksum(None),
        value: "0x0".into(),
        data: exec_transaction_data(&plan.tx, &signatures)?,
    });
    Ok(calls)
}

// ---------------------------------------------------------------------------
// Importing from Safe{Wallet}
// ---------------------------------------------------------------------------

/// One Safe in a Safe{Wallet} export, across the chains it is listed on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedSafe {
    pub address: String,
    /// The person's own name for it (the export's address book), if one is
    /// displayable: a note on this device, never part of the Safe.
    pub name: Option<String>,
    pub chains: Vec<ExportedChain>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedChain {
    pub chain_id: u64,
    /// Listed among the deployed Safes (`addedSafes`); otherwise only its
    /// creation parameters are (`undeployedSafes`).
    pub deployed: bool,
    pub owners: Vec<String>,
    pub threshold: u64,
    /// An undeployed Safe's creation parameters, as Safe{Wallet} keeps them
    /// (the genesis service's `safeWalletProps`).
    pub props: Option<serde_json::Value>,
    pub version: Option<String>,
}

/// Read a Safe{Wallet} export (Settings → Data → Export, version 3): every
/// Safe in it, merged across chains, by address. Nothing in it is trusted —
/// owners and thresholds are what the export says, to choose by; what is
/// imported is checked against the chain and the genesis service.
pub fn parse_safe_export(json: &str) -> Result<Vec<ExportedSafe>, CoreError> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|e| err(format!("not a Safe{{Wallet}} export: {e}")))?;
    let data = root
        .get("data")
        .and_then(|d| d.as_object())
        .ok_or_else(|| err("not a Safe{Wallet} export: no data"))?;
    let mut safes: std::collections::BTreeMap<Address, ExportedSafe> = Default::default();
    let entry = |safes: &mut std::collections::BTreeMap<Address, ExportedSafe>, a: Address| {
        safes.entry(a).or_insert_with(|| ExportedSafe {
            address: a.to_checksum(None),
            name: None,
            chains: Vec::new(),
        });
    };
    let chains_of = |key: &str| {
        data.get(key)
            .and_then(|v| v.as_object())
            .into_iter()
            .flatten()
            .filter_map(|(chain, v)| Some((chain.parse::<u64>().ok()?, v.as_object()?)))
    };
    let owners_of = |list: Option<&serde_json::Value>| -> Vec<String> {
        list.and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|o| {
                o.get("value")
                    .and_then(|v| v.as_str())
                    .or_else(|| o.as_str())
            })
            .filter_map(|o| o.parse::<Address>().ok())
            .map(|o| o.to_checksum(None))
            .collect()
    };
    for (chain_id, list) in chains_of("addedSafes") {
        for (address, v) in list {
            let Ok(a) = address.parse::<Address>() else {
                continue;
            };
            entry(&mut safes, a);
            let owners = owners_of(v.get("owners"));
            let threshold = v.get("threshold").and_then(|t| t.as_u64()).unwrap_or(0);
            if let Some(s) = safes.get_mut(&a) {
                s.chains.push(ExportedChain {
                    chain_id,
                    deployed: true,
                    owners,
                    threshold,
                    props: None,
                    version: None,
                });
            }
        }
    }
    for (chain_id, list) in chains_of("undeployedSafes") {
        for (address, v) in list {
            let Ok(a) = address.parse::<Address>() else {
                continue;
            };
            let Some(props) = v.get("props") else {
                continue;
            };
            entry(&mut safes, a);
            let config = props.get("safeAccountConfig");
            let owners = owners_of(config.and_then(|c| c.get("owners")));
            let threshold = config
                .and_then(|c| c.get("threshold"))
                .and_then(|t| t.as_u64())
                .unwrap_or(0);
            let version = props
                .get("safeVersion")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            if let Some(s) = safes.get_mut(&a) {
                if s.chains.iter().any(|c| c.chain_id == chain_id) {
                    continue; // listed deployed there already
                }
                s.chains.push(ExportedChain {
                    chain_id,
                    deployed: false,
                    owners,
                    threshold,
                    props: Some(props.clone()),
                    version,
                });
            }
        }
    }
    // The address book names a Safe per chain; the first displayable name wins.
    for (_, book) in chains_of("addressBook") {
        for (address, name) in book {
            let (Ok(a), Some(name)) = (address.parse::<Address>(), name.as_str()) else {
                continue;
            };
            let name = name.trim();
            if let Some(s) = safes.get_mut(&a) {
                if s.name.is_none() && valid_name(name) {
                    s.name = Some(name.to_owned());
                }
            }
        }
    }
    let mut out: Vec<ExportedSafe> = safes.into_values().collect();
    for s in &mut out {
        s.chains.sort_by_key(|c| c.chain_id);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Building and reading transactions
// ---------------------------------------------------------------------------

fn plain_tx(to: Address, value: U256, data: Vec<u8>, operation: u8, nonce: U256) -> SafeTx {
    let zero = "0x0".to_owned();
    SafeTx {
        to: to.to_checksum(None),
        value: hex_quantity(value),
        data: hex(&data),
        operation,
        safe_tx_gas: zero.clone(),
        base_gas: zero.clone(),
        gas_price: zero,
        gas_token: Address::ZERO.to_checksum(None),
        refund_receiver: Address::ZERO.to_checksum(None),
        nonce: hex_quantity(nonce),
    }
}

/// One transfer: native when `token` is empty, else an ERC-20 `transfer`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    #[serde(default)]
    pub token: Option<String>,
    pub to: String,
    /// Base units, hex or decimal.
    pub amount: String,
}

fn transfer_call(t: &Transfer) -> Result<(Address, U256, Vec<u8>), CoreError> {
    let to = addr(&t.to)?;
    let amount = quantity(&t.amount)?;
    match t.token.as_deref().filter(|s| !s.is_empty()) {
        None => Ok((to, amount, Vec::new())),
        Some(token) => {
            let mut d = selector("transfer(address,uint256)").to_vec();
            d.extend_from_slice(&word_addr(to));
            d.extend_from_slice(&word_u256(amount));
            Ok((addr(token)?, U256::ZERO, d))
        }
    }
}

/// The MultiSendCallOnly transaction carrying `calls` (every leg a CALL).
fn multi_send_tx(calls: &[(Address, U256, Vec<u8>)], nonce: U256) -> Result<SafeTx, CoreError> {
    let mut packed = Vec::new();
    for (to, value, data) in calls {
        packed.push(0u8);
        packed.extend_from_slice(to.as_slice());
        packed.extend_from_slice(&word_u256(*value));
        packed.extend_from_slice(&word_usize(data.len()));
        packed.extend_from_slice(data);
    }
    let mut d = selector("multiSend(bytes)").to_vec();
    d.extend_from_slice(&word_usize(0x20));
    d.extend(bytes_tail(&packed));
    Ok(plain_tx(
        addr(MULTI_SEND_CALL_ONLY)?,
        U256::ZERO,
        d,
        1,
        nonce,
    ))
}

/// A SafeTx for one or more transfers; several go through MultiSendCallOnly.
pub fn transfers_tx(transfers: &[Transfer], nonce: &str) -> Result<SafeTx, CoreError> {
    let nonce = quantity(nonce)?;
    match transfers {
        [] => Err(err("nothing to send")),
        [one] => {
            let (to, value, data) = transfer_call(one)?;
            Ok(plain_tx(to, value, data, 0, nonce))
        }
        many => multi_send_tx(
            &many
                .iter()
                .map(transfer_call)
                .collect::<Result<Vec<_>, _>>()?,
            nonce,
        ),
    }
}

/// One call a site asks a multisig to make (`eth_sendTransaction`, or one
/// of a `wallet_sendCalls`' calls): `value` a quantity (hex or decimal,
/// empty = 0), `data` hex (empty = none).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallArg {
    pub to: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub data: String,
}

fn call_arg(c: &CallArg) -> Result<(Address, U256, Vec<u8>), CoreError> {
    let value = if c.value.trim().is_empty() {
        U256::ZERO
    } else {
        quantity(&c.value)?
    };
    let data = if c.data.is_empty() || c.data == "0x" {
        Vec::new()
    } else {
        unhex(&c.data)?
    };
    Ok((addr(&c.to)?, value, data))
}

/// A SafeTx for a site's calls: one is a plain CALL; several are ONE
/// transaction through MultiSendCallOnly, every leg a CALL, so they run
/// together or not at all (EIP-5792 `atomic`). Whether a multisig signs it
/// is still [`review`]'s to say.
pub fn calls_tx(calls: &[CallArg], nonce: &str) -> Result<SafeTx, CoreError> {
    let nonce = quantity(nonce)?;
    let legs = calls.iter().map(call_arg).collect::<Result<Vec<_>, _>>()?;
    match legs.as_slice() {
        [] => Err(err("nothing to call")),
        [(to, value, data)] => Ok(plain_tx(*to, *value, data.clone(), 0, nonce)),
        many => multi_send_tx(many, nonce),
    }
}

/// The transaction that voids every proposal at `nonce`: nothing, sent to the
/// multisig itself.
pub fn cancel_tx(multisig: &str, nonce: &str) -> Result<SafeTx, CoreError> {
    Ok(plain_tx(
        addr(multisig)?,
        U256::ZERO,
        Vec::new(),
        0,
        quantity(nonce)?,
    ))
}

/// What a SafeTx does, as the core reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Action {
    /// A native transfer (`data` empty) or an ERC-20 `transfer`.
    Transfer {
        token: Option<String>,
        to: String,
        amount: String,
    },
    /// Several calls in ONE transaction, through MultiSendCallOnly, every one
    /// a CALL (a delegatecall leg is refused), each read on its own.
    Batch {
        calls: Vec<BatchCall>,
    },
    /// Nothing, to the multisig itself: voids its nonce.
    Cancel,
    AddOwner {
        owner: String,
        threshold: String,
    },
    RemoveOwner {
        owner: String,
        threshold: String,
    },
    SwapOwner {
        old_owner: String,
        new_owner: String,
    },
    ChangeThreshold {
        threshold: String,
    },
    /// The multisig renames its own record in the genesis registry
    /// (`rename(name)` to [`GENESIS_REGISTRY`]): a label, read by anyone who
    /// looks the multisig up; it changes nothing about who controls it.
    Rename {
        name: String,
    },
    /// Any other call to another contract (a dApp's): `value` in base units,
    /// decimal; `data` as sent. Never a call into the multisig itself, nor a
    /// delegatecall — those can rewrite the Safe and stay refused.
    Call {
        to: String,
        value: String,
        data: String,
    },
}

/// One call of a batch: what it sends, as packed (`value` in base units,
/// decimal; `data` hex), and what the core reads it as — a
/// [`Action::Transfer`], a [`Action::Call`] to another contract, or, for a
/// call to the multisig itself, one of the four member changes (each one
/// changes who controls the multisig: a shell says so loudly). Never a
/// batch, a cancellation or any other call into the multisig.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCall {
    pub to: String,
    pub value: String,
    pub data: String,
    pub action: Action,
}

impl BatchCall {
    /// This call changes the multisig's own members or threshold.
    pub fn changes_members(&self) -> bool {
        matches!(
            self.action,
            Action::AddOwner { .. }
                | Action::RemoveOwner { .. }
                | Action::SwapOwner { .. }
                | Action::ChangeThreshold { .. }
        )
    }
}

/// The core's reading of a SafeTx and whether Vela signs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub safe_tx_hash: String,
    pub nonce: String,
    /// `None` when the transaction is not one Vela signs; `refusal` says why.
    pub action: Option<Action>,
    pub refusal: Option<String>,
}

fn read_transfer(to: Address, value: U256, data: &[u8]) -> Option<Action> {
    if data.is_empty() {
        return Some(Action::Transfer {
            token: None,
            to: to.to_checksum(None),
            amount: value.to_string(),
        });
    }
    if value.is_zero()
        && data.len() == 68
        && data[..4] == selector("transfer(address,uint256)")
        && data[4..16] == [0u8; 12]
    {
        let recipient = Address::from_slice(&data[16..36]);
        let amount = U256::from_be_slice(&data[36..68]);
        return Some(Action::Transfer {
            token: Some(to.to_checksum(None)),
            to: recipient.to_checksum(None),
            amount: amount.to_string(),
        });
    }
    None
}

pub(crate) fn read_owner_management(data: &[u8]) -> Option<Action> {
    let w = |i: usize| {
        data.get(4 + i * 32..4 + (i + 1) * 32)
            .map(U256::from_be_slice)
    };
    let a = |i: usize| w(i).map(|v| Address::from_word(B256::from(v)).to_checksum(None));
    let sel = data.get(..4)?;
    if sel == selector("addOwnerWithThreshold(address,uint256)") && data.len() == 68 {
        return Some(Action::AddOwner {
            owner: a(0)?,
            threshold: w(1)?.to_string(),
        });
    }
    if sel == selector("removeOwner(address,address,uint256)") && data.len() == 100 {
        return Some(Action::RemoveOwner {
            owner: a(1)?,
            threshold: w(2)?.to_string(),
        });
    }
    if sel == selector("swapOwner(address,address,address)") && data.len() == 100 {
        return Some(Action::SwapOwner {
            old_owner: a(1)?,
            new_owner: a(2)?,
        });
    }
    if sel == selector("changeThreshold(uint256)") && data.len() == 36 {
        return Some(Action::ChangeThreshold {
            threshold: w(0)?.to_string(),
        });
    }
    None
}

// -- changing the members ----------------------------------------------------------
//
// A Safe keeps its owners as a linked list (`owners[SENTINEL] → first → … →
// SENTINEL`), so removing or replacing an owner names the one BEFORE it
// (`prevOwner`); `getOwners()` returns the list in that order. Each network
// has its own list (a Safe's owners on one chain never change another's), so a
// change is built against one network's members as they stand now, read from
// that chain — never the genesis', never another network's.

/// A change to a multisig's members or the approvals it needs, as the person
/// asks for it. Built per network ([`member_change_tx`]). The one name for
/// it everywhere: the members machine's plan carries it per network, inside
/// the proposals machine's `ProposalKind::MemberChange { change }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMemberChange"))]
pub enum MemberChange {
    /// `addOwnerWithThreshold(owner, threshold)`; no threshold keeps the
    /// network's.
    Add {
        owner: String,
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        threshold: Option<u64>,
    },
    /// `removeOwner(prevOwner, owner, threshold)`; no threshold keeps the
    /// network's (and is refused when too few would be left for it).
    Remove {
        owner: String,
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        threshold: Option<u64>,
    },
    /// `swapOwner(prevOwner, oldOwner, newOwner)`: the way back from a lost
    /// passkey — the other members put the member's new account in its place.
    Replace {
        old_owner: String,
        new_owner: String,
    },
    /// `changeThreshold(threshold)`.
    Threshold {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        threshold: u64,
    },
}

/// One network's members as they stand now: `owners` in the order
/// `getOwners()` returns them (the Safe's list order), and whether a member
/// change is already waiting there (not executed, not voided).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberState {
    pub owners: Vec<String>,
    pub threshold: u64,
    #[serde(default)]
    pub change_waiting: bool,
}

/// Owners and threshold, before or after a change (owners in list order).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberSet {
    pub owners: Vec<String>,
    pub threshold: u64,
}

/// Why a change cannot be made on a network: each a state the Safe would
/// refuse (or one that would lock the funds), said before anything is signed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MemberRefusal {
    /// Not an address.
    NotAnAddress { address: String },
    /// The zero address or the list's sentinel (`0x…01`): no one holds it.
    NoOne { address: String },
    /// The multisig itself: it cannot approve its own transactions.
    TheMultisig,
    /// Already a member on this network.
    AlreadyMember { owner: String },
    /// Not a member on this network.
    NotMember { owner: String },
    /// Removing the only member would leave no one.
    LastMember,
    /// More approvals than members (after the change).
    ThresholdTooHigh { threshold: u64, owners: u64 },
    /// No approvals at all.
    ThresholdZero,
    /// Removing would leave fewer members than must approve: lower the
    /// approvals in the same change, or replace the member instead.
    BelowThreshold { owners: u64, threshold: u64 },
    /// The change changes nothing (the threshold it already has).
    Unchanged,
    /// More members than a Vela multisig has ([`MAX_OWNERS`]).
    TooMany { max: u64 },
    /// Another member change waits on this network: built on today's list, a
    /// second one could name the wrong `prevOwner` once the first runs.
    ChangeWaiting,
}

/// What [`member_change_tx`] answers for one network.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MemberVerdict {
    /// The transaction to propose, and the members before and after it.
    Ready {
        tx: Box<SafeTx>,
        before: MemberSet,
        after: MemberSet,
    },
    Refused {
        refusal: MemberRefusal,
    },
}

/// The owner before `owner` in the Safe's list (`owners` as `getOwners()`
/// returns them): the sentinel for the first. `None` when it is not one.
pub fn prev_owner(owners: &[Address], owner: Address) -> Option<Address> {
    let i = owners.iter().position(|o| *o == owner)?;
    Some(if i == 0 { SENTINEL } else { owners[i - 1] })
}

fn typed_address(address: &str) -> Result<Address, MemberRefusal> {
    address
        .trim()
        .parse::<Address>()
        .map_err(|_| MemberRefusal::NotAnAddress {
            address: address.to_owned(),
        })
}

/// A member to be: an address someone can hold, other than the multisig.
fn new_member(address: &str, me: Address) -> Result<Address, MemberRefusal> {
    let a = typed_address(address)?;
    if a == Address::ZERO || a == SENTINEL {
        return Err(MemberRefusal::NoOne {
            address: a.to_checksum(None),
        });
    }
    if a == me {
        return Err(MemberRefusal::TheMultisig);
    }
    Ok(a)
}

fn checked_threshold(threshold: u64, owners: usize) -> Result<u64, MemberRefusal> {
    if threshold == 0 {
        return Err(MemberRefusal::ThresholdZero);
    }
    if threshold > owners as u64 {
        return Err(MemberRefusal::ThresholdTooHigh {
            threshold,
            owners: owners as u64,
        });
    }
    Ok(threshold)
}

fn change_data(signature: &str, words: &[[u8; 32]]) -> Vec<u8> {
    let mut d = selector(signature).to_vec();
    for w in words {
        d.extend_from_slice(w);
    }
    d
}

/// The calldata of `change` into `multisig` on a network whose members are
/// `state`, and the members after it runs — or why the change cannot be made
/// there. Mirrors what the Safe's `OwnerManager` itself requires (GS200–GS205)
/// and adds the wallet's own: no change that changes nothing, at most
/// [`MAX_OWNERS`], and one member change at a time per network.
pub fn member_change_call(
    multisig: &str,
    state: &MemberState,
    change: &MemberChange,
) -> Result<Result<(Vec<u8>, MemberSet), MemberRefusal>, CoreError> {
    let me = addr(multisig)?;
    let owners = state
        .owners
        .iter()
        .map(|o| addr(o))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(build_member_change(me, &owners, state, change))
}

fn build_member_change(
    me: Address,
    owners: &[Address],
    state: &MemberState,
    change: &MemberChange,
) -> Result<(Vec<u8>, MemberSet), MemberRefusal> {
    let shown = |list: &[Address]| list.iter().map(|a| a.to_checksum(None)).collect();
    if state.change_waiting {
        return Err(MemberRefusal::ChangeWaiting);
    }
    let member = |address: &str| -> Result<Address, MemberRefusal> {
        let a = typed_address(address)?;
        if !owners.contains(&a) {
            return Err(MemberRefusal::NotMember {
                owner: a.to_checksum(None),
            });
        }
        Ok(a)
    };
    let not_yet = |a: Address| -> Result<Address, MemberRefusal> {
        if owners.contains(&a) {
            return Err(MemberRefusal::AlreadyMember {
                owner: a.to_checksum(None),
            });
        }
        Ok(a)
    };
    match change {
        MemberChange::Add { owner, threshold } => {
            let new = not_yet(new_member(owner, me)?)?;
            if owners.len() >= MAX_OWNERS {
                return Err(MemberRefusal::TooMany {
                    max: MAX_OWNERS as u64,
                });
            }
            // The Safe puts a new owner at the head of its list.
            let after: Vec<Address> = std::iter::once(new).chain(owners.iter().copied()).collect();
            let t = checked_threshold(threshold.unwrap_or(state.threshold), after.len())?;
            let d = change_data(
                "addOwnerWithThreshold(address,uint256)",
                &[word_addr(new), word_u256(U256::from(t))],
            );
            Ok((
                d,
                MemberSet {
                    owners: shown(&after),
                    threshold: t,
                },
            ))
        }
        MemberChange::Remove { owner, threshold } => {
            let gone = member(owner)?;
            if owners.len() == 1 {
                return Err(MemberRefusal::LastMember);
            }
            let after: Vec<Address> = owners.iter().copied().filter(|o| *o != gone).collect();
            let t = match threshold {
                Some(t) => checked_threshold(*t, after.len())?,
                None if state.threshold > after.len() as u64 => {
                    return Err(MemberRefusal::BelowThreshold {
                        owners: after.len() as u64,
                        threshold: state.threshold,
                    })
                }
                None => state.threshold,
            };
            let prev = prev_owner(owners, gone).ok_or(MemberRefusal::NotMember {
                owner: gone.to_checksum(None),
            })?;
            let d = change_data(
                "removeOwner(address,address,uint256)",
                &[word_addr(prev), word_addr(gone), word_u256(U256::from(t))],
            );
            Ok((
                d,
                MemberSet {
                    owners: shown(&after),
                    threshold: t,
                },
            ))
        }
        MemberChange::Replace {
            old_owner,
            new_owner,
        } => {
            let old = member(old_owner)?;
            let new = not_yet(new_member(new_owner, me)?)?;
            let prev = prev_owner(owners, old).ok_or(MemberRefusal::NotMember {
                owner: old.to_checksum(None),
            })?;
            // The new owner takes the old one's place in the list.
            let after: Vec<Address> = owners
                .iter()
                .map(|o| if *o == old { new } else { *o })
                .collect();
            let d = change_data(
                "swapOwner(address,address,address)",
                &[word_addr(prev), word_addr(old), word_addr(new)],
            );
            Ok((
                d,
                MemberSet {
                    owners: shown(&after),
                    threshold: state.threshold,
                },
            ))
        }
        MemberChange::Threshold { threshold } => {
            let t = checked_threshold(*threshold, owners.len())?;
            if t == state.threshold {
                return Err(MemberRefusal::Unchanged);
            }
            let d = change_data("changeThreshold(uint256)", &[word_u256(U256::from(t))]);
            Ok((
                d,
                MemberSet {
                    owners: shown(owners),
                    threshold: t,
                },
            ))
        }
    }
}

/// `change` as the multisig's own transaction at `nonce` on a network whose
/// members are `state` (a call into the multisig itself, nothing sent), with
/// the members before and after — or why it cannot be made there. [`review`]
/// reads it back as the member change it is.
pub fn member_change_tx(
    multisig: &str,
    state: &MemberState,
    change: &MemberChange,
    nonce: &str,
) -> Result<MemberVerdict, CoreError> {
    let nonce = quantity(nonce)?;
    let before = MemberSet {
        owners: state
            .owners
            .iter()
            .map(|o| addr(o).map(|a| a.to_checksum(None)))
            .collect::<Result<_, _>>()?,
        threshold: state.threshold,
    };
    Ok(match member_change_call(multisig, state, change)? {
        Ok((data, after)) => MemberVerdict::Ready {
            tx: Box::new(plain_tx(addr(multisig)?, U256::ZERO, data, 0, nonce)),
            before,
            after,
        },
        Err(refusal) => MemberVerdict::Refused { refusal },
    })
}

/// The calls of a MultiSendCallOnly batch, or why it is not one Vela signs.
fn read_batch(data: &[u8], me: Address) -> Result<Vec<BatchCall>, &'static str> {
    const UNREADABLE: &str = "a batch Vela cannot read in full";
    let header = || -> Option<usize> {
        if data.get(..4)? != selector("multiSend(bytes)")
            || U256::from_be_slice(data.get(4..36)?) != U256::from(0x20)
        {
            return None;
        }
        U256::from_be_slice(data.get(36..68)?).try_into().ok()
    };
    let registry: Address = GENESIS_REGISTRY.parse().map_err(|_| UNREADABLE)?;
    let len = header().ok_or(UNREADABLE)?;
    let packed = data.get(68..68 + len).ok_or(UNREADABLE)?;
    // Canonical encoding only: nothing may hide in padding or trailing bytes.
    if data.len() != 68 + len + pad32(len) || data[68 + len..].iter().any(|b| *b != 0) {
        return Err(UNREADABLE);
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < packed.len() {
        let leg = packed.get(i..i + 85).ok_or(UNREADABLE)?;
        // A delegatecall leg runs another contract's code AS the multisig: it
        // could rewrite it (MultiSendCallOnly refuses one anyway).
        if leg[0] != 0 {
            return Err("a call in the batch is a delegatecall");
        }
        let to = Address::from_slice(&leg[1..21]);
        let value = U256::from_be_slice(&leg[21..53]);
        let n: usize = U256::from_be_slice(&leg[53..85])
            .try_into()
            .map_err(|_| UNREADABLE)?;
        let d = packed.get(i + 85..i + 85 + n).ok_or(UNREADABLE)?;
        // MultiSend reads the zero address as the multisig itself: a call
        // that hides where it goes.
        if to == Address::ZERO {
            return Err("a call in the batch goes to the zero address");
        }
        let action = if to == me {
            // Into the multisig itself: only the four member changes, with no
            // coin. Modules, a guard, a fallback handler, a new singleton —
            // what can bypass every owner — stay refused, as they are alone.
            let members = if value.is_zero() {
                read_owner_management(d)
            } else {
                None
            };
            members.ok_or("a call in the batch changes the multisig in a way Vela does not sign")?
        } else if to == registry {
            read_registry_call(to, value, d)?
        } else {
            read_transfer(to, value, d).unwrap_or_else(|| Action::Call {
                to: to.to_checksum(None),
                value: value.to_string(),
                data: hex(d),
            })
        };
        out.push(BatchCall {
            to: to.to_checksum(None),
            value: value.to_string(),
            data: hex(d),
            action,
        });
        i += 85 + n;
    }
    if out.is_empty() {
        return Err(UNREADABLE);
    }
    Ok(out)
}

/// Read `tx` for `multisig` on `chain_id`. Vela signs only: transfers and
/// calls to other contracts — one, or a batch of them through
/// MultiSendCallOnly, whose calls may also change the members — and owner
/// management and cancellation sent to the multisig itself; with every gas
/// field zero, so execution can never pay a refund out of the multisig.
pub fn review(chain_id: u64, multisig: &str, tx: &SafeTx) -> Result<Review, CoreError> {
    let t = parse_tx(tx)?;
    let me = addr(multisig)?;
    let hash = safe_tx_hash(chain_id, multisig, tx)?;
    let refuse = |why: &str| Review {
        safe_tx_hash: hash.clone(),
        nonce: t.nonce.to_string(),
        action: None,
        refusal: Some(why.to_owned()),
    };
    if !(t.safe_tx_gas.is_zero() && t.base_gas.is_zero() && t.gas_price.is_zero())
        || t.gas_token != Address::ZERO
        || t.refund_receiver != Address::ZERO
    {
        return Ok(refuse("pays gas or a refund out of the multisig"));
    }
    let action = match t.operation {
        1 if t.to == addr(MULTI_SEND_CALL_ONLY)? && t.value.is_zero() => {
            match read_batch(&t.data, me) {
                Ok(calls) => Some(Action::Batch { calls }),
                Err(why) => return Ok(refuse(why)),
            }
        }
        1 => return Ok(refuse("delegatecall to a contract Vela does not sign for")),
        0 if t.to == me => {
            if t.value.is_zero() && t.data.is_empty() {
                Some(Action::Cancel)
            } else if t.value.is_zero() {
                read_owner_management(&t.data)
            } else {
                None
            }
        }
        0 if t.to == addr(GENESIS_REGISTRY)? => match read_registry_call(t.to, t.value, &t.data) {
            Ok(action) => Some(action),
            Err(why) => return Ok(refuse(why)),
        },
        0 => read_transfer(t.to, t.value, &t.data).or_else(|| {
            Some(Action::Call {
                to: t.to.to_checksum(None),
                value: t.value.to_string(),
                data: hex(&t.data),
            })
        }),
        _ => return Ok(refuse("unknown operation")),
    };
    Ok(match action {
        Some(action) => Review {
            safe_tx_hash: hash,
            nonce: t.nonce.to_string(),
            action: Some(action),
            refusal: None,
        },
        None => refuse(
            "not a transaction Vela signs (only transfers, calls, member changes, renames and cancellations)",
        ),
    })
}

/// What an owner's passkey signs to ask the genesis service to record a
/// multisig deployed nowhere yet (its `ownerProof`):
/// `keccak256("safe-genesis-registry/sponsor/v1" ‖ multisig)`.
pub fn sponsor_challenge(multisig: &str) -> Result<String, CoreError> {
    let mut p = b"safe-genesis-registry/sponsor/v1".to_vec();
    p.extend_from_slice(addr(multisig)?.as_slice());
    Ok(hex(keccak256(p).as_slice()))
}

// -- where a multisig can live, and where it does -------------------------------------

/// The Safe v1.4.1 contracts a multisig Vela creates names in its genesis
/// ([`multisig_genesis`]): the factory that deploys it, the singleton its
/// proxy runs, and the fallback handler its setup installs. A chain with
/// code at all three can deploy it at its one address; the Safe Genesis
/// Registry plays no part — it is only the index where the genesis is
/// recorded (Gnosis, or Ethereum), not a bound on where the multisig lives.
pub const MULTISIG_CONTRACTS: [(&str, &str); 3] = [
    ("Safe Proxy Factory", safe::SAFE_PROXY_FACTORY),
    ("Safe L2", safe::SAFE_SINGLETON),
    (
        "Compatibility Fallback Handler",
        COMPATIBILITY_FALLBACK_HANDLER,
    ),
];

/// Whether a chain holds the Safe v1.4.1 contracts a new multisig needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SafeContracts {
    /// Every one holds Safe's own code there.
    Present,
    /// These have no code there, or code that is not Safe's: it cannot be
    /// deployed on this chain.
    Missing { names: Vec<String> },
    /// The chain did not answer for some, and none of those it answered for
    /// is missing: nothing can be said either way.
    Unknown,
}

/// The verdict on one chain from `eth_getCode` of each of
/// [`MULTISIG_CONTRACTS`], in that order: `Some(code)` as the chain answered,
/// `None` where it did not. A missing contract is a verdict even while
/// another went unanswered; an empty code (`0x`) is a missing one, and so is
/// code whose hash is not the canonical contract's.
pub fn safe_contracts(codes: &[Option<String>]) -> SafeContracts {
    let mut missing = Vec::new();
    let mut unanswered = codes.len() < MULTISIG_CONTRACTS.len();
    for (i, (name, address)) in MULTISIG_CONTRACTS.iter().enumerate() {
        let canonical = address.parse::<Address>().ok().and_then(canonical_at);
        match (canonical, codes.get(i).and_then(|c| c.as_deref())) {
            (_, None) => unanswered = true,
            (Some(c), Some(code)) => match code_is(Some(code), c) {
                CodeIs::Canonical | CodeIs::Unread => {}
                CodeIs::Missing | CodeIs::Wrong => missing.push((*name).to_owned()),
            },
            (None, Some(_)) => missing.push((*name).to_owned()),
        }
    }
    if !missing.is_empty() {
        SafeContracts::Missing { names: missing }
    } else if unanswered {
        SafeContracts::Unknown
    } else {
        SafeContracts::Present
    }
}

/// What says which networks a multisig is used on. A multisig is the same on
/// every chain that has the Safe v1.4.1 contracts; it is "on" a network only
/// because something there or here says so.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NetworkEvidence {
    /// Chains it has code on, read through the wallet's own RPC.
    pub deployed: Vec<u64>,
    /// Chains the proposal mailbox holds proposals of it for.
    pub proposals: Vec<u64>,
    /// The network a proposal link names, or the one the person picked when
    /// asked.
    pub named: Option<u64>,
    /// What this device already keeps for it: the network its creator
    /// picked, and every one it was used on here since.
    pub kept: Vec<u64>,
    /// The networks this device has (built in or added). Given, a network
    /// it lacks is never one of the multisig's here — nothing can be read
    /// there — and is listed by [`missing_networks`] instead.
    pub have: Option<Vec<u64>>,
}

/// The networks a multisig is used on: where it is deployed, then where the
/// mailbox holds proposals of it, then the network named for it (a link, or
/// the person's pick), then what this device kept — each once, in that
/// order. Never the chain its genesis is recorded on: the registry is an
/// index, not a home. Empty: nothing says, and the person is asked — no
/// network is guessed.
pub fn multisig_networks(e: &NetworkEvidence) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    for id in e
        .deployed
        .iter()
        .chain(&e.proposals)
        .chain(e.named.iter())
        .chain(&e.kept)
    {
        let here = e.have.as_ref().is_none_or(|have| have.contains(id));
        if *id > 0 && here && !out.contains(id) {
            out.push(*id);
        }
    }
    out
}

/// The networks something says the multisig is on — the mailbox's
/// proposals, a link — that this device does not have (`have`): each once,
/// in that order. Nothing there is read or judged; the person is offered to
/// add the network. Empty when `have` is not given.
pub fn missing_networks(e: &NetworkEvidence) -> Vec<u64> {
    let Some(have) = &e.have else {
        return Vec::new();
    };
    let mut out: Vec<u64> = Vec::new();
    for id in e.proposals.iter().chain(e.named.iter()) {
        if *id > 0 && !have.contains(id) && !out.contains(id) {
            out.push(*id);
        }
    }
    out
}

/// How a network's members now differ from the ones a multisig was created
/// with (its genesis): who was added, who removed, and the threshold before
/// and now. Owners compare by address, whatever their case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberDrift {
    /// Nothing differs.
    pub same: bool,
    /// Members now who were not among the original ones, in today's order.
    pub added: Vec<String>,
    /// Original members who are no longer members, in the original order.
    pub removed: Vec<String>,
    pub threshold_was: u64,
    pub threshold_is: u64,
}

pub fn member_drift(
    original: &[String],
    original_threshold: u64,
    current: &[String],
    current_threshold: u64,
) -> Result<MemberDrift, CoreError> {
    let parse = |list: &[String]| list.iter().map(|o| addr(o)).collect::<Result<Vec<_>, _>>();
    let (was, is) = (parse(original)?, parse(current)?);
    let shown = |a: &Address| a.to_checksum(None);
    let added: Vec<String> = is.iter().filter(|a| !was.contains(a)).map(shown).collect();
    let removed: Vec<String> = was.iter().filter(|a| !is.contains(a)).map(shown).collect();
    Ok(MemberDrift {
        same: added.is_empty() && removed.is_empty() && original_threshold == current_threshold,
        added,
        removed,
        threshold_was: original_threshold,
        threshold_is: current_threshold,
    })
}

// -- who can act for a Safe: its chain, or the counterfactual rule ---------------------
//
// Neither the registry nor the mailbox decides who can act for a Safe
// (docs/multisig-vault-design.md §3.5). Where it is deployed, the chain does:
// its owners and threshold, and anything else that can act — a module, a
// guard, an unknown fallback handler or singleton ([`deployed_check`]).
// Where it is not deployed yet, only this rule does ([`counterfactual_authority`]):
// a genesis is the Safe it deploys only if everything its `setup` runs is
// known and read in full here, and the contracts it names hold Safe's own code
// on that chain.

/// A Safe release this wallet reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Release {
    V130,
    V141,
    V150,
}

/// What a Safe contract is in a creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Factory,
    Singleton,
    SingletonL2,
    /// `CompatibilityFallbackHandler`.
    Handler,
    /// `SafeToL2Setup`: `setupToL2(l2Singleton)` switches a new Safe to the
    /// L2 singleton on any chain but Ethereum, nothing else.
    ToL2Setup,
}

/// A canonical Safe deployment and the runtime code it must hold.
struct Canonical {
    name: &'static str,
    address: &'static str,
    release: Release,
    role: Role,
    /// keccak256 of its runtime code, as `eth_getCode` answers it.
    code_hash: &'static str,
}

/// The Safe contracts a multisig this wallet uses may depend on — the
/// factories and singletons the import path accepts (v1.3.0 canonical and
/// eip155, v1.4.1, v1.5.0; L1 and L2), each release's
/// CompatibilityFallbackHandler and SafeToL2Setup — with their runtime code
/// hashes. Source: safe-deployments @ b45417dd (the genesis service's
/// `genesis-rules/data/known-contracts.json`); every hash recomputed on
/// 2026-10-29 as keccak256(`eth_getCode`) on Gnosis (100) and Arc Testnet
/// (5042002), identical on both. The v1.4.1 codes are kept as test vectors
/// (`tests/vectors/safe-code/`), and the onchain suite recomputes every hash
/// on the fork.
const CANONICAL: [Canonical; 18] = [
    Canonical {
        name: "Safe Proxy Factory 1.3.0",
        address: "0xa6B71E26C5e0845f74c812102Ca7114b6a896AB2",
        release: Release::V130,
        role: Role::Factory,
        code_hash: "0x337d7f54be11b6ed55fef7b667ea5488db53db8320a05d1146aa4bd169a39a9b",
    },
    Canonical {
        name: "Safe Proxy Factory 1.3.0",
        address: "0xC22834581EbC8527d974F8a1c97E1bEA4EF910BC",
        release: Release::V130,
        role: Role::Factory,
        code_hash: "0x337d7f54be11b6ed55fef7b667ea5488db53db8320a05d1146aa4bd169a39a9b",
    },
    Canonical {
        name: "Safe Proxy Factory 1.4.1",
        address: safe::SAFE_PROXY_FACTORY,
        release: Release::V141,
        role: Role::Factory,
        code_hash: "0x50c3cdc4074750a7a974204a716c999edd37482f907608d960b2b025ee0b3317",
    },
    Canonical {
        name: "Safe Proxy Factory 1.5.0",
        address: "0x14F2982D601c9458F93bd70B218933A6f8165e7b",
        release: Release::V150,
        role: Role::Factory,
        code_hash: "0x967dae4cda22b0c9ef7f31b010bdc1ceb0af9904b0c3dc060b5302e4c18a4529",
    },
    Canonical {
        name: "Safe 1.3.0",
        address: "0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552",
        release: Release::V130,
        role: Role::Singleton,
        code_hash: "0xbba688fbdb21ad2bb58bc320638b43d94e7d100f6f3ebaab0a4e4de6304b1c2e",
    },
    Canonical {
        name: "Safe 1.3.0",
        address: "0x69f4D1788e39c87893C980c06EdF4b7f686e2938",
        release: Release::V130,
        role: Role::Singleton,
        code_hash: "0xbba688fbdb21ad2bb58bc320638b43d94e7d100f6f3ebaab0a4e4de6304b1c2e",
    },
    Canonical {
        name: "Safe L2 1.3.0",
        address: "0x3E5c63644E683549055b9Be8653de26E0B4CD36E",
        release: Release::V130,
        role: Role::SingletonL2,
        code_hash: "0x21842597390c4c6e3c1239e434a682b054bd9548eee5e9b1d6a4482731023c0f",
    },
    Canonical {
        name: "Safe L2 1.3.0",
        address: "0xfb1bffC9d739B8D520DaF37dF666da4C687191EA",
        release: Release::V130,
        role: Role::SingletonL2,
        code_hash: "0x21842597390c4c6e3c1239e434a682b054bd9548eee5e9b1d6a4482731023c0f",
    },
    Canonical {
        name: "Safe 1.4.1",
        address: "0x41675C099F32341bf84BFc5382aF534df5C7461a",
        release: Release::V141,
        role: Role::Singleton,
        code_hash: "0x1fe2df852ba3299d6534ef416eefa406e56ced995bca886ab7a553e6d0c5e1c4",
    },
    Canonical {
        name: "Safe L2 1.4.1",
        address: safe::SAFE_SINGLETON,
        release: Release::V141,
        role: Role::SingletonL2,
        code_hash: "0xb1f926978a0f44a2c0ec8fe822418ae969bd8c3f18d61e5103100339894f81ff",
    },
    Canonical {
        name: "Safe 1.5.0",
        address: "0xFf51A5898e281Db6DfC7855790607438dF2ca44b",
        release: Release::V150,
        role: Role::Singleton,
        code_hash: "0xdda019cbd7c867a533a2a86e5c53434fdc50b13122b5a5ddb4a8df61b31c20f2",
    },
    Canonical {
        name: "Safe L2 1.5.0",
        address: "0xEdd160fEBBD92E350D4D398fb636302fccd67C7e",
        release: Release::V150,
        role: Role::SingletonL2,
        code_hash: "0x180193227186ccb85316c94db1f0d156ed932b14712cfaac78901899178572dc",
    },
    Canonical {
        name: "Compatibility Fallback Handler 1.3.0",
        address: "0xf48f2B2d2a534e402487b3ee7C18c33Aec0Fe5e4",
        release: Release::V130,
        role: Role::Handler,
        code_hash: "0x03e69f7ce809e81687c69b19a7d7cca45b6d551ffdec73d9bb87178476de1abf",
    },
    Canonical {
        name: "Compatibility Fallback Handler 1.3.0",
        address: "0x017062a1dE2FE6b99BE3d9d37841FeD19F573804",
        release: Release::V130,
        role: Role::Handler,
        code_hash: "0x03e69f7ce809e81687c69b19a7d7cca45b6d551ffdec73d9bb87178476de1abf",
    },
    Canonical {
        name: "Compatibility Fallback Handler 1.4.1",
        address: COMPATIBILITY_FALLBACK_HANDLER,
        release: Release::V141,
        role: Role::Handler,
        code_hash: "0x7c6007a5d711cea8dfd5d91f5940ec29c7f200fe511eb1fc1397b367af3c42f9",
    },
    Canonical {
        name: "Compatibility Fallback Handler 1.5.0",
        address: "0x3EfCBb83A4A7AfcB4F68D501E2c2203a38be77f4",
        release: Release::V150,
        role: Role::Handler,
        code_hash: "0x3c6a85bcf7b563daa624b884b4e9a1b9fa5371edde7be945d998071a48f28bbc",
    },
    Canonical {
        name: "Safe To L2 Setup 1.4.1",
        address: "0xBD89A1CE4DDe368FFAB0eC35506eEcE0b1fFdc54",
        release: Release::V141,
        role: Role::ToL2Setup,
        code_hash: "0x2f25df28caf984366ee584e13241707e85dcd5a6ea0c14267928dafc1fd6274b",
    },
    Canonical {
        name: "Safe To L2 Setup 1.5.0",
        address: "0x900C7589200010D6C6eCaaE5B06EBe653bc2D82a",
        release: Release::V150,
        role: Role::ToL2Setup,
        code_hash: "0xf6034d841bcbff8912aa55526b0f1609212536aaf60bb16f5e8a269a4ab38f18",
    },
];

/// Safe's `SafeModuleSetup` (safe-modules v0.2.0, v0.3.0): `enableModules`
/// in a setup is read as what it is — modules enabled from the start.
const MODULE_SETUPS: [&str; 2] = [
    "0x8EcD4ec46D4D2a6B64fE960B3D64e8B94B2234eb",
    safe::SAFE_MODULE_SETUP,
];

/// Safe's `GuardManager` slot: `keccak256("guard_manager.guard.address")`.
pub const GUARD_STORAGE_SLOT: &str =
    "0x4a204f620c8c5ccdca3fd54d003badd85ba500436a431f0cbda4f558c93c34c8";
/// Safe v1.5.0's module guard: `keccak256("module_manager.module_guard.address")`.
pub const MODULE_GUARD_STORAGE_SLOT: &str =
    "0xb104e0b93118902c651344349b610029d694cfdec91c589c91ebafbcd0289947";
/// Safe's `FallbackManager` slot: `keccak256("fallback_manager.handler.address")`.
pub const FALLBACK_HANDLER_STORAGE_SLOT: &str =
    "0x6c9a6c4a39284e37ed1cf53d337577d14212a4870fb976a4366c693b939918d5";
/// The proxy's singleton: storage slot 0.
pub const SINGLETON_STORAGE_SLOT: &str = "0x0";
/// How many modules one `getModulesPaginated` read asks for. More than this
/// shows as the page's `next`, itself a module.
const MODULES_PAGE: usize = 10;
/// The most owners a setup read here may list (Safe sets no bound).
const MAX_SETUP_OWNERS: usize = 64;

const SETUP_SIGNATURE: &str =
    "setup(address[],uint256,address,bytes,address,address,uint256,address)";

fn canonical_at(a: Address) -> Option<&'static Canonical> {
    CANONICAL
        .iter()
        .find(|c| c.address.parse::<Address>().ok() == Some(a))
}

fn canonical_as(a: Address, roles: &[Role]) -> Option<&'static Canonical> {
    canonical_at(a).filter(|c| roles.contains(&c.role))
}

/// A Safe `setup` call, read in full: every field, and nothing besides —
/// the bytes must be exactly Safe's own ABI encoding of what was read, so
/// nothing can hide in an offset, a padding or a tail.
fn read_setup(init: &[u8]) -> Option<SetupArgs> {
    if init.get(..4)? != selector(SETUP_SIGNATURE) {
        return None;
    }
    let body = &init[4..];
    let word = |at: usize| body.get(at..at.checked_add(32)?).map(U256::from_be_slice);
    let small = |at: usize| -> Option<usize> { word(at)?.try_into().ok() };
    let address = |at: usize| {
        let w = word(at)?;
        (w >> 160 == U256::ZERO).then(|| Address::from_word(B256::from(w)))
    };
    let owners_at = small(0)?;
    let count = small(owners_at)?;
    if count > MAX_SETUP_OWNERS {
        return None;
    }
    let owners = (0..count)
        .map(|i| address(owners_at + 32 + i * 32))
        .collect::<Option<Vec<_>>>()?;
    let data_at = small(96)?;
    let len = small(data_at)?;
    let from = data_at.checked_add(32)?;
    let data = body.get(from..from.checked_add(len)?)?.to_vec();
    let a = SetupArgs {
        owners,
        threshold: word(32)?,
        to: address(64)?,
        data,
        fallback_handler: address(128)?,
        payment_token: address(160)?,
        payment: word(192)?,
        payment_receiver: address(224)?,
    };
    (encode_setup_full(&a) == init).then_some(a)
}

/// The modules in an `enableModules(address[])` call, if `data` is exactly one.
fn read_enable_modules(data: &[u8]) -> Option<Vec<Address>> {
    if data.get(..4)? != selector("enableModules(address[])") {
        return None;
    }
    let body = &data[4..];
    let word = |at: usize| body.get(at..at.checked_add(32)?).map(U256::from_be_slice);
    let at: usize = word(0)?.try_into().ok()?;
    let count: usize = word(at)?.try_into().ok()?;
    if count > MAX_SETUP_OWNERS {
        return None;
    }
    (0..count)
        .map(|i| {
            let w = word(at + 32 + i * 32)?;
            (w >> 160 == U256::ZERO).then(|| Address::from_word(B256::from(w)))
        })
        .collect()
}

/// The L2 singleton in a `setupToL2(address)` call, if `data` is exactly one.
fn read_setup_to_l2(data: &[u8]) -> Option<Address> {
    if data.len() != 36 || data[..4] != selector("setupToL2(address)") {
        return None;
    }
    let w = U256::from_be_slice(&data[4..36]);
    (w >> 160 == U256::ZERO).then(|| Address::from_word(B256::from(w)))
}

/// Why a genesis is not the Safe it claims, or not yet one Vela can stand
/// behind on a chain. Each says what it found, for the person to read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Refusal {
    /// It does not make the address it was offered for.
    WrongAddress,
    /// Its initializer is not a Safe `setup` call in Safe's own encoding.
    NotSetup,
    /// Its members or rule are ones Safe's `setup` refuses: it can never deploy.
    BadMembers,
    /// Deployed through a contract that is not a Safe proxy factory Vela knows.
    UnknownFactory { address: String },
    /// Runs a singleton that is not a Safe release Vela supports.
    UnknownSingleton { address: String },
    /// Its setup enables a module: it can move funds without the members.
    Module { address: String },
    /// Its setup runs code at `address` (a delegatecall) that Vela cannot
    /// read in full: anything may be done to the Safe there.
    SetupCode { address: String },
    /// Its fallback handler is not Safe's CompatibilityFallbackHandler for
    /// its release.
    Handler { address: String },
    /// Its setup pays `amount` of `token` (zero: the chain's coin) out of the
    /// Safe to `receiver` as it is deployed.
    Payment {
        token: String,
        amount: String,
        receiver: String,
    },
    /// On this chain, a contract it depends on has no code.
    MissingCode { name: String, address: String },
    /// On this chain, a contract it depends on holds code that is not Safe's.
    WrongCode { name: String, address: String },
}

/// Who can act for a Safe that is not deployed on a chain yet — the one
/// answer to that question the wallet uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Authority {
    /// Deployed there, it is exactly these owners and this threshold, and
    /// nothing else can act for it.
    Verified { owners: Vec<String>, threshold: u64 },
    /// Vela neither deploys it there nor signs for it, for these reasons.
    Refused { reasons: Vec<Refusal> },
    /// Nothing wrong found, but the chain did not answer for these contracts:
    /// it cannot be verified yet (and is not Verified).
    Unknown { unread: Vec<String> },
}

/// What the wallet read on the target chain for a counterfactual verdict:
/// `eth_getCode` of each address [`counterfactual_reads`] names — the code,
/// or `null` where the chain did not answer. Addresses in any case.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChainFacts {
    pub codes: BTreeMap<String, Option<String>>,
}

impl ChainFacts {
    /// `Some(code)` as the chain answered for `a`, `None` if it did not.
    fn code(&self, a: Address) -> Option<&str> {
        self.codes
            .iter()
            .find(|(k, _)| k.parse::<Address>().ok() == Some(a))
            .and_then(|(_, v)| v.as_deref())
    }
}

/// What a code answer is, against the code a canonical contract must hold.
enum CodeIs {
    Unread,
    Missing,
    Wrong,
    Canonical,
}

fn code_is(code: Option<&str>, c: &Canonical) -> CodeIs {
    let Some(code) = code else {
        return CodeIs::Unread;
    };
    let body = code.trim().strip_prefix("0x").unwrap_or(code.trim());
    if body.trim_start_matches('0').is_empty() {
        return CodeIs::Missing;
    }
    match unhex(code) {
        Ok(bytes) if hex(keccak256(&bytes).as_slice()) == c.code_hash => CodeIs::Canonical,
        _ => CodeIs::Wrong,
    }
}

/// A genesis that passed every rule that needs no chain: its members, and
/// the canonical contracts whose code it needs on the chain it deploys on.
struct Vetted {
    owners: Vec<Address>,
    threshold: u64,
    depends: Vec<&'static Canonical>,
}

/// The rules a genesis must meet on every chain (§3.5, 1 and 3–5): it makes
/// `safe`; its factory and singleton are canonical Safe deployments; its
/// setup runs nothing, or exactly `SafeToL2Setup.setupToL2(<that release's
/// L2 singleton>)`; its fallback handler is none or that release's
/// CompatibilityFallbackHandler; it pays nothing; its members are ones Safe
/// accepts.
fn vet_genesis(g: &Genesis, safe: Address) -> Result<Vetted, Vec<Refusal>> {
    let parsed = |s: &str| s.parse::<Address>().unwrap_or(Address::ZERO);
    let factory_at = parsed(&g.factory);
    let singleton_at = parsed(&g.singleton);
    let factory = canonical_as(factory_at, &[Role::Factory]);
    let singleton = canonical_as(singleton_at, &[Role::Singleton, Role::SingletonL2]);
    let mut reasons = Vec::new();
    if factory.is_none() {
        reasons.push(Refusal::UnknownFactory {
            address: factory_at.to_checksum(None),
        });
    }
    if singleton.is_none() {
        reasons.push(Refusal::UnknownSingleton {
            address: singleton_at.to_checksum(None),
        });
    }
    let (Some(factory), Some(singleton)) = (factory, singleton) else {
        return Err(reasons);
    };
    match genesis_address(g) {
        Ok(a) if a == safe.to_checksum(None) => {}
        _ => return Err(vec![Refusal::WrongAddress]),
    }
    let Some(setup) = unhex(&g.initializer).ok().and_then(|i| read_setup(&i)) else {
        return Err(vec![Refusal::NotSetup]);
    };
    let mut depends = vec![factory, singleton];
    let threshold = u64::try_from(setup.threshold).unwrap_or(u64::MAX);
    let members_ok = !setup.owners.is_empty()
        && threshold >= 1
        && threshold as usize <= setup.owners.len()
        && setup.owners.iter().enumerate().all(|(i, o)| {
            *o != Address::ZERO && *o != SENTINEL && *o != safe && !setup.owners[..i].contains(o)
        });
    if !members_ok {
        reasons.push(Refusal::BadMembers);
    }
    // The setup hook: nothing, or the one Safe{Wallet} adds (SafeToL2Setup).
    if setup.to == Address::ZERO {
        if !setup.data.is_empty() {
            reasons.push(Refusal::NotSetup);
        }
    } else if let Some(hook) = canonical_as(setup.to, &[Role::ToL2Setup]) {
        let l2 = read_setup_to_l2(&setup.data)
            .and_then(|a| canonical_as(a, &[Role::SingletonL2]))
            .filter(|l2| l2.release == hook.release && singleton.release == hook.release);
        match l2 {
            Some(l2) => {
                depends.push(hook);
                if l2.address != singleton.address {
                    depends.push(l2);
                }
            }
            None => reasons.push(Refusal::SetupCode {
                address: setup.to.to_checksum(None),
            }),
        }
    } else if let Some(modules) = MODULE_SETUPS
        .iter()
        .any(|m| m.parse::<Address>().ok() == Some(setup.to))
        .then(|| read_enable_modules(&setup.data))
        .flatten()
        .filter(|m| !m.is_empty())
    {
        reasons.extend(modules.into_iter().map(|m| Refusal::Module {
            address: m.to_checksum(None),
        }));
    } else {
        reasons.push(Refusal::SetupCode {
            address: setup.to.to_checksum(None),
        });
    }
    if setup.fallback_handler != Address::ZERO {
        match canonical_as(setup.fallback_handler, &[Role::Handler])
            .filter(|h| h.release == singleton.release)
        {
            Some(h) => depends.push(h),
            None => reasons.push(Refusal::Handler {
                address: setup.fallback_handler.to_checksum(None),
            }),
        }
    }
    if setup.payment_token != Address::ZERO
        || !setup.payment.is_zero()
        || setup.payment_receiver != Address::ZERO
    {
        reasons.push(Refusal::Payment {
            token: setup.payment_token.to_checksum(None),
            amount: setup.payment.to_string(),
            receiver: setup.payment_receiver.to_checksum(None),
        });
    }
    if !reasons.is_empty() {
        return Err(reasons);
    }
    Ok(Vetted {
        owners: setup.owners,
        threshold,
        depends,
    })
}

/// What a Safe not deployed on a chain holds the moment its genesis' setup
/// has run there — for a simulation that puts it in place with a state
/// override (`multisig_review`): the singleton it runs (the L2 one when the
/// allowed `setupToL2` hook switches it, on any chain but Ethereum), its
/// owners in list order, its threshold and its fallback handler. `None` for
/// a genesis the counterfactual rule refuses.
#[cfg(feature = "crux")]
pub(crate) struct StartState {
    pub singleton: Address,
    pub owners: Vec<Address>,
    pub threshold: u64,
    pub handler: Address,
}

#[cfg(feature = "crux")]
pub(crate) fn counterfactual_start(
    g: &Genesis,
    safe: Address,
    chain_id: u64,
) -> Option<StartState> {
    let vetted = vet_genesis(g, safe).ok()?;
    let setup = read_setup(&unhex(&g.initializer).ok()?)?;
    let mut singleton: Address = g.singleton.parse().ok()?;
    if chain_id != 1 && canonical_as(setup.to, &[Role::ToL2Setup]).is_some() {
        singleton = read_setup_to_l2(&setup.data)?;
    }
    Some(StartState {
        singleton,
        owners: vetted.owners,
        threshold: vetted.threshold,
        handler: setup.fallback_handler,
    })
}

/// `code` is the runtime code of the canonical Safe singleton at `singleton`.
#[cfg(feature = "crux")]
pub(crate) fn canonical_singleton_code(singleton: Address, code: &str) -> bool {
    canonical_as(singleton, &[Role::Singleton, Role::SingletonL2])
        .is_some_and(|c| matches!(code_is(Some(code), c), CodeIs::Canonical))
}

/// The contracts whose code [`counterfactual_authority`] needs read on the
/// target chain for `g` (`eth_getCode` of each): its factory, its
/// singleton, its fallback handler and its setup hook with the singleton
/// that hook switches to. Empty for a genesis refused on every chain — no
/// read can change that verdict.
pub fn counterfactual_reads(g: &Genesis, safe: &str) -> Result<Vec<String>, CoreError> {
    let Ok(vetted) = vet_genesis(g, addr(safe)?) else {
        return Ok(Vec::new());
    };
    vetted
        .depends
        .iter()
        .map(|c| addr(c.address).map(|a| a.to_checksum(None)))
        .collect()
}

/// Who can act for `safe`, not deployed on a chain yet, once deployed there
/// from `g` (§3.5): Verified — exactly the genesis's owners and threshold,
/// nothing else — only when every rule holds (it makes `safe`; canonical
/// factory, singleton and handler; no setup hook but SafeToL2Setup to its
/// own L2 singleton; no payment) AND every contract it depends on holds
/// Safe's own code on that chain (`facts`). A contract the chain did not
/// answer for leaves it Unknown: never Verified.
pub fn counterfactual_authority(
    g: &Genesis,
    safe: &str,
    facts: &ChainFacts,
) -> Result<Authority, CoreError> {
    let vetted = match vet_genesis(g, addr(safe)?) {
        Ok(v) => v,
        Err(reasons) => return Ok(Authority::Refused { reasons }),
    };
    let mut reasons = Vec::new();
    let mut unread = Vec::new();
    for c in &vetted.depends {
        let at = addr(c.address)?;
        let (name, address) = (c.name.to_owned(), at.to_checksum(None));
        match code_is(facts.code(at), c) {
            CodeIs::Canonical => {}
            CodeIs::Unread => unread.push(address),
            CodeIs::Missing => reasons.push(Refusal::MissingCode { name, address }),
            CodeIs::Wrong => reasons.push(Refusal::WrongCode { name, address }),
        }
    }
    Ok(if !reasons.is_empty() {
        Authority::Refused { reasons }
    } else if !unread.is_empty() {
        Authority::Unknown { unread }
    } else {
        Authority::Verified {
            owners: vetted.owners.iter().map(|o| o.to_checksum(None)).collect(),
            threshold: vetted.threshold,
        }
    })
}

/// What can act for a deployed Safe besides its owners, as read from its
/// chain: each one is shown, plainly, and makes the multisig high-risk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Risk {
    /// An enabled module: it can move funds without the members.
    Module { address: String },
    /// A transaction guard (or v1.5.0 module guard): it can block or change
    /// what the members do.
    Guard { address: String },
    /// A fallback handler that is not a Safe CompatibilityFallbackHandler.
    Handler { address: String },
    /// Its proxy runs code that is not a Safe release Vela knows.
    Singleton { address: String },
}

/// What the wallet read on a deployed Safe's chain for [`deployed_check`]:
/// the `getModulesPaginated(SENTINEL, 10)` answer, the guard, module guard,
/// fallback handler and singleton storage words — `null` where the chain did
/// not answer — and `eth_getCode` of the addresses a first check named in
/// `needCode`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeployedFacts {
    pub modules: Option<String>,
    pub guard: Option<String>,
    pub module_guard: Option<String>,
    pub handler: Option<String>,
    pub singleton: Option<String>,
    pub codes: BTreeMap<String, Option<String>>,
}

/// The core's reading of a deployed Safe beyond its owners.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployedCheck {
    /// What can act for it besides its members.
    pub risks: Vec<Risk>,
    /// Read `eth_getCode` of these and ask again with them in `codes`.
    pub need_code: Vec<String>,
    /// The chain did not answer for something: nothing can be said yet.
    pub unread: bool,
}

/// The reads [`deployed_check`] takes, for the shell to make: the
/// `getModulesPaginated` call data and the storage slots.
pub fn deployed_reads() -> serde_json::Value {
    let mut modules = selector("getModulesPaginated(address,uint256)").to_vec();
    modules.extend_from_slice(&word_addr(SENTINEL));
    modules.extend_from_slice(&word_usize(MODULES_PAGE));
    serde_json::json!({
        "modulesCall": hex(&modules),
        "guardSlot": GUARD_STORAGE_SLOT,
        "moduleGuardSlot": MODULE_GUARD_STORAGE_SLOT,
        "handlerSlot": FALLBACK_HANDLER_STORAGE_SLOT,
        "singletonSlot": SINGLETON_STORAGE_SLOT,
    })
}

/// The address in a storage word, if it is one.
fn word_address(word: &str) -> Option<Address> {
    let w = quantity(word).ok()?;
    (w >> 160 == U256::ZERO).then(|| Address::from_word(B256::from(w)))
}

/// The modules a `getModulesPaginated` answer lists, with the page's `next`
/// when it is another module (more than one page).
fn read_modules(ret: &str) -> Option<Vec<Address>> {
    let ret = unhex(ret).ok()?;
    let word = |at: usize| ret.get(at..at.checked_add(32)?).map(U256::from_be_slice);
    let address = |at: usize| {
        let w = word(at)?;
        (w >> 160 == U256::ZERO).then(|| Address::from_word(B256::from(w)))
    };
    let at: usize = word(0)?.try_into().ok()?;
    let next = address(32)?;
    let count: usize = word(at)?.try_into().ok()?;
    if count > MODULES_PAGE {
        return None;
    }
    let mut out = (0..count)
        .map(|i| address(at + 32 + i * 32))
        .collect::<Option<Vec<_>>>()?;
    if next != SENTINEL && next != Address::ZERO && !out.contains(&next) {
        out.push(next);
    }
    Some(out)
}

/// What can act for a deployed Safe besides its owners (§3.5): its modules,
/// its guards, a fallback handler that is not a canonical
/// CompatibilityFallbackHandler, a singleton that is not a canonical Safe
/// release — each with Safe's own code on this chain. Called once with the
/// storage reads, then again with the code of what it names in `needCode`.
pub fn deployed_check(f: &DeployedFacts) -> DeployedCheck {
    let mut out = DeployedCheck::default();
    let codes = ChainFacts {
        codes: f.codes.clone(),
    };
    match f.modules.as_deref().map(read_modules) {
        None | Some(None) => out.unread = true,
        Some(Some(modules)) => out.risks.extend(modules.into_iter().map(|m| Risk::Module {
            address: m.to_checksum(None),
        })),
    }
    for guard in [&f.guard, &f.module_guard] {
        match guard.as_deref().map(word_address) {
            None | Some(None) => out.unread = true,
            Some(Some(a)) if a == Address::ZERO => {}
            Some(Some(a)) => out.risks.push(Risk::Guard {
                address: a.to_checksum(None),
            }),
        }
    }
    let mut known = |word: &Option<String>, roles: &[Role], risk: fn(String) -> Risk, zero_ok| {
        let Some(a) = word.as_deref().map(word_address) else {
            out.unread = true;
            return;
        };
        let Some(a) = a else {
            out.unread = true;
            return;
        };
        if a == Address::ZERO && zero_ok {
            return;
        }
        let Some(c) = canonical_as(a, roles) else {
            out.risks.push(risk(a.to_checksum(None)));
            return;
        };
        let asked = f.codes.keys().any(|k| k.parse::<Address>().ok() == Some(a));
        match code_is(codes.code(a), c) {
            CodeIs::Canonical => {}
            CodeIs::Unread if !asked => out.need_code.push(a.to_checksum(None)),
            CodeIs::Unread => out.unread = true,
            CodeIs::Missing | CodeIs::Wrong => out.risks.push(risk(a.to_checksum(None))),
        }
    };
    known(
        &f.handler,
        &[Role::Handler],
        |address| Risk::Handler { address },
        true,
    );
    known(
        &f.singleton,
        &[Role::Singleton, Role::SingletonL2],
        |address| Risk::Singleton { address },
        false,
    );
    out
}

/// The verdict the executor needs before it deploys `plan`'s multisig.
fn deploy_verified(plan: &ExecutionPlan, g: &Genesis) -> Result<(), CoreError> {
    let facts = plan
        .deploy_facts
        .as_ref()
        .ok_or_else(|| err("deploying a multisig needs its chain's code read first"))?;
    match counterfactual_authority(g, &plan.multisig, facts)? {
        Authority::Verified { .. } => Ok(()),
        Authority::Unknown { .. } => Err(err(
            "the chain did not answer for the Safe contracts this multisig needs: it is not deployed",
        )),
        Authority::Refused { reasons } => Err(err(format!(
            "Vela does not deploy this multisig: {}",
            serde_json::to_string(&reasons).unwrap_or_default()
        ))),
    }
}

/// The one door the shells use: `method` and its arguments as a JSON object,
/// the answer as JSON. Every rule stays here; the shell only moves JSON.
pub fn call_json(method: &str, args_json: &str) -> Result<String, CoreError> {
    let args: serde_json::Value =
        serde_json::from_str(args_json).map_err(|e| err(format!("arguments: {e}")))?;
    fn arg<T: serde::de::DeserializeOwned>(
        args: &serde_json::Value,
        name: &str,
    ) -> Result<T, CoreError> {
        serde_json::from_value(args.get(name).cloned().unwrap_or(serde_json::Value::Null))
            .map_err(|e| err(format!("argument {name}: {e}")))
    }
    let out = |v: serde_json::Value| Ok(v.to_string());
    let json = |v: &dyn erased::Ser| v.to_value();
    match method {
        "genesis" => out(json(&multisig_genesis(
            &arg::<Vec<String>>(&args, "owners")?,
            arg(&args, "threshold")?,
            &arg::<String>(&args, "name")?,
            &arg::<String>(&args, "saltNonce")?,
        )?)),
        "validName" => out(serde_json::json!(valid_name(&arg::<String>(
            &args, "name"
        )?))),
        "validOwner" => out(serde_json::json!(valid_owner(&arg::<String>(
            &args, "address"
        )?))),
        "genesisAddress" => out(serde_json::json!(genesis_address(&arg(&args, "genesis")?)?)),
        "genesisMembers" => {
            let (owners, threshold) = genesis_members(&arg(&args, "genesis")?)?;
            out(serde_json::json!({"owners": owners, "threshold": threshold}))
        }
        "velaAccountGenesis" => {
            let (address, genesis) = vela_account_genesis(
                &arg::<Vec<KeyArg>>(&args, "keys")?
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>(),
            )?;
            out(serde_json::json!({"address": address, "genesis": genesis}))
        }
        "genesisFromCalldata" => out(json(&genesis_from_calldata(
            &arg::<String>(&args, "input")?,
            &arg::<String>(&args, "safe")?,
        )?)),
        "recordCall" => out(json(&record_call(&arg(&args, "genesis")?)?)),
        "recordableName" => out(serde_json::json!(recordable_name(&arg::<String>(
            &args, "name"
        )?))),
        "defaultName" => out(serde_json::json!(default_name(&arg::<String>(
            &args, "safe"
        )?)?)),
        "namedGenesis" => out(json(&named_genesis(&arg(&args, "genesis")?)?)),
        "renameTx" => out(json(&rename_tx(
            &arg::<String>(&args, "name")?,
            &arg::<String>(&args, "nonce")?,
        )?)),
        "genesisOfCall" => out(serde_json::json!(genesis_of_call(&arg::<String>(
            &args, "safe"
        )?)?)),
        "readGenesisOf" => out(json(&read_genesis_of(
            &arg::<String>(&args, "safe")?,
            &arg::<String>(&args, "ret")?,
        )?)),
        "deployCall" => out(json(&deploy_call(&arg(&args, "genesis")?)?)),
        "safeTxHash" => out(serde_json::json!(safe_tx_hash(
            arg(&args, "chainId")?,
            &arg::<String>(&args, "safe")?,
            &arg(&args, "tx")?
        )?)),
        "approvalChallenge" => out(serde_json::json!(approval_challenge(
            arg(&args, "chainId")?,
            &arg::<String>(&args, "owner")?,
            &arg::<String>(&args, "safeTxHash")?
        )?)),
        "sponsorChallenge" => out(serde_json::json!(sponsor_challenge(&arg::<String>(
            &args, "multisig"
        )?)?)),
        // §16: an owner proof's params, read as the trusted page reads them.
        "ownerProofRead" => out(crate::trusted_signer::owner_proof::read_json(
            &args.get("params").cloned().unwrap_or(serde_json::Value::Null),
        )),
        "ownerProofParams" => out(crate::trusted_signer::owner_proof::params(
            &arg::<String>(&args, "multisig")?,
            &arg::<String>(&args, "member")?,
            &arg(&args, "genesis")?,
            &arg::<Vec<u64>>(&args, "chainIds")?,
        )),
        "velaMultisigOf" => out(json(&vela_multisig_of(&arg(&args, "genesis")?)?)),
        "approvalFromAssertion" => out(json(&approval_from_assertion(
            &arg::<Vec<KeyArg>>(&args, "keys")?
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>(),
            &arg::<String>(&args, "credentialId")?,
            &arg::<String>(&args, "authenticatorData")?,
            &arg::<String>(&args, "clientDataJSON")?,
            &arg::<String>(&args, "signature")?,
        )?)),
        "verifyApproval" => out(serde_json::json!(verify_approval(
            arg(&args, "chainId")?,
            &arg::<String>(&args, "safeTxHash")?,
            &arg(&args, "approval")?
        )
        .is_ok())),
        "verifyEcdsaApproval" => out(json(&verify_ecdsa_approval(
            &arg::<String>(&args, "safeTxHash")?,
            &arg(&args, "approval")?,
        )?)),
        "ecdsaForm" => out(json(&ecdsa_form(&arg::<String>(&args, "signature")?)?)),
        "safeTxHashes" => out(json(&safe_tx_hashes(
            arg(&args, "chainId")?,
            &arg::<String>(&args, "safe")?,
            &arg(&args, "tx")?,
        )?)),
        "safeTxTypedData" => out(safe_tx_typed_data(
            arg(&args, "chainId")?,
            &arg::<String>(&args, "safe")?,
            &arg(&args, "tx")?,
        )?),
        "signatures" => out(serde_json::json!(assemble_signatures(
            &arg::<Vec<Approval>>(&args, "approvals")?,
            arg::<Option<String>>(&args, "executor")?.as_deref(),
        )?)),
        "execTransactionData" => out(serde_json::json!(exec_transaction_data(
            &arg(&args, "tx")?,
            &arg::<String>(&args, "signatures")?,
        )?)),
        "parseSafeExport" => out(json(&parse_safe_export(&arg::<String>(&args, "json")?)?)),
        "safeWalletPropsGenesis" => out(json(&safe_wallet_props_genesis(
            &arg::<serde_json::Value>(&args, "props")?,
            &arg::<String>(&args, "address")?,
        )?)),
        "executionCalls" => out(json(&execution_calls(&arg(&args, "plan")?)?)),
        "review" => out(json(&review(
            arg(&args, "chainId")?,
            &arg::<String>(&args, "multisig")?,
            &arg(&args, "tx")?,
        )?)),
        "transfersTx" => out(json(&transfers_tx(
            &arg::<Vec<Transfer>>(&args, "transfers")?,
            &arg::<String>(&args, "nonce")?,
        )?)),
        "callsTx" => out(json(&calls_tx(
            &arg::<Vec<CallArg>>(&args, "calls")?,
            &arg::<String>(&args, "nonce")?,
        )?)),
        "cancelTx" => out(json(&cancel_tx(
            &arg::<String>(&args, "multisig")?,
            &arg::<String>(&args, "nonce")?,
        )?)),
        "multisigContracts" => out(serde_json::json!(MULTISIG_CONTRACTS
            .iter()
            .map(|(name, address)| serde_json::json!({"name": name, "address": address}))
            .collect::<Vec<_>>())),
        "safeContracts" => out(json(&safe_contracts(&arg::<Vec<Option<String>>>(
            &args, "codes",
        )?))),
        "multisigNetworks" => out(serde_json::json!(multisig_networks(&arg(
            &args, "evidence"
        )?))),
        "missingNetworks" => out(serde_json::json!(missing_networks(&arg(
            &args, "evidence"
        )?))),
        "memberChangeTx" => out(json(&member_change_tx(
            &arg::<String>(&args, "multisig")?,
            &arg(&args, "state")?,
            &arg(&args, "change")?,
            &arg::<String>(&args, "nonce")?,
        )?)),
        "memberDrift" => out(json(&member_drift(
            &arg::<Vec<String>>(&args, "original")?,
            arg(&args, "originalThreshold")?,
            &arg::<Vec<String>>(&args, "current")?,
            arg(&args, "currentThreshold")?,
        )?)),
        "counterfactualReads" => out(serde_json::json!(counterfactual_reads(
            &arg(&args, "genesis")?,
            &arg::<String>(&args, "safe")?,
        )?)),
        "counterfactualAuthority" => out(json(&counterfactual_authority(
            &arg(&args, "genesis")?,
            &arg::<String>(&args, "safe")?,
            &arg::<Option<ChainFacts>>(&args, "facts")?.unwrap_or_default(),
        )?)),
        "deployedReads" => out(deployed_reads()),
        // The pinned table, for a suite that recomputes it from a chain's code.
        "canonicalContracts" => out(serde_json::json!(CANONICAL
            .iter()
            .map(|c| serde_json::json!({"name": c.name, "address": c.address, "codeHash": c.code_hash}))
            .collect::<Vec<_>>())),
        "deployedCheck" => out(json(&deployed_check(&arg(&args, "facts")?))),
        // What a transaction changes about the members, and what an executed
        // one's receipt says (`multisig_reading`).
        "memberChanges" => out(json(&crate::multisig_reading::member_changes(
            &arg::<Action>(&args, "action")?,
            &arg::<Vec<String>>(&args, "owners")?,
            arg(&args, "threshold")?,
        )?)),
        "executionFacts" => out(json(&crate::multisig_reading::execution_facts(
            arg(&args, "chainId")?,
            &arg::<String>(&args, "safe")?,
            &arg::<String>(&args, "safeTxHash")?,
            &arg::<Vec<crate::multisig_reading::ReceiptLog>>(&args, "logs")?,
        )?)),
        // An execution in flight (`multisig_watch`).
        "executionVerdict" => out(json(&crate::multisig_watch::execution_verdict(&arg(
            &args, "facts",
        )?))),
        // The one review, its reads, and the gate before every signature
        // (`multisig_review`, §3.6).
        #[cfg(feature = "crux")]
        "reviewReads" => out(json(&crate::multisig_review::review_reads(&arg(
            &args, "input",
        )?)?)),
        #[cfg(feature = "crux")]
        "multisigReview" => out(json(&crate::multisig_review::multisig_review(&arg(
            &args, "input",
        )?)?)),
        #[cfg(feature = "crux")]
        "signingGate" => out(json(&crate::multisig_review::signing_gate(&arg(
            &args, "input",
        )?))),
        // How the multisig service's health answer reads (`app::genesis_service`).
        #[cfg(feature = "crux")]
        "serviceAnswer" => out(json(&crate::app::genesis_service::service_answer(
            arg(&args, "status")?,
            &arg::<String>(&args, "body")?,
        ))),
        other => Err(err(format!("unknown multisig method {other}"))),
    }
}

/// A founding key as the shells store it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyArg {
    credential_id: String,
    public_key_hex: String,
}

impl From<KeyArg> for WalletKey {
    fn from(k: KeyArg) -> Self {
        WalletKey {
            credential_id: k.credential_id,
            public_key_hex: k.public_key_hex,
        }
    }
}

mod erased {
    pub trait Ser {
        fn to_value(&self) -> serde_json::Value;
    }
    impl<T: serde::Serialize> Ser for T {
        fn to_value(&self) -> serde_json::Value {
            serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// The unnamed genesis for a salt nonce: the hashes pinned below were
    /// checked against the contracts with these.
    fn salted(
        owners: &[String],
        threshold: u64,
        salt_nonce: &str,
    ) -> Result<MultisigGenesis, CoreError> {
        genesis_with_salt(owners, threshold, quantity(salt_nonce)?)
    }

    const A: &str = "0x1111111111111111111111111111111111111111";
    const B: &str = "0x2222222222222222222222222222222222222222";

    /// The service's health answer is read through the multisig door as JSON.
    #[cfg(feature = "crux")]
    #[test]
    fn the_service_answer_reads_through_the_door() {
        let body = serde_json::json!({
            "status": 200,
            "body": r#"{"service":"safe-genesis-registry","chains":[{"chainId":100}]}"#
        });
        let answer: serde_json::Value =
            serde_json::from_str(&call_json("serviceAnswer", &body.to_string()).unwrap()).unwrap();
        assert_eq!(
            answer,
            serde_json::json!({"kind":"genesis","chains":[{"chainId":100,"name":null,"registry":false}],"homeChainId":null,"ok":true})
        );
    }

    /// The contracts a chain must hold are exactly the ones a new multisig's
    /// genesis names: its factory, its singleton, the handler its setup sets.
    #[test]
    fn the_safe_contracts_are_the_ones_its_genesis_names() {
        let g = multisig_genesis(&[A.into(), B.into()], 2, "Team", "0x7").unwrap();
        let [(_, factory), (_, singleton), (_, handler)] = MULTISIG_CONTRACTS;
        assert_eq!(g.genesis.factory.to_lowercase(), factory.to_lowercase());
        assert_eq!(g.genesis.singleton.to_lowercase(), singleton.to_lowercase());
        assert!(g
            .genesis
            .initializer
            .to_lowercase()
            .contains(&handler[2..].to_lowercase()));
    }

    #[test]
    fn a_chain_with_every_safe_contract_can_hold_a_multisig() {
        let [factory, singleton, handler] = [
            Some(safe_code("safe-proxy-factory-1.4.1")),
            Some(safe_code("safe-l2-1.4.1")),
            Some(safe_code("compatibility-fallback-handler-1.4.1")),
        ];
        assert_eq!(
            safe_contracts(&[factory.clone(), singleton.clone(), handler.clone()]),
            SafeContracts::Present
        );
        // One without code: missing, named — even while another went unanswered.
        assert_eq!(
            safe_contracts(&[factory.clone(), None, Some("0x".into())]),
            SafeContracts::Missing {
                names: vec!["Compatibility Fallback Handler".into()]
            }
        );
        assert_eq!(
            safe_contracts(&[Some("0x0".into()), singleton.clone(), handler.clone()]),
            SafeContracts::Missing {
                names: vec!["Safe Proxy Factory".into()]
            }
        );
        // Code that is not Safe's at Safe's address is no Safe contract.
        assert_eq!(
            safe_contracts(&[factory.clone(), Some("0x6080".into()), handler.clone()]),
            SafeContracts::Missing {
                names: vec!["Safe L2".into()]
            }
        );
        // The chain did not answer: nothing said either way.
        assert_eq!(
            safe_contracts(&[factory.clone(), None, handler]),
            SafeContracts::Unknown
        );
        assert_eq!(safe_contracts(&[factory]), SafeContracts::Unknown);
        // Through the door, as the shells read it.
        let answer = call_json(
            "safeContracts",
            &serde_json::json!({"codes": [
                safe_code("safe-proxy-factory-1.4.1"),
                "0x",
                safe_code("compatibility-fallback-handler-1.4.1")
            ]})
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&answer).unwrap(),
            serde_json::json!({"kind": "missing", "names": ["Safe L2"]})
        );
        let listed: serde_json::Value =
            serde_json::from_str(&call_json("multisigContracts", "{}").unwrap()).unwrap();
        assert_eq!(listed[0]["address"], safe::SAFE_PROXY_FACTORY);
        assert_eq!(listed.as_array().unwrap().len(), 3);
    }

    /// A multisig is on a network because something says so: deployed there,
    /// proposals there, a link or the person naming it, what this device
    /// kept. Nothing says: empty, and the person is asked.
    #[test]
    fn a_multisig_is_on_the_networks_something_says() {
        let on = |e: NetworkEvidence| multisig_networks(&e);
        assert_eq!(
            on(NetworkEvidence {
                deployed: vec![42161],
                ..Default::default()
            }),
            vec![42161]
        );
        assert_eq!(
            on(NetworkEvidence {
                deployed: vec![42161, 8453],
                proposals: vec![8453, 10],
                named: Some(137),
                kept: vec![137, 1],
                have: None,
            }),
            vec![42161, 8453, 10, 137, 1]
        );
        // Nothing says: no network — never the registry's chain.
        assert_eq!(on(NetworkEvidence::default()), Vec::<u64>::new());
        assert_eq!(
            on(NetworkEvidence {
                named: Some(0),
                ..Default::default()
            }),
            Vec::<u64>::new()
        );
        let answer = call_json(
            "multisigNetworks",
            r#"{"evidence":{"proposals":[10],"named":42161}}"#,
        )
        .unwrap();
        assert_eq!(answer, "[10,42161]");
    }

    /// Proposals (or a link) on a network this device lacks: never one of
    /// its networks here — nothing can be read there — but listed, so the
    /// person can add it.
    #[test]
    fn a_network_this_device_lacks_is_missing_not_used() {
        let e = NetworkEvidence {
            deployed: vec![100],
            proposals: vec![100, 99999, 8453],
            named: Some(77777),
            kept: vec![1],
            have: Some(vec![100, 8453, 1]),
        };
        assert_eq!(multisig_networks(&e), vec![100, 8453, 1]);
        assert_eq!(missing_networks(&e), vec![99999, 77777]);
        // Without `have`, nothing is missing and every network is used.
        let all = NetworkEvidence { have: None, ..e };
        assert_eq!(missing_networks(&all), Vec::<u64>::new());
        assert_eq!(multisig_networks(&all), vec![100, 99999, 8453, 77777, 1]);
        let answer = call_json(
            "missingNetworks",
            r#"{"evidence":{"proposals":[10,99999],"have":[10]}}"#,
        )
        .unwrap();
        assert_eq!(answer, "[99999]");
    }

    #[test]
    fn member_drift_names_who_was_added_and_removed() {
        let original = vec![A.to_string(), B.to_string()];
        let same = member_drift(&original, 2, &[B.to_lowercase(), A.to_lowercase()], 2).unwrap();
        assert!(
            same.same,
            "the same members in another order and case are the same"
        );
        let c = "0x00000000000000000000000000000000000000c3";
        let swapped = member_drift(&original, 2, &[A.to_string(), c.to_string()], 1).unwrap();
        assert!(!swapped.same);
        assert_eq!(swapped.added, vec![addr(c).unwrap().to_checksum(None)]);
        assert_eq!(swapped.removed, vec![addr(B).unwrap().to_checksum(None)]);
        assert_eq!((swapped.threshold_was, swapped.threshold_is), (2, 1));
        assert!(!member_drift(&original, 2, &original, 1).unwrap().same);
        assert!(member_drift(&original, 2, &["nope".to_string()], 1).is_err());
        let answer = call_json(
            "memberDrift",
            &serde_json::json!({"original": [A], "originalThreshold": 1, "current": [A, c], "currentThreshold": 1})
                .to_string(),
        )
        .unwrap();
        assert!(answer.contains(r#""same":false"#) && answer.contains(r#""thresholdIs":1"#));
    }

    /// A rename never deploys: executed where the multisig has no code, it
    /// would create it there with its original members.
    #[test]
    fn a_rename_is_never_executed_with_a_deployment() {
        let ms = salted(&[A.into(), B.into()], 1, "0x7").unwrap();
        let plan = |deploy: bool| ExecutionPlan {
            multisig: ms.address.clone(),
            deploy_multisig: deploy.then(|| ms.genesis.clone()),
            deploy_owners: vec![],
            tx: rename_tx("Team treasury", "0").unwrap(),
            approvals: vec![],
            executor: Some(A.into()),
            // The rename is refused before any genesis is judged.
            deploy_facts: None,
        };
        let refused = execution_calls(&plan(true)).unwrap_err();
        assert!(format!("{refused:?}").contains("never deploys"));
        assert_eq!(execution_calls(&plan(false)).unwrap().len(), 1);
    }

    #[test]
    fn owner_order_is_part_of_the_address() {
        let ab = salted(&[A.into(), B.into()], 2, "0x7").unwrap();
        let ba = salted(&[B.into(), A.into()], 2, "0x7").unwrap();
        assert_ne!(ab.address, ba.address);
        assert_eq!(
            genesis_members(&ab.genesis).unwrap(),
            (
                vec![
                    primitives::checksum_address(A).unwrap(),
                    primitives::checksum_address(B).unwrap()
                ],
                2
            )
        );
        assert_eq!(genesis_address(&ab.genesis).unwrap(), ab.address);
    }

    #[test]
    fn a_genesis_that_could_never_deploy_is_refused() {
        assert!(salted(&[A.into(), A.into()], 1, "0x1").is_err());
        assert!(salted(
            &["0x0000000000000000000000000000000000000001".into()],
            1,
            "0x1"
        )
        .is_err());
        assert!(salted(&[A.into()], 2, "0x1").is_err());
        assert!(salted(&[A.into()], 0, "0x1").is_err());
        assert!(salted(&[], 1, "0x1").is_err());
    }

    #[test]
    fn the_review_signs_only_what_it_reads() {
        let ms = salted(&[A.into(), B.into()], 2, "0x7").unwrap().address;
        let one = transfers_tx(
            &[Transfer {
                token: None,
                to: B.into(),
                amount: "1000".into(),
            }],
            "0",
        )
        .unwrap();
        assert!(matches!(
            review(100, &ms, &one).unwrap().action,
            Some(Action::Transfer { token: None, .. })
        ));
        let batch = transfers_tx(
            &[
                Transfer {
                    token: None,
                    to: B.into(),
                    amount: "1".into(),
                },
                Transfer {
                    token: Some(A.into()),
                    to: B.into(),
                    amount: "2".into(),
                },
            ],
            "0",
        )
        .unwrap();
        assert!(matches!(
            review(100, &ms, &batch).unwrap().action,
            Some(Action::Batch { .. })
        ));
        assert!(matches!(
            review(100, &ms, &cancel_tx(&ms, "3").unwrap())
                .unwrap()
                .action,
            Some(Action::Cancel)
        ));

        let mut refund = one.clone();
        refund.gas_price = "0x1".into();
        assert!(review(100, &ms, &refund).unwrap().refusal.is_some());
        let mut delegate = one.clone();
        delegate.operation = 1;
        assert!(review(100, &ms, &delegate).unwrap().refusal.is_some());
        // A dApp's call to another contract: read as what it is, a call.
        let mut call = one.clone();
        call.data = "0x12345678".into();
        assert!(matches!(
            review(100, &ms, &call).unwrap().action,
            Some(Action::Call { ref data, .. }) if data == "0x12345678"
        ));
        // A self-call that is not member management.
        let mut module = cancel_tx(&ms, "0").unwrap();
        module.data = hex(&[
            &selector("enableModule(address)")[..],
            &word_addr(addr(A).unwrap())[..],
        ]
        .concat());
        assert!(review(100, &ms, &module).unwrap().refusal.is_some());
        // A batch with trailing garbage.
        let mut tail = batch.clone();
        tail.data
            .push_str("00000000000000000000000000000000000000000000000000000000000000ff");
        assert!(review(100, &ms, &tail).unwrap().refusal.is_some());
    }

    /// A packed MultiSendCallOnly batch, each leg `(operation, to, value, data)`.
    fn packed_batch(legs: &[(u8, Address, U256, Vec<u8>)]) -> SafeTx {
        let mut packed = Vec::new();
        for (op, to, value, data) in legs {
            packed.push(*op);
            packed.extend_from_slice(to.as_slice());
            packed.extend_from_slice(&word_u256(*value));
            packed.extend_from_slice(&word_usize(data.len()));
            packed.extend_from_slice(data);
        }
        let mut d = selector("multiSend(bytes)").to_vec();
        d.extend_from_slice(&word_usize(0x20));
        d.extend(bytes_tail(&packed));
        plain_tx(
            addr(MULTI_SEND_CALL_ONLY).unwrap(),
            U256::ZERO,
            d,
            1,
            U256::ZERO,
        )
    }

    #[test]
    fn a_batch_of_a_sites_calls_is_read_call_by_call() {
        let ms = salted(&[A.into(), B.into()], 2, "0x7").unwrap().address;
        // An approve, then a swap with coin: ONE transaction, two CALLs.
        let approve = format!("0x095ea7b3{}{}", "0".repeat(24) + &B[2..], "f".repeat(64));
        let tx = calls_tx(
            &[
                CallArg {
                    to: A.into(),
                    value: String::new(),
                    data: approve.clone(),
                },
                CallArg {
                    to: B.into(),
                    value: "0x10".into(),
                    data: "0x12345678".into(),
                },
            ],
            "0x2",
        )
        .unwrap();
        assert_eq!(tx.operation, 1);
        assert_eq!(tx.to, MULTI_SEND_CALL_ONLY);
        assert_eq!(tx.nonce, "0x2");
        let Some(Action::Batch { calls }) = review(100, &ms, &tx).unwrap().action else {
            panic!("a batch of calls is read");
        };
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].data, approve);
        assert!(matches!(calls[0].action, Action::Call { .. }));
        assert_eq!(calls[1].value, "16");
        assert!(matches!(
            calls[1].action,
            Action::Call { ref value, .. } if value == "16"
        ));
        assert!(!calls.iter().any(BatchCall::changes_members));

        // One call is a plain CALL, the same transaction `eth_sendTransaction` gives.
        let one = calls_tx(
            &[CallArg {
                to: B.into(),
                value: "0x0".into(),
                data: "0x12345678".into(),
            }],
            "0",
        )
        .unwrap();
        assert_eq!(one.operation, 0);
        assert!(matches!(
            review(100, &ms, &one).unwrap().action,
            Some(Action::Call { .. })
        ));
        assert!(calls_tx(&[], "0").is_err());
    }

    #[test]
    fn a_batch_that_could_rewrite_the_multisig_is_refused_and_a_member_change_is_marked() {
        let ms = salted(&[A.into(), B.into()], 2, "0x7").unwrap().address;
        let me = addr(&ms).unwrap();
        let a = addr(A).unwrap();
        let send = (0u8, a, U256::from(1), Vec::new());
        let refusal = |tx: &SafeTx| review(100, &ms, tx).unwrap().refusal;

        // A delegatecall leg.
        let delegate = packed_batch(&[send.clone(), (1, a, U256::ZERO, vec![1, 2, 3, 4])]);
        assert_eq!(
            refusal(&delegate).as_deref(),
            Some("a call in the batch is a delegatecall")
        );
        // A leg to the zero address runs as the multisig itself.
        let zero = packed_batch(&[send.clone(), (0, Address::ZERO, U256::ZERO, vec![])]);
        assert!(refusal(&zero).is_some());
        // A module, a guard or a fallback handler, set from inside a batch.
        for signature in [
            "enableModule(address)",
            "setGuard(address)",
            "setFallbackHandler(address)",
        ] {
            let data = [&selector(signature)[..], &word_addr(a)[..]].concat();
            let tx = packed_batch(&[send.clone(), (0, me, U256::ZERO, data)]);
            assert!(refusal(&tx).is_some(), "{signature}");
        }
        // Coin sent into the multisig with data.
        let threshold = [
            &selector("changeThreshold(uint256)")[..],
            &word_u256(U256::from(1))[..],
        ]
        .concat();
        let paid = packed_batch(&[send.clone(), (0, me, U256::from(1), threshold.clone())]);
        assert!(refusal(&paid).is_some());

        // A member change is read, and marked as one.
        let change = packed_batch(&[send, (0, me, U256::ZERO, threshold)]);
        let Some(Action::Batch { calls }) = review(100, &ms, &change).unwrap().action else {
            panic!("a member change in a batch is read");
        };
        assert!(matches!(
            calls[0].action,
            Action::Transfer { token: None, .. }
        ));
        assert!(matches!(
            calls[1].action,
            Action::ChangeThreshold { ref threshold } if threshold == "1"
        ));
        assert!(calls[1].changes_members());
    }

    #[test]
    fn the_executor_counts_once_and_signatures_are_sorted() {
        let a = |owner: &str| PasskeyApproval {
            owner: owner.into(),
            owner_genesis: Genesis {
                factory: String::new(),
                singleton: String::new(),
                salt_nonce: "0x0".into(),
                initializer: "0x".into(),
                name: None,
            },
            signer: safe::WEBAUTHN_SIGNER.into(),
            authenticator_data: hex(&[0u8; 37]),
            client_data_fields: "\"origin\":\"https://getvela.app\",\"crossOrigin\":false".into(),
            r: "0x1".into(),
            s: "0x2".into(),
        };
        let sigs = unhex(
            &assemble_signatures(
                &[
                    Approval::Passkey(Box::new(a(B))),
                    Approval::Passkey(Box::new(a(A))),
                ],
                Some(B),
            )
            .unwrap(),
        )
        .unwrap();
        // A (contract signature) before B (the executor, v = 1).
        assert_eq!(&sigs[12..32], addr(A).unwrap().as_slice());
        assert_eq!(sigs[64], 0);
        assert_eq!(U256::from_be_slice(&sigs[32..64]), U256::from(130));
        assert_eq!(&sigs[65 + 12..65 + 32], addr(B).unwrap().as_slice());
        assert_eq!(sigs[129], 1);
    }
    #[test]
    fn the_sponsor_challenge_is_the_genesis_services() {
        // keccak256("safe-genesis-registry/sponsor/v1" ‖ 0x1111…1111), as
        // genesis-rules' `owner_proof::challenge` computes it.
        assert_eq!(
            sponsor_challenge(A).unwrap(),
            "0x0c0e452a9670a1d8e16942787f9d45fc10c32417bc67abd93860522358821f66"
        );
    }

    /// Pinned from a run that executed on a Gnosis fork (Safe v1.4.1): this
    /// transaction, approved over this challenge by fixture #2 through its
    /// own Safe's EIP-1271, moved the funds — the hash the owner's handler
    /// checks, not the dApp message hash.
    #[cfg(feature = "dev-fixtures")]
    #[test]
    fn the_approval_challenge_is_the_one_the_chain_accepted() {
        let a = crate::dev_fixtures::account(0).unwrap().address;
        let b = crate::dev_fixtures::account(1).unwrap().address;
        let ms = salted(&[a, b.clone()], 2, "0x2a").unwrap();
        assert_eq!(ms.address, "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83");
        let tx = transfers_tx(
            &[Transfer {
                token: None,
                to: "0x000000000000000000000000000000000000dEaD".into(),
                amount: "1000000000000000000".into(),
            }],
            "0",
        )
        .unwrap();
        let hash = safe_tx_hash(100, &ms.address, &tx).unwrap();
        assert_eq!(
            hash,
            "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa"
        );
        assert_eq!(
            approval_challenge(100, &b, &hash).unwrap(),
            "0xe8fcaaa0e72186d1a5246be96acdfeca865088110371ce7135f78a1c9c64e208"
        );
        // Never the dApp path's hash, whose extra keccak the chain rejects.
        let dapp =
            crate::user_op::compute_safe_message_hash(&unhex(&hash).unwrap(), 100, &b).unwrap();
        assert_ne!(hex(&dapp), approval_challenge(100, &b, &hash).unwrap());
    }

    #[cfg(feature = "dev-fixtures")]
    #[test]
    fn a_fixture_approval_verifies_and_a_tampered_one_does_not() {
        use crate::dev_fixtures as fx;
        let owner = fx::account(1).unwrap();
        let keys = vec![WalletKey {
            credential_id: owner.credential_id_hex.clone(),
            public_key_hex: owner.public_key_hex.clone(),
        }];
        let ms = salted(
            &[fx::account(0).unwrap().address, owner.address.clone()],
            2,
            "0x2a",
        )
        .unwrap();
        let tx = transfers_tx(
            &[Transfer {
                token: None,
                to: B.into(),
                amount: "5".into(),
            }],
            "0",
        )
        .unwrap();
        let hash = safe_tx_hash(100, &ms.address, &tx).unwrap();
        let challenge = unhex(&approval_challenge(100, &owner.address, &hash).unwrap()).unwrap();
        let assertion =
            fx::build_assertion(&owner, &challenge, "getvela.app", "https://getvela.app").unwrap();
        let approval = approval_from_assertion(
            &keys,
            &owner.credential_id_hex,
            &assertion.authenticator_data_hex,
            &assertion.client_data_json_hex,
            &assertion.signature_der_hex,
        )
        .unwrap();
        assert!(verify_approval(100, &hash, &approval).is_ok());

        // Another transaction, another chain, another owner's genesis, edited client data.
        let other = safe_tx_hash(
            100,
            &ms.address,
            &transfers_tx(
                &[Transfer {
                    token: None,
                    to: B.into(),
                    amount: "6".into(),
                }],
                "0",
            )
            .unwrap(),
        )
        .unwrap();
        assert!(verify_approval(100, &other, &approval).is_err());
        assert!(verify_approval(1, &hash, &approval).is_err());
        let mut wrong = approval.clone();
        wrong.owner_genesis = vela_account_genesis(&[WalletKey {
            credential_id: "x".into(),
            public_key_hex: fx::account(2).unwrap().public_key_hex,
        }])
        .unwrap()
        .1;
        assert!(verify_approval(100, &hash, &wrong).is_err());
        let mut edited = approval.clone();
        edited.client_data_fields =
            "\"origin\":\"https://evil.example\",\"crossOrigin\":false".into();
        assert!(verify_approval(100, &hash, &edited).is_err());

        // Assembled for execution: one contract signature, then the executor.
        let plan = ExecutionPlan {
            multisig: ms.address.clone(),
            deploy_multisig: Some(ms.genesis.clone()),
            deploy_owners: vec![],
            tx: tx.clone(),
            approvals: vec![Approval::Passkey(Box::new(approval))],
            executor: Some(fx::account(0).unwrap().address),
            deploy_facts: Some(safe_141_facts()),
        };
        let calls = execution_calls(&plan).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].to, safe::SAFE_PROXY_FACTORY);
        assert_eq!(addr(&calls[1].to).unwrap(), addr(&ms.address).unwrap());
    }

    #[test]
    fn a_genesis_for_another_address_is_not_deployed() {
        let ms = salted(&[A.into(), B.into()], 2, "0x7").unwrap();
        let other = salted(&[A.into(), B.into()], 2, "0x8").unwrap();
        let tx = transfers_tx(
            &[Transfer {
                token: None,
                to: B.into(),
                amount: "1".into(),
            }],
            "0",
        )
        .unwrap();
        let plan = ExecutionPlan {
            multisig: ms.address,
            deploy_multisig: Some(other.genesis),
            deploy_owners: vec![],
            tx,
            approvals: vec![],
            executor: Some(A.into()),
            deploy_facts: Some(safe_141_facts()),
        };
        assert!(execution_calls(&plan).is_err());
    }

    #[test]
    fn the_json_door_answers_and_refuses() {
        let out = call_json("genesis", r#"{"owners":["0x1111111111111111111111111111111111111111","0x2222222222222222222222222222222222222222"],"threshold":2,"name":"Team fund","saltNonce":"0x7"}"#).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["threshold"], 2);
        assert_eq!(v["genesis"]["name"], "Team fund");
        assert_eq!(
            call_json("validName", r#"{"name":" padded"}"#).unwrap(),
            "false"
        );
        assert!(call_json("nope", "{}").is_err());
        assert!(call_json("genesis", "not json").is_err());
    }

    /// A named multisig: the name is written with its record and takes no
    /// part in the address — the same owners and salt give the same address
    /// whatever it is called.
    #[test]
    fn a_name_is_a_label_not_part_of_the_address() {
        let ms = multisig_genesis(&[A.into(), B.into()], 2, "Team fund", "0x7").unwrap();
        let unnamed = salted(&[A.into(), B.into()], 2, "0x7").unwrap();
        assert_eq!(ms.address, unnamed.address);
        assert_eq!(ms.genesis.name.as_deref(), Some("Team fund"));
        assert_eq!(genesis_address(&ms.genesis).unwrap(), ms.address);
        let renamed = multisig_genesis(&[A.into(), B.into()], 2, "Another", "0x7").unwrap();
        assert_eq!(renamed.address, ms.address);
        assert_eq!(
            ms.genesis.salt_nonce,
            salted(&[A.into(), B.into()], 2, "0x7")
                .unwrap()
                .genesis
                .salt_nonce
        );
    }

    #[test]
    fn a_name_must_be_one_the_registry_takes() {
        let make = |name: &str| multisig_genesis(&[A.into(), B.into()], 2, name, "0x7");
        assert!(make("").is_err());
        assert!(make(" Team").is_err());
        assert!(make("Team\u{0007}").is_err());
        assert!(make(&"x".repeat(65)).is_err());
        assert!(make(&"x".repeat(64)).is_ok());
        assert!(make(&"多".repeat(22)).is_err(), "66 bytes");
        assert!(make(&"多".repeat(21)).is_ok());
    }

    /// The name is permanent and every member reads it: nothing in it may be
    /// invisible or turn the text around.
    #[test]
    fn a_name_hides_nothing_and_turns_nothing_around() {
        for hidden in [
            "Team\u{202E}dnuf",
            "\u{2066}Team\u{2069} fund",
            "Team\u{200F}fund",
            "Team\u{200B}fund",
            "Te\u{200C}am",
            "Team\u{2060}fund",
            "\u{FEFF}Team",
            "Team\u{00AD}fund",
            "Team\u{200D}fund",
            "\u{200D}\u{1F469}",
            "\u{1F469}\u{200D}",
        ] {
            assert!(!valid_name(hidden), "{hidden:?}");
            assert!(
                multisig_genesis(&[A.into(), B.into()], 2, hidden, "0x7").is_err(),
                "{hidden:?}"
            );
        }
        // An emoji's joiner, between two pictographs, is part of the emoji.
        assert!(valid_name(
            "Family \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"
        ));
        assert!(valid_name("\u{2764}\u{FE0F}\u{200D}\u{1F525} fund"));
        assert!(valid_name("Équipe — fonds"));
        assert!(valid_name("فريق"));
    }

    #[test]
    fn the_zero_address_and_the_sentinel_are_no_members() {
        assert!(!valid_owner("0x0000000000000000000000000000000000000000"));
        assert!(!valid_owner("0x0000000000000000000000000000000000000001"));
        assert!(!valid_owner("0x12"));
        assert!(valid_owner(A));
        assert_eq!(
            call_json(
                "validOwner",
                r#"{"address":"0x0000000000000000000000000000000000000001"}"#
            )
            .unwrap(),
            "false"
        );
    }

    // -- plain accounts (EOA owners) and other Safe versions --------------------

    fn eoa(seed: u8) -> (k256::ecdsa::SigningKey, String) {
        let key = k256::ecdsa::SigningKey::from_slice(&[seed; 32]).unwrap();
        let point = key.verifying_key().to_sec1_point(false);
        let a = Address::from_slice(&keccak256(&point.as_bytes()[1..])[12..]);
        (key, a.to_checksum(None))
    }

    fn eoa_sign(key: &k256::ecdsa::SigningKey, hash: &str) -> String {
        let h: B256 = hash.parse().unwrap();
        let (sig, id) = key.sign_prehash_recoverable(h.as_slice());
        let mut out = sig.to_bytes().to_vec();
        out.push(27 + id.to_byte());
        hex(&out)
    }

    /// What a wallet signs for `eth_signTypedData_v4` is the safeTxHash: the
    /// typed data, hashed by the core's own EIP-712 code (the dApp path).
    #[test]
    fn the_typed_data_a_wallet_signs_is_the_safe_tx_hash() {
        let tx = transfers_tx(
            &[Transfer {
                token: None,
                to: B.into(),
                amount: "0x2386f26fc10000".into(),
            }],
            "0x3",
        )
        .unwrap();
        let safe = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83";
        let typed = safe_tx_typed_data(100, safe, &tx).unwrap();
        let params = serde_json::json!([A, typed.to_string()]).to_string();
        let hash = crate::sign_message::original_hash("eth_signTypedData_v4", &params).unwrap();
        assert_eq!(hex(&hash), safe_tx_hash(100, safe, &tx).unwrap());
    }

    #[test]
    fn a_plain_accounts_approval_is_checked_as_the_safe_checks_it() {
        let (key, owner) = eoa(7);
        let (other, _) = eoa(8);
        let hash = "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa";
        let good = EcdsaApproval {
            owner: owner.clone(),
            signature: eoa_sign(&key, hash),
        };
        assert_eq!(verify_ecdsa_approval(hash, &good).unwrap(), good);
        // A wallet's v of 0/1 comes back as 27/28.
        let mut sig = unhex(&good.signature).unwrap();
        sig[64] -= 27;
        let low = EcdsaApproval {
            owner: owner.clone(),
            signature: hex(&sig),
        };
        assert_eq!(verify_ecdsa_approval(hash, &low).unwrap(), good);
        // Someone else's key, another hash, a mangled signature.
        let forged = EcdsaApproval {
            owner: owner.clone(),
            signature: eoa_sign(&other, hash),
        };
        assert!(verify_ecdsa_approval(hash, &forged).is_err());
        let elsewhere = "0x00000000000000000000000000000000000000000000000000000000000000aa";
        assert!(verify_ecdsa_approval(elsewhere, &good).is_err());
        let short = EcdsaApproval {
            owner,
            signature: "0x1234".into(),
        };
        assert!(verify_ecdsa_approval(hash, &short).is_err());
    }

    fn eoa_personal_sign(key: &k256::ecdsa::SigningKey, hash: &str) -> String {
        let h: B256 = hash.parse().unwrap();
        let (sig, id) = key.sign_prehash_recoverable(eth_signed_hash(h).as_slice());
        let mut out = sig.to_bytes().to_vec();
        out.push(27 + id.to_byte());
        hex(&out)
    }

    /// Safe's `eth_sign` form: `personal_sign` over the safeTxHash, `v + 4`.
    /// Checked as the Safe checks it (over the EIP-191 hash), kept as 31/32,
    /// and told apart from the typed form; the bare `personal_sign` answer
    /// (`v` 27/28 over the EIP-191 hash) is NOT an approval of the safeTxHash.
    #[test]
    fn the_eth_sign_form_is_checked_over_the_eip191_hash() {
        let (key, owner) = eoa(7);
        let hash = "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa";
        let personal = eoa_personal_sign(&key, hash);
        let mut sig = unhex(&personal).unwrap();
        assert!(verify_ecdsa_approval(
            hash,
            &EcdsaApproval {
                owner: owner.clone(),
                signature: personal.clone()
            }
        )
        .is_err());
        sig[64] += 4;
        let form = EcdsaApproval {
            owner: owner.clone(),
            signature: hex(&sig),
        };
        let checked = verify_ecdsa_approval(hash, &form).unwrap();
        assert_eq!(checked.signature, hex(&sig));
        assert_eq!(ecdsa_form(&checked.signature).unwrap(), EcdsaForm::EthSign);
        assert_eq!(
            ecdsa_form(&eoa_sign(&key, hash)).unwrap(),
            EcdsaForm::TypedData
        );
        assert!(ecdsa_form("0x1234").is_err());
        // Another hash, another key: refused.
        let elsewhere = "0x00000000000000000000000000000000000000000000000000000000000000aa";
        assert!(verify_ecdsa_approval(elsewhere, &form).is_err());
        let (other, _) = eoa(8);
        let mut forged = unhex(&eoa_personal_sign(&other, hash)).unwrap();
        forged[64] += 4;
        assert!(verify_ecdsa_approval(
            hash,
            &EcdsaApproval {
                owner: owner.clone(),
                signature: hex(&forged)
            }
        )
        .is_err());
        // Assembled as it is: the Safe reads v 31/32 as eth_sign.
        let sigs = unhex(&assemble_signatures(&[Approval::Ecdsa(form)], None).unwrap()).unwrap();
        assert_eq!(sigs, sig);
        let hashes = safe_tx_hashes(
            100,
            "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83",
            &transfers_tx(
                &[Transfer {
                    token: None,
                    to: B.into(),
                    amount: "0x1".into(),
                }],
                "0x0",
            )
            .unwrap(),
        )
        .unwrap();
        let h: B256 = hashes.safe_tx_hash.parse().unwrap();
        assert_eq!(hashes.eth_sign_hash, hex(eth_signed_hash(h).as_slice()));
    }

    /// The hashes a hardware wallet shows make the safeTxHash:
    /// `keccak(0x1901 ‖ domain ‖ message)`, the domain Safe's
    /// `domainSeparator()` and the message the SafeTx struct hash.
    #[test]
    fn the_review_hashes_make_the_safe_tx_hash() {
        let safe = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83";
        let tx = transfers_tx(
            &[Transfer {
                token: None,
                to: B.into(),
                amount: "0x2386f26fc10000".into(),
            }],
            "0x3",
        )
        .unwrap();
        let h = safe_tx_hashes(100, safe, &tx).unwrap();
        assert_eq!(h.safe_tx_hash, safe_tx_hash(100, safe, &tx).unwrap());
        let mut p = vec![0x19, 0x01];
        p.extend(unhex(&h.domain_hash).unwrap());
        p.extend(unhex(&h.message_hash).unwrap());
        assert_eq!(hex(keccak256(p).as_slice()), h.safe_tx_hash);
        assert_eq!(
            h.domain_hash,
            hex(domain_separator(100, addr(safe).unwrap()).as_slice())
        );
        let via_call = call_json(
            "safeTxHashes",
            &serde_json::json!({"chainId": 100, "safe": safe, "tx": tx}).to_string(),
        )
        .unwrap();
        assert!(via_call.contains("\"domainHash\""));
        assert!(via_call.contains(&h.message_hash));
    }

    /// An owner that approved on chain (`approveHash`) sits in the
    /// signatures as the executor does, `r = owner, v = 1`, and reads back
    /// from the JSON as that kind (not a passkey's, not a plain account's).
    #[test]
    fn an_on_chain_approval_is_assembled_as_v1() {
        let onchain: Approval =
            serde_json::from_value(serde_json::json!({"owner": A, "onChain": true})).unwrap();
        assert!(matches!(onchain, Approval::OnChain(_)));
        let sigs = unhex(&assemble_signatures(&[onchain], None).unwrap()).unwrap();
        assert_eq!(sigs.len(), 65);
        assert_eq!(&sigs[12..32], addr(A).unwrap().as_slice());
        assert_eq!(&sigs[32..64], &[0u8; 32]);
        assert_eq!(sigs[64], 1);
    }

    /// EOA and passkey approvals and the executor, sorted by owner: an EOA's
    /// entry is its own 65 bytes, a passkey owner's points into the tail.
    #[test]
    fn mixed_signatures_are_sorted_and_shaped_per_kind() {
        let (key, owner) = eoa(7);
        let hash = "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa";
        let ecdsa = Approval::Ecdsa(EcdsaApproval {
            owner: owner.clone(),
            signature: eoa_sign(&key, hash),
        });
        let sigs =
            unhex(&assemble_signatures(std::slice::from_ref(&ecdsa), Some(B)).unwrap()).unwrap();
        assert_eq!(sigs.len(), 130);
        let mut entries = [(addr(&owner).unwrap(), 0usize), (addr(B).unwrap(), 1)];
        entries.sort_by_key(|(o, _)| *o);
        for (slot, (o, kind)) in entries.iter().enumerate() {
            let e = &sigs[slot * 65..slot * 65 + 65];
            if *kind == 0 {
                assert_eq!(hex(e), eoa_sign(&key, hash), "the EOA's own signature");
            } else {
                assert_eq!(&e[12..32], o.as_slice());
                assert_eq!(e[64], 1, "the executor approves as msg.sender");
            }
        }
        // Without an executor (an external wallet sends it): just the approvals.
        let alone = unhex(&assemble_signatures(&[ecdsa], None).unwrap()).unwrap();
        assert_eq!(alone.len(), 65);
    }

    /// A real Safe v1.3.0 (Gnosis, `createProxyWithNonce`, from the genesis
    /// registry's fixtures): its genesis derives its own address.
    #[test]
    fn a_safe_v130_genesis_derives_its_address() {
        let g = Genesis {
            factory: "0xa6B71E26C5e0845f74c812102Ca7114b6A896AB2".into(),
            singleton: "0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552".into(),
            salt_nonce: "12596526766320250484971449312609726242167314162064473215341362975032004421818".into(),
            initializer: "0xb63e800d0000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000140000000000000000000000000f48f2b2d2a534e402487b3ee7c18c33aec0fe5e400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000004e42010578eec11fd35f985dc0b3bc22520fd8550000000000000000000000000000000000000000000000000000000000000000".into(),
            name: None,
        };
        assert_eq!(
            genesis_address(&g).unwrap().to_lowercase(),
            "0xa42f6c68d9e0e13ac0910837356c0a0155413e8c"
        );
        let (owners, threshold) = genesis_members(&g).unwrap();
        assert_eq!(threshold, 1);
        assert_eq!(owners.len(), 1);
        // A factory or singleton this wallet does not know is refused.
        let mut odd = g.clone();
        odd.singleton = "0x0000000000000000000000000000000000000001".into();
        assert!(genesis_address(&odd).is_err());
    }

    /// A Safe{Wallet} export: deployed and undeployed Safes merged by address
    /// across chains, the address book's name kept, junk skipped.
    #[test]
    fn a_safe_wallet_export_is_read_by_address() {
        let json = r#"{"version":"3.0","data":{
            "addedSafes":{
                "1":{"0x1111111111111111111111111111111111111111":{"owners":[{"value":"0x2222222222222222222222222222222222222222"},{"value":"0x3333333333333333333333333333333333333333"}],"threshold":2}},
                "100":{"0x1111111111111111111111111111111111111111":{"owners":[{"value":"0x2222222222222222222222222222222222222222"}],"threshold":1},
                       "not an address":{"owners":[],"threshold":1}}},
            "undeployedSafes":{
                "8453":{"0x4444444444444444444444444444444444444444":{"props":{"factoryAddress":"0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67","masterCopy":"0x29fcB43b46531BcA003ddC8FCB67FFE91900C762","safeAccountConfig":{"threshold":1,"owners":["0x2222222222222222222222222222222222222222"],"fallbackHandler":"0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99"},"safeVersion":"1.4.1","saltNonce":"7"},"status":{"status":"AWAITING_EXECUTION","type":"PayLater"}}},
                "1":{"0x1111111111111111111111111111111111111111":{"props":{"safeAccountConfig":{"threshold":9,"owners":[]}}}}},
            "addressBook":{"1":{"0x1111111111111111111111111111111111111111":"Treasury","0x4444444444444444444444444444444444444444":"  "},
                           "8453":{"0x4444444444444444444444444444444444444444":"Base vault"}},
            "settings":{}
        }}"#;
        let safes = parse_safe_export(json).unwrap();
        assert_eq!(safes.len(), 2);
        let treasury = &safes[0];
        assert_eq!(
            treasury.address,
            "0x1111111111111111111111111111111111111111"
        );
        assert_eq!(treasury.name.as_deref(), Some("Treasury"));
        assert_eq!(
            treasury
                .chains
                .iter()
                .map(|c| c.chain_id)
                .collect::<Vec<_>>(),
            vec![1, 100]
        );
        assert!(
            treasury.chains.iter().all(|c| c.deployed),
            "deployed wins over an undeployed entry"
        );
        assert_eq!(treasury.chains[0].threshold, 2);
        assert_eq!(treasury.chains[0].owners.len(), 2);
        let vault = &safes[1];
        assert_eq!(
            vault.name.as_deref(),
            Some("Base vault"),
            "a blank name is skipped"
        );
        assert!(!vault.chains[0].deployed);
        assert_eq!(vault.chains[0].version.as_deref(), Some("1.4.1"));
        assert!(vault.chains[0].props.is_some());
        assert!(parse_safe_export("{}").is_err());
        assert!(parse_safe_export("not json").is_err());
    }

    /// An undeployed Safe as Safe{Wallet} keeps it: its props build the
    /// genesis the core builds for the same owners and salt (pinned address).
    #[test]
    fn safe_wallet_props_build_the_same_genesis() {
        let props = serde_json::json!({
            "factoryAddress": safe::SAFE_PROXY_FACTORY,
            "masterCopy": safe::SAFE_SINGLETON,
            "safeAccountConfig": {
                "threshold": 2,
                "owners": [A, B],
                "fallbackHandler": COMPATIBILITY_FALLBACK_HANDLER,
                "to": "0x0000000000000000000000000000000000000000",
                "data": "0x",
                "paymentReceiver": "0x0000000000000000000000000000000000000000"
            },
            "safeVersion": "1.4.1",
            "saltNonce": "7"
        });
        let expected = salted(&[A.into(), B.into()], 2, "0x7").unwrap();
        let g = safe_wallet_props_genesis(&props, &expected.address).unwrap();
        assert_eq!(g, expected.genesis);
        assert!(
            safe_wallet_props_genesis(&props, B).is_err(),
            "another address is refused"
        );
    }

    /// The record call, as registry v4 decodes it — one `record`, the name
    /// required: checked against `cast calldata` of the same arguments
    /// (`tests/vectors/multisig-record-v4.hex`), and sent on a Gnosis fork to
    /// the deployed v4 code, where it recorded the Safe whose `genesisOf`
    /// answer is `multisig-genesis-of.hex`.
    #[test]
    fn the_record_call_is_the_registrys() {
        let ms = multisig_genesis(&[A.into(), B.into()], 2, "Team fund", "0x7").unwrap();
        let call = record_call(&ms.genesis).unwrap();
        assert_eq!(call.to, GENESIS_REGISTRY);
        assert_eq!(call.value, "0x0");
        assert!(call.data.starts_with(&hex(&selector(
            "record(address,bytes,address,bytes,uint256,string)"
        ))));
        assert_eq!(
            call.data,
            include_str!("../tests/vectors/multisig-record-v4.hex").trim()
        );
        // A genesis with no name (an imported Safe's) is recorded under the
        // name the service would give it: never an empty one the registry
        // refuses (`EmptyName`), never the device's own label.
        let mut unnamed = ms.genesis.clone();
        unnamed.name = None;
        let plain = record_call(&unnamed).unwrap();
        let named = Genesis {
            name: Some(default_name(&ms.address).unwrap()),
            ..unnamed.clone()
        };
        assert_eq!(plain, record_call(&named).unwrap());
        unnamed.name = Some(String::new());
        assert_eq!(plain, record_call(&unnamed).unwrap());
    }

    /// v4's address: the deterministic deployer, `keccak256("safe-genesis-registry/v4")`
    /// and the registry's init code hash (`forge script script/Deploy.s.sol`).
    #[test]
    fn the_registry_is_v4_at_its_deterministic_address() {
        let deployer: Address = "0x4e59b44847b379578588920cA78FbF26c0B4956C"
            .parse()
            .unwrap();
        let salt = keccak256("safe-genesis-registry/v4");
        let init_code_hash: B256 =
            "0x0cda7b2c51511808f74be8b6b67914b0c9eadff5455dee95f57b357c288aeb83"
                .parse()
                .unwrap();
        assert_eq!(
            deployer.create2(salt, init_code_hash).to_checksum(None),
            GENESIS_REGISTRY
        );
    }

    /// The registry's own limits (`EmptyName`, `NameTooLong`: 1 to 64 bytes,
    /// UTF-8 unchecked) hold every name the wallet allows.
    #[test]
    fn every_name_the_wallet_writes_the_registry_takes() {
        assert!(!recordable_name(""));
        assert!(recordable_name(&"x".repeat(64)));
        assert!(!recordable_name(&"x".repeat(65)));
        assert!(recordable_name(&"多".repeat(21)), "63 bytes");
        assert!(!recordable_name(&"多".repeat(22)), "66 bytes");
        // Bytes, not characters: 16 four-byte emoji are 64 bytes; one more is not.
        assert!(recordable_name(&"🦊".repeat(16)));
        assert!(!recordable_name(&"🦊".repeat(17)));
        assert!(valid_name(&"🦊".repeat(16)));
        assert!(!valid_name(&"🦊".repeat(17)));
        // The registry takes what Vela would not show; the wallet's rule is the narrower one.
        assert!(recordable_name(" Team"));
        assert!(!valid_name(" Team"));
        for name in ["Team fund", "家庭基金", "👨‍👩‍👧 fund", &"x".repeat(64)] {
            assert!(valid_name(name) && recordable_name(name), "{name}");
        }
    }

    /// The service's default name (genesis-rules `default_name`): the same two
    /// words from `keccak256(address)` — 0x1111…1111 hashes to 0xe2c0…, so
    /// adjective 0xe2 % 32 = 2 and noun 0xc0 % 32 = 0.
    #[test]
    fn the_default_name_is_the_services() {
        assert_eq!(default_name(A).unwrap(), "Bright Badger");
        assert!(valid_name(&default_name(B).unwrap()));
        // Case does not matter: it is the address that is hashed.
        assert_eq!(
            default_name(&A.to_uppercase().replace("0X", "0x")).unwrap(),
            "Bright Badger"
        );
        // What the v4 service named these unnamed geneses (salt nonces 8, 9,
        // 10 of the A/B 2-of-2): its `POST /api/import` answer's `genesis.name`.
        for (nonce, name) in [
            ("0x8", "Bright Harbor"),
            ("0x9", "Brave Cedar"),
            ("0xa", "Vivid Thistle"),
        ] {
            let ms = salted(&[A.into(), B.into()], 2, nonce).unwrap();
            assert_eq!(default_name(&ms.address).unwrap(), name, "{nonce}");
            assert_eq!(
                named_genesis(&ms.genesis).unwrap().name.as_deref(),
                Some(name)
            );
        }
        let ms = salted(&[A.into(), B.into()], 2, "0x7").unwrap();
        let named = named_genesis(&ms.genesis).unwrap();
        assert_eq!(named.name, Some(default_name(&ms.address).unwrap()));
        // A name it has stays; the address does not move.
        let own = multisig_genesis(&[A.into(), B.into()], 2, "Team fund", "0x7").unwrap();
        assert_eq!(named_genesis(&own.genesis).unwrap(), own.genesis);
        assert_eq!(genesis_address(&named).unwrap(), ms.address);
    }

    /// `rename(string)` as the registry decodes it (`cast calldata
    /// 'rename(string)' 'Team fund'`), and the multisig's transaction for it.
    #[test]
    fn a_rename_is_the_registrys_call() {
        let call = rename_call("Team fund").unwrap();
        assert_eq!(call.to, GENESIS_REGISTRY);
        assert_eq!(
            call.data,
            include_str!("../tests/vectors/multisig-rename.hex").trim()
        );
        for refused in ["", " Team", "Team\u{202E}dnuf", &"x".repeat(65)] {
            assert!(rename_call(refused).is_err(), "{refused:?}");
        }
        let tx = rename_tx("Team fund", "0x3").unwrap();
        assert_eq!(tx.to, GENESIS_REGISTRY);
        assert_eq!(
            (tx.value.as_str(), tx.operation, tx.nonce.as_str()),
            ("0x0", 0, "0x3")
        );
        assert_eq!(tx.data, call.data);
        assert_eq!(
            read_rename(&unhex(&call.data).unwrap()).as_deref(),
            Some("Team fund")
        );
    }

    /// A rename reads as one, for every member, alone or in a batch — and only
    /// with a name Vela shows in full and an encoding with nothing hidden in it.
    #[test]
    fn a_rename_reads_as_a_rename() {
        let me = "0x4851114Fca55f17B1a5fC76BcE6c59107D003359";
        let tx = rename_tx("Team fund", "0x0").unwrap();
        let r = review(100, me, &tx).unwrap();
        assert_eq!(
            r.action,
            Some(Action::Rename {
                name: "Team fund".into()
            })
        );
        let json = serde_json::to_value(&r.action).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"kind": "rename", "name": "Team fund"})
        );

        let raw = |name: &str| {
            let mut d = selector("rename(string)").to_vec();
            d.extend_from_slice(&word_usize(0x20));
            d.extend(bytes_tail(name.as_bytes()));
            SafeTx {
                data: hex(&d),
                ..tx.clone()
            }
        };
        // A name that hides or turns text around: refused, not read as a plain call.
        for hidden in ["Team\u{202E}dnuf", "Team\u{200B}fund", " Team"] {
            let r = review(100, me, &raw(hidden)).unwrap();
            assert_eq!(r.action, None, "{hidden:?}");
            assert!(r.refusal.unwrap().contains("cannot show"));
        }
        // Trailing bytes after the name: not canonical, refused.
        let mut padded = unhex(&tx.data).unwrap();
        padded.extend_from_slice(&[0u8; 32]);
        let r = review(
            100,
            me,
            &SafeTx {
                data: hex(&padded),
                ..tx.clone()
            },
        )
        .unwrap();
        assert_eq!(r.action, None);
        // Coins to the registry: refused.
        let r = review(
            100,
            me,
            &SafeTx {
                value: "0x1".into(),
                ..tx.clone()
            },
        )
        .unwrap();
        assert_eq!(r.action, None);
        // Anything else to the registry is an ordinary call (a Safe recording itself).
        let record =
            record_call(&salted(&[A.into(), B.into()], 2, "0x7").unwrap().genesis).unwrap();
        let r = review(
            100,
            me,
            &SafeTx {
                data: record.data.clone(),
                ..tx.clone()
            },
        )
        .unwrap();
        assert!(matches!(r.action, Some(Action::Call { .. })));
        // In a batch, the rename leg reads as one too.
        let batch = calls_tx(
            &[
                CallArg {
                    to: tx.to.clone(),
                    value: "0x0".into(),
                    data: tx.data.clone(),
                },
                CallArg {
                    to: B.into(),
                    value: "0x1".into(),
                    data: "0x".into(),
                },
            ],
            "0x0",
        )
        .unwrap();
        let Some(Action::Batch { calls }) = review(100, me, &batch).unwrap().action else {
            panic!("a batch");
        };
        assert_eq!(
            calls[0].action,
            Action::Rename {
                name: "Team fund".into()
            }
        );
        let hidden_leg = calls_tx(
            &[
                CallArg {
                    to: tx.to.clone(),
                    value: "0x0".into(),
                    data: raw("Team\u{202E}x").data,
                },
                CallArg {
                    to: B.into(),
                    value: "0x1".into(),
                    data: "0x".into(),
                },
            ],
            "0x0",
        )
        .unwrap();
        assert_eq!(review(100, me, &hidden_leg).unwrap().action, None);
    }

    /// `genesisOf` as registry v4 answered it on a Gnosis fork for the record
    /// of `multisig-record-v4.hex`, written by anvil's account #0.
    #[test]
    fn a_genesis_of_answer_is_read_and_re_derived() {
        assert_eq!(
            genesis_of_call("0x4851114Fca55f17B1a5fC76BcE6c59107D003359").unwrap(),
            "0xf6913a5d0000000000000000000000004851114fca55f17b1a5fc76bce6c59107d003359"
        );
        let ret = include_str!("../tests/vectors/multisig-genesis-of.hex").trim();
        let ms = multisig_genesis(&[A.into(), B.into()], 2, "Team fund", "0x7").unwrap();
        assert_eq!(ms.address, "0x4851114Fca55f17B1a5fC76BcE6c59107D003359");
        let r = read_genesis_of(&ms.address, ret).unwrap().unwrap();
        assert_eq!(r.genesis, ms.genesis);
        assert_eq!(r.recorder, "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        // The fourth record on that fork (index 3), never renamed.
        assert_eq!((r.index, r.renamed_at), (3, 0));
        assert!(r.recorded_at > 0);
        assert!(!r.name_by_safe, "the first writer named it, not the Safe");
        // The same answer for another address is no registry's.
        assert!(read_genesis_of(B, ret).is_err());
        // Not recorded.
        let none = format!("0x{}{}", "0".repeat(64), "0".repeat(62) + "40");
        let mut none = none;
        none.push_str(&"0".repeat(64 * 10));
        assert_eq!(read_genesis_of(&ms.address, &none).unwrap(), None);
        // Truncated or garbled: an error, never a record.
        assert!(read_genesis_of(&ms.address, &ret[..ret.len() - 64]).is_err());
        assert!(read_genesis_of(&ms.address, "0x").is_err());
        assert!(read_genesis_of(&ms.address, &format!("0x{}", "f".repeat(64))).is_err());
    }

    /// The name is the Safe's own once it renamed itself, or when it wrote
    /// the record itself.
    #[test]
    fn a_name_is_the_safes_own_after_a_rename() {
        let ret = include_str!("../tests/vectors/multisig-genesis-of.hex").trim();
        let me = "0x4851114Fca55f17B1a5fC76BcE6c59107D003359";
        let mut bytes = unhex(ret).unwrap();
        // renamedAt is the tuple's third word; the tuple starts at 0x40.
        let at = 0x40 + 2 * 32;
        bytes[at + 31] = 9;
        let renamed = read_genesis_of(me, &hex(&bytes)).unwrap().unwrap();
        assert_eq!(renamed.renamed_at, 9);
        assert!(renamed.name_by_safe);
        let mut bytes = unhex(ret).unwrap();
        let recorder = 0x40 + 6 * 32;
        bytes[recorder + 12..recorder + 32].copy_from_slice(addr(me).unwrap().as_slice());
        assert!(
            read_genesis_of(me, &hex(&bytes))
                .unwrap()
                .unwrap()
                .name_by_safe
        );
    }

    /// A creation buried in other calldata (here: a 4337 init code — factory
    /// address, then the call — inside an outer call) is found and read.
    #[test]
    fn a_genesis_is_read_out_of_creating_calldata() {
        let ms = salted(&[A.into(), B.into()], 2, "0x7").unwrap();
        let call = deploy_call(&ms.genesis).unwrap();
        let mut outer = selector("handleOps((address)[],address)").to_vec();
        outer.extend_from_slice(&[0u8; 37]);
        outer.extend_from_slice(addr(&ms.genesis.factory).unwrap().as_slice());
        outer.extend(unhex(&call.data).unwrap());
        outer.extend_from_slice(&[0u8; 11]);
        let found = genesis_from_calldata(&hex(&outer), &ms.address).unwrap();
        assert_eq!(found, Some(ms.genesis.clone()));
        // Another Safe's address finds nothing in it.
        assert_eq!(genesis_from_calldata(&hex(&outer), B).unwrap(), None);
        assert_eq!(genesis_from_calldata("0x1234", &ms.address).unwrap(), None);
    }

    // -- who can act for a Safe (§3.5) -------------------------------------------------

    /// A canonical contract's runtime code, as `eth_getCode` answered it on
    /// Gnosis on 2026-10-29 (`tests/vectors/safe-code/`).
    fn safe_code(name: &str) -> String {
        let code = match name {
            "safe-proxy-factory-1.4.1" => {
                include_str!("../tests/vectors/safe-code/safe-proxy-factory-1.4.1.hex")
            }
            "safe-1.4.1" => include_str!("../tests/vectors/safe-code/safe-1.4.1.hex"),
            "safe-l2-1.4.1" => include_str!("../tests/vectors/safe-code/safe-l2-1.4.1.hex"),
            "compatibility-fallback-handler-1.4.1" => {
                include_str!("../tests/vectors/safe-code/compatibility-fallback-handler-1.4.1.hex")
            }
            "safe-to-l2-setup-1.4.1" => {
                include_str!("../tests/vectors/safe-code/safe-to-l2-setup-1.4.1.hex")
            }
            other => panic!("no code kept for {other}"),
        };
        code.trim().to_owned()
    }

    const SAFE_141: &str = "0x41675C099F32341bf84BFc5382aF534df5C7461a";
    const TO_L2_SETUP_141: &str = "0xBD89A1CE4DDe368FFAB0eC35506eEcE0b1fFdc54";
    const HANDLER_130: &str = "0xf48f2B2d2a534e402487b3ee7C18c33Aec0Fe5e4";

    /// Gnosis's answers for every Safe v1.4.1 contract a multisig may depend on.
    fn safe_141_facts() -> ChainFacts {
        ChainFacts {
            codes: [
                (safe::SAFE_PROXY_FACTORY, "safe-proxy-factory-1.4.1"),
                (SAFE_141, "safe-1.4.1"),
                (safe::SAFE_SINGLETON, "safe-l2-1.4.1"),
                (
                    COMPATIBILITY_FALLBACK_HANDLER,
                    "compatibility-fallback-handler-1.4.1",
                ),
                (TO_L2_SETUP_141, "safe-to-l2-setup-1.4.1"),
            ]
            .into_iter()
            .map(|(a, n)| (a.to_lowercase(), Some(safe_code(n))))
            .collect(),
        }
    }

    /// A genesis of `setup` through the v1.4.1 factory, with its address.
    fn setup_genesis(singleton: &str, a: &SetupArgs) -> (String, Genesis) {
        let g = Genesis {
            factory: safe::SAFE_PROXY_FACTORY.into(),
            singleton: singleton.into(),
            salt_nonce: "0x7".into(),
            initializer: hex(&encode_setup_full(a)),
            name: None,
        };
        (genesis_address(&g).unwrap(), g)
    }

    fn plain_setup() -> SetupArgs {
        SetupArgs {
            owners: vec![addr(A).unwrap(), addr(B).unwrap()],
            threshold: U256::from(2),
            to: Address::ZERO,
            data: Vec::new(),
            fallback_handler: addr(COMPATIBILITY_FALLBACK_HANDLER).unwrap(),
            payment_token: Address::ZERO,
            payment: U256::ZERO,
            payment_receiver: Address::ZERO,
        }
    }

    fn refusals(g: &Genesis, safe: &str) -> Vec<Refusal> {
        match counterfactual_authority(g, safe, &safe_141_facts()).unwrap() {
            Authority::Refused { reasons } => reasons,
            other => panic!("not refused: {other:?}"),
        }
    }

    /// The pinned hashes are the code's: recomputed from the code read on
    /// Gnosis (kept as vectors), and the table names every factory and
    /// singleton an address may be derived with — no fewer, no more.
    #[test]
    fn the_canonical_code_hashes_are_the_codes() {
        for (address, name) in [
            (safe::SAFE_PROXY_FACTORY, "safe-proxy-factory-1.4.1"),
            (SAFE_141, "safe-1.4.1"),
            (safe::SAFE_SINGLETON, "safe-l2-1.4.1"),
            (
                COMPATIBILITY_FALLBACK_HANDLER,
                "compatibility-fallback-handler-1.4.1",
            ),
            (TO_L2_SETUP_141, "safe-to-l2-setup-1.4.1"),
        ] {
            let c = canonical_at(addr(address).unwrap()).unwrap();
            assert_eq!(
                hex(keccak256(unhex(&safe_code(name)).unwrap()).as_slice()),
                c.code_hash,
                "{name}"
            );
        }
        let roles = |roles: &[Role]| -> Vec<Address> {
            let mut v: Vec<Address> = CANONICAL
                .iter()
                .filter(|c| roles.contains(&c.role))
                .map(|c| addr(c.address).unwrap())
                .collect();
            v.sort();
            v
        };
        let mut factories: Vec<Address> = KNOWN_FACTORIES
            .iter()
            .map(|(f, _)| addr(f).unwrap())
            .collect();
        factories.sort();
        assert_eq!(roles(&[Role::Factory]), factories);
        let mut singletons: Vec<Address> =
            KNOWN_SINGLETONS.iter().map(|s| addr(s).unwrap()).collect();
        singletons.sort();
        assert_eq!(roles(&[Role::Singleton, Role::SingletonL2]), singletons);
        for c in &CANONICAL {
            assert_eq!(c.code_hash.len(), 66, "{}", c.name);
            assert_eq!(addr(c.address).unwrap().to_checksum(None), c.address);
        }
        // The slots are Safe's.
        assert_eq!(
            hex(keccak256("guard_manager.guard.address").as_slice()),
            GUARD_STORAGE_SLOT
        );
        assert_eq!(
            hex(keccak256("module_manager.module_guard.address").as_slice()),
            MODULE_GUARD_STORAGE_SLOT
        );
        assert_eq!(
            hex(keccak256("fallback_manager.handler.address").as_slice()),
            FALLBACK_HANDLER_STORAGE_SLOT
        );
    }

    /// The multisig Vela creates is Verified wherever Safe's code is: its
    /// genesis's owners and threshold, nothing else — and the code it needs is
    /// what the wallet is told to read.
    #[test]
    fn a_vela_genesis_is_verified_where_safe_code_is() {
        let ms = multisig_genesis(&[A.into(), B.into()], 2, "Team fund", "0x7").unwrap();
        assert_eq!(
            counterfactual_authority(&ms.genesis, &ms.address, &safe_141_facts()).unwrap(),
            Authority::Verified {
                owners: ms.owners.clone(),
                threshold: 2
            }
        );
        assert_eq!(
            counterfactual_reads(&ms.genesis, &ms.address).unwrap(),
            vec![
                safe::SAFE_PROXY_FACTORY.to_owned(),
                safe::SAFE_SINGLETON.to_owned(),
                COMPATIBILITY_FALLBACK_HANDLER.to_owned()
            ]
        );
        // No handler at all is a plain Safe too.
        let (address, g) = setup_genesis(
            safe::SAFE_SINGLETON,
            &SetupArgs {
                fallback_handler: Address::ZERO,
                ..plain_setup()
            },
        );
        assert!(matches!(
            counterfactual_authority(&g, &address, &safe_141_facts()).unwrap(),
            Authority::Verified { .. }
        ));
        // Another address for the same genesis: refused before anything else.
        assert_eq!(refusals(&ms.genesis, B), vec![Refusal::WrongAddress]);
    }

    /// The chain decides the code: none there, code that is not Safe's, or no
    /// answer — never Verified.
    #[test]
    fn safe_code_must_be_on_the_chain() {
        let ms = multisig_genesis(&[A.into(), B.into()], 2, "Team fund", "0x7").unwrap();
        let handler = COMPATIBILITY_FALLBACK_HANDLER.to_lowercase();
        let mut facts = safe_141_facts();
        facts.codes.insert(handler.clone(), Some("0x".into()));
        assert_eq!(
            counterfactual_authority(&ms.genesis, &ms.address, &facts).unwrap(),
            Authority::Refused {
                reasons: vec![Refusal::MissingCode {
                    name: "Compatibility Fallback Handler 1.4.1".into(),
                    address: COMPATIBILITY_FALLBACK_HANDLER.into()
                }]
            }
        );
        facts
            .codes
            .insert(handler.clone(), Some("0x6080604052".into()));
        assert_eq!(
            counterfactual_authority(&ms.genesis, &ms.address, &facts).unwrap(),
            Authority::Refused {
                reasons: vec![Refusal::WrongCode {
                    name: "Compatibility Fallback Handler 1.4.1".into(),
                    address: COMPATIBILITY_FALLBACK_HANDLER.into()
                }]
            }
        );
        // Unanswered, or never asked: it cannot be verified yet.
        facts.codes.insert(handler.clone(), None);
        assert_eq!(
            counterfactual_authority(&ms.genesis, &ms.address, &facts).unwrap(),
            Authority::Unknown {
                unread: vec![COMPATIBILITY_FALLBACK_HANDLER.into()]
            }
        );
        assert!(matches!(
            counterfactual_authority(&ms.genesis, &ms.address, &ChainFacts::default()).unwrap(),
            Authority::Unknown { unread } if unread.len() == 3
        ));
    }

    /// The factory and singleton must be Safe's own deployments.
    #[test]
    fn a_factory_or_singleton_vela_does_not_know_is_refused() {
        let ms = multisig_genesis(&[A.into(), B.into()], 2, "Team fund", "0x7").unwrap();
        let attacker = "0x9813461E88aDD60D0eC0943B74A83368CA2E36D1";
        let g = Genesis {
            factory: attacker.into(),
            ..ms.genesis.clone()
        };
        assert_eq!(
            refusals(&g, &ms.address),
            vec![Refusal::UnknownFactory {
                address: attacker.into()
            }]
        );
        let g = Genesis {
            singleton: attacker.into(),
            ..ms.genesis.clone()
        };
        assert_eq!(
            refusals(&g, &ms.address),
            vec![Refusal::UnknownSingleton {
                address: attacker.into()
            }]
        );
    }

    /// Safe{Wallet}'s v1.4.1 genesis: the L1 singleton with
    /// `SafeToL2Setup.setupToL2(SafeL2)` — allowed, decoded in full, its
    /// hook and the L2 singleton it switches to checked on the chain. Any
    /// other target for it, or another release's, is code Vela cannot read.
    #[test]
    fn safe_to_l2_setup_is_the_one_hook_allowed() {
        let mut to_l2 = selector("setupToL2(address)").to_vec();
        to_l2.extend_from_slice(&word_addr(addr(safe::SAFE_SINGLETON).unwrap()));
        let hooked = SetupArgs {
            to: addr(TO_L2_SETUP_141).unwrap(),
            data: to_l2.clone(),
            ..plain_setup()
        };
        let (address, g) = setup_genesis(SAFE_141, &hooked);
        assert_eq!(
            counterfactual_authority(&g, &address, &safe_141_facts()).unwrap(),
            Authority::Verified {
                owners: vec![
                    addr(A).unwrap().to_checksum(None),
                    addr(B).unwrap().to_checksum(None)
                ],
                threshold: 2
            }
        );
        assert_eq!(
            counterfactual_reads(&g, &address).unwrap(),
            vec![
                safe::SAFE_PROXY_FACTORY.to_owned(),
                SAFE_141.to_owned(),
                TO_L2_SETUP_141.to_owned(),
                safe::SAFE_SINGLETON.to_owned(),
                COMPATIBILITY_FALLBACK_HANDLER.to_owned()
            ]
        );
        // Its own code is checked on the chain too.
        let mut facts = safe_141_facts();
        facts.codes.remove(&TO_L2_SETUP_141.to_lowercase());
        assert_eq!(
            counterfactual_authority(&g, &address, &facts).unwrap(),
            Authority::Unknown {
                unread: vec![TO_L2_SETUP_141.into()]
            }
        );
        // Switching to anything but Safe's L2 singleton of its release.
        for target in ["0x9813461E88aDD60D0eC0943B74A83368CA2E36D1", SAFE_141] {
            let mut data = selector("setupToL2(address)").to_vec();
            data.extend_from_slice(&word_addr(addr(target).unwrap()));
            let (address, g) = setup_genesis(
                SAFE_141,
                &SetupArgs {
                    data,
                    ..hooked.clone()
                },
            );
            assert_eq!(
                refusals(&g, &address),
                vec![Refusal::SetupCode {
                    address: TO_L2_SETUP_141.into()
                }]
            );
        }
        // Anything after the call, or another function of it.
        let mut longer = to_l2.clone();
        longer.extend_from_slice(&[0u8; 32]);
        let (address, g) = setup_genesis(
            SAFE_141,
            &SetupArgs {
                data: longer,
                ..hooked.clone()
            },
        );
        assert!(matches!(
            refusals(&g, &address)[..],
            [Refusal::SetupCode { .. }]
        ));
        // v1.4.1's hook under a v1.5.0 singleton.
        let (address, g) = setup_genesis(
            "0xFf51A5898e281Db6DfC7855790607438dF2ca44b",
            &SetupArgs {
                fallback_handler: Address::ZERO,
                ..hooked.clone()
            },
        );
        assert!(matches!(
            refusals(&g, &address)[..],
            [Refusal::SetupCode { .. }]
        ));
    }

    /// A module the setup enables, or code it runs, is said for what it is.
    #[test]
    fn a_setup_that_enables_a_module_or_runs_code_is_refused() {
        let module = addr("0x9813461E88aDD60D0eC0943B74A83368CA2E36D1").unwrap();
        let mut enable = selector("enableModules(address[])").to_vec();
        enable.extend_from_slice(&word_usize(0x20));
        enable.extend_from_slice(&word_usize(1));
        enable.extend_from_slice(&word_addr(module));
        for setup in MODULE_SETUPS {
            let (address, g) = setup_genesis(
                safe::SAFE_SINGLETON,
                &SetupArgs {
                    to: addr(setup).unwrap(),
                    data: enable.clone(),
                    ..plain_setup()
                },
            );
            assert_eq!(
                refusals(&g, &address),
                vec![Refusal::Module {
                    address: module.to_checksum(None)
                }]
            );
        }
        // The same call to a contract that is no Safe module setup: code at it.
        let (address, g) = setup_genesis(
            safe::SAFE_SINGLETON,
            &SetupArgs {
                to: module,
                data: enable,
                ..plain_setup()
            },
        );
        assert_eq!(
            refusals(&g, &address),
            vec![Refusal::SetupCode {
                address: module.to_checksum(None)
            }]
        );
        // A target with no data still runs its code; data with no target is
        // not Safe's plain setup.
        let (address, g) = setup_genesis(
            safe::SAFE_SINGLETON,
            &SetupArgs {
                to: module,
                ..plain_setup()
            },
        );
        assert!(matches!(
            refusals(&g, &address)[..],
            [Refusal::SetupCode { .. }]
        ));
        let (address, g) = setup_genesis(
            safe::SAFE_SINGLETON,
            &SetupArgs {
                data: vec![1, 2, 3, 4],
                ..plain_setup()
            },
        );
        assert_eq!(refusals(&g, &address), vec![Refusal::NotSetup]);
    }

    /// The handler must be that release's CompatibilityFallbackHandler, and
    /// nothing may be paid out on deployment.
    #[test]
    fn a_strange_handler_or_a_payment_is_refused() {
        let attacker = addr("0x9813461E88aDD60D0eC0943B74A83368CA2E36D1").unwrap();
        for handler in [attacker, addr(HANDLER_130).unwrap()] {
            let (address, g) = setup_genesis(
                safe::SAFE_SINGLETON,
                &SetupArgs {
                    fallback_handler: handler,
                    ..plain_setup()
                },
            );
            assert_eq!(
                refusals(&g, &address),
                vec![Refusal::Handler {
                    address: handler.to_checksum(None)
                }]
            );
        }
        let (address, g) = setup_genesis(
            safe::SAFE_SINGLETON,
            &SetupArgs {
                payment: U256::from(5),
                payment_receiver: attacker,
                ..plain_setup()
            },
        );
        assert_eq!(
            refusals(&g, &address),
            vec![Refusal::Payment {
                token: Address::ZERO.to_checksum(None),
                amount: "5".into(),
                receiver: attacker.to_checksum(None)
            }]
        );
        // A receiver alone (a zero payment) is still not Safe's plain setup.
        let (address, g) = setup_genesis(
            safe::SAFE_SINGLETON,
            &SetupArgs {
                payment_receiver: attacker,
                ..plain_setup()
            },
        );
        assert!(matches!(
            refusals(&g, &address)[..],
            [Refusal::Payment { .. }]
        ));
    }

    /// Members Safe refuses, and a setup not in Safe's own encoding.
    #[test]
    fn members_safe_refuses_and_odd_encodings_are_refused() {
        for (owners, threshold) in [
            (vec![A, A], 1u64),
            (vec![A, B], 3),
            (vec![A, B], 0),
            (vec!["0x0000000000000000000000000000000000000001"], 1),
        ] {
            let (address, g) = setup_genesis(
                safe::SAFE_SINGLETON,
                &SetupArgs {
                    owners: owners.iter().map(|o| addr(o).unwrap()).collect(),
                    threshold: U256::from(threshold),
                    ..plain_setup()
                },
            );
            assert_eq!(refusals(&g, &address), vec![Refusal::BadMembers]);
        }
        // A word after the encoded call: the same fields, other bytes.
        let ms = multisig_genesis(&[A.into(), B.into()], 2, "Team fund", "0x7").unwrap();
        let g = Genesis {
            initializer: format!("{}{}", ms.genesis.initializer, "00".repeat(32)),
            ..ms.genesis.clone()
        };
        let address = genesis_address(&g).unwrap();
        assert_eq!(refusals(&g, &address), vec![Refusal::NotSetup]);
        let g = Genesis {
            initializer: "0x12345678".into(),
            ..ms.genesis
        };
        let address = genesis_address(&g).unwrap();
        assert_eq!(refusals(&g, &address), vec![Refusal::NotSetup]);
    }

    /// The tester's payloads (r4-e): the clean one passes, every other is
    /// refused with what it does — among them the genesis of a Safe really
    /// deployed on Arc Testnet with the attacker's module enabled.
    #[test]
    fn the_hostile_geneses_are_refused_for_what_they_do() {
        let file: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/vectors/multisig/hostile-geneses.json"
        ))
        .unwrap();
        let attacker = file["attacker"].as_str().unwrap().to_owned();
        let verdict = |name: &str| {
            let row = file["geneses"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["name"] == name)
                .unwrap();
            let g = Genesis {
                factory: file["factory"].as_str().unwrap().into(),
                singleton: file["singleton"].as_str().unwrap().into(),
                salt_nonce: row["saltNonce"].as_str().unwrap().into(),
                initializer: row["initializer"].as_str().unwrap().into(),
                name: None,
            };
            let address = genesis_address(&g).unwrap();
            if let Some(a) = row["address"].as_str() {
                assert_eq!(address, a);
            }
            counterfactual_authority(&g, &address, &safe_141_facts()).unwrap()
        };
        assert!(matches!(
            verdict("CLEAN"),
            Authority::Verified { threshold: 1, .. }
        ));
        let refused = |reasons: Vec<Refusal>| Authority::Refused { reasons };
        assert_eq!(
            verdict("HOOK_MODULE"),
            refused(vec![Refusal::Module {
                address: attacker.clone()
            }])
        );
        for name in ["HOOK", "ADDOWNER_HOOK"] {
            assert_eq!(
                verdict(name),
                refused(vec![Refusal::SetupCode {
                    address: attacker.clone()
                }])
            );
        }
        assert_eq!(
            verdict("BADHANDLER"),
            refused(vec![Refusal::Handler {
                address: attacker.clone()
            }])
        );
        assert!(matches!(
            verdict("PAYR"),
            Authority::Refused { reasons } if matches!(&reasons[..], [Refusal::Payment { receiver, .. }] if *receiver == attacker)
        ));
    }

    /// The executor deploys a multisig only from a genesis Verified on its
    /// chain, read just now — a hooked one never, nor one with no code read.
    #[test]
    fn the_executor_deploys_only_a_verified_genesis() {
        let file: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/vectors/multisig/hostile-geneses.json"
        ))
        .unwrap();
        let row = &file["geneses"][5];
        let hooked = Genesis {
            factory: file["factory"].as_str().unwrap().into(),
            singleton: file["singleton"].as_str().unwrap().into(),
            salt_nonce: row["saltNonce"].as_str().unwrap().into(),
            initializer: row["initializer"].as_str().unwrap().into(),
            name: None,
        };
        let ms = salted(&[A.into(), B.into()], 1, "0x7").unwrap();
        let tx = cancel_tx(&ms.address, "0x0").unwrap();
        let plan = |multisig: &str, g: &Genesis, facts: Option<ChainFacts>| ExecutionPlan {
            multisig: multisig.into(),
            deploy_multisig: Some(g.clone()),
            deploy_owners: vec![],
            tx: tx.clone(),
            approvals: vec![],
            executor: Some(A.into()),
            deploy_facts: facts,
        };
        let calls =
            execution_calls(&plan(&ms.address, &ms.genesis, Some(safe_141_facts()))).unwrap();
        assert_eq!(calls[0], deploy_call(&ms.genesis).unwrap());
        assert!(execution_calls(&plan(&ms.address, &ms.genesis, None)).is_err());
        assert!(
            execution_calls(&plan(&ms.address, &ms.genesis, Some(ChainFacts::default()))).is_err()
        );
        let error = execution_calls(&plan(
            "0x2F53cC750594bB97c21Bf3c18A4837bb5Ad911f6",
            &hooked,
            Some(safe_141_facts()),
        ))
        .unwrap_err();
        assert!(error.to_string().contains("module"), "{error}");
        // Already deployed: nothing to deploy, nothing to check here.
        let deployed = ExecutionPlan {
            deploy_multisig: None,
            ..plan(&ms.address, &ms.genesis, None)
        };
        assert_eq!(execution_calls(&deployed).unwrap().len(), 1);
    }

    fn word_of(a: &str) -> String {
        hex(&word_addr(addr(a).unwrap()))
    }

    fn modules_answer(modules: &[&str], next: &str) -> String {
        let mut out = word_usize(0x40).to_vec();
        out.extend_from_slice(&word_addr(addr(next).unwrap()));
        out.extend_from_slice(&word_usize(modules.len()));
        for m in modules {
            out.extend_from_slice(&word_addr(addr(m).unwrap()));
        }
        hex(&out)
    }

    /// A deployed Safe: the chain says what can act for it besides its
    /// members — a module, a guard, a handler or singleton that is not Safe's.
    #[test]
    fn a_deployed_safe_shows_what_else_can_act_for_it() {
        let zero = hex(&[0u8; 32]);
        let sentinel = "0x0000000000000000000000000000000000000001";
        let clean = DeployedFacts {
            modules: Some(modules_answer(&[], sentinel)),
            guard: Some(zero.clone()),
            module_guard: Some("0x".into()),
            handler: Some(word_of(COMPATIBILITY_FALLBACK_HANDLER)),
            singleton: Some(word_of(safe::SAFE_SINGLETON)),
            codes: BTreeMap::new(),
        };
        // First the storage, then the code of what it names.
        let first = deployed_check(&clean);
        assert_eq!(
            first,
            DeployedCheck {
                risks: vec![],
                need_code: vec![
                    COMPATIBILITY_FALLBACK_HANDLER.into(),
                    safe::SAFE_SINGLETON.into()
                ],
                unread: false
            }
        );
        let read = DeployedFacts {
            codes: safe_141_facts().codes,
            ..clean.clone()
        };
        assert_eq!(deployed_check(&read), DeployedCheck::default());
        // No handler at all is fine.
        let no_handler = DeployedFacts {
            handler: Some(zero.clone()),
            ..read.clone()
        };
        assert_eq!(deployed_check(&no_handler), DeployedCheck::default());

        let attacker = "0x9813461E88aDD60D0eC0943B74A83368CA2E36D1";
        let other = "0x2222222222222222222222222222222222222222";
        let risky = DeployedFacts {
            modules: Some(modules_answer(&[attacker], other)),
            guard: Some(word_of(other)),
            handler: Some(word_of(attacker)),
            singleton: Some(word_of(attacker)),
            ..read.clone()
        };
        assert_eq!(
            deployed_check(&risky).risks,
            vec![
                Risk::Module {
                    address: attacker.into()
                },
                // The page's `next`: another module, past the page.
                Risk::Module {
                    address: addr(other).unwrap().to_checksum(None)
                },
                Risk::Guard {
                    address: addr(other).unwrap().to_checksum(None)
                },
                Risk::Handler {
                    address: attacker.into()
                },
                Risk::Singleton {
                    address: attacker.into()
                },
            ]
        );
        // Safe's addresses holding other code are not Safe's contracts.
        let mut codes = safe_141_facts().codes;
        codes.insert(safe::SAFE_SINGLETON.to_lowercase(), Some("0x6080".into()));
        assert_eq!(
            deployed_check(&DeployedFacts {
                codes,
                ..read.clone()
            })
            .risks,
            vec![Risk::Singleton {
                address: safe::SAFE_SINGLETON.into()
            }]
        );
        // A read the chain did not answer: nothing said yet.
        assert!(
            deployed_check(&DeployedFacts {
                modules: None,
                ..read.clone()
            })
            .unread
        );
        let mut codes = safe_141_facts().codes;
        codes.insert(safe::SAFE_SINGLETON.to_lowercase(), None);
        assert!(deployed_check(&DeployedFacts { codes, ..read }).unread);
    }

    /// The door: the reads, and the verdicts as the shells take them.
    #[test]
    fn the_authority_answers_through_the_door() {
        let reads: serde_json::Value =
            serde_json::from_str(&call_json("deployedReads", "{}").unwrap()).unwrap();
        assert_eq!(
            reads["modulesCall"],
            format!(
                "0xcc2f8452{}{}",
                "0".repeat(63) + "1",
                "0".repeat(62) + "0a"
            )
        );
        let ms = multisig_genesis(&[A.into(), B.into()], 2, "Team fund", "0x7").unwrap();
        let facts = serde_json::json!({"codes": safe_141_facts().codes});
        let answer: serde_json::Value = serde_json::from_str(
            &call_json(
                "counterfactualAuthority",
                &serde_json::json!({"genesis": ms.genesis, "safe": ms.address, "facts": facts})
                    .to_string(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            answer,
            serde_json::json!({"kind": "verified", "owners": ms.owners, "threshold": 2})
        );
        let answer: serde_json::Value = serde_json::from_str(
            &call_json(
                "counterfactualAuthority",
                &serde_json::json!({"genesis": ms.genesis, "safe": B}).to_string(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            answer,
            serde_json::json!({"kind": "refused", "reasons": [{"kind": "wrongAddress"}]})
        );
        let check: serde_json::Value =
            serde_json::from_str(&call_json("deployedCheck", r#"{"facts":{}}"#).unwrap()).unwrap();
        assert_eq!(check["unread"], true);
    }

    // -- changing the members ---------------------------------------------------

    const C: &str = "0x3333333333333333333333333333333333333333";
    const D: &str = "0x4444444444444444444444444444444444444444";

    fn members(owners: &[&str], threshold: u64) -> MemberState {
        MemberState {
            owners: owners.iter().map(|o| (*o).to_owned()).collect(),
            threshold,
            change_waiting: false,
        }
    }

    fn ready(ms: &str, state: &MemberState, change: MemberChange) -> (SafeTx, MemberSet) {
        match member_change_tx(ms, state, &change, "0x5").unwrap() {
            MemberVerdict::Ready { tx, before, after } => {
                assert_eq!(before.threshold, state.threshold);
                assert_eq!(before.owners.len(), state.owners.len());
                (*tx, after)
            }
            MemberVerdict::Refused { refusal } => panic!("refused: {refusal:?}"),
        }
    }

    fn refused(ms: &str, state: &MemberState, change: MemberChange) -> MemberRefusal {
        match member_change_tx(ms, state, &change, "0x5").unwrap() {
            MemberVerdict::Refused { refusal } => refusal,
            MemberVerdict::Ready { .. } => panic!("built {change:?}"),
        }
    }

    fn checksummed(list: &[&str]) -> Vec<String> {
        list.iter()
            .map(|a| addr(a).unwrap().to_checksum(None))
            .collect()
    }

    /// The owner before another in the Safe's list (`getOwners()` order): the
    /// sentinel before the first.
    #[test]
    fn the_previous_owner_comes_from_the_chains_list() {
        let list = [A, B, C].map(|a| addr(a).unwrap());
        assert_eq!(prev_owner(&list, list[0]), Some(SENTINEL));
        assert_eq!(prev_owner(&list, list[1]), Some(list[0]));
        assert_eq!(prev_owner(&list, list[2]), Some(list[1]));
        assert_eq!(prev_owner(&list, addr(D).unwrap()), None);
    }

    /// Each change's calldata is the Safe's own, byte for byte as `cast
    /// calldata` encodes it, sent to the multisig itself with nothing, at the
    /// nonce given — and the review reads each back as that change.
    #[test]
    fn each_member_change_is_the_safes_call_as_cast_encodes_it() {
        let ms = salted(&[A.into(), B.into()], 2, "0x7").unwrap().address;
        let state = members(&[A, B, C], 2);

        // cast calldata "addOwnerWithThreshold(address,uint256)" D 2
        let (tx, after) = ready(
            &ms,
            &state,
            MemberChange::Add {
                owner: D.into(),
                threshold: None,
            },
        );
        assert_eq!(
            tx.data,
            "0x0d582f1300000000000000000000000044444444444444444444444444444444444444440000000000000000000000000000000000000000000000000000000000000002"
        );
        assert_eq!(
            after.owners,
            checksummed(&[D, A, B, C]),
            "a new owner heads the list"
        );
        assert_eq!(after.threshold, 2);
        assert_eq!(
            (tx.to.as_str(), tx.value.as_str(), tx.operation),
            (ms.as_str(), "0x0", 0)
        );
        assert_eq!(tx.nonce, "0x5");
        assert_eq!(
            review(5042002, &ms, &tx).unwrap().action,
            Some(Action::AddOwner {
                owner: addr(D).unwrap().to_checksum(None),
                threshold: "2".into()
            })
        );

        // cast calldata "removeOwner(address,address,uint256)" A B 2
        let (tx, after) = ready(
            &ms,
            &state,
            MemberChange::Remove {
                owner: B.into(),
                threshold: None,
            },
        );
        assert_eq!(
            tx.data,
            "0xf8dc5dd9000000000000000000000000111111111111111111111111111111111111111100000000000000000000000022222222222222222222222222222222222222220000000000000000000000000000000000000000000000000000000000000002"
        );
        assert_eq!(after.owners, checksummed(&[A, C]));
        assert!(matches!(
            review(5042002, &ms, &tx).unwrap().action,
            Some(Action::RemoveOwner { .. })
        ));

        // The first owner's previous one is the sentinel; the threshold lowered with it.
        // cast calldata "removeOwner(address,address,uint256)" 0x…01 A 1
        let (tx, after) = ready(
            &ms,
            &state,
            MemberChange::Remove {
                owner: A.into(),
                threshold: Some(1),
            },
        );
        assert_eq!(
            tx.data,
            "0xf8dc5dd9000000000000000000000000000000000000000000000000000000000000000100000000000000000000000011111111111111111111111111111111111111110000000000000000000000000000000000000000000000000000000000000001"
        );
        assert_eq!((after.owners, after.threshold), (checksummed(&[B, C]), 1));

        // cast calldata "swapOwner(address,address,address)" B C D
        let (tx, after) = ready(
            &ms,
            &state,
            MemberChange::Replace {
                old_owner: C.into(),
                new_owner: D.into(),
            },
        );
        assert_eq!(
            tx.data,
            "0xe318b52b000000000000000000000000222222222222222222222222222222222222222200000000000000000000000033333333333333333333333333333333333333330000000000000000000000004444444444444444444444444444444444444444"
        );
        assert_eq!(
            after.owners,
            checksummed(&[A, B, D]),
            "in the old one's place"
        );
        assert_eq!(
            review(5042002, &ms, &tx).unwrap().action,
            Some(Action::SwapOwner {
                old_owner: addr(C).unwrap().to_checksum(None),
                new_owner: addr(D).unwrap().to_checksum(None),
            })
        );

        // cast calldata "changeThreshold(uint256)" 3
        let (tx, after) = ready(&ms, &state, MemberChange::Threshold { threshold: 3 });
        assert_eq!(
            tx.data,
            "0x694e80c30000000000000000000000000000000000000000000000000000000000000003"
        );
        assert_eq!((after.owners.len(), after.threshold), (3, 3));
        assert_eq!(
            review(5042002, &ms, &tx).unwrap().action,
            Some(Action::ChangeThreshold {
                threshold: "3".into()
            })
        );
    }

    /// Every state the Safe would refuse, or that would leave the funds stuck,
    /// is refused before anything is signed — with the reason, not an error.
    #[test]
    fn an_impossible_member_change_is_refused_with_its_reason() {
        let ms = salted(&[A.into(), B.into()], 2, "0x7").unwrap().address;
        let state = members(&[A, B, C], 2);
        let add = |owner: &str, threshold: Option<u64>| MemberChange::Add {
            owner: owner.into(),
            threshold,
        };
        assert!(matches!(
            refused(
                &ms,
                &state,
                add("0x0000000000000000000000000000000000000000", None)
            ),
            MemberRefusal::NoOne { .. }
        ));
        assert!(matches!(
            refused(
                &ms,
                &state,
                add("0x0000000000000000000000000000000000000001", None)
            ),
            MemberRefusal::NoOne { .. }
        ));
        assert_eq!(
            refused(&ms, &state, add(&ms, None)),
            MemberRefusal::TheMultisig
        );
        assert_eq!(
            refused(
                &ms,
                &state,
                add(&B.to_uppercase().replace("0X", "0x"), None)
            ),
            MemberRefusal::AlreadyMember {
                owner: addr(B).unwrap().to_checksum(None)
            },
            "the same address in another case is the same member"
        );
        assert!(matches!(
            refused(&ms, &state, add("vitalik.eth", None)),
            MemberRefusal::NotAnAddress { .. }
        ));
        assert_eq!(
            refused(&ms, &state, add(D, Some(5))),
            MemberRefusal::ThresholdTooHigh {
                threshold: 5,
                owners: 4
            }
        );
        assert_eq!(
            refused(&ms, &state, add(D, Some(0))),
            MemberRefusal::ThresholdZero
        );
        let full: Vec<String> = (1..=MAX_OWNERS as u8)
            .map(|i| Address::from([i + 0x10; 20]).to_checksum(None))
            .collect();
        let full = MemberState {
            owners: full,
            threshold: 2,
            change_waiting: false,
        };
        assert_eq!(
            refused(&ms, &full, add(D, None)),
            MemberRefusal::TooMany {
                max: MAX_OWNERS as u64
            }
        );

        let remove = |owner: &str, threshold: Option<u64>| MemberChange::Remove {
            owner: owner.into(),
            threshold,
        };
        assert!(matches!(
            refused(&ms, &state, remove(D, None)),
            MemberRefusal::NotMember { .. }
        ));
        assert_eq!(
            refused(&ms, &members(&[A], 1), remove(A, None)),
            MemberRefusal::LastMember
        );
        // 3 of 3, one removed: two left, three must approve.
        assert_eq!(
            refused(&ms, &members(&[A, B, C], 3), remove(B, None)),
            MemberRefusal::BelowThreshold {
                owners: 2,
                threshold: 3
            }
        );
        assert_eq!(
            refused(&ms, &state, remove(B, Some(3))),
            MemberRefusal::ThresholdTooHigh {
                threshold: 3,
                owners: 2
            }
        );

        assert!(matches!(
            refused(
                &ms,
                &state,
                MemberChange::Replace {
                    old_owner: D.into(),
                    new_owner: "0x5555555555555555555555555555555555555555".into()
                }
            ),
            MemberRefusal::NotMember { .. }
        ));
        assert!(matches!(
            refused(
                &ms,
                &state,
                MemberChange::Replace {
                    old_owner: C.into(),
                    new_owner: A.into()
                }
            ),
            MemberRefusal::AlreadyMember { .. }
        ));
        assert_eq!(
            refused(
                &ms,
                &state,
                MemberChange::Replace {
                    old_owner: C.into(),
                    new_owner: ms.clone()
                }
            ),
            MemberRefusal::TheMultisig
        );

        assert_eq!(
            refused(&ms, &state, MemberChange::Threshold { threshold: 4 }),
            MemberRefusal::ThresholdTooHigh {
                threshold: 4,
                owners: 3
            }
        );
        assert_eq!(
            refused(&ms, &state, MemberChange::Threshold { threshold: 0 }),
            MemberRefusal::ThresholdZero
        );
        assert_eq!(
            refused(&ms, &state, MemberChange::Threshold { threshold: 2 }),
            MemberRefusal::Unchanged
        );

        // One member change at a time on a network.
        let waiting = MemberState {
            change_waiting: true,
            ..state.clone()
        };
        assert_eq!(
            refused(&ms, &waiting, MemberChange::Threshold { threshold: 3 }),
            MemberRefusal::ChangeWaiting
        );
    }

    /// The JSON door speaks the shells' shapes: a verdict tagged by `kind`.
    #[test]
    fn a_member_change_goes_through_the_door() {
        let ms = salted(&[A.into(), B.into()], 2, "0x7").unwrap().address;
        let args = serde_json::json!({
            "multisig": ms,
            "state": { "owners": [A, B, C], "threshold": 2 },
            "change": { "kind": "replace", "oldOwner": C, "newOwner": D },
            "nonce": "0x2"
        });
        let out: serde_json::Value =
            serde_json::from_str(&call_json("memberChangeTx", &args.to_string()).unwrap()).unwrap();
        assert_eq!(out["kind"], "ready");
        assert_eq!(out["tx"]["nonce"], "0x2");
        assert_eq!(out["after"]["threshold"], 2);
        let args = serde_json::json!({
            "multisig": ms,
            "state": { "owners": [A, B, C], "threshold": 2, "changeWaiting": false },
            "change": { "kind": "threshold", "threshold": 9 },
            "nonce": "0x2"
        });
        let out: serde_json::Value =
            serde_json::from_str(&call_json("memberChangeTx", &args.to_string()).unwrap()).unwrap();
        assert_eq!(out["kind"], "refused");
        assert_eq!(out["refusal"]["kind"], "thresholdTooHigh");
        assert_eq!(out["refusal"]["owners"], 3);
    }
}
