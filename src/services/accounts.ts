/**
 * Pure helpers for the multi-account UI. Kept out of the components so the
 * ordering rule can be unit-tested away from React.
 */

export interface OrderedAccount<T> {
  account: T;
  /** The account's index in the ORIGINAL list — what SWITCH_ACCOUNT expects. */
  index: number;
}

/**
 * Order accounts for the switcher: highest cached balance first, then by name.
 * An account with no cached balance sorts last (treated as -1). The original
 * index rides along so the caller can still dispatch SWITCH_ACCOUNT correctly
 * after the reorder.
 *
 * This comparator was copy-pasted (byte-identical) into the Home, Assets and
 * Settings account-switcher modals; this is the one tested copy.
 */
export function sortAccountsByBalance<T extends { address: string; name: string }>(
  accounts: T[],
  balances: Map<string, number>,
): OrderedAccount<T>[] {
  return accounts
    .map((account, index) => ({ account, index }))
    .sort((a, b) => {
      const balA = balances.get(a.account.address) ?? -1;
      const balB = balances.get(b.account.address) ?? -1;
      if (balB !== balA) return balB - balA;
      return a.account.name.localeCompare(b.account.name);
    });
}

/** Sum of all cached account balances (USD) — the switcher header total. */
export function totalAccountBalance(balances: Map<string, number>): number {
  let sum = 0;
  for (const v of balances.values()) sum += v;
  return sum;
}
