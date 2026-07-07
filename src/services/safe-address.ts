/**
 * Computes the deterministic Safe wallet address from a P-256 public key.
 * TypeScript port of SafeAddressComputer.swift — must produce identical results.
 */

import {
  keccak256,
  abiEncodeAddress,
  abiEncodeUint256,
  abiEncodeUint256Hex,
  abiEncodeBytes32,
  functionSelector,
  create2Address,
  checksumAddress,
} from './eth-crypto';

import { fromHex, toHex, concatBytes, stripHexPrefix } from './hex';

// MARK: - Contract Addresses (all EVM chains)

export const SAFE_PROXY_FACTORY = '0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67';
export const SAFE_SINGLETON = '0x29fcB43b46531BcA003ddC8FCB67FFE91900C762';
export const FALLBACK_HANDLER = '0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99';
export const ENTRY_POINT = '0x0000000071727De22E5E9d8BAf0edAc6f37da032';
export const SAFE_4337_MODULE = '0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226';
export const SAFE_MODULE_SETUP = '0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47';
export const WEBAUTHN_SIGNER = '0x94a4F6affBd8975951142c3999aEAB7ecee555c2';
export const MULTI_SEND = '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526';

// Safe Proxy creation code (from SafeProxyFactory)
export const PROXY_CREATION_CODE =
  '608060405234801561001057600080fd5b506040516101e63803806101e68339818101604052602081101561003357600080fd5b8101908080519060200190929190505050600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614156100ca576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260228152602001806101c46022913960400191505060405180910390fd5b806000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505060ab806101196000396000f3fe608060405273ffffffffffffffffffffffffffffffffffffffff600054167fa619486e0000000000000000000000000000000000000000000000000000000060003514156050578060005260206000f35b3660008037600080366000845af43d6000803e60008114156070573d6000fd5b3d6000f3fea264697066735822122003d1488ee65e08fa41e58e888a9865554c535f2c77126a82cb4c0f917f31441364736f6c63430007060033496e76616c69642073696e676c65746f6e20616464726573732070726f7669646564';

/**
 * Deployed (runtime) bytecode of a Safe v1.4.1 SafeProxy — what `eth_getCode`
 * returns once the account is on-chain. It is IDENTICAL for every proxy from
 * this factory: the proxy reads its singleton from storage slot 0 at call time
 * (`sload(0)`), so the singleton address is NOT embedded in the code. Only
 * storage differs between accounts, not the runtime code.
 *
 * Derived from PROXY_CREATION_CODE rather than hardcoded separately, so the two
 * can never drift: the constructor ends with `…6000396000f3fe` (CODECOPY; RETURN;
 * INVALID separator) and returns the following 0xab (171) bytes as runtime.
 *
 * Used to answer `eth_getCode` for a counterfactual (not-yet-deployed) Vela
 * account with non-empty code, so dApps detect it as a smart contract wallet
 * (EIP-1271) instead of an EOA.
 */
const _PROXY_RUNTIME_SEPARATOR = '6000396000f3fe';
const _PROXY_RUNTIME_LEN_BYTES = 0xab; // declared by the constructor's `PUSH1 0xab`
export const SAFE_PROXY_RUNTIME_CODE: string = (() => {
  const start = PROXY_CREATION_CODE.indexOf(_PROXY_RUNTIME_SEPARATOR) + _PROXY_RUNTIME_SEPARATOR.length;
  return '0x' + PROXY_CREATION_CODE.slice(start, start + _PROXY_RUNTIME_LEN_BYTES * 2);
})();

// MARK: - VelaGasSettlementSplitter (handleOps beneficiary on native chains)

/**
 * The bundler pays the collected gas fees to a VelaGasSettlementSplitter deployed once per
 * chain via the Arachnid CREATE2 factory; the contract splits msg.value 50/50 between the
 * bundler EOA (tx.origin) and the treasury. When it isn't deployed yet, the wallet prepends a
 * CREATE2 deploy call into the UserOp's MultiSend batch so it exists before the EntryPoint
 * pays the beneficiary (mirrors the Safe self-deploy gate).
 *
 * CRITICAL: these three constants MUST be byte-identical to the bundler's
 * (vela-bundler/shared/contracts/splitter.ts). If any of them drift, the wallet deploys a
 * splitter at a different address than the bundler pays, and the split silently breaks. The
 * creation code is the pinned metadata-light / PUSH0-free build; regenerate ONLY via
 * `forge inspect VelaGasSettlementSplitter bytecode` and update both repos + the golden test
 * vectors together.
 */
export const VELA_SPLITTER_FACTORY = '0x4e59b44847b379578588920cA78FbF26c0B4956C';

/** keccak256("vela.gas-settlement-splitter.v1") — bare hex (no 0x), matches the bundler. */
export const VELA_SPLITTER_SALT =
  '650cb20978a0e7efdcf6f077240c609a59f2f02401ed16fb4a222a2b51cb9720';

/** Compiled VelaGasSettlementSplitter creation code — bare hex, byte-identical to the bundler. */
export const VELA_SPLITTER_CREATION_CODE =
  '60a060405234801561001057600080fd5b506040516105f13803806105f183398181016040528101906100329190610135565b600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1603610098576040517fd92e233d00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b8073ffffffffffffffffffffffffffffffffffffffff1660808173ffffffffffffffffffffffffffffffffffffffff168152505050610162565b600080fd5b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b6000610102826100d7565b9050919050565b610112816100f7565b811461011d57600080fd5b50565b60008151905061012f81610109565b92915050565b60006020828403121561014b5761014a6100d2565b5b600061015984828501610120565b91505092915050565b60805161045f6101926000396000818161010a01528181610199015281816101fa01526102b5015261045f6000f3fe6080604052600436106100225760003560e01c806361d027b31461028857610283565b36610283576000329050600060023461003b9190610310565b905060008111156100f85760008273ffffffffffffffffffffffffffffffffffffffff168260405161006c90610372565b60006040518083038185875af1925050503d80600081146100a9576040519150601f19603f3d011682016040523d82523d6000602084013e6100ae565b606091505b50509050806100f65782826040517f1c43b9760000000000000000000000000000000000000000000000000000000081526004016100ed9291906103d7565b60405180910390fd5b505b600047905060008111156101f85760007f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff168260405161014c90610372565b60006040518083038185875af1925050503d8060008114610189576040519150601f19603f3d011682016040523d82523d6000602084013e61018e565b606091505b50509050806101f6577f0000000000000000000000000000000000000000000000000000000000000000826040517f1c43b9760000000000000000000000000000000000000000000000000000000081526004016101ed9291906103d7565b60405180910390fd5b505b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff168373ffffffffffffffffffffffffffffffffffffffff167fa4e98e523c3e239a66755f9f6a3d3559544e2da7102a26ec45994c3a29599d4234858560405161027993929190610400565b60405180910390a3005b600080fd5b34801561029457600080fd5b5061029d6102b3565b6040516102aa9190610437565b60405180910390f35b7f000000000000000000000000000000000000000000000000000000000000000081565b6000819050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601260045260246000fd5b600061031b826102d7565b9150610326836102d7565b925082610336576103356102e1565b5b828204905092915050565b600081905092915050565b50565b600061035c600083610341565b91506103678261034c565b600082019050919050565b600061037d8261034f565b9150819050919050565b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b60006103b282610387565b9050919050565b6103c2816103a7565b82525050565b6103d1816102d7565b82525050565b60006040820190506103ec60008301856103b9565b6103f960208301846103c8565b9392505050565b600060608201905061041560008301866103c8565b61042260208301856103c8565b61042f60408301846103c8565b949350505050565b600060208201905061044c60008301846103b9565b9291505056fea164736f6c634300081c000a';

/** VelaGasSettlementSplitter CREATE2 init code = creationCode ++ abi.encode(address treasury). */
function splitterInitCode(treasury: string): Uint8Array {
  return concatBytes(fromHex(VELA_SPLITTER_CREATION_CODE), abiEncodeAddress(treasury));
}

/**
 * Deterministic VelaGasSettlementSplitter address for a treasury.
 * = keccak256(0xff ++ factory ++ salt ++ keccak256(creationCode ++ abi.encode(treasury)))[12:]
 * Must equal the bundler's computeSplitterAddress(treasury) byte-for-byte.
 */
export function computeSplitterAddress(treasury: string): string {
  const initCodeHash = keccak256(splitterInitCode(treasury));
  return create2Address(VELA_SPLITTER_FACTORY, fromHex(VELA_SPLITTER_SALT), initCodeHash);
}

/**
 * Raw calldata for a deploy through the Arachnid factory: salt(32) ++ initCode.
 * This is the `data` for the MultiSend sub-call `{ to: VELA_SPLITTER_FACTORY, value: 0, data }`.
 */
export function encodeSplitterDeployCall(treasury: string): Uint8Array {
  return concatBytes(fromHex(VELA_SPLITTER_SALT), splitterInitCode(treasury));
}

// MARK: - Parse Public Key

/** Parse uncompressed P-256 public key hex into x, y coordinates (32 bytes each). */
export function parsePublicKey(hex: string): { x: Uint8Array; y: Uint8Array } {
  let clean = stripHexPrefix(hex);
  if (clean.startsWith('04')) {
    clean = clean.slice(2);
  }
  if (clean.length !== 128) {
    return { x: new Uint8Array(0), y: new Uint8Array(0) };
  }
  const x = fromHex(clean.slice(0, 64));
  const y = fromHex(clean.slice(64));
  return { x, y };
}

// MARK: - Salt Nonce

/** saltNonce = keccak256(abi.encode(x, y)) */
export function calculateSaltNonce(x: Uint8Array, y: Uint8Array): Uint8Array {
  const encoded = concatBytes(abiEncodeBytes32(x), abiEncodeBytes32(y));
  return keccak256(encoded);
}

// MARK: - MultiSend Transaction Encoding

/**
 * Encode a single transaction for MultiSend.
 * Format: operation(1 byte) + to(20 bytes) + value(32 bytes, zero) + dataLength(32 bytes) + data
 */
export function encodeMultiSendTx(
  to: string,
  data: Uint8Array,
  operation: number,
): Uint8Array {
  const toBytes = fromHex(stripHexPrefix(to)); // 20 bytes
  const operationByte = new Uint8Array([operation]); // 1 byte
  const value = new Uint8Array(32); // 32 bytes of zero

  // data length as 32 bytes big-endian
  // Use abiEncodeUint256 instead of manual shift — JS >>> wraps at 32 bits,
  // causing (len >>> 32) === len instead of 0.
  const lenBytes = abiEncodeUint256(data.length);

  return concatBytes(operationByte, toBytes, value, lenBytes, data);
}

// MARK: - Setup Data

/** Encode Safe.setup() call data with MultiSend delegatecall. */
export function encodeSetupData(x: Uint8Array, y: Uint8Array): Uint8Array {
  // 1. enableModules([safe4337Module])
  const enableModulesSelector = functionSelector('enableModules(address[])');
  const enableModulesData = concatBytes(
    enableModulesSelector,
    abiEncodeUint256(32), // offset
    abiEncodeUint256(1), // length
    abiEncodeAddress(SAFE_4337_MODULE),
  );

  // 2. configure((uint256,uint256,uint176))
  const configureSelector = functionSelector(
    'configure((uint256,uint256,uint176))',
  );
  const verifiers = abiEncodeUint256Hex('100'); // RIP-7212 P256 precompile
  const configureData = concatBytes(
    configureSelector,
    abiEncodeBytes32(x),
    abiEncodeBytes32(y),
    verifiers,
  );

  // MultiSend transactions: delegatecall to moduleSetup + delegatecall to webAuthnSigner
  const tx1 = encodeMultiSendTx(SAFE_MODULE_SETUP, enableModulesData, 1);
  const tx2 = encodeMultiSendTx(WEBAUTHN_SIGNER, configureData, 1);
  const packed = concatBytes(tx1, tx2);

  // multiSend(bytes)
  const multiSendSelector = functionSelector('multiSend(bytes)');
  const paddingLen = (32 - (packed.length % 32)) % 32;
  const multiSendData = concatBytes(
    multiSendSelector,
    abiEncodeUint256(32), // offset
    abiEncodeUint256(packed.length), // length
    packed,
    new Uint8Array(paddingLen), // padding
  );

  // Safe.setup(address[],uint256,address,bytes,address,address,uint256,address)
  const setupSelector = functionSelector(
    'setup(address[],uint256,address,bytes,address,address,uint256,address)',
  );

  // Offsets: 8 params * 32 bytes = 256 bytes before owners array
  const ownersOffset = abiEncodeUint256(256);
  const threshold = abiEncodeUint256(1);
  const to = abiEncodeAddress(MULTI_SEND);
  const dataOffset = abiEncodeUint256(256 + 64); // after owners array (32 len + 32 addr)
  const fallback = abiEncodeAddress(SAFE_4337_MODULE);
  const paymentToken = abiEncodeAddress(
    '0x0000000000000000000000000000000000000000',
  );
  const payment = abiEncodeUint256(0);
  const paymentReceiver = abiEncodeAddress(
    '0x0000000000000000000000000000000000000000',
  );

  // owners array: length=1, [webAuthnSigner]
  const ownersArrayLen = abiEncodeUint256(1);
  const ownersArrayData = abiEncodeAddress(WEBAUTHN_SIGNER);

  // multiSendData as bytes: length + data + padding
  const dataLen = abiEncodeUint256(multiSendData.length);
  const dataPaddingLen = (32 - (multiSendData.length % 32)) % 32;
  const dataPadding = new Uint8Array(dataPaddingLen);

  return concatBytes(
    setupSelector,
    ownersOffset,
    threshold,
    to,
    dataOffset,
    fallback,
    paymentToken,
    payment,
    paymentReceiver,
    ownersArrayLen,
    ownersArrayData,
    dataLen,
    multiSendData,
    dataPadding,
  );
}

// MARK: - Proxy Address Calculation

/** Calculate CREATE2 address for Safe Proxy. */
export function calculateProxyAddress(
  setupData: Uint8Array,
  nonce: Uint8Array,
): string {
  // deploymentCode = proxyCreationCode + abi.encode(safeSingleton)
  const singletonEncoded = abiEncodeAddress(SAFE_SINGLETON);
  const deploymentCode = concatBytes(fromHex(PROXY_CREATION_CODE), singletonEncoded);
  const initCodeHash = keccak256(deploymentCode);

  // salt = keccak256(abi.encode(keccak256(setupData), nonce))
  const initializerHash = keccak256(setupData);
  const saltInput = concatBytes(
    abiEncodeBytes32(initializerHash),
    abiEncodeBytes32(nonce),
  );
  const salt = keccak256(saltInput);

  return create2Address(SAFE_PROXY_FACTORY, salt, initCodeHash);
}

// MARK: - Main Entry Point

/**
 * Compute the Safe wallet address for a given P-256 public key.
 * @param publicKeyHex Uncompressed P-256 public key hex ("04" + x + y, 130 chars).
 * @returns Checksummed Safe address.
 */
export function computeAddress(publicKeyHex: string): string {
  const { x, y } = parsePublicKey(publicKeyHex);
  const saltNonce = calculateSaltNonce(x, y);
  const setupData = encodeSetupData(x, y);
  return calculateProxyAddress(setupData, saltNonce);
}
