/**
 * Wallet state management using React Context.
 * Matches iOS WalletState.swift.
 */
import React, { createContext, useContext, useReducer, useEffect, useState, type Dispatch } from 'react';
import type { Account } from './types';
import { loadAccounts, saveAccount, loadActiveAccountIndex, saveActiveAccountIndex } from '@/services/storage';
import { computeAddress } from '@/services/safe-address';

// MARK: - State Shape

export interface WalletState {
  hasWallet: boolean;
  address: string;
  isConnectedToBrowser: boolean;
  accounts: Account[];
  activeAccountIndex: number;
  /** True until storage has been read on startup. */
  isLoading: boolean;
}

export const INITIAL_STATE: WalletState = {
  hasWallet: false,
  address: '',
  isConnectedToBrowser: false,
  accounts: [],
  activeAccountIndex: 0,
  isLoading: true,
};

// MARK: - Actions

export type WalletAction =
  | { type: 'SET_WALLET'; accounts: Account[]; activeIndex?: number }
  | { type: 'ADD_ACCOUNT'; account: Account }
  | { type: 'SWITCH_ACCOUNT'; index: number }
  | { type: 'SET_CONNECTED'; connected: boolean }
  | { type: 'LOADED_EMPTY' }
  | { type: 'LOGOUT' };

export function walletReducer(state: WalletState, action: WalletAction): WalletState {
  switch (action.type) {
    case 'SET_WALLET': {
      const idx = action.activeIndex ?? 0;
      const account = action.accounts[idx];
      return {
        ...state,
        hasWallet: action.accounts.length > 0,
        accounts: action.accounts,
        activeAccountIndex: idx,
        address: account?.address ?? '',
        isLoading: false,
      };
    }
    case 'ADD_ACCOUNT': {
      const accounts = [...state.accounts, action.account];
      const idx = accounts.length - 1;
      return {
        ...state,
        hasWallet: true,
        accounts,
        activeAccountIndex: idx,
        address: action.account.address,
        isLoading: false,
      };
    }
    case 'SWITCH_ACCOUNT': {
      const account = state.accounts[action.index];
      if (!account) return state;
      return {
        ...state,
        activeAccountIndex: action.index,
        address: account.address,
      };
    }
    case 'SET_CONNECTED':
      return { ...state, isConnectedToBrowser: action.connected };
    case 'LOADED_EMPTY':
      return { ...state, isLoading: false };
    case 'LOGOUT':
      return { ...INITIAL_STATE, isLoading: false };
    default:
      return state;
  }
}

// MARK: - Context

interface WalletContextValue {
  state: WalletState;
  dispatch: Dispatch<WalletAction>;
  activeAccount: Account | undefined;
}

export const WalletContext = createContext<WalletContextValue>({
  state: INITIAL_STATE,
  dispatch: () => {},
  activeAccount: undefined,
});

export function useWallet(): WalletContextValue {
  return useContext(WalletContext);
}

// MARK: - Provider Component

export function WalletProvider({ children }: { children: React.ReactNode }) {
  const [state, dispatch] = useReducer(walletReducer, INITIAL_STATE);
  const activeAccount = state.accounts[state.activeAccountIndex];

  // Restore wallet state from storage on mount, fixing any bad addresses
  useEffect(() => {
    Promise.all([loadAccounts(), loadActiveAccountIndex()])
      .then(async ([accounts, savedIndex]) => {
        if (accounts.length > 0) {
          // Migrate: fix accounts that have credentialId as address
          for (const acct of accounts) {
            if (!acct.publicKeyHex) continue;
            try {
              const correct = computeAddress(acct.publicKeyHex);
              if (acct.address !== correct) {
                console.log(`[wallet] Migrating address for ${acct.name}: ${acct.address.slice(0, 10)} → ${correct.slice(0, 10)}`);
                acct.address = correct;
                await saveAccount(acct);
              }
            } catch (err) {
              console.error(`[wallet] Address migration failed for ${acct.name}:`, err);
              // Keep existing address rather than corrupting storage
            }
          }
          // Clamp saved index to valid range
          const activeIndex = savedIndex < accounts.length ? savedIndex : 0;
          dispatch({ type: 'SET_WALLET', accounts, activeIndex });
        } else {
          dispatch({ type: 'LOADED_EMPTY' });
        }
      })
      .catch(() => {
        dispatch({ type: 'LOADED_EMPTY' });
      });
  }, []);

  // Persist active account index whenever it changes
  useEffect(() => {
    if (!state.isLoading && state.hasWallet) {
      saveActiveAccountIndex(state.activeAccountIndex);
    }
  }, [state.activeAccountIndex, state.isLoading, state.hasWallet]);

  const value = React.useMemo(
    () => ({ state, dispatch, activeAccount }),
    [state, activeAccount],
  );

  return React.createElement(WalletContext.Provider, { value }, children);
}

// MARK: - Utility

/** Shorten an address to "0x1234...abcd". */
export function shortAddress(address: string): string {
  if (address.length <= 10) return address;
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}
