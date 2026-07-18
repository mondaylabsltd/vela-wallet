/**
 * useRecipientIdentity — resolve a recipient's live identity (Vela/passkey name, then ENS/
 * name-service) for display, with an in-process cache so the same address isn't re-resolved
 * across rows or re-renders. `resolveRecipientIdentity` already persists to AsyncStorage; this
 * adds a session memo on top so a batch of N recipients (payroll / sweep) each resolves at most
 * once, and a cache hit is synchronous (no flash of "unknown").
 *
 * Pass a `hint` (an already-resolved identity) to skip resolution entirely — the confirm flow's
 * single recipient already has one.
 */
import { useEffect, useState } from 'react';
import { resolveRecipientIdentity, type RecipientIdentity } from '@/services/recipient-identity';

const memo = new Map<string, RecipientIdentity | null>();
/** De-dupe concurrent in-flight resolutions of the same address (N rows mount together). */
const inflight = new Map<string, Promise<RecipientIdentity | null>>();

function resolveCached(address: string): Promise<RecipientIdentity | null> {
  const key = address.toLowerCase();
  if (memo.has(key)) return Promise.resolve(memo.get(key)!);
  let p = inflight.get(key);
  if (!p) {
    p = resolveRecipientIdentity(address)
      .then((r) => { memo.set(key, r); inflight.delete(key); return r; })
      .catch(() => { inflight.delete(key); return null; });
    inflight.set(key, p);
  }
  return p;
}

export function useRecipientIdentity(
  address?: string,
  hint?: RecipientIdentity | null,
): RecipientIdentity | null {
  const key = address?.toLowerCase();
  const [id, setId] = useState<RecipientIdentity | null>(
    hint ?? (key && memo.has(key) ? memo.get(key)! : null),
  );

  useEffect(() => {
    if (hint) { setId(hint); return; }
    if (!key || !address) { setId(null); return; }
    if (memo.has(key)) { setId(memo.get(key)!); return; }
    let cancelled = false;
    resolveCached(address).then((r) => { if (!cancelled) setId(r); });
    return () => { cancelled = true; };
  }, [key, address, hint]);

  return hint ?? id;
}
