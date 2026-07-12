/**
 * useAddressIdentity — resolve an address to a human label AND its kind for the
 * technical-details panel. ADDITIVE ONLY: the caller always keeps the raw address
 * on screen; this is a convenience name, never a replacement for the ground-truth
 * bytes (a look-alike ENS or a mis-saved contact must never hide the hex).
 *
 * Resolution (sync first, then async upgrade — the panel renders synchronously):
 *   token    — known token symbol → on-chain symbol()               [logo]
 *   contract — descriptor name → known protocol contract            [glyph]
 *   wallet   — my own account → saved contact → ENS/Basename/passkey [identicon]
 *
 * The known-address maps + my accounts are checked for EVERY address regardless of
 * the caller's hint, so a spender/operator that a best-effort decode left untyped
 * still resolves to "Uniswap Universal Router" (contract) rather than a nameless
 * identicon. The returned `kind` drives which avatar the row draws.
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
export type ResolvedKind = 'wallet' | 'contract' | 'token';

export function useAddressIdentity(
  address: string | undefined,
  chainId: number,
  hintKind: AddrKind,
  /** A name already known for this exact address (e.g. clearSign.contractName). */
  seedName?: string,
): { name?: string; kind: ResolvedKind } {
  const { state } = useWallet();

  // Sync — available on the first render, no round-trip. Known maps win over the
  // caller's hint (they're definitive about token vs contract vs wallet).
  const sync = useMemo<{ name?: string; kind: ResolvedKind }>(() => {
    if (!address || !isAddress(address)) return { name: seedName, kind: hintKind };
    const sym = knownTokenSymbol(address);
    if (sym) return { name: seedName ?? sym, kind: 'token' };
    const contractName = seedName ?? knownContract(address)?.name;
    if (contractName) return { name: contractName, kind: 'contract' };
    const own = state.accounts.find((a) => a.address.toLowerCase() === address.toLowerCase())?.name;
    if (own) return { name: own, kind: 'wallet' };
    return { name: undefined, kind: hintKind };
  }, [address, hintKind, seedName, state.accounts]);

  const [asyncName, setAsyncName] = useState<string | undefined>(undefined);
  const [asyncKind, setAsyncKind] = useState<ResolvedKind | undefined>(undefined);

  useEffect(() => {
    setAsyncName(undefined);
    setAsyncKind(undefined);
    if (!address || !isAddress(address) || sync.name) return; // already named synchronously
    let cancelled = false;
    (async () => {
      if (hintKind === 'wallet') {
        // A saved contact is the highest-trust label for a wallet.
        try {
          const c = await getSavedContact(address);
          const cn = c ? contactDisplayName(c) : '';
          if (cn) { if (!cancelled) { setAsyncName(cn); setAsyncKind('wallet'); } return; }
        } catch { /* fall through */ }
        try {
          const id = await resolveRecipientIdentity(address);
          if (id?.name && !cancelled) { setAsyncName(id.name); setAsyncKind('wallet'); }
        } catch { /* leave as address */ }
      } else {
        // Unknown contract → try its on-chain token symbol (→ a token row w/ logo).
        try {
          const meta = await resolveTokenMetadata(chainId, [address]);
          const sym = meta.get(address.toLowerCase())?.symbol;
          if (sym && !cancelled) { setAsyncName(sym); setAsyncKind('token'); }
        } catch { /* leave as address */ }
      }
    })();
    return () => { cancelled = true; };
  }, [address, chainId, hintKind, sync.name]);

  return { name: asyncName ?? sync.name, kind: asyncKind ?? sync.kind };
}
