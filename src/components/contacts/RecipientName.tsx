/**
 * RecipientName — resolves the best display name for an address and renders it as text:
 * saved contact › Vela/passkey › ENS/name-service › a caller-stored name › the short address.
 *
 * Pairs with RecipientTypeBadge so the NAME and the trust marker resolve from the SAME identity
 * (via the shared, cached useRecipientIdentity) — fixing the "badge says Vela but the name shows
 * 0x…" mismatch. Drop it anywhere a recipient/counterparty name is shown (tx detail, history).
 */
import React, { useEffect, useState } from 'react';
import { Text, type StyleProp, type TextStyle } from 'react-native';
import { getSavedContact, contactDisplayName } from '@/services/contacts';
import { useRecipientIdentity } from '@/hooks/use-recipient-identity';
import { shortAddress } from '@/models/wallet-state';

export function RecipientName({
  address,
  storedName,
  style,
}: {
  address: string;
  /** A name already recorded with the tx (e.g. at send time). Used only as a fallback. */
  storedName?: string | null;
  style?: StyleProp<TextStyle>;
}) {
  const identity = useRecipientIdentity(address);
  const [contactName, setContactName] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSavedContact(address)
      .then((c) => { if (!cancelled) setContactName(c ? (contactDisplayName(c) || null) : null); })
      .catch(() => { if (!cancelled) setContactName(null); });
    return () => { cancelled = true; };
  }, [address]);

  const name = contactName || identity?.name || storedName || shortAddress(address);
  return <Text style={style} numberOfLines={1}>{name}</Text>;
}
