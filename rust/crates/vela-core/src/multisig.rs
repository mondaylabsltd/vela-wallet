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

use alloy_primitives::{keccak256, Address, B256, U256};
use serde::{Deserialize, Serialize};

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
    /// A name written with the record (the registry's `recordNamed`): a
    /// label, not part of the address, never changed, not unique.
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

/// One owner's approval, either kind — the genesis service's `Approval`; the
/// two are told apart by their fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Approval {
    Passkey(Box<PasskeyApproval>),
    Ecdsa(EcdsaApproval),
}

impl Approval {
    pub fn owner(&self) -> &str {
        match self {
            Self::Passkey(a) => &a.owner,
            Self::Ecdsa(a) => &a.owner,
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

/// The genesis registry (v2): the same address on every chain.
pub const GENESIS_REGISTRY: &str = "0x70b35C0FC8885019ff938542CD4E431725874802";

/// The registry call that records `g` on any chain — `recordNamed` with its
/// name, else `record` — with its factory's proxy creation code (the
/// registry re-derives the address from it). Anyone may send it.
pub fn record_call(g: &Genesis) -> Result<Call, CoreError> {
    let factory = addr(&g.factory)?;
    let code = KNOWN_FACTORIES
        .iter()
        .find(|(f, _)| f.parse::<Address>().ok() == Some(factory))
        .map(|(_, c)| unhex(c))
        .transpose()?
        .ok_or_else(|| err("not a proxy factory this wallet knows"))?;
    let init = unhex(&g.initializer)?;
    let name = g.name.as_deref().filter(|n| !n.is_empty());
    let mut data = selector(if name.is_some() {
        "recordNamed(address,bytes,address,bytes,uint256,string)"
    } else {
        "record(address,bytes,address,bytes,uint256)"
    })
    .to_vec();
    let head_words = if name.is_some() { 6 } else { 5 };
    let code_tail = bytes_tail(&code);
    let init_tail = bytes_tail(&init);
    data.extend_from_slice(&word_addr(factory));
    data.extend_from_slice(&word_usize(head_words * 32)); // proxyCreationCode offset
    data.extend_from_slice(&word_addr(addr(&g.singleton)?));
    data.extend_from_slice(&word_usize(head_words * 32 + code_tail.len())); // initializer offset
    data.extend_from_slice(&word_u256(quantity(&g.salt_nonce)?));
    if name.is_some() {
        data.extend_from_slice(&word_usize(
            head_words * 32 + code_tail.len() + init_tail.len(),
        ));
    }
    data.extend(code_tail);
    data.extend(init_tail);
    if let Some(n) = name {
        data.extend(bytes_tail(n.as_bytes()));
    }
    Ok(Call {
        to: addr(GENESIS_REGISTRY)?.to_checksum(None),
        value: "0x0".into(),
        data: hex(&data),
    })
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

/// The safeTxHash of `tx` for `safe` on `chain_id`.
pub fn safe_tx_hash(chain_id: u64, safe: &str, tx: &SafeTx) -> Result<String, CoreError> {
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
    Ok(hex(eip712(
        domain_separator(chain_id, addr(safe)?),
        keccak256(s),
    )
    .as_slice()))
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

/// `ecrecover(hash, v, r, s)` over a 65-byte `r ‖ s ‖ v` (`v` 27/28, or 0/1).
fn recover(hash: B256, signature: &[u8]) -> Result<Address, CoreError> {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    if signature.len() != 65 {
        return Err(err("not a 65-byte signature"));
    }
    let parity = match signature[64] {
        0 | 27 => false,
        1 | 28 => true,
        v => return Err(err(format!("v {v} is not an EIP-712 signature's"))),
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
/// safeTxHash is the owner). Returns it with `v` as the Safe reads it (27/28).
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
/// per EOA approval, and `r = executor, v = 1` for the executor, whose
/// `msg.sender` approves on its own. An executor that also sent an approval
/// counts once.
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
}

/// The executor's calls: deploy what has no code yet, then `execTransaction`.
pub fn execution_calls(plan: &ExecutionPlan) -> Result<Vec<Call>, CoreError> {
    let mut calls = Vec::new();
    if let Some(g) = &plan.deploy_multisig {
        if genesis_address(g)? != addr(&plan.multisig)?.to_checksum(None) {
            return Err(err("the genesis does not deploy this multisig"));
        }
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

fn read_owner_management(data: &[u8]) -> Option<Action> {
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
            "not a transaction Vela signs (only transfers, calls, member changes and cancellations)",
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

    /// The record call, as the registry decodes it: checked against
    /// `cast calldata` of the same arguments.
    #[test]
    fn the_record_call_is_the_registrys() {
        let ms = multisig_genesis(&[A.into(), B.into()], 2, "Team fund", "0x7").unwrap();
        let call = record_call(&ms.genesis).unwrap();
        assert_eq!(call.to, GENESIS_REGISTRY);
        assert!(call.data.starts_with(&hex(&selector(
            "recordNamed(address,bytes,address,bytes,uint256,string)"
        ))));
        assert_eq!(
            call.data,
            include_str!("../tests/vectors/multisig-record-named.hex").trim()
        );
        let mut unnamed = ms.genesis.clone();
        unnamed.name = None;
        let plain = record_call(&unnamed).unwrap();
        assert!(plain.data.starts_with(&hex(&selector(
            "record(address,bytes,address,bytes,uint256)"
        ))));
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
}
