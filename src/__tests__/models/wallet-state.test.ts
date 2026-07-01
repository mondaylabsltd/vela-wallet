/**
 * Tests for the wallet-state reducer (US 6.4 — account switch / logout is
 * fund-isolation-critical) and the shortAddress utility.
 *
 * The reducer is pure: (state, action) → state. LOGOUT must fully clear the
 * previous account so no address/accounts leak across a logout boundary.
 */
import {
  walletReducer,
  INITIAL_STATE,
  type WalletState,
  type WalletAction,
  shortAddress,
} from '@/models/wallet-state';
import type { Account } from '@/models/types';

const acct = (address: string, name = address.slice(0, 4)): Account => ({
  id: `cred-${name}`,
  name,
  address,
  createdAt: '2026-01-01T00:00:00.000Z',
});

const A = acct('0xAAAA000000000000000000000000000000000001', 'A');
const B = acct('0xBBBB000000000000000000000000000000000002', 'B');

/** A logged-in state with two accounts, active = A. */
const loggedIn: WalletState = {
  ...INITIAL_STATE,
  hasWallet: true,
  accounts: [A, B],
  activeAccountIndex: 0,
  address: A.address,
  isConnectedToBrowser: true,
  isLoading: false,
};

describe('walletReducer', () => {
  describe('SET_WALLET', () => {
    test('populates accounts and defaults to index 0', () => {
      const next = walletReducer(INITIAL_STATE, { type: 'SET_WALLET', accounts: [A, B] });
      expect(next.hasWallet).toBe(true);
      expect(next.accounts).toEqual([A, B]);
      expect(next.activeAccountIndex).toBe(0);
      expect(next.address).toBe(A.address);
      expect(next.isLoading).toBe(false);
    });

    test('honours an explicit activeIndex', () => {
      const next = walletReducer(INITIAL_STATE, { type: 'SET_WALLET', accounts: [A, B], activeIndex: 1 });
      expect(next.activeAccountIndex).toBe(1);
      expect(next.address).toBe(B.address);
    });

    test('empty account list → no wallet, blank address', () => {
      const next = walletReducer(loggedIn, { type: 'SET_WALLET', accounts: [] });
      expect(next.hasWallet).toBe(false);
      expect(next.address).toBe('');
      expect(next.accounts).toEqual([]);
    });

    test('out-of-range activeIndex → blank address rather than crash', () => {
      const next = walletReducer(INITIAL_STATE, { type: 'SET_WALLET', accounts: [A], activeIndex: 5 });
      expect(next.address).toBe('');
      expect(next.activeAccountIndex).toBe(5);
    });
  });

  describe('ADD_ACCOUNT', () => {
    test('adds to empty wallet and activates it', () => {
      const next = walletReducer(INITIAL_STATE, { type: 'ADD_ACCOUNT', account: A });
      expect(next.hasWallet).toBe(true);
      expect(next.accounts).toEqual([A]);
      expect(next.activeAccountIndex).toBe(0);
      expect(next.address).toBe(A.address);
      expect(next.isLoading).toBe(false);
    });

    test('appends to existing accounts and switches to the new one', () => {
      const start = walletReducer(INITIAL_STATE, { type: 'ADD_ACCOUNT', account: A });
      const next = walletReducer(start, { type: 'ADD_ACCOUNT', account: B });
      expect(next.accounts).toEqual([A, B]);
      expect(next.activeAccountIndex).toBe(1);
      expect(next.address).toBe(B.address);
    });

    test('does not mutate the previous accounts array', () => {
      const next = walletReducer(loggedIn, { type: 'ADD_ACCOUNT', account: acct('0xCCCC', 'C') });
      expect(loggedIn.accounts).toHaveLength(2);
      expect(next.accounts).not.toBe(loggedIn.accounts);
    });
  });

  describe('SWITCH_ACCOUNT', () => {
    test('switches active index and address', () => {
      const next = walletReducer(loggedIn, { type: 'SWITCH_ACCOUNT', index: 1 });
      expect(next.activeAccountIndex).toBe(1);
      expect(next.address).toBe(B.address);
    });

    test('invalid index is a no-op (returns same state reference)', () => {
      const next = walletReducer(loggedIn, { type: 'SWITCH_ACCOUNT', index: 99 });
      expect(next).toBe(loggedIn);
    });
  });

  describe('SET_CONNECTED', () => {
    test('toggles browser connection without touching accounts', () => {
      const off = walletReducer(loggedIn, { type: 'SET_CONNECTED', connected: false });
      expect(off.isConnectedToBrowser).toBe(false);
      expect(off.accounts).toBe(loggedIn.accounts);
    });
  });

  describe('LOADED_EMPTY', () => {
    test('clears the loading flag but keeps hasWallet false', () => {
      const next = walletReducer(INITIAL_STATE, { type: 'LOADED_EMPTY' });
      expect(next.isLoading).toBe(false);
      expect(next.hasWallet).toBe(false);
    });
  });

  describe('LOGOUT', () => {
    test('fully clears wallet — no account/address/connection leaks', () => {
      const next = walletReducer(loggedIn, { type: 'LOGOUT' });
      expect(next.hasWallet).toBe(false);
      expect(next.accounts).toEqual([]);
      expect(next.address).toBe('');
      expect(next.activeAccountIndex).toBe(0);
      expect(next.isConnectedToBrowser).toBe(false);
      expect(next.isLoading).toBe(false);
    });
  });

  describe('unknown action', () => {
    test('returns the same state reference', () => {
      const next = walletReducer(loggedIn, { type: 'NOPE' } as unknown as WalletAction);
      expect(next).toBe(loggedIn);
    });
  });

  test('never mutates the input state (immutability)', () => {
    const snapshot = JSON.parse(JSON.stringify(loggedIn));
    walletReducer(loggedIn, { type: 'SWITCH_ACCOUNT', index: 1 });
    walletReducer(loggedIn, { type: 'LOGOUT' });
    walletReducer(loggedIn, { type: 'ADD_ACCOUNT', account: A });
    expect(loggedIn).toEqual(snapshot);
  });
});

describe('shortAddress', () => {
  test('shortens a full address to 0x1234...abcd', () => {
    expect(shortAddress('0xAAAA00000000000000000000000000000000BEEF')).toBe('0xAAAA...BEEF');
  });

  test('returns short strings unchanged', () => {
    expect(shortAddress('0x1234')).toBe('0x1234');
    expect(shortAddress('')).toBe('');
  });
});
