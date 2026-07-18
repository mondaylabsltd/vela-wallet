/**
 * RecipientTypeBadge — a small trailing marker (shown to the RIGHT of a recipient name)
 * that encodes WHO/WHAT the address is, at a glance:
 *
 *   1. Saved contact        → green ✓ (you vouched for this address; anti-poisoning signal)
 *   2. Vela user (passkey)  → the Vela sailboat mark (a Vela smart account — its being a
 *                             contract is expected, so it is NOT flagged as "合约")
 *   3. Unknown EOA          → "unknown" + a wallet mark (an ordinary externally-owned account
 *                             you've not saved)
 *   4. Unknown contract     → "unknown" + a contract mark (an unsaved contract address)
 *
 * Priority is contact → vela → (contract ? unknown-contract : unknown-eoa). Renders nothing
 * until the saved-contact lookup resolves, so a contact never flashes as "unknown" first.
 * Shared by the Send confirm flow and the transaction receipt so the marker reads identically.
 */
import React, { useEffect, useState } from 'react';
import { View, Image } from 'react-native';
import { BadgeCheck, Globe, HelpCircle, Wallet, FileText } from 'lucide-react-native';
import { getSavedContact } from '@/services/contacts';
import { useRecipientIdentity } from '@/hooks/use-recipient-identity';
import type { RecipientIdentity } from '@/services/recipient-identity';
import { color, createStyles, space } from '@/constants/theme';

/** The Vela brand mark (app icon) — used as the "this is a Vela account" badge. */
const VELA_LOGO = require('@/../assets/images/icon.png');

export function RecipientTypeBadge({
  address,
  identity,
  isContract,
  size = 15,
}: {
  address?: string;
  identity?: RecipientIdentity | null;
  /** From the recipient-risk probe — decides EOA vs contract for an unsaved address. */
  isContract?: boolean | null;
  size?: number;
}) {
  const [isContact, setIsContact] = useState<boolean | null>(null);

  useEffect(() => {
    setIsContact(null);
    if (!address) return;
    let cancelled = false;
    getSavedContact(address)
      .then((c) => { if (!cancelled) setIsContact(!!c); })
      .catch(() => { if (!cancelled) setIsContact(false); });
    return () => { cancelled = true; };
  }, [address]);

  // Resolve the live identity (Vela/passkey, then a name service) — cached, so a batch of
  // recipients each resolves at most once. Skipped when the caller already passed `identity`.
  const resolved = useRecipientIdentity(address, identity ?? undefined);

  // Wait for the contact lookup so a saved contact never flashes as "unknown".
  if (isContact === null) return null;

  const isVela = resolved?.source === 'passkey';
  // A name from a name service (ENS / Basename / .bnb / .arb …) — not passkey, but named.
  const isNamed = !isVela && !!resolved?.name;

  if (isContact) {
    return (
      <View style={styles.wrap}>
        <BadgeCheck size={size} color={color.success.base} strokeWidth={2.4} />
      </View>
    );
  }
  if (isVela) {
    return (
      <View style={styles.wrap}>
        <Image source={VELA_LOGO} style={{ width: size + 1, height: size + 1, borderRadius: (size + 1) / 2 }} resizeMode="contain" />
      </View>
    );
  }
  if (isNamed) {
    // ENS / name-service identity — a calm blue globe (not the accent orange).
    return (
      <View style={styles.wrap}>
        <Globe size={size} color={color.info.base} strokeWidth={2} />
      </View>
    );
  }
  // Unknown address — "unknown" paired with its account kind (EOA vs contract).
  return (
    <View style={styles.wrap}>
      <HelpCircle size={size} color={color.fg.subtle} strokeWidth={2} />
      {isContract === true
        ? <FileText size={size} color={color.fg.subtle} strokeWidth={2} />
        : <Wallet size={size} color={color.fg.subtle} strokeWidth={2} />}
    </View>
  );
}

const styles = createStyles(() => ({
  wrap: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
  },
}));
