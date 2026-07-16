export const TRANSACTION_SUBMISSION_METHODS = new Set([
  'eth_sendTransaction',
  'wallet_sendTransaction',
]);

export function isTransactionSubmissionMethod(method: string): boolean {
  return TRANSACTION_SUBMISSION_METHODS.has(method);
}
