/**
 * The site's own EVM toolkit: hex, Keccak/EIP-55, ABI, a failover JSON-RPC
 * client, and legacy-transaction signing. Hash and curve math come from
 * `@noble/hashes` / `@noble/curves`; everything else is here, in a few
 * hundred lines, so every byte the site reads or sends can be audited.
 */
export * from './abi';
export * from './client';
export * from './hash';
export * from './hex';
export * from './tx';
