/**
 * RecipientTrust — a single, deduplicated recipient identity line.
 *
 * One name, one leading icon that encodes trust at a glance:
 *   - Saved contact **and** starred (favourite) → green {@link BadgeCheck}. This is
 *     the strong "you vouched for this address" signal and doubles as anti-address-
 *     poisoning reassurance (a poisoned look-alike won't be a starred contact).
 *   - Anyone else with a resolvable identity (a Vela/passkey user, an ENS/Basename)
 *     → a neutral person icon ({@link UserRound}) — identified, but not a starred
 *     contact.
 *
 * It replaces the old split display where the live identity ("👤 Name · Vela User")
 * and a separate green "✓ Name" saved-contact pill both rendered the same name.
 * Pass the already-resolved `identity` (if the caller has it) to avoid a second
 * lookup; the saved-contact status is always resolved here.
 *
 * Renders nothing until (and unless) there's a name to show. Safe to drop anywhere
 * a recipient address is displayed.
 *
 * Variants:
 *   - default  — inline line with the source tag ("Vela User" / "ENS"), for the
 *                Send address-entry step.
 *   - compact  — a small pill, for dense per-recipient rows.
 *   - prominent — a bold name, for the confirm step's "To" block.
 */
import React, { useEffect, useState } from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { BadgeCheck, UserRound } from 'lucide-react-native';
import { getSavedContact, contactDisplayName, updateContact } from '@/services/contacts';
import { resolveRecipientIdentity, type RecipientIdentity } from '@/services/recipient-identity';
import { useRecipientIdentity } from '@/hooks/use-recipient-identity';
import { color, text, inter, space, radius, createStyles } from '@/constants/theme';

interface Meta {
  name: string;
  /** Saved AND starred — the only state that earns the green check. */
  favorite: boolean;
}

export function RecipientTrust({
  address,
  identity,
  compact,
  prominent,
  nameOnly,
}: {
  address?: string;
  /** Already-resolved live identity, to skip a duplicate lookup and to name a
   *  saved-but-unnamed contact. */
  identity?: RecipientIdentity | null;
  compact?: boolean;
  prominent?: boolean;
  /** Render JUST the name (no leading trust icon) — for callers that show the trust
   *  signal as a separate trailing badge (e.g. RecipientTypeBadge on the confirm row). */
  nameOnly?: boolean;
}) {
  const { t } = useTranslation();
  const [meta, setMeta] = useState<Meta | null>(null);
  // Live identity (Vela/passkey › ENS/name-service), cached + shared, so a name shows even
  // when the caller (e.g. the split recipient list) passes only an address.
  const resolvedIdentity = useRecipientIdentity(address, identity ?? undefined);

  useEffect(() => {
    setMeta(null);
    if (!address) return;
    let cancelled = false;
    getSavedContact(address)
      .then((c) => {
        if (cancelled || !c) return;
        const stored = contactDisplayName(c);
        if (stored) { setMeta({ name: stored, favorite: !!c.favorite }); return; }
        // Saved without a name: show whatever identity we already have (or a
        // generic label), then upgrade to a live-resolved identity and cache it
        // back so the picker and future renders show the real name.
        setMeta({ name: identity?.name || t('contacts.savedContact'), favorite: !!c.favorite });
        if (!identity?.name) {
          resolveRecipientIdentity(address)
            .then((id) => {
              if (cancelled || !id?.name) return;
              setMeta({ name: id.name, favorite: !!c.favorite });
              updateContact(address, { resolvedName: id.name, resolvedSource: id.source }).catch(() => {});
            })
            .catch(() => {});
        }
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [address, identity, t]);

  // Contact name wins over the live identity; fall back to the live identity for a
  // recipient who isn't saved (e.g. a Vela user you've never sent to).
  const name = meta?.name || resolvedIdentity?.name;
  if (!name) return null;
  const favorite = !!meta?.favorite;

  const iconSize = compact ? 12 : prominent ? 16 : 14;
  const Icon = favorite ? BadgeCheck : UserRound;
  const icon = (
    <Icon
      size={iconSize}
      color={favorite ? color.success.base : color.accent.base}
      strokeWidth={favorite ? 2.5 : 2}
    />
  );

  if (compact) {
    return (
      <View style={[styles.pill, favorite ? styles.pillFav : styles.pillPlain]}>
        {icon}
        <Text style={[styles.pillText, favorite ? styles.pillTextFav : styles.pillTextPlain]} numberOfLines={1}>
          {name}
        </Text>
      </View>
    );
  }

  if (prominent) {
    // nameOnly: the trust signal is shown as a separate trailing badge, so drop the
    // leading icon here and render the bare name.
    if (nameOnly) {
      return <Text style={[styles.promName, favorite && styles.promNameFav]} numberOfLines={1}>{name}</Text>;
    }
    return (
      <View style={styles.promRow}>
        {icon}
        <Text style={[styles.promName, favorite && styles.promNameFav]} numberOfLines={1}>{name}</Text>
      </View>
    );
  }

  // default — inline line with the source tag on the right.
  const source = resolvedIdentity
    ? (resolvedIdentity.source === 'passkey' ? t('send.velaUser') : resolvedIdentity.source)
    : undefined;
  return (
    <View style={styles.line}>
      <View style={styles.lineLeft}>
        {icon}
        <Text style={[styles.lineName, favorite && styles.lineNameFav]} numberOfLines={1}>{name}</Text>
      </View>
      {source && <Text style={styles.source}>{source}</Text>}
    </View>
  );
}

const styles = createStyles(() => ({
  // default line
  line: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    gap: space.sm, paddingLeft: space.sm,
  },
  lineLeft: { flexDirection: 'row', alignItems: 'center', gap: space.xs, flexShrink: 1 },
  lineName: { fontSize: text.sm, ...inter.semibold, color: color.accent.base, flexShrink: 1 },
  lineNameFav: { color: color.success.base },
  source: { fontSize: text.xs, ...inter.medium, color: color.fg.subtle },
  // prominent (confirm "To")
  promRow: { flexDirection: 'row', alignItems: 'center', gap: space.xs },
  promName: { fontSize: text.base, ...inter.bold, color: color.fg.base, flexShrink: 1 },
  promNameFav: { color: color.success.base },
  // compact pill (dense rows)
  pill: {
    flexDirection: 'row', alignItems: 'center', gap: space.xs,
    alignSelf: 'flex-start', borderRadius: radius.full,
    paddingVertical: 2, paddingHorizontal: space.sm,
  },
  pillFav: { backgroundColor: color.success.soft },
  pillPlain: { backgroundColor: color.bg.sunken },
  pillText: { fontSize: text.sm, ...inter.semibold, flexShrink: 1 },
  pillTextFav: { color: color.success.base },
  pillTextPlain: { color: color.fg.muted },
}));
