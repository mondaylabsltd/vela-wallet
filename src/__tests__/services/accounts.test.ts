/**
 * Tests for the account-switcher ordering rule.
 */
import { sortAccountsByBalance, totalAccountBalance } from '@/services/accounts';

const acct = (address: string, name: string) => ({ address, name });

describe('sortAccountsByBalance', () => {
  test('orders by cached balance desc, carrying the original index', () => {
    const accounts = [acct('0xa', 'A'), acct('0xb', 'B'), acct('0xc', 'C')];
    const balances = new Map([['0xa', 10], ['0xb', 50], ['0xc', 5]]);
    const out = sortAccountsByBalance(accounts, balances);
    expect(out.map((o) => o.account.name)).toEqual(['B', 'A', 'C']);
    expect(out.map((o) => o.index)).toEqual([1, 0, 2]); // SWITCH_ACCOUNT needs original index
  });

  test('breaks balance ties by name (localeCompare)', () => {
    const accounts = [acct('0xa', 'Zed'), acct('0xb', 'Alice')];
    const balances = new Map([['0xa', 10], ['0xb', 10]]);
    expect(sortAccountsByBalance(accounts, balances).map((o) => o.account.name)).toEqual(['Alice', 'Zed']);
  });

  test('accounts with no cached balance sort last', () => {
    const accounts = [acct('0xa', 'A'), acct('0xb', 'B')];
    const balances = new Map([['0xb', 1]]); // A has no cached balance → treated as -1
    expect(sortAccountsByBalance(accounts, balances).map((o) => o.account.name)).toEqual(['B', 'A']);
  });

  test('does not mutate the input array', () => {
    const accounts = [acct('0xa', 'A'), acct('0xb', 'B')];
    const snapshot = [...accounts];
    sortAccountsByBalance(accounts, new Map([['0xa', 1], ['0xb', 2]]));
    expect(accounts).toEqual(snapshot);
  });
});

describe('totalAccountBalance', () => {
  test('sums all cached balances', () => {
    expect(totalAccountBalance(new Map([['0xa', 10], ['0xb', 5.5]]))).toBe(15.5);
  });
  test('empty map → 0', () => {
    expect(totalAccountBalance(new Map())).toBe(0);
  });
});
