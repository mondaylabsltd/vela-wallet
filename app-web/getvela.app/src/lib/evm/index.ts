/**
 * The site's own EVM toolkit: hex, Keccak/EIP-55, ABI, a failover JSON-RPC
 * client, and legacy-transaction signing. Hash and curve math run in
 * rust/vela-site-crypto (wasm; `loadEvm()` once before use); everything else
 * is here, so every byte the site reads or sends can be audited.
 */
export * from './abi';
export * from './client';
export * from './hash';
export * from './hex';
export * from './tx';
export { evmLoaded, instantiateEvm, loadEvm } from './wasm';
