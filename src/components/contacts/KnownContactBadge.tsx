/**
 * KnownContactBadge — a green "✓ <name>" pill shown when a recipient address is
 * one the user has SAVED. It's an anti-address-poisoning signal: a poisoned
 * look-alike address won't match a saved contact, so the *presence* of the check
 * (with the right name) is reassurance, and its *absence* on a supposedly-known
 * party is a quiet prompt to look twice.
 *
 * Best-effort + async: renders nothing until (and unless) the address resolves to
 * a saved contact. Safe to drop anywhere a recipient address is shown.
 */
import React, { useEffect, useState } from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { BadgeCheck } from 'lucide-react-native';
import { getSavedContact, contactDisplayName } from '@/services/contacts';
import { color, text, inter, space, radius, createStyles } from '@/constants/theme';

export function KnownContactBadge({ address, compact }: { address?: string; compact?: boolean }) {
  const { t } = useTranslation();
  const [label, setLabel] = useState<string | null>(null);

  useEffect(() => {
    setLabel(null);
    if (!address) return;
    let cancelled = false;
    getSavedContact(address)
      .then((c) => { if (!cancelled && c) setLabel(contactDisplayName(c) || t('contacts.savedContact')); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [address, t]);

  if (!label) return null;

  return (
    <View style={[styles.pill, compact && styles.pillCompact]}>
      <BadgeCheck size={compact ? 12 : 13} color={color.success.base} strokeWidth={2.5} />
      <Text style={styles.text} numberOfLines={1}>{label}</Text>
    </View>
  );
}

const styles = createStyles(() => ({
  pill: {
    flexDirection: 'row', alignItems: 'center', gap: space.xs,
    alignSelf: 'flex-start',
    backgroundColor: color.success.soft, borderRadius: radius.full,
    paddingVertical: 3, paddingHorizontal: space.md,
  },
  pillCompact: { paddingVertical: 2, paddingHorizontal: space.sm },
  text: { fontSize: text.sm, ...inter.semibold, color: color.success.base, flexShrink: 1 },
}));
