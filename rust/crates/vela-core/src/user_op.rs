//! The Safe ERC-4337 user operation — assembled, hashed and signed, without I/O.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/services/safe-transaction.ts`
//! @ `origin/main` (itself a verbatim port of the Expo module and, before
//! that, `SafeTransactionService.swift`), with its vector suite. Every byte
//! here is money: a wrong offset signs an operation the Safe cannot execute,
//! a wrong verifier address in the `r` field signs one it will never accept.
//! So the port is literal, and the parts that CAN be cross-checked against an
//! independent implementation in this crate are: the SafeOp and SafeMessage
//! hashes against [`crate::eip712::hash_typed_data`], the WebAuthn signature's
//! dynamic payload against alloy's own ABI encoder.
//!
//! ## What is here and what is not
//!
//! Here: the pure assembly the three native tiers would otherwise each write
//! by hand — `executeUserOp` and MultiSend calldata, `initCode` for one key or
//! a founding set, the SafeOp EIP-712 hash the passkey signs, the contract
//! signature envelope (`r` = the verifier, `s` = 65, `v` = 0, then the
//! WebAuthn payload), the estimation dummy, the v0.7 wire dictionary, and the
//! two parsers of relay wording that decide a retry.
//!
//! Not here: anything that reads the chain or the relay — deployment status,
//! the nonce, gas signals, the quote, the estimate, the submit and its retry
//! loop. Those are a shell's, and the desktop's `executor/user_op.rs` performs
//! them in the order `sendUserOpInBand` does. The fee MATH (`calc_max_fee_per_gas`,
//! `derive_chain_gas_price`, the in-band amount) is already `fee_policy`'s and
//! is not repeated.
//!
//! ## Two deliberate divergences from the TypeScript, both stricter
//!
//! - `r` and `s` must be exactly 32 bytes; the TS right-padded a short slice.
//!   A raw signature always yields 32 + 32, so nothing changes for a real one.
//! - `parse_hex_quantity` errors on non-hex, where `BigInt('0x' + junk)` threw
//!   a `SyntaxError` — the same outcome, typed.

use serde_json::{Map, Value};

use crate::error::CoreError;
use crate::primitives::{
    self, abi_encode_address, abi_encode_bytes32, abi_encode_uint256, function_selector, keccak256,
    parse_address,
};
use crate::safe::{
    compute_safe_address, compute_safe_address_multi, compute_webauthn_signer_address,
    parse_public_key, ENTRY_POINT, MULTI_SEND, SAFE_4337_MODULE, SAFE_PROXY_FACTORY,
    SAFE_SINGLETON, WEBAUTHN_SIGNER,
};

// ---------------------------------------------------------------------------
// Constants (`safe-transaction.ts:69-82`)
// ---------------------------------------------------------------------------

/// Verification gas requested from the estimator for a deployed Safe, and the
/// floor its padded answer is held to.
pub const VERIFICATION_GAS_DEPLOYED: u128 = 300_000;
/// The same for an undeployed Safe: `sendUserOp` floors at 2M, so the estimate
/// must match.
pub const VERIFICATION_GAS_UNDEPLOYED: u128 = 2_000_000;
/// Simple transfers; the estimator may raise it.
pub const CALL_GAS_LIMIT: u128 = 200_000;
/// Must exceed the bundler's calculated preVerificationGas.
pub const PRE_VERIFICATION_GAS: u128 = 100_000;
/// Above this callData size the static defaults cannot be trusted; a failed
/// estimate refuses rather than submits.
pub const ESTIMATION_REQUIRED_CALLDATA: usize = 1024;
/// The EVM identity precompile — accepts arbitrary bytes, always succeeds —
/// so a no-transaction fee preview is a valid Safe execution.
pub const ESTIMATION_DUMMY_TARGET: &str = "0x0000000000000000000000000000000000000004";
/// Same payload size as ERC-20 transfer calldata.
pub const ESTIMATION_DUMMY_DATA_LENGTH: usize = 68;

const SAFE_OP_TYPE: &str = "SafeOp(address safe,uint256 nonce,bytes initCode,bytes callData,uint128 verificationGasLimit,uint128 callGasLimit,uint256 preVerificationGas,uint128 maxPriorityFeePerGas,uint128 maxFeePerGas,bytes paymasterAndData,uint48 validAfter,uint48 validUntil,address entryPoint)";
const DOMAIN_TYPE: &str = "EIP712Domain(uint256 chainId,address verifyingContract)";
const SAFE_MESSAGE_TYPE: &str = "SafeMessage(bytes message)";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A v0.6-shaped user operation, the form the hash and the v0.7 dictionary
/// are both derived from. Gas fields are plain integers; `nonce` is the
/// QUANTITY the EntryPoint answered (`0x0` for an undeployed account).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserOperation {
    pub sender: String,
    pub nonce: String,
    pub init_code: Vec<u8>,
    pub call_data: Vec<u8>,
    pub verification_gas_limit: u128,
    pub call_gas_limit: u128,
    pub pre_verification_gas: u128,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub paymaster_and_data: Vec<u8>,
    pub signature: Vec<u8>,
}

/// One sub-call of a MultiSend batch. `value_hex` is hex, `0x` optional,
/// empty meaning zero — `MultiSendCall.value`'s format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiSendCall {
    pub to: String,
    pub value_hex: String,
    pub data: Vec<u8>,
}

/// One founding key of a wallet, as the signer-address rule needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletKey {
    pub credential_id: String,
    /// Uncompressed P-256 point, `04‖x‖y` hex.
    pub public_key_hex: String,
}

/// The estimator's three answers, raw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GasEstimate {
    pub verification_gas_limit: u128,
    pub call_gas_limit: u128,
    pub pre_verification_gas: u128,
}

// ---------------------------------------------------------------------------
// Small encoders
// ---------------------------------------------------------------------------

fn word_u128(value: u128) -> Vec<u8> {
    let mut out = vec![0u8; 32];
    out[16..].copy_from_slice(&value.to_be_bytes());
    out
}

fn word_usize(value: usize) -> Vec<u8> {
    word_u128(value as u128)
}

fn pad_to_word(len: usize) -> Vec<u8> {
    vec![0u8; (32 - (len % 32)) % 32]
}

/// `transfer(address,uint256)` calldata.
pub fn encode_erc20_transfer(to: &str, amount: u128) -> Result<Vec<u8>, CoreError> {
    let mut out = function_selector("transfer(address,uint256)")?;
    out.extend(abi_encode_address(to)?);
    out.extend(word_u128(amount));
    Ok(out)
}

/// A plain value/token transfer — a native send (no data) or a standard
/// ERC-20 `transfer` (selector `0xa9059cbb` + two words = 68 bytes). Keys off
/// the call SHAPE, not the calldata SIZE: `claim()` is 4 bytes and heavy.
pub fn is_plain_transfer_call(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    data.len() == 68 && data[..4] == [0xa9, 0x05, 0x9c, 0xbb]
}

// ---------------------------------------------------------------------------
// CallData (`safe-transaction.ts:1889-1975`)
// ---------------------------------------------------------------------------

/// `Safe.executeUserOp(address to, uint256 value, bytes data, uint8 operation)`
/// with `operation = CALL`.
pub fn build_execute_call_data(
    to: &str,
    value_hex: &str,
    data: &[u8],
) -> Result<Vec<u8>, CoreError> {
    let mut out = function_selector("executeUserOp(address,uint256,bytes,uint8)")?;
    out.extend(abi_encode_address(to)?);
    out.extend(abi_encode_uint256(value_hex)?);
    out.extend(word_u128(128)); // 4 * 32
    out.extend(word_u128(0)); // CALL
    out.extend(word_usize(data.len()));
    out.extend_from_slice(data);
    out.extend(pad_to_word(data.len()));
    Ok(out)
}

/// One packed MultiSend transaction:
/// `operation(1) ‖ to(20) ‖ value(32) ‖ dataLen(32) ‖ data`, always a CALL.
fn encode_multi_send_call(call: &MultiSendCall) -> Result<Vec<u8>, CoreError> {
    let value = call.value_hex.strip_prefix("0x").unwrap_or(&call.value_hex);
    let value = if value.is_empty() { "0" } else { value };
    let to = parse_address(&call.to)?;
    let mut out = Vec::with_capacity(85 + call.data.len());
    out.push(0);
    out.extend_from_slice(to.as_slice());
    out.extend(abi_encode_uint256(value)?);
    out.extend(word_usize(call.data.len()));
    out.extend_from_slice(&call.data);
    Ok(out)
}

/// `Safe.executeUserOp(MultiSend, 0, multiSend(packedCalls), DELEGATECALL)` —
/// N sub-calls, atomically.
pub fn build_multi_send_execute_call_data(calls: &[MultiSendCall]) -> Result<Vec<u8>, CoreError> {
    let mut packed = Vec::new();
    for call in calls {
        packed.extend(encode_multi_send_call(call)?);
    }

    let mut payload = function_selector("multiSend(bytes)")?;
    payload.extend(word_u128(32)); // offset
    payload.extend(word_usize(packed.len()));
    payload.extend_from_slice(&packed);
    payload.extend(pad_to_word(packed.len()));

    let mut out = function_selector("executeUserOp(address,uint256,bytes,uint8)")?;
    out.extend(abi_encode_address(MULTI_SEND)?);
    out.extend(word_u128(0));
    out.extend(word_u128(128)); // data offset (4 * 32)
    out.extend(word_u128(1)); // DELEGATECALL
    out.extend(word_usize(payload.len()));
    out.extend_from_slice(&payload);
    out.extend(pad_to_word(payload.len()));
    Ok(out)
}

/// A lone call stays a single `executeUserOp` (unless `always_multi_send`);
/// a batch is a MultiSend.
pub fn build_native_call_data(
    calls: &[MultiSendCall],
    always_multi_send: bool,
) -> Result<Vec<u8>, CoreError> {
    match calls {
        [only] if !always_multi_send => {
            build_execute_call_data(&only.to, &only.value_hex, &only.data)
        }
        _ => build_multi_send_execute_call_data(calls),
    }
}

/// The single fee leg batched into an in-band operation: a native-value
/// transfer to the relay's recipient, or a stablecoin `transfer`. The shape
/// must stay a plain CALL to the exact recipient — that is what the relay's
/// reimbursement parser counts.
pub fn build_in_band_fee_leg(
    gas_fee_token: Option<&str>,
    recipient: &str,
    amount: u128,
) -> Result<MultiSendCall, CoreError> {
    Ok(match gas_fee_token {
        Some(token) => MultiSendCall {
            to: token.to_owned(),
            value_hex: "0".to_owned(),
            data: encode_erc20_transfer(recipient, amount)?,
        },
        None => MultiSendCall {
            to: recipient.to_owned(),
            value_hex: format!("0x{amount:x}"),
            data: Vec::new(),
        },
    })
}

// ---------------------------------------------------------------------------
// InitCode (`safe-transaction.ts:1989-2040`)
// ---------------------------------------------------------------------------

/// `factory ‖ createProxyWithNonce(singleton, setupData, saltNonce)` — one
/// assembler for the single-key and multi-key paths, so they cannot diverge.
fn assemble_init_code(setup_data: &[u8], salt_nonce: &[u8]) -> Result<Vec<u8>, CoreError> {
    let mut create = function_selector("createProxyWithNonce(address,bytes,uint256)")?;
    create.extend(abi_encode_address(SAFE_SINGLETON)?);
    create.extend(word_u128(96)); // 3 * 32
    create.extend(abi_encode_bytes32(salt_nonce)?);
    create.extend(word_usize(setup_data.len()));
    create.extend_from_slice(setup_data);
    create.extend(pad_to_word(setup_data.len()));

    let mut out = parse_address(SAFE_PROXY_FACTORY)?.as_slice().to_vec();
    out.extend(create);
    Ok(out)
}

/// InitCode for a founding key set. One key is byte-identical to the
/// historical single-owner setup; more take their setup data and salt from
/// `compute_safe_address_multi` — extra owners plus their signer proxies,
/// deployed inside the setup MultiSend. An empty set is refused.
pub fn build_init_code_for_keys(public_key_hexes: &[String]) -> Result<Vec<u8>, CoreError> {
    let info = match public_key_hexes {
        [] => {
            return Err(CoreError::InvalidPublicKey(
                "a wallet has at least one founding key".to_owned(),
            ));
        }
        [only] => {
            let key = parse_public_key(only)?;
            compute_safe_address(&key.x, &key.y)?
        }
        many => {
            let keys = many
                .iter()
                .map(|hex| parse_public_key(hex))
                .collect::<Result<Vec<_>, _>>()?;
            compute_safe_address_multi(&keys)?
        }
    };
    assemble_init_code(&info.setup_data, &info.salt_nonce)
}

/// The on-chain verifier the signature's `r` field must name for the key
/// that signed: the shared `WEBAUTHN_SIGNER` for the first (pinned) key, the
/// key's own counterfactual proxy for any later one. A one-key set always
/// uses the shared signer — byte-identical to the historical path. A
/// multi-key set with an unrecognised credential is an error: encoding the
/// wrong verifier would produce a signature the Safe can never accept.
pub fn signer_address_for(
    keys: &[WalletKey],
    credential_id: Option<&str>,
) -> Result<String, CoreError> {
    if keys.len() <= 1 {
        return Ok(WEBAUTHN_SIGNER.to_owned());
    }
    let Some(credential_id) = credential_id else {
        return Ok(WEBAUTHN_SIGNER.to_owned());
    };
    let normalize = |id: &str| id.strip_prefix("0x").unwrap_or(id).to_ascii_lowercase();
    let wanted = normalize(credential_id);
    let index = keys
        .iter()
        .position(|key| normalize(&key.credential_id) == wanted)
        .ok_or_else(|| {
            CoreError::InvalidSignature(
                "signing credential does not belong to this wallet".to_owned(),
            )
        })?;
    if index == 0 {
        return Ok(WEBAUTHN_SIGNER.to_owned());
    }
    let key = parse_public_key(&keys[index].public_key_hex)?;
    compute_webauthn_signer_address(&key.x, &key.y)
}

// ---------------------------------------------------------------------------
// Hashes (`safe-transaction.ts:2043-2158`)
// ---------------------------------------------------------------------------

fn domain_separator(chain_id: u64, verifying_contract: &str) -> Result<Vec<u8>, CoreError> {
    let mut preimage = keccak256(DOMAIN_TYPE.as_bytes());
    preimage.extend(word_u128(u128::from(chain_id)));
    preimage.extend(abi_encode_address(verifying_contract)?);
    Ok(keccak256(&preimage))
}

fn eip712_digest(domain_separator: &[u8], struct_hash: &[u8]) -> Vec<u8> {
    let mut preimage = vec![0x19, 0x01];
    preimage.extend_from_slice(domain_separator);
    preimage.extend_from_slice(struct_hash);
    keccak256(&preimage)
}

/// The SafeOp EIP-712 hash the passkey signs — the Safe4337Module's domain,
/// `validAfter = validUntil = 0`.
pub fn calculate_safe_op_hash(op: &UserOperation, chain_id: u64) -> Result<Vec<u8>, CoreError> {
    let mut preimage = keccak256(SAFE_OP_TYPE.as_bytes());
    preimage.extend(abi_encode_address(&op.sender)?);
    preimage.extend(abi_encode_uint256(&op.nonce)?);
    preimage.extend(keccak256(&op.init_code));
    preimage.extend(keccak256(&op.call_data));
    preimage.extend(word_u128(op.verification_gas_limit));
    preimage.extend(word_u128(op.call_gas_limit));
    preimage.extend(word_u128(op.pre_verification_gas));
    preimage.extend(word_u128(op.max_priority_fee_per_gas));
    preimage.extend(word_u128(op.max_fee_per_gas));
    preimage.extend(keccak256(&op.paymaster_and_data));
    preimage.extend(word_u128(0)); // validAfter
    preimage.extend(word_u128(0)); // validUntil
    preimage.extend(abi_encode_address(ENTRY_POINT)?);
    let struct_hash = keccak256(&preimage);
    Ok(eip712_digest(
        &domain_separator(chain_id, SAFE_4337_MODULE)?,
        &struct_hash,
    ))
}

/// The Safe message hash a passkey signs for EIP-1271 verification:
/// `Safe4337Module.isValidSignature` wraps the original hash in a
/// `SafeMessage(bytes message)` struct under the SAFE's own domain.
pub fn compute_safe_message_hash(
    original_hash: &[u8],
    chain_id: u64,
    safe_address: &str,
) -> Result<Vec<u8>, CoreError> {
    let message_hash = keccak256(&abi_encode_bytes32(original_hash)?);
    let mut preimage = keccak256(SAFE_MESSAGE_TYPE.as_bytes());
    preimage.extend(message_hash);
    let struct_hash = keccak256(&preimage);
    Ok(eip712_digest(
        &domain_separator(chain_id, safe_address)?,
        &struct_hash,
    ))
}

// ---------------------------------------------------------------------------
// WebAuthn signature (`safe-transaction.ts:2160-2330`)
// ---------------------------------------------------------------------------

/// Everything in clientDataJSON after the challenge's closing `",` up to
/// (not including) the final `}` — e.g.
/// `"origin":"https://getvela.app","crossOrigin":false`. The contract's
/// template already supplies the `,"` before it, so the leading comma is
/// NOT included. Empty when the JSON is not shaped as expected.
pub fn extract_client_data_fields(client_data_json: &[u8]) -> String {
    let json = String::from_utf8_lossy(client_data_json);
    const KEY: &str = "\"challenge\":\"";
    let Some(key_index) = json.find(KEY) else {
        return String::new();
    };
    let value_start = key_index + KEY.len();
    let Some(closing) = json[value_start..].find('"') else {
        return String::new();
    };
    // Skip the closing quote and the comma.
    let skip_index = value_start + closing + 2;
    let end_index = json.len() - 1;
    if skip_index >= end_index {
        return String::new();
    }
    json.get(skip_index..end_index)
        .unwrap_or_default()
        .to_owned()
}

fn require_word(name: &str, bytes: &[u8]) -> Result<(), CoreError> {
    if bytes.len() != 32 {
        return Err(CoreError::InvalidSignature(format!(
            "{name} must be 32 bytes, got {}",
            bytes.len()
        )));
    }
    Ok(())
}

/// `abi.encode(bytes authenticatorData, string clientDataFields, uint256 r, uint256 s)`.
fn abi_encode_webauthn_sig(
    authenticator_data: &[u8],
    client_data_fields: &str,
    r: &[u8],
    s: &[u8],
) -> Result<Vec<u8>, CoreError> {
    require_word("r", r)?;
    require_word("s", s)?;
    let fields = client_data_fields.as_bytes();

    let mut auth_tail = word_usize(authenticator_data.len());
    auth_tail.extend_from_slice(authenticator_data);
    auth_tail.extend(pad_to_word(authenticator_data.len()));

    let mut client_tail = word_usize(fields.len());
    client_tail.extend_from_slice(fields);
    client_tail.extend(pad_to_word(fields.len()));

    let auth_offset = 128usize; // 4 * 32
    let client_offset = auth_offset + auth_tail.len();

    let mut out = word_usize(auth_offset);
    out.extend(word_usize(client_offset));
    out.extend(abi_encode_bytes32(r)?);
    out.extend(abi_encode_bytes32(s)?);
    out.extend(auth_tail);
    out.extend(client_tail);
    Ok(out)
}

/// `r(32) ‖ s(32) ‖ v(1) ‖ dataLength(32) ‖ dynamicData` where `r` = the
/// verifier address, `s` = 65 (the offset past r‖s‖v), `v` = 0 (contract
/// signature).
fn build_contract_signature_core(
    authenticator_data: &[u8],
    client_data_fields: &str,
    r: &[u8],
    s: &[u8],
    signer_address: &str,
) -> Result<Vec<u8>, CoreError> {
    let dynamic = abi_encode_webauthn_sig(authenticator_data, client_data_fields, r, s)?;
    let mut out = abi_encode_address(signer_address)?;
    out.extend(word_u128(65));
    out.push(0x00);
    out.extend(word_usize(dynamic.len()));
    out.extend(dynamic);
    Ok(out)
}

/// The 4337 user-operation signature: a 12-byte validity window
/// (`validAfter(6) ‖ validUntil(6)`, zeros) before the contract signature.
pub fn build_user_op_signature(
    authenticator_data: &[u8],
    client_data_fields: &str,
    r: &[u8],
    s: &[u8],
    signer_address: &str,
) -> Result<Vec<u8>, CoreError> {
    let mut out = vec![0u8; 12];
    out.extend(build_contract_signature_core(
        authenticator_data,
        client_data_fields,
        r,
        s,
        signer_address,
    )?);
    Ok(out)
}

/// The EIP-1271 `isValidSignature` form: the same envelope WITHOUT the
/// validity window, because `checkNSignatures` is called directly.
pub fn build_eip1271_signature(
    authenticator_data: &[u8],
    client_data_fields: &str,
    r: &[u8],
    s: &[u8],
    signer_address: &str,
) -> Result<Vec<u8>, CoreError> {
    build_contract_signature_core(authenticator_data, client_data_fields, r, s, signer_address)
}

/// The signature an estimate carries: a real assertion's 37-byte
/// authenticator-data shape (`rpIdHash ‖ UP|UV ‖ counter`) and the real
/// client-data fields, with `r = s = 1`. Built by the real encoder so ABI
/// offsets, padding and the validity window cannot diverge from a submitted
/// operation; it deliberately cannot verify — a valid one would cost a
/// passkey prompt before the person confirms.
pub fn build_dummy_signature() -> Result<Vec<u8>, CoreError> {
    let mut auth_data =
        primitives::from_hex("a69533717b230610f14ea657c0bd8231dd6fc7b7108f1215a874fbb1d14df349")?;
    auth_data.push(0x05);
    auth_data.extend_from_slice(&[0, 0, 0, 0]);
    let mut one = [0u8; 32];
    one[31] = 0x01;
    build_user_op_signature(
        &auth_data,
        "\"origin\":\"https://getvela.app\",\"crossOrigin\":false",
        &one,
        &one,
        WEBAUTHN_SIGNER,
    )
}

// ---------------------------------------------------------------------------
// Gas padding (`sendUserOpInBand`, `safe-transaction.ts:1760-1768`)
// ---------------------------------------------------------------------------

/// The limits a submitted operation carries, from the estimator's raw
/// answer: ×1.5 on the two limits, each held to its floor, and +10,000 on
/// preVerificationGas. The floors are the caller's because they differ by
/// path (deployed / undeployed / Tempo).
pub fn pad_gas_estimate(
    estimate: GasEstimate,
    verification_floor: u128,
    call_floor: u128,
) -> GasEstimate {
    GasEstimate {
        verification_gas_limit: (estimate.verification_gas_limit * 15 / 10).max(verification_floor),
        call_gas_limit: (estimate.call_gas_limit * 15 / 10).max(call_floor),
        pre_verification_gas: estimate.pre_verification_gas + 10_000,
    }
}

// ---------------------------------------------------------------------------
// Wire (`safe-transaction.ts:2942-2988`)
// ---------------------------------------------------------------------------

fn quantity(value: u128) -> Value {
    Value::String(format!("0x{value:x}"))
}

/// The v0.7 JSON-RPC dictionary: `initCode` split into `factory` +
/// `factoryData`, `paymasterAndData` into its four fields, plus any Vela
/// extension fields (Tempo's `feeToken`) the relay reads and strips.
pub fn user_op_to_json(op: &UserOperation, extra: &[(&str, &str)]) -> Value {
    let mut dict = Map::new();
    dict.insert("sender".to_owned(), Value::String(op.sender.clone()));
    dict.insert("nonce".to_owned(), Value::String(op.nonce.clone()));
    dict.insert(
        "callData".to_owned(),
        Value::String(primitives::to_hex(&op.call_data, true)),
    );
    dict.insert("callGasLimit".to_owned(), quantity(op.call_gas_limit));
    dict.insert(
        "verificationGasLimit".to_owned(),
        quantity(op.verification_gas_limit),
    );
    dict.insert(
        "preVerificationGas".to_owned(),
        quantity(op.pre_verification_gas),
    );
    dict.insert("maxFeePerGas".to_owned(), quantity(op.max_fee_per_gas));
    dict.insert(
        "maxPriorityFeePerGas".to_owned(),
        quantity(op.max_priority_fee_per_gas),
    );
    dict.insert(
        "signature".to_owned(),
        Value::String(primitives::to_hex(&op.signature, true)),
    );
    if op.init_code.len() >= 20 {
        dict.insert(
            "factory".to_owned(),
            Value::String(primitives::to_hex(&op.init_code[..20], true)),
        );
        dict.insert(
            "factoryData".to_owned(),
            Value::String(primitives::to_hex(&op.init_code[20..], true)),
        );
    }
    if op.paymaster_and_data.len() >= 20 {
        dict.insert(
            "paymaster".to_owned(),
            Value::String(primitives::to_hex(&op.paymaster_and_data[..20], true)),
        );
        dict.insert(
            "paymasterData".to_owned(),
            Value::String(primitives::to_hex(&op.paymaster_and_data[20..], true)),
        );
        dict.insert(
            "paymasterVerificationGasLimit".to_owned(),
            Value::String("0x0".to_owned()),
        );
        dict.insert(
            "paymasterPostOpGasLimit".to_owned(),
            Value::String("0x0".to_owned()),
        );
    }
    for (key, value) in extra {
        dict.insert((*key).to_owned(), Value::String((*value).to_owned()));
    }
    Value::Object(dict)
}

// ---------------------------------------------------------------------------
// Parsers (`safe-transaction.ts:2990-3000`)
// ---------------------------------------------------------------------------

/// The relay's `[existingHash:0x…]` marker in a submit error: a previous
/// operation for the account is still pending, and this is its hash to poll
/// instead of failing.
pub fn parse_existing_user_op_hash(message: &str) -> Option<String> {
    const MARK: &str = "[existingHash:0x";
    for (at, _) in message.match_indices(MARK) {
        let rest = &message[at + MARK.len()..];
        let hex_len = rest.bytes().take_while(u8::is_ascii_hexdigit).count();
        if hex_len > 0 && rest.as_bytes().get(hex_len) == Some(&b']') {
            return Some(format!("0x{}", &rest[..hex_len]));
        }
    }
    None
}

/// A hex QUANTITY as an integer: `0x` optional, absent / empty / bare `0x`
/// meaning zero (`parseHexUInt64`). Non-hex is an error.
pub fn parse_hex_quantity(value: Option<&str>) -> Result<u128, CoreError> {
    let Some(value) = value else {
        return Ok(0);
    };
    let clean = value.strip_prefix("0x").unwrap_or(value);
    if clean.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(clean, 16)
        .map_err(|e| CoreError::InvalidQuantity(format!("`{value}`: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eip712::hash_typed_data;
    use alloy_dyn_abi::DynSolValue;
    use alloy_primitives::U256;

    fn ok<T>(result: Result<T, CoreError>) -> T {
        result.unwrap_or_else(|error| unreachable!("{error}"))
    }

    fn hex(bytes: &[u8]) -> String {
        primitives::to_hex(bytes, false)
    }

    /// The i-th 32-byte ABI word after a 4-byte selector, as hex.
    fn word(bytes: &[u8], i: usize) -> String {
        hex(&bytes[4 + i * 32..4 + (i + 1) * 32])
    }

    fn selector(signature: &str) -> String {
        hex(&ok(function_selector(signature)))
    }

    const ZERO_WORD: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    const TO: &str = "0x1111111111111111111111111111111111111111";
    const KEY_A: &str = "04a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90b1c2d3e4f50617283940a1b2c3d4e5f6b1c2d3e4f50617283940a1b2c3d4e5f6";
    const KEY_B: &str = "048f9b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff07a8b9cadbecfd0e1f20314253647586970818293a4b5c6d7e8f9010203040506";

    fn padded(value: u128) -> String {
        format!("{value:064x}")
    }

    // --- the web vector suite, ported ---------------------------------------

    #[test]
    fn erc20_transfer_matches_the_canonical_layout() {
        let out = ok(encode_erc20_transfer(USDC, 1_000_000));
        assert_eq!(out.len(), 4 + 32 + 32);
        assert_eq!(&hex(&out)[..8], "a9059cbb");
        assert_eq!(&hex(&out)[..8], selector("transfer(address,uint256)"));
        assert_eq!(word(&out, 0), format!("{:0>64}", USDC[2..].to_lowercase()));
        assert_eq!(word(&out, 1), padded(1_000_000));
        assert_eq!(word(&ok(encode_erc20_transfer(USDC, 0)), 1), ZERO_WORD);
    }

    #[test]
    fn plain_transfers_are_classified_by_shape_not_size() {
        assert!(is_plain_transfer_call(&[]));
        assert!(is_plain_transfer_call(&ok(encode_erc20_transfer(
            USDC, 1_000_000
        ))));
        assert!(!is_plain_transfer_call(&[0x4e, 0x71, 0xd9, 0x2d])); // claim()
        assert!(!is_plain_transfer_call(&[0u8; 36])); // deposit(uint256)
        assert!(!is_plain_transfer_call(&[0u8; 100]));
        let mut approve = vec![0u8; 68];
        approve[..4].copy_from_slice(&[0x09, 0x5e, 0xa7, 0xb3]);
        assert!(!is_plain_transfer_call(&approve));
    }

    #[test]
    fn execute_call_data_is_selector_plus_five_words() {
        let out = ok(build_execute_call_data(TO, "0x0", &[]));
        assert_eq!(out.len(), 4 + 32 * 5);
        assert_eq!(
            &hex(&out)[..8],
            selector("executeUserOp(address,uint256,bytes,uint8)")
        );
        assert_eq!(word(&out, 0), format!("{:0>64}", &TO[2..]));
        assert_eq!(word(&out, 1), ZERO_WORD);
        assert_eq!(word(&out, 2), padded(128));
        assert_eq!(word(&out, 3), ZERO_WORD);
        assert_eq!(word(&out, 4), ZERO_WORD);
    }

    #[test]
    fn execute_call_data_pads_the_payload_to_a_word() {
        let out = ok(build_execute_call_data(TO, "0x0", &[0xde, 0xad]));
        assert_eq!(word(&out, 4), padded(2));
        assert_eq!(out.len(), 4 + 32 * 5 + 32);
        assert_eq!(&hex(&out)[8 + 5 * 64..8 + 5 * 64 + 4], "dead");
    }

    #[test]
    fn multi_send_wraps_the_batch_as_a_delegatecall() {
        let out = ok(build_multi_send_execute_call_data(&[
            MultiSendCall {
                to: TO.to_owned(),
                value_hex: "0x0".to_owned(),
                data: Vec::new(),
            },
            MultiSendCall {
                to: "0x2222222222222222222222222222222222222222".to_owned(),
                value_hex: "0x0".to_owned(),
                data: vec![0xaa],
            },
        ]));
        assert_eq!(
            &hex(&out)[..8],
            selector("executeUserOp(address,uint256,bytes,uint8)")
        );
        assert_eq!(
            word(&out, 0),
            format!("{:0>64}", MULTI_SEND[2..].to_lowercase())
        );
        assert_eq!(word(&out, 2), padded(128));
        assert_eq!(
            word(&out, 3),
            padded(1),
            "DELEGATECALL — the load-bearing bit"
        );
        assert!(hex(&out).contains(&selector("multiSend(bytes)")));
        // The packed transactions: op(1) ‖ to(20) ‖ value(32) ‖ len(32) ‖ data.
        let packed_len = (1 + 20 + 32 + 32) * 2 + 1;
        let payload_len = 4 + 32 + 32 + packed_len + (32 - packed_len % 32) % 32;
        assert_eq!(word(&out, 4), padded(payload_len as u128));
        assert_eq!(out.len() % 32, 4, "selector plus whole words");
    }

    #[test]
    fn a_lone_call_stays_single_unless_asked_otherwise() {
        let call = MultiSendCall {
            to: TO.to_owned(),
            value_hex: "0x1".to_owned(),
            data: Vec::new(),
        };
        let single = ok(build_native_call_data(std::slice::from_ref(&call), false));
        assert_eq!(single, ok(build_execute_call_data(TO, "0x1", &[])));
        let forced = ok(build_native_call_data(std::slice::from_ref(&call), true));
        assert_eq!(
            forced,
            ok(build_multi_send_execute_call_data(std::slice::from_ref(
                &call
            )))
        );
    }

    #[test]
    fn an_empty_multisend_value_means_zero() {
        let call = MultiSendCall {
            to: TO.to_owned(),
            value_hex: String::new(),
            data: Vec::new(),
        };
        let a = ok(build_multi_send_execute_call_data(std::slice::from_ref(
            &call,
        )));
        let b = ok(build_multi_send_execute_call_data(&[MultiSendCall {
            value_hex: "0x0".to_owned(),
            ..call
        }]));
        assert_eq!(a, b);
    }

    #[test]
    fn the_in_band_fee_leg_has_two_shapes() {
        let native = ok(build_in_band_fee_leg(None, TO, 255));
        assert_eq!(native.to, TO);
        assert_eq!(native.value_hex, "0xff");
        assert!(native.data.is_empty());
        let stable = ok(build_in_band_fee_leg(Some(USDC), TO, 255));
        assert_eq!(stable.to, USDC);
        assert_eq!(stable.value_hex, "0");
        assert_eq!(stable.data, ok(encode_erc20_transfer(TO, 255)));
        assert!(is_plain_transfer_call(&stable.data));
    }

    #[test]
    fn init_code_is_factory_then_create_proxy_with_nonce_and_deterministic() {
        let keys = [KEY_A.to_owned()];
        let out = ok(build_init_code_for_keys(&keys));
        assert_eq!(
            &hex(&out)[..40],
            SAFE_PROXY_FACTORY[2..].to_lowercase(),
            "20-byte factory first"
        );
        assert_eq!(
            &hex(&out)[40..48],
            selector("createProxyWithNonce(address,bytes,uint256)")
        );
        assert_eq!(out, ok(build_init_code_for_keys(&keys)));
    }

    #[test]
    fn two_keys_produce_a_larger_multi_owner_init_code() {
        let one = ok(build_init_code_for_keys(&[KEY_A.to_owned()]));
        let two = ok(build_init_code_for_keys(&[
            KEY_A.to_owned(),
            KEY_B.to_owned(),
        ]));
        assert_ne!(one, two);
        assert_eq!(one[..20], two[..20]);
        assert!(two.len() > one.len());
        assert!(
            build_init_code_for_keys(&[]).is_err(),
            "an empty key set is refused"
        );
    }

    #[test]
    fn the_signer_address_rule_matches_the_web() {
        let keys = vec![
            WalletKey {
                credential_id: "aa01".to_owned(),
                public_key_hex: KEY_A.to_owned(),
            },
            WalletKey {
                credential_id: "bb02".to_owned(),
                public_key_hex: KEY_B.to_owned(),
            },
        ];
        assert_eq!(
            ok(signer_address_for(&keys[..1], Some("zz"))),
            WEBAUTHN_SIGNER
        );
        assert_eq!(ok(signer_address_for(&keys, None)), WEBAUTHN_SIGNER);
        assert_eq!(ok(signer_address_for(&keys, Some("aa01"))), WEBAUTHN_SIGNER);
        let b = ok(parse_public_key(KEY_B));
        let proxy = ok(compute_webauthn_signer_address(&b.x, &b.y));
        assert_eq!(ok(signer_address_for(&keys, Some("bb02"))), proxy);
        assert_eq!(ok(signer_address_for(&keys, Some("0xBB02"))), proxy);
        assert!(
            signer_address_for(&keys, Some("cc03")).is_err(),
            "a foreign credential is refused, never mis-encoded"
        );
    }

    #[test]
    fn the_existing_hash_marker_is_parsed() {
        assert_eq!(
            parse_existing_user_op_hash("AA25 invalid nonce [existingHash:0xabc123]").as_deref(),
            Some("0xabc123")
        );
        assert_eq!(
            parse_existing_user_op_hash("[existingHash:0xDEADbeef] and more").as_deref(),
            Some("0xDEADbeef")
        );
        assert_eq!(
            parse_existing_user_op_hash("some unrelated bundler error"),
            None
        );
        assert_eq!(parse_existing_user_op_hash("[existingHash:nothex]"), None);
        assert_eq!(parse_existing_user_op_hash("[existingHash:0x]"), None);
        assert_eq!(parse_existing_user_op_hash(""), None);
    }

    #[test]
    fn hex_quantities_parse_with_or_without_prefix() {
        assert_eq!(ok(parse_hex_quantity(None)), 0);
        assert_eq!(ok(parse_hex_quantity(Some(""))), 0);
        assert_eq!(ok(parse_hex_quantity(Some("0x"))), 0);
        assert_eq!(ok(parse_hex_quantity(Some("0x0"))), 0);
        assert_eq!(ok(parse_hex_quantity(Some("0xff"))), 255);
        assert_eq!(ok(parse_hex_quantity(Some("ff"))), 255);
        assert!(parse_hex_quantity(Some("0xzz")).is_err());
    }

    // --- cross-checks against independent implementations in this crate ------

    fn sample_op() -> UserOperation {
        UserOperation {
            sender: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
            nonce: "0x7".to_owned(),
            init_code: ok(build_init_code_for_keys(&[KEY_A.to_owned()])),
            call_data: ok(build_execute_call_data(TO, "0x2386f26fc10000", &[])),
            verification_gas_limit: 300_000,
            call_gas_limit: 200_000,
            pre_verification_gas: 110_000,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            paymaster_and_data: Vec::new(),
            signature: ok(build_dummy_signature()),
        }
    }

    /// The hand-assembled SafeOp hash equals what alloy's EIP-712 hasher
    /// produces for the same typed data — two implementations, one digest.
    #[test]
    fn the_safe_op_hash_agrees_with_the_eip712_hasher() {
        let op = sample_op();
        let typed = serde_json::json!({
            "types": {
                "EIP712Domain": [
                    {"name": "chainId", "type": "uint256"},
                    {"name": "verifyingContract", "type": "address"}
                ],
                "SafeOp": [
                    {"name": "safe", "type": "address"},
                    {"name": "nonce", "type": "uint256"},
                    {"name": "initCode", "type": "bytes"},
                    {"name": "callData", "type": "bytes"},
                    {"name": "verificationGasLimit", "type": "uint128"},
                    {"name": "callGasLimit", "type": "uint128"},
                    {"name": "preVerificationGas", "type": "uint256"},
                    {"name": "maxPriorityFeePerGas", "type": "uint128"},
                    {"name": "maxFeePerGas", "type": "uint128"},
                    {"name": "paymasterAndData", "type": "bytes"},
                    {"name": "validAfter", "type": "uint48"},
                    {"name": "validUntil", "type": "uint48"},
                    {"name": "entryPoint", "type": "address"}
                ]
            },
            "primaryType": "SafeOp",
            "domain": {"chainId": 100, "verifyingContract": SAFE_4337_MODULE},
            "message": {
                "safe": op.sender,
                "nonce": op.nonce,
                "initCode": primitives::to_hex(&op.init_code, true),
                "callData": primitives::to_hex(&op.call_data, true),
                "verificationGasLimit": op.verification_gas_limit.to_string(),
                "callGasLimit": op.call_gas_limit.to_string(),
                "preVerificationGas": op.pre_verification_gas.to_string(),
                "maxPriorityFeePerGas": op.max_priority_fee_per_gas.to_string(),
                "maxFeePerGas": op.max_fee_per_gas.to_string(),
                "paymasterAndData": "0x",
                "validAfter": "0",
                "validUntil": "0",
                "entryPoint": ENTRY_POINT
            }
        });
        let expected = ok(hash_typed_data(&typed.to_string()));
        assert_eq!(ok(calculate_safe_op_hash(&op, 100)), expected);
        assert_ne!(
            ok(calculate_safe_op_hash(&op, 1)),
            expected,
            "the chain id is in the domain"
        );
    }

    #[test]
    fn the_safe_message_hash_agrees_with_the_eip712_hasher() {
        let original = keccak256(b"hello");
        let safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let typed = serde_json::json!({
            "types": {
                "EIP712Domain": [
                    {"name": "chainId", "type": "uint256"},
                    {"name": "verifyingContract", "type": "address"}
                ],
                "SafeMessage": [{"name": "message", "type": "bytes"}]
            },
            "primaryType": "SafeMessage",
            "domain": {"chainId": 100, "verifyingContract": safe},
            "message": {"message": primitives::to_hex(&original, true)}
        });
        let expected = ok(hash_typed_data(&typed.to_string()));
        assert_eq!(
            ok(compute_safe_message_hash(&original, 100, safe)),
            expected
        );
    }

    /// The hand-laid `(bytes, string, uint256, uint256)` payload equals
    /// alloy's ABI encoder's, for a 37-byte authData and a 51-byte string —
    /// both of which need padding.
    #[test]
    fn the_webauthn_payload_agrees_with_the_abi_encoder() {
        let auth = ok(primitives::from_hex(
            "a69533717b230610f14ea657c0bd8231dd6fc7b7108f1215a874fbb1d14df3490500000000",
        ));
        let fields = "\"origin\":\"https://getvela.app\",\"crossOrigin\":false";
        let r = keccak256(b"r");
        let s = keccak256(b"s");
        let expected = DynSolValue::Tuple(vec![
            DynSolValue::Bytes(auth.clone()),
            DynSolValue::String(fields.to_owned()),
            DynSolValue::Uint(U256::from_be_slice(&r), 256),
            DynSolValue::Uint(U256::from_be_slice(&s), 256),
        ])
        .abi_encode_params();
        assert_eq!(ok(abi_encode_webauthn_sig(&auth, fields, &r, &s)), expected);
        assert!(abi_encode_webauthn_sig(&auth, fields, &r[..31], &s).is_err());
    }

    #[test]
    fn the_signature_envelope_has_the_documented_layout() {
        let auth = vec![0u8; 37];
        let fields = "\"origin\":\"https://getvela.app\"";
        let r = [1u8; 32];
        let s = [2u8; 32];
        let core = ok(build_eip1271_signature(
            &auth,
            fields,
            &r,
            &s,
            WEBAUTHN_SIGNER,
        ));
        let dynamic = ok(abi_encode_webauthn_sig(&auth, fields, &r, &s));
        assert_eq!(core.len(), 32 + 32 + 1 + 32 + dynamic.len());
        assert_eq!(&core[..32], &ok(abi_encode_address(WEBAUTHN_SIGNER))[..]);
        assert_eq!(&core[32..64], &word_u128(65)[..]);
        assert_eq!(core[64], 0x00);
        assert_eq!(&core[65..97], &word_usize(dynamic.len())[..]);
        assert_eq!(&core[97..], &dynamic[..]);

        let user_op = ok(build_user_op_signature(
            &auth,
            fields,
            &r,
            &s,
            WEBAUTHN_SIGNER,
        ));
        assert_eq!(&user_op[..12], &[0u8; 12]);
        assert_eq!(&user_op[12..], &core[..]);
    }

    #[test]
    fn the_dummy_signature_is_a_real_envelope_that_cannot_verify() {
        let dummy = ok(build_dummy_signature());
        assert_eq!(&dummy[..12], &[0u8; 12]);
        assert_eq!(&dummy[12..44], &ok(abi_encode_address(WEBAUTHN_SIGNER))[..]);
        // authData sits at the first dynamic tail: length 37, flags byte UP|UV.
        let dynamic = &dummy[12 + 97..];
        assert_eq!(&dynamic[..32], &word_u128(128)[..]);
        assert_eq!(&dynamic[128..160], &word_u128(37)[..]);
        assert_eq!(dynamic[160 + 32], 0x05);
        // r = s = 1.
        assert_eq!(&dynamic[64..96], &word_u128(1)[..]);
        assert_eq!(&dynamic[96..128], &word_u128(1)[..]);
    }

    #[test]
    fn client_data_fields_are_everything_after_the_challenge() {
        let web = b"{\"type\":\"webauthn.get\",\"challenge\":\"abc\",\"origin\":\"https://getvela.app\",\"crossOrigin\":false}";
        assert_eq!(
            extract_client_data_fields(web),
            "\"origin\":\"https://getvela.app\",\"crossOrigin\":false"
        );
        let proof =
            b"{\"type\":\"webauthn.get\",\"challenge\":\"abc\",\"origin\":\"https://getvela.app\"}";
        assert_eq!(
            extract_client_data_fields(proof),
            "\"origin\":\"https://getvela.app\""
        );
        assert_eq!(
            extract_client_data_fields(b"{\"type\":\"webauthn.get\"}"),
            ""
        );
        assert_eq!(extract_client_data_fields(b"{\"challenge\":\"abc"), "");
        assert_eq!(extract_client_data_fields(b"{\"challenge\":\"abc\"}"), "");
        assert_eq!(extract_client_data_fields(b""), "");
    }

    #[test]
    fn the_wire_dictionary_is_v07_shaped() {
        let op = sample_op();
        let json = user_op_to_json(&op, &[("feeToken", USDC)]);
        assert_eq!(json["sender"], op.sender);
        assert_eq!(json["nonce"], "0x7");
        assert_eq!(json["callGasLimit"], "0x30d40");
        assert_eq!(json["maxFeePerGas"], "0x0");
        assert_eq!(json["factory"], SAFE_PROXY_FACTORY.to_lowercase());
        assert_eq!(
            json["factoryData"],
            primitives::to_hex(&op.init_code[20..], true)
        );
        assert_eq!(json["signature"], primitives::to_hex(&op.signature, true));
        assert_eq!(json["feeToken"], USDC);
        assert!(json.get("paymaster").is_none());

        let deployed = UserOperation {
            init_code: Vec::new(),
            ..op
        };
        let json = user_op_to_json(&deployed, &[]);
        assert!(json.get("factory").is_none());
        assert!(json.get("factoryData").is_none());
        assert!(json.get("feeToken").is_none());
    }

    #[test]
    fn gas_padding_applies_the_factor_the_floors_and_the_adder() {
        let raw = GasEstimate {
            verification_gas_limit: 100_000,
            call_gas_limit: 50_000,
            pre_verification_gas: 40_000,
        };
        let padded = pad_gas_estimate(raw, VERIFICATION_GAS_DEPLOYED, CALL_GAS_LIMIT);
        assert_eq!(padded.verification_gas_limit, 300_000, "floored");
        assert_eq!(padded.call_gas_limit, 200_000, "floored");
        assert_eq!(padded.pre_verification_gas, 50_000);
        let big = GasEstimate {
            verification_gas_limit: 400_000,
            call_gas_limit: 400_000,
            pre_verification_gas: 0,
        };
        let padded = pad_gas_estimate(big, VERIFICATION_GAS_UNDEPLOYED, CALL_GAS_LIMIT);
        assert_eq!(padded.verification_gas_limit, 2_000_000);
        assert_eq!(padded.call_gas_limit, 600_000);
    }
}
