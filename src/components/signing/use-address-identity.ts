/**
 * useAddressIdentity — resolve an address to a human label for the technical-
 * details panel. ADDITIVE ONLY: the caller always keeps the raw address on
 * screen; this is a convenience name, never a replacement for the ground-truth
 * bytes (a look-alike ENS or a mis-saved contact must never hide the hex).
 *
 * Wallet priority:   saved contact  →  my own account  →  ENS / Basename / passkey
 * Contract priority: descriptor name → known contract → known token symbol → on-chain symbol()
 *
 * Two-phase, because the panel renders synchronously: the sync sources (my
 * accounts, the known-address maps, a descriptor name already on the field) seed
 * an immediate label; the async sources (the contacts cache, the ENS reverse
 * lookup, on-chain token metadata) upgrade it via effect. Mirrors the same
 * pattern as useResolvedName / ContractBar.
 */
import { useEffect, useMemo, useState } from 'react';
import { isAddress } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import { knownContract } from '@/services/local-descriptors';
import { knownTokenSymbol } from '@/services/tokens';
import { resolveTokenMetadata } from '@/services/token-metadata';
import { getSavedContact, contactDisplayName } from '@/services/contacts';
import { resolveRecipientIdentity } from '@/services/recipient-identity';

export type AddrKind = 'wallet' | 'contract';

export function useAddressIdentity(
  address: string | undefined,
  chainId: number,
  kind: AddrKind,
  /** A name already known for this exact address (e.g. clearSign.contractName). */
  seedName?: string,
): { name?: string } {
  const { state } = useWallet();

  // Sync seed — available on first render, no round-trip.
  const sync = useMemo<string | undefined>(() => {
    if (!address || !isAddress(address)) return undefined;
    if (seedName) return seedName;
    if (kind === 'contract') {
      return knownContract(address)?.name || knownTokenSymbol(address) || undefined;
    }
    // Wallet: is it one of MY accounts? (self / inter-account send)
    const lower = address.toLowerCase();
    return state.accounts.find((a) => a.address.toLowerCase() === lower)?.name;
  }, [address, kind, seedName, state.accounts]);

  const [asyncName, setAsyncName] = useState<string | undefined>(undefined);

  useEffect(() => {
    setAsyncName(undefined);
    if (!address || !isAddress(address)) return;
    let cancelled = false;
    (async () => {
      if (kind === 'wallet') {
        // A saved contact is the highest-trust label and OVERRIDES the account seed.
        try {
          const c = await getSavedContact(address);
          const cn = c ? contactDisplayName(c) : '';
          if (cn) { if (!cancelled) setAsyncName(cn); return; }
        } catch { /* fall through */ }
        // Already named by one of my accounts → keep it (don't downgrade to ENS).
        if (sync) return;
        try {
          const id = await resolveRecipientIdentity(address);
          if (id?.name && !cancelled) setAsyncName(id.name);
        } catch { /* leave as address */ }
      } else {
        if (sync) return; // already named from a sync map / descriptor
        try {
          const meta = await resolveTokenMetadata(chainId, [address]);
          const sym = meta.get(address.toLowerCase())?.symbol;
          if (sym && !cancelled) setAsyncName(sym);
        } catch { /* leave as address */ }
      }
    })();
    return () => { cancelled = true; };
  }, [address, chainId, kind, sync]);

  // Async (contact / ENS / on-chain symbol) wins when present; else the sync seed.
  return { name: asyncName ?? sync };
}
