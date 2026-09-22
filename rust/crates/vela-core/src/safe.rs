//! Counterfactual Safe + gas-splitter address assembly — the user's on-chain
//! identity across all supported chains.
//!
//! Port of src/services/safe-address.ts (itself a port of
//! SafeAddressComputer.swift — the drift class this crate retires). Deployment
//! constants are chain-independent; the splitter trio MUST stay byte-identical
//! to vela-relay/shared/contracts/splitter.ts (cross-repo contract). The
//! `compute_safe_address` identity vector
//! (`0x762EdA60D3B68755c271D608644650278f88329F`) is a release blocker:
//! existing users' addresses must never change.
//!
//! The multi-passkey surface (`compute_safe_address_multi`,
//! `compute_webauthn_signer_address`, the passkey-module factory constants)
//! is new Rust with no TS ancestor. Its release blocker is the invariant
//! that the multi path with one key stays byte-identical to
//! `compute_safe_address`.

use crate::error::CoreError;
use crate::primitives::{self, parse_address};
use crate::types::{P256PublicKey, SafeAddressInfo};
use alloy_primitives::keccak256;

pub const SAFE_PROXY_FACTORY: &str = "0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67";
pub const SAFE_SINGLETON: &str = "0x29fcB43b46531BcA003ddC8FCB67FFE91900C762";
pub const ENTRY_POINT: &str = "0x0000000071727De22E5E9d8BAf0edAc6f37da032";
pub const SAFE_4337_MODULE: &str = "0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226";
pub const SAFE_MODULE_SETUP: &str = "0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47";
pub const WEBAUTHN_SIGNER: &str = "0x94a4F6affBd8975951142c3999aEAB7ecee555c2";
pub const MULTI_SEND: &str = "0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526";

// MARK: Safe passkey module v0.2.1 — per-key WebAuthn signer proxies. Extra
// owners beyond the shared WEBAUTHN_SIGNER are counterfactual
// SafeWebAuthnSignerProxy instances deployed by this factory (CREATE2, salt 0).

pub const WEBAUTHN_SIGNER_FACTORY: &str = "0x1d31F259eE307358a26dFb23EB365939E8641195";
pub const WEBAUTHN_SIGNER_SINGLETON: &str = "0x4E27b51350e6c2083EE19011120F50DAfEc5CA50";

/// Hard cap on keys per multi-passkey Safe. Deployment cost is linear
/// (~119k gas + ~220 setup bytes per extra key, measured on Gnosis): 7 keys
/// ≈ 1.2M deploy gas, comfortably under the relay's ~10M simulation ceiling
/// (~80 keys) while bounding worst-case initCode size for every consumer.
pub const MAX_MULTI_KEYS: usize = 7;

/// RIP-7212 precompile address as the `P256.Verifiers` word (bare hex).
/// One constant, two INDEPENDENTLY frozen uses — sharing it is convenience,
/// not a requirement that they agree: the shared-signer `configure()` word is
/// baked into every existing single-key Safe address (the 0x762EdA… identity
/// vector), and the factory word is baked into every multi-key Safe address
/// and must equal whatever deployment later passes to `factory.createSigner`
/// (the chain-verified getSigner test vectors). Changing either use moves its
/// own frozen address set; changing them "together for consistency" breaks
/// both.
const WEBAUTHN_VERIFIERS_HEX: &str = "100";

/// SafeWebAuthnSignerProxy creation code (bare hex, no 0x). NOT the npm
/// `@safe-global/safe-passkey` 0.2.0 artifact (solc 0.8.24 — different
/// metadata hash, different addresses): extracted from the deployed v0.2.1
/// factory's runtime bytecode (solc 0.8.26, viaIR) and verified against
/// on-chain `getSigner` results — see the signer-address tests below.
pub const WEBAUTHN_SIGNER_PROXY_CREATION_CODE: &str = "610100346100ad57601f6101b538819003918201601f19168301916001600160401b038311848410176100b2578084926080946040528339810103126100ad578051906001600160a01b03821682036100ad5760208101516040820151606090920151926001600160b01b03841684036100ad5760805260a05260c05260e05260405160ec90816100c98239608051816082015260a05181604d015260c051816027015260e0518160010152f35b600080fd5b634e487b7160e01b600052604160045260246000fdfe7f000000000000000000000000000000000000000000000000000000000000000060b63601527f000000000000000000000000000000000000000000000000000000000000000060a03601527f000000000000000000000000000000000000000000000000000000000000000036608001523660006080376000806056360160807f00000000000000000000000000000000000000000000000000000000000000005af43d600060803e60b1573d6080fd5b3d6080f3fea26469706673582212201660515548d15702d720bbc046b457ca85e941a4559ab9f9518488e4c82e5ee964736f6c634300081a0033";

/// Safe v1.4.1 SafeProxyFactory proxy creation code (bare hex, no 0x).
pub const PROXY_CREATION_CODE: &str = "608060405234801561001057600080fd5b506040516101e63803806101e68339818101604052602081101561003357600080fd5b8101908080519060200190929190505050600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614156100ca576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260228152602001806101c46022913960400191505060405180910390fd5b806000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505060ab806101196000396000f3fe608060405273ffffffffffffffffffffffffffffffffffffffff600054167fa619486e0000000000000000000000000000000000000000000000000000000060003514156050578060005260206000f35b3660008037600080366000845af43d6000803e60008114156070573d6000fd5b3d6000f3fea264697066735822122003d1488ee65e08fa41e58e888a9865554c535f2c77126a82cb4c0f917f31441364736f6c63430007060033496e76616c69642073696e676c65746f6e20616464726573732070726f7669646564";

/// Constructor tail `CODECOPY; RETURN; INVALID` — the runtime region starts
/// right after it. Derived, never hardcoded, so creation/runtime cannot drift.
const PROXY_RUNTIME_SEPARATOR: &str = "6000396000f3fe";
/// Declared by the constructor's `PUSH1 0xab`.
const PROXY_RUNTIME_LEN_BYTES: usize = 0xab;

/// Deployed (runtime) bytecode of a Safe v1.4.1 proxy, 0x-prefixed — what
/// `eth_getCode` returns for any proxy from this factory (singleton lives in
/// storage, not code). Used to present counterfactual accounts as contracts.
pub fn safe_proxy_runtime_code() -> Result<String, CoreError> {
    let start = PROXY_CREATION_CODE
        .find(PROXY_RUNTIME_SEPARATOR)
        .ok_or_else(|| CoreError::Internal("proxy runtime separator not found".to_owned()))?
        + PROXY_RUNTIME_SEPARATOR.len();
    let end = start + PROXY_RUNTIME_LEN_BYTES * 2;
    let runtime = PROXY_CREATION_CODE
        .get(start..end)
        .ok_or_else(|| CoreError::Internal("proxy runtime region out of range".to_owned()))?;
    Ok(format!("0x{runtime}"))
}

// MARK: VelaGasSettlementSplitter — MUST stay byte-identical to
// vela-relay/shared/contracts/splitter.ts (see safe-address.ts CRITICAL note).

pub const VELA_SPLITTER_FACTORY: &str = "0x4e59b44847b379578588920cA78FbF26c0B4956C";
/// keccak256("vela.gas-settlement-splitter.v1") — bare hex, matches the bundler.
pub const VELA_SPLITTER_SALT: &str =
    "650cb20978a0e7efdcf6f077240c609a59f2f02401ed16fb4a222a2b51cb9720";
/// Pinned metadata-light / PUSH0-free forge build — bare hex, matches the bundler.
pub const VELA_SPLITTER_CREATION_CODE: &str = "60a060405234801561001057600080fd5b506040516105f13803806105f183398181016040528101906100329190610135565b600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1603610098576040517fd92e233d00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b8073ffffffffffffffffffffffffffffffffffffffff1660808173ffffffffffffffffffffffffffffffffffffffff168152505050610162565b600080fd5b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b6000610102826100d7565b9050919050565b610112816100f7565b811461011d57600080fd5b50565b60008151905061012f81610109565b92915050565b60006020828403121561014b5761014a6100d2565b5b600061015984828501610120565b91505092915050565b60805161045f6101926000396000818161010a01528181610199015281816101fa01526102b5015261045f6000f3fe6080604052600436106100225760003560e01c806361d027b31461028857610283565b36610283576000329050600060023461003b9190610310565b905060008111156100f85760008273ffffffffffffffffffffffffffffffffffffffff168260405161006c90610372565b60006040518083038185875af1925050503d80600081146100a9576040519150601f19603f3d011682016040523d82523d6000602084013e6100ae565b606091505b50509050806100f65782826040517f1c43b9760000000000000000000000000000000000000000000000000000000081526004016100ed9291906103d7565b60405180910390fd5b505b600047905060008111156101f85760007f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff168260405161014c90610372565b60006040518083038185875af1925050503d8060008114610189576040519150601f19603f3d011682016040523d82523d6000602084013e61018e565b606091505b50509050806101f6577f0000000000000000000000000000000000000000000000000000000000000000826040517f1c43b9760000000000000000000000000000000000000000000000000000000081526004016101ed9291906103d7565b60405180910390fd5b505b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff168373ffffffffffffffffffffffffffffffffffffffff167fa4e98e523c3e239a66755f9f6a3d3559544e2da7102a26ec45994c3a29599d4234858560405161027993929190610400565b60405180910390a3005b600080fd5b34801561029457600080fd5b5061029d6102b3565b6040516102aa9190610437565b60405180910390f35b7f000000000000000000000000000000000000000000000000000000000000000081565b6000819050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601260045260246000fd5b600061031b826102d7565b9150610326836102d7565b925082610336576103356102e1565b5b828204905092915050565b600081905092915050565b50565b600061035c600083610341565b91506103678261034c565b600082019050919050565b600061037d8261034f565b9150819050919050565b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b60006103b282610387565b9050919050565b6103c2816103a7565b82525050565b6103d1816102d7565b82525050565b60006040820190506103ec60008301856103b9565b6103f960208301846103c8565b9392505050565b600060608201905061041560008301866103c8565b61042260208301856103c8565b61042f60408301846103c8565b949350505050565b600060208201905061044c60008301846103b9565b9291505056fea164736f6c634300081c000a";

/// Strict uncompressed P-256 public-key parse: optional 0x, optional 04 SEC1
/// tag, then exactly 64 bytes of x‖y. Format validation only — no curve check
/// (keys here come from the passkey layer; on-curve checks live in webauthn
/// extraction/recovery where keys originate). The TS original returned empty
/// arrays on bad input — enumerated divergence.
pub fn parse_public_key(hex: &str) -> Result<P256PublicKey, CoreError> {
    let clean = hex.strip_prefix("0x").unwrap_or(hex);
    let clean = clean.strip_prefix("04").unwrap_or(clean);
    if clean.len() != 128 {
        return Err(CoreError::InvalidPublicKey(format!(
            "expected 128 hex chars of x‖y, got {}",
            clean.len()
        )));
    }
    let bytes = primitives::from_hex(clean)
        .map_err(|_| CoreError::InvalidPublicKey("not valid hex".to_owned()))?;
    Ok(P256PublicKey {
        x: bytes[..32].to_vec(),
        y: bytes[32..].to_vec(),
    })
}

/// One MultiSend sub-transaction:
/// `operation(1) ‖ to(20) ‖ value(32, zero) ‖ dataLen(32) ‖ data`.
fn encode_multisend_tx(to: &str, data: &[u8], operation: u8) -> Result<Vec<u8>, CoreError> {
    let to_addr = parse_address(to)?;
    let mut out = Vec::with_capacity(85 + data.len());
    out.push(operation);
    out.extend_from_slice(to_addr.as_slice());
    out.extend_from_slice(&[0u8; 32]);
    out.extend_from_slice(&primitives::abi_encode_uint256(&format!(
        "{:x}",
        data.len()
    ))?);
    out.extend_from_slice(data);
    Ok(out)
}

/// Counterfactual per-key WebAuthn signer owner:
/// `factory.getSigner(x, y, verifiers)` reproduced offline — CREATE2 with
/// salt 0 over `proxyCreationCode ‖ uint256(singleton) ‖ x ‖ y ‖
/// uint256(verifiers)`. An owner with no code cannot pass Safe's ERC-1271
/// check (`isValidSignature` staticcall — empty return, no magic value,
/// GS024), so the multi-key setup data deploys every extra signer inside the
/// setup MultiSend (see `encode_setup_data`): any key — not just keys[0] —
/// can sign the very first user operation, in the same transaction that
/// deploys the Safe.
pub fn compute_webauthn_signer_address(x: &[u8], y: &[u8]) -> Result<String, CoreError> {
    require_coords(x, y)?;
    let mut init_code =
        primitives::from_hex(WEBAUTHN_SIGNER_PROXY_CREATION_CODE).map_err(|_| {
            CoreError::Internal("WEBAUTHN_SIGNER_PROXY_CREATION_CODE not hex".to_owned())
        })?;
    init_code.extend_from_slice(&primitives::abi_encode_address(WEBAUTHN_SIGNER_SINGLETON)?);
    init_code.extend_from_slice(&primitives::abi_encode_bytes32(x)?);
    init_code.extend_from_slice(&primitives::abi_encode_bytes32(y)?);
    init_code.extend_from_slice(&primitives::abi_encode_uint256(WEBAUTHN_VERIFIERS_HEX)?);
    let init_code_hash = keccak256(&init_code);
    primitives::create2_address(
        WEBAUTHN_SIGNER_FACTORY,
        &[0u8; 32],
        init_code_hash.as_slice(),
    )
}

fn require_coords(x: &[u8], y: &[u8]) -> Result<(), CoreError> {
    if x.len() != 32 || y.len() != 32 {
        return Err(CoreError::InvalidPublicKey(format!(
            "coordinates must be 32 bytes each, got {}/{}",
            x.len(),
            y.len()
        )));
    }
    Ok(())
}

/// Safe.setup() calldata: owners=[WEBAUTHN_SIGNER, one factory signer per
/// extra key], threshold=1, `to`=MultiSend delegatecall packing (1)
/// SAFE_MODULE_SETUP.enableModules([SAFE_4337_MODULE]), (2)
/// WEBAUTHN_SIGNER.configure((x,y,verifiers=0x100 — the RIP-7212 precompile
/// address)) and (3) one FACTORY.createSigner per extra key;
/// fallbackHandler = SAFE_4337_MODULE. Only the FIRST key is configured on
/// the shared signer; extra owners carry their own key via their proxy's
/// immutables.
///
/// DESIGN DECISION: extra signer proxies ARE deployed here, inside setup.
/// First use stays atomic — even with keys[0] lost, any other key signs the
/// deployment userOp in one transaction (initCode runs setup before
/// validateUserOp, so every owner has code by signature-check time). The
/// cost is a fatter initCode on multi-key wallets only. Do not change
/// casually: setupData feeds the CREATE2 salt, so ANY byte here moves every
/// multi-key Safe address.
fn encode_setup_data(
    x: &[u8],
    y: &[u8],
    extra_keys: &[&P256PublicKey],
) -> Result<Vec<u8>, CoreError> {
    let enable_modules = {
        let mut d = primitives::function_selector("enableModules(address[])")?;
        d.extend_from_slice(&primitives::abi_encode_uint256("0x20")?);
        d.extend_from_slice(&primitives::abi_encode_uint256("0x1")?);
        d.extend_from_slice(&primitives::abi_encode_address(SAFE_4337_MODULE)?);
        d
    };
    let configure = {
        let mut d = primitives::function_selector("configure((uint256,uint256,uint176))")?;
        d.extend_from_slice(&primitives::abi_encode_bytes32(x)?);
        d.extend_from_slice(&primitives::abi_encode_bytes32(y)?);
        d.extend_from_slice(&primitives::abi_encode_uint256(WEBAUTHN_VERIFIERS_HEX)?); // RIP-7212 precompile @ 0x100
        d
    };

    let mut packed = encode_multisend_tx(SAFE_MODULE_SETUP, &enable_modules, 1)?;
    packed.extend_from_slice(&encode_multisend_tx(WEBAUTHN_SIGNER, &configure, 1)?);

    // Deploy each extra key's signer proxy in the same setup transaction.
    // MUST be a plain CALL (operation 0): a delegatecall would run CREATE2 in
    // the Safe's context, the deployed address would not match the owner
    // below, and the factory's `assert(address(created) == signer)` would
    // revert the whole deployment.
    let mut extra_owners = Vec::with_capacity(extra_keys.len());
    for key in extra_keys {
        extra_owners.push(compute_webauthn_signer_address(&key.x, &key.y)?);
        let create_signer = {
            let mut d = primitives::function_selector("createSigner(uint256,uint256,uint176)")?;
            d.extend_from_slice(&primitives::abi_encode_bytes32(&key.x)?);
            d.extend_from_slice(&primitives::abi_encode_bytes32(&key.y)?);
            d.extend_from_slice(&primitives::abi_encode_uint256(WEBAUTHN_VERIFIERS_HEX)?);
            d
        };
        packed.extend_from_slice(&encode_multisend_tx(
            WEBAUTHN_SIGNER_FACTORY,
            &create_signer,
            0,
        )?);
    }

    let multi_send_data = {
        let mut d = primitives::function_selector("multiSend(bytes)")?;
        d.extend_from_slice(&primitives::abi_encode_uint256("0x20")?);
        d.extend_from_slice(&primitives::abi_encode_uint256(&format!(
            "{:x}",
            packed.len()
        ))?);
        d.extend_from_slice(&packed);
        let padding = (32 - (packed.len() % 32)) % 32;
        d.extend_from_slice(&vec![0u8; padding]);
        d
    };

    let owner_count = 1 + extra_owners.len();
    let mut out = primitives::function_selector(
        "setup(address[],uint256,address,bytes,address,address,uint256,address)",
    )?;
    out.extend_from_slice(&primitives::abi_encode_uint256("0x100")?); // owners offset (8 head words)
    out.extend_from_slice(&primitives::abi_encode_uint256("0x1")?); // threshold (1-of-N)
    out.extend_from_slice(&primitives::abi_encode_address(MULTI_SEND)?);
    // data offset = owners offset + owners.length word + N address words
    out.extend_from_slice(&primitives::abi_encode_uint256(&format!(
        "{:x}",
        0x120 + owner_count * 0x20
    ))?);
    out.extend_from_slice(&primitives::abi_encode_address(SAFE_4337_MODULE)?); // fallbackHandler
    out.extend_from_slice(&primitives::abi_encode_address(
        "0x0000000000000000000000000000000000000000",
    )?);
    out.extend_from_slice(&primitives::abi_encode_uint256("0x0")?);
    out.extend_from_slice(&primitives::abi_encode_address(
        "0x0000000000000000000000000000000000000000",
    )?);
    out.extend_from_slice(&primitives::abi_encode_uint256(&format!(
        "{owner_count:x}"
    ))?); // owners.length
    out.extend_from_slice(&primitives::abi_encode_address(WEBAUTHN_SIGNER)?);
    for owner in &extra_owners {
        out.extend_from_slice(&primitives::abi_encode_address(owner)?);
    }
    out.extend_from_slice(&primitives::abi_encode_uint256(&format!(
        "{:x}",
        multi_send_data.len()
    ))?);
    out.extend_from_slice(&multi_send_data);
    let padding = (32 - (multi_send_data.len() % 32)) % 32;
    out.extend_from_slice(&vec![0u8; padding]);
    Ok(out)
}

/// The wallet identity: counterfactual Safe address + assembly ingredients.
pub fn compute_safe_address(x: &[u8], y: &[u8]) -> Result<SafeAddressInfo, CoreError> {
    require_coords(x, y)?;
    // saltNonce = keccak256(bytes32(x) ‖ bytes32(y))
    let mut xy = primitives::abi_encode_bytes32(x)?;
    xy.extend_from_slice(&primitives::abi_encode_bytes32(y)?);
    let salt_nonce = keccak256(&xy);
    let setup_data = encode_setup_data(x, y, &[])?;
    assemble_safe_address(salt_nonce.to_vec(), setup_data)
}

/// Multi-device wallet identity: one Safe owned by N passkeys, threshold 1.
/// `keys[0]` drives the shared WEBAUTHN_SIGNER exactly as the single-key path
/// does (owner 0 + `configure()`); every later key becomes its own
/// counterfactual factory signer owner — deployed lazily via
/// `factory.createSigner` before that key's first signature (see
/// `encode_setup_data` — deployed inside the setup MultiSend, so any key can
/// sign from the very first transaction). `compute_safe_address_multi(&[k])`
/// is byte-identical to `compute_safe_address(&k.x, &k.y)` — existing
/// single-key addresses never change.
///
/// Canonical identity: `keys[0]` is pinned (it is the shared-signer key) and
/// later keys are SORTED by x‖y before hashing and owner encoding, so the
/// caller's enumeration order can never move the address. Duplicate keys are
/// rejected: a repeated owner makes `setup()` revert (GS203/GS204) while
/// CREATE2 commits to exactly that initializer — the result would be a
/// fundable address nothing can ever deploy. At most [`MAX_MULTI_KEYS`]
/// keys. saltNonce generalizes to keccak256(x₀‖y₀‖…‖xₙ‖yₙ) over the
/// canonical order.
pub fn compute_safe_address_multi(keys: &[P256PublicKey]) -> Result<SafeAddressInfo, CoreError> {
    if keys.len() > MAX_MULTI_KEYS {
        return Err(CoreError::InvalidPublicKey(format!(
            "at most {MAX_MULTI_KEYS} keys per wallet, got {}",
            keys.len()
        )));
    }
    let (first, rest) = keys
        .split_first()
        .ok_or_else(|| CoreError::InvalidPublicKey("at least one key required".to_owned()))?;
    for key in keys {
        require_coords(&key.x, &key.y)?;
    }
    let mut rest: Vec<&P256PublicKey> = rest.iter().collect();
    rest.sort_by(|a, b| a.x.cmp(&b.x).then_with(|| a.y.cmp(&b.y)));
    let duplicated = rest
        .windows(2)
        .any(|pair| pair[0].x == pair[1].x && pair[0].y == pair[1].y)
        || rest.iter().any(|k| k.x == first.x && k.y == first.y);
    if duplicated {
        return Err(CoreError::InvalidPublicKey(
            "duplicate public key in owner set".to_owned(),
        ));
    }
    let mut all_xy = Vec::with_capacity(keys.len() * 64);
    for key in std::iter::once(&first).chain(rest.iter()) {
        all_xy.extend_from_slice(&primitives::abi_encode_bytes32(&key.x)?);
        all_xy.extend_from_slice(&primitives::abi_encode_bytes32(&key.y)?);
    }
    let salt_nonce = keccak256(&all_xy);
    let setup_data = encode_setup_data(&first.x, &first.y, &rest)?;
    assemble_safe_address(salt_nonce.to_vec(), setup_data)
}

/// Shared CREATE2 tail: salt derivation + proxy init-code hash + address.
fn assemble_safe_address(
    salt_nonce: Vec<u8>,
    setup_data: Vec<u8>,
) -> Result<SafeAddressInfo, CoreError> {
    // initCodeHash = keccak256(creationCode ‖ abi.encode(singleton))
    let mut deployment_code = primitives::from_hex(PROXY_CREATION_CODE)
        .map_err(|_| CoreError::Internal("PROXY_CREATION_CODE not hex".to_owned()))?;
    deployment_code.extend_from_slice(&primitives::abi_encode_address(SAFE_SINGLETON)?);
    let init_code_hash = keccak256(&deployment_code);

    // salt = keccak256(bytes32(keccak256(setupData)) ‖ bytes32(saltNonce))
    let mut salt_input = keccak256(&setup_data).to_vec();
    salt_input.extend_from_slice(&salt_nonce);
    let salt = keccak256(&salt_input);

    let address = primitives::create2_address(
        SAFE_PROXY_FACTORY,
        salt.as_slice(),
        init_code_hash.as_slice(),
    )?;
    Ok(SafeAddressInfo {
        address,
        salt_nonce,
        setup_data,
        init_code_hash: init_code_hash.to_vec(),
    })
}

/// CREATE2 init code for the splitter: `creationCode ‖ abi.encode(treasury)`.
fn splitter_init_code(treasury: &str) -> Result<Vec<u8>, CoreError> {
    let mut code = primitives::from_hex(VELA_SPLITTER_CREATION_CODE)
        .map_err(|_| CoreError::Internal("VELA_SPLITTER_CREATION_CODE not hex".to_owned()))?;
    code.extend_from_slice(&primitives::abi_encode_address(treasury)?);
    Ok(code)
}

/// Deterministic splitter address for a treasury — must equal the bundler's
/// computeSplitterAddress byte-for-byte.
pub fn compute_splitter_address(treasury_hex: &str) -> Result<String, CoreError> {
    let init_code_hash = keccak256(&splitter_init_code(treasury_hex)?);
    let salt = primitives::from_hex(VELA_SPLITTER_SALT)
        .map_err(|_| CoreError::Internal("VELA_SPLITTER_SALT not hex".to_owned()))?;
    primitives::create2_address(VELA_SPLITTER_FACTORY, &salt, init_code_hash.as_slice())
}

/// Raw Arachnid-factory deploy calldata: `salt(32) ‖ initCode`.
pub fn encode_splitter_deploy_call(treasury_hex: &str) -> Result<Vec<u8>, CoreError> {
    let mut out = primitives::from_hex(VELA_SPLITTER_SALT)
        .map_err(|_| CoreError::Internal("VELA_SPLITTER_SALT not hex".to_owned()))?;
    out.extend_from_slice(&splitter_init_code(treasury_hex)?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(x_hex: &str, y_hex: &str) -> Result<P256PublicKey, CoreError> {
        Ok(P256PublicKey {
            x: primitives::from_hex(x_hex)?,
            y: primitives::from_hex(y_hex)?,
        })
    }

    fn word(data: &[u8], index: usize) -> &[u8] {
        &data[4 + 32 * index..4 + 32 * (index + 1)]
    }

    /// Chain-verified 2026-08-14 against `factory.getSigner(x, y, 0x100)`
    /// eth_calls to 0x1d31F259eE307358a26dFb23EB365939E8641195 on Gnosis.
    /// A mismatch here means WEBAUTHN_SIGNER_PROXY_CREATION_CODE drifted from
    /// the deployed factory — extra-owner addresses would be unspendable.
    #[test]
    fn signer_address_matches_onchain_get_signer() -> Result<(), CoreError> {
        let cases = [
            (
                "0000000000000000000000000000000000000000000000000000000000000001",
                "0000000000000000000000000000000000000000000000000000000000000002",
                "0xE03432AB61033B62072aE0CCAb7550C731955De7",
            ),
            (
                "8f9b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff0",
                "7a8b9cadbecfd0e1f20314253647586970818293a4b5c6d7e8f9010203040506",
                "0xEBc27e114a6386c331098F1d1d6B53555c1DD5C7",
            ),
            (
                "04d2163f5c2c9a5a3f0e1d2c3b4a59687766554433221100ffeeddccbbaa9988",
                "1122334455667788990a0b0c0d0e0f102132435465768798a9bacbdcedfe0f21",
                "0xfD26982E29A3C9378fA9e39795529021Dcc72fdC",
            ),
        ];
        for (x_hex, y_hex, expected) in cases {
            let k = key(x_hex, y_hex)?;
            assert_eq!(compute_webauthn_signer_address(&k.x, &k.y)?, expected);
        }
        Ok(())
    }

    /// The release-blocking invariant: the multi path with one key MUST stay
    /// byte-identical to the legacy single-key path.
    #[test]
    fn multi_with_one_key_equals_single() -> Result<(), CoreError> {
        let k = key(
            "8f9b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff0",
            "7a8b9cadbecfd0e1f20314253647586970818293a4b5c6d7e8f9010203040506",
        )?;
        let single = compute_safe_address(&k.x, &k.y)?;
        let multi = compute_safe_address_multi(std::slice::from_ref(&k))?;
        assert_eq!(single, multi);
        Ok(())
    }

    /// Head layout independently cross-checked byte-for-byte against foundry
    /// `cast calldata` for the same logical setup() arguments.
    #[test]
    fn multi_owner_setup_layout() -> Result<(), CoreError> {
        let keys = [
            key(
                "8f9b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff0",
                "7a8b9cadbecfd0e1f20314253647586970818293a4b5c6d7e8f9010203040506",
            )?,
            key(
                "0000000000000000000000000000000000000000000000000000000000000001",
                "0000000000000000000000000000000000000000000000000000000000000002",
            )?,
            key(
                "04d2163f5c2c9a5a3f0e1d2c3b4a59687766554433221100ffeeddccbbaa9988",
                "1122334455667788990a0b0c0d0e0f102132435465768798a9bacbdcedfe0f21",
            )?,
        ];
        let info = compute_safe_address_multi(&keys)?;
        let setup = &info.setup_data;

        assert_eq!(word(setup, 0), primitives::abi_encode_uint256("0x100")?);
        assert_eq!(word(setup, 1), primitives::abi_encode_uint256("0x1")?); // threshold
        assert_eq!(word(setup, 3), primitives::abi_encode_uint256("0x180")?); // data offset
        assert_eq!(word(setup, 8), primitives::abi_encode_uint256("0x3")?); // owners.length
        assert_eq!(
            word(setup, 9),
            primitives::abi_encode_address(WEBAUTHN_SIGNER)?
        );
        assert_eq!(
            word(setup, 10),
            primitives::abi_encode_address("0xE03432AB61033B62072aE0CCAb7550C731955De7")?
        );
        assert_eq!(
            word(setup, 11),
            primitives::abi_encode_address("0xfD26982E29A3C9378fA9e39795529021Dcc72fdC")?
        );

        // First two MultiSend sub-txs (enableModules + configure with
        // keys[0]) are the single-key payload verbatim; each extra key then
        // appends a createSigner CALL deploying its signer proxy at setup.
        let single = compute_safe_address(&keys[0].x, &keys[0].y)?;
        let sub = 85 + 100; // op‖to‖value‖dataLen + selector‖3 arg words
        let m_base = 4 + 0x180 + 32 + 4 + 32 + 32; // multiSend inner packed
        let s_base = 4 + 0x140 + 32 + 4 + 32 + 32;
        assert_eq!(
            &setup[m_base..m_base + 2 * sub],
            &single.setup_data[s_base..s_base + 2 * sub]
        );
        for (i, k) in keys[1..].iter().enumerate() {
            let tx = &setup[m_base + (2 + i) * sub..m_base + (3 + i) * sub];
            assert_eq!(tx[0], 0, "createSigner must be CALL, not delegatecall");
            assert_eq!(
                &tx[1..21],
                &primitives::abi_encode_address(WEBAUTHN_SIGNER_FACTORY)?[12..]
            );
            assert_eq!(&tx[21..53], &[0u8; 32][..]); // value
            assert_eq!(
                &tx[53..85],
                primitives::abi_encode_uint256("64")?.as_slice()
            ); // dataLen
            assert_eq!(&tx[85..89], [0x0d, 0x2f, 0x04, 0x89]); // createSigner selector
            assert_eq!(
                &tx[89..121],
                primitives::abi_encode_bytes32(&k.x)?.as_slice()
            );
            assert_eq!(
                &tx[121..153],
                primitives::abi_encode_bytes32(&k.y)?.as_slice()
            );
            assert_eq!(
                &tx[153..185],
                primitives::abi_encode_uint256(WEBAUTHN_VERIFIERS_HEX)?.as_slice()
            );
        }
        Ok(())
    }

    /// keys[1..] are functionally symmetric, so their enumeration order must
    /// never move the address — the canonical sort makes [k0,k1,k2] and
    /// [k0,k2,k1] the same wallet, while moving keys[0] (the shared-signer
    /// key) legitimately changes identity.
    #[test]
    fn multi_is_order_canonical_beyond_first_key() -> Result<(), CoreError> {
        let k0 = key(
            "8f9b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff0",
            "7a8b9cadbecfd0e1f20314253647586970818293a4b5c6d7e8f9010203040506",
        )?;
        let k1 = key(
            "0000000000000000000000000000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000002",
        )?;
        let k2 = key(
            "04d2163f5c2c9a5a3f0e1d2c3b4a59687766554433221100ffeeddccbbaa9988",
            "1122334455667788990a0b0c0d0e0f102132435465768798a9bacbdcedfe0f21",
        )?;
        let sorted = compute_safe_address_multi(&[k0.clone(), k1.clone(), k2.clone()])?;
        let shuffled = compute_safe_address_multi(&[k0.clone(), k2.clone(), k1.clone()])?;
        assert_eq!(sorted, shuffled);
        let other_first = compute_safe_address_multi(&[k1, k0, k2])?;
        assert_ne!(sorted.address, other_first.address);
        Ok(())
    }

    #[test]
    fn multi_rejects_empty_bad_coords_and_duplicates() -> Result<(), CoreError> {
        assert!(compute_safe_address_multi(&[]).is_err());
        let good = key(
            "0000000000000000000000000000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000002",
        )?;
        let other = key(
            "04d2163f5c2c9a5a3f0e1d2c3b4a59687766554433221100ffeeddccbbaa9988",
            "1122334455667788990a0b0c0d0e0f102132435465768798a9bacbdcedfe0f21",
        )?;
        let bad = P256PublicKey {
            x: vec![1u8; 31],
            y: vec![2u8; 32],
        };
        assert!(compute_safe_address_multi(&[good.clone(), bad]).is_err());
        // Duplicate among keys[1..]: setup() would revert (GS203/GS204) yet
        // CREATE2 commits to that initializer — a permanent black hole.
        assert!(compute_safe_address_multi(&[good.clone(), other.clone(), other.clone()]).is_err());
        // keys[0] repeated later would deploy, but is always a caller bug.
        assert!(compute_safe_address_multi(&[good.clone(), good]).is_err());
        Ok(())
    }

    /// Absolute anchors: fixed inputs → full pinned outputs. The three-key
    /// vector was triple-verified externally (foundry cast byte-identical
    /// setup encoding, an independent review probe, and the same code path
    /// deployed real 3/13/21-key wallets on Gnosis); the one-key case must
    /// equal the frozen single-key identity vector. Relative invariants
    /// (multi-vs-single, sorted-vs-shuffled) cannot catch a systematic
    /// change that moves every variant together — THIS test does.
    #[test]
    fn multi_pinned_output_vectors() -> Result<(), CoreError> {
        let keys = [
            key(
                "8f9b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff0",
                "7a8b9cadbecfd0e1f20314253647586970818293a4b5c6d7e8f9010203040506",
            )?,
            key(
                "0000000000000000000000000000000000000000000000000000000000000001",
                "0000000000000000000000000000000000000000000000000000000000000002",
            )?,
            key(
                "04d2163f5c2c9a5a3f0e1d2c3b4a59687766554433221100ffeeddccbbaa9988",
                "1122334455667788990a0b0c0d0e0f102132435465768798a9bacbdcedfe0f21",
            )?,
        ];
        let info = compute_safe_address_multi(&keys)?;
        assert_eq!(info.address, "0x5AF6Cd8689C013192e157826f7C4574d7C2f9446");
        assert_eq!(
            primitives::to_hex(&info.salt_nonce, true),
            "0x66c406059aeec0effec5c28a2e3accc5f3fe0553c94afb643e3a33fc4aaab313"
        );
        assert_eq!(info.setup_data.len(), 1284);
        assert_eq!(
            primitives::to_hex(keccak256(&info.setup_data).as_slice(), true),
            "0x7e1d099d46166dff8b1ad992a8cb4b0cf834bc7c6c2253b1f8289fffd1797088"
        );

        // Two keys: pins the N=2 shape (data offset 0x160) the layout test
        // does not touch.
        let two = compute_safe_address_multi(&[keys[0].clone(), keys[2].clone()])?;
        assert_eq!(two.address, "0xaBeF0bf37A03a2Af821Cf409a52eB9C01524b2E0");
        assert_eq!(
            word(&two.setup_data, 3),
            primitives::abi_encode_uint256("0x160")?
        );
        assert_eq!(
            word(&two.setup_data, 8),
            primitives::abi_encode_uint256("0x2")?
        );

        // One key: the frozen single-key identity vector, through multi.
        let identity = key(
            "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90",
            "b1c2d3e4f50617283940a1b2c3d4e5f6b1c2d3e4f50617283940a1b2c3d4e5f6",
        )?;
        let one = compute_safe_address_multi(std::slice::from_ref(&identity))?;
        assert_eq!(one.address, "0x762EdA60D3B68755c271D608644650278f88329F");
        Ok(())
    }

    /// Full-cap pin: N = MAX_MULTI_KEYS freezes the largest reachable setup
    /// encoding (address + bytes). Every reachable count now sits inside the
    /// decimal/hex coincidence zone (N ≤ 9) of `format!("{owner_count:x}")`
    /// feeding the hex-parsing `abi_encode_uint256`; if the cap ever rises
    /// past 9 again, re-pin above the zone — at N ≥ 10 a decimal regression
    /// moves every address and setup() reverts on-chain (the retired 21-key
    /// pin existed for exactly that).
    #[test]
    fn multi_full_cap_layout_and_pin() -> Result<(), CoreError> {
        let keys: Vec<P256PublicKey> = (0..MAX_MULTI_KEYS as u8)
            .map(|i| {
                let mut x = [0u8; 32];
                let mut y = [0u8; 32];
                x[31] = i;
                y[0] = 1;
                y[31] = i;
                P256PublicKey {
                    x: x.to_vec(),
                    y: y.to_vec(),
                }
            })
            .collect();
        let info = compute_safe_address_multi(&keys)?;
        assert_eq!(info.address, "0xc94f9367f7DD7cd5c8e790b59356093Bc6AA0365");
        assert_eq!(
            primitives::to_hex(keccak256(&info.setup_data).as_slice(), true),
            "0x6a24ad2f51afa8858b15982512e94743a5c594a0f96e9d9210b5010ae360cfd0"
        );
        // owners.length = 7; data offset = 0x120 + 7·0x20 = 0x200.
        assert_eq!(
            word(&info.setup_data, 8),
            primitives::abi_encode_uint256("0x7")?
        );
        assert_eq!(
            word(&info.setup_data, 3),
            primitives::abi_encode_uint256("0x200")?
        );
        Ok(())
    }

    /// Keys sharing an x are a real P-256 shape ((x, y) and (x, p−y) are
    /// both on-curve): the y tie-break must order them canonically, and the
    /// duplicate check must NOT fire on them.
    #[test]
    fn multi_sort_tiebreak_accepts_shared_x() -> Result<(), CoreError> {
        let k0 = key(
            "8f9b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff0",
            "7a8b9cadbecfd0e1f20314253647586970818293a4b5c6d7e8f9010203040506",
        )?;
        let shared_x = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let y1 = key(
            shared_x,
            "0000000000000000000000000000000000000000000000000000000000000001",
        )?;
        let y2 = key(
            shared_x,
            "0000000000000000000000000000000000000000000000000000000000000002",
        )?;
        let a = compute_safe_address_multi(&[k0.clone(), y2.clone(), y1.clone()])?;
        let b = compute_safe_address_multi(&[k0, y1, y2])?;
        assert_eq!(a, b);
        assert_eq!(a.address, "0x689FaD35F595204ea5cB82cfa80046A45a81288D");
        Ok(())
    }

    /// Duplicates that are NOT adjacent in caller order must still be caught
    /// (the check runs on the sorted list), and a bad first key or bad
    /// signer coords must be rejected everywhere they can enter.
    #[test]
    fn multi_rejects_non_adjacent_duplicates_and_bad_first_key() -> Result<(), CoreError> {
        let k0 = key(
            "8f9b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff0",
            "7a8b9cadbecfd0e1f20314253647586970818293a4b5c6d7e8f9010203040506",
        )?;
        let k1 = key(
            "0000000000000000000000000000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000002",
        )?;
        let k2 = key(
            "04d2163f5c2c9a5a3f0e1d2c3b4a59687766554433221100ffeeddccbbaa9988",
            "1122334455667788990a0b0c0d0e0f102132435465768798a9bacbdcedfe0f21",
        )?;
        // k1 duplicated with k2 between the copies in caller order.
        assert!(compute_safe_address_multi(&[k0, k1.clone(), k2.clone(), k1.clone()]).is_err());
        // Bad coords in keys[0], not just in the rest.
        let bad = P256PublicKey {
            x: vec![1u8; 31],
            y: vec![2u8; 32],
        };
        assert!(compute_safe_address_multi(&[bad, k1, k2]).is_err());
        // The standalone signer-address entry point validates too.
        assert!(compute_webauthn_signer_address(&[1u8; 31], &[2u8; 32]).is_err());
        Ok(())
    }

    #[test]
    fn multi_enforces_max_key_count() -> Result<(), CoreError> {
        let make = |i: u8| {
            let mut x = [0u8; 32];
            let mut y = [0u8; 32];
            x[31] = i;
            y[0] = 1; // distinct from every x so no cross-duplicates
            y[31] = i;
            P256PublicKey {
                x: x.to_vec(),
                y: y.to_vec(),
            }
        };
        let at_cap: Vec<P256PublicKey> = (0..MAX_MULTI_KEYS as u8).map(make).collect();
        assert!(compute_safe_address_multi(&at_cap).is_ok());
        let over: Vec<P256PublicKey> = (0..=MAX_MULTI_KEYS as u8).map(make).collect();
        assert!(compute_safe_address_multi(&over).is_err());
        Ok(())
    }
}
