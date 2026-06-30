/**
 * Wei → human-readable ETH-like string.
 *
 * The single source of truth for the threshold ladder that was copy-pasted
 * (byte-identical) into safe-transaction, bundler-service, the Settings treasury
 * screen and the bundler-funding modal — and near-identically into deployer-api.
 * Consolidating here means the display contract is unit-tested in exactly one
 * place and the four surfaces can never drift apart.
 *
 * Note: this returns a bare number string (no symbol). For value+symbol display
 * with trailing-zero trimming, see SigningRequestModal's `formatTxValue`, which
 * deliberately uses a coarser `< 0.0001` threshold.
 */
export function formatWeiToEth(wei: bigint): string {
  const eth = Number(wei) / 1e18;
  if (eth === 0) return '0';
  if (eth < 0.000001) return '< 0.000001';
  if (eth < 0.001) return eth.toFixed(6);
  if (eth < 1) return eth.toFixed(4);
  return eth.toFixed(3);
}
