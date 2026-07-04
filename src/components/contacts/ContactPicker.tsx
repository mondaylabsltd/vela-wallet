/**
 * ContactPicker — the unified recipient chooser.
 *
 * One sheet for: favorites, recent recipients (from send history), saved
 * contacts, and live search by name or address. Picking a row fills the caller's
 * recipient field. If you type/paste a fresh valid address, it offers to use it
 * directly (and save it) — so the picker never gets in the way of a one-off send.
 *
 * Quiet by design: every entry is a plain de-boxed row (avatar/icon + name +
 * hairline), no accent slabs — the primary act is picking a row, so accent is
 * spent on nothing here (design language rules 1/6/8).
 */
import React, { useEffect, useMemo, useState } from 'react';
import { View, Text, ScrollView, Pressable, TextInput, ActivityIndicator } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Search, X, Star, ScanLine, ChevronRight, Users } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { SectionLabel } from '@/components/ui/SectionLabel';
import { Divider } from '@/components/ui/DetailRow';
import { ContactAvatar } from '@/components/contacts/ContactAvatar';
import { shortAddr, isAddress } from '@/models/types';
import {
  getAllContacts, sortContacts, matchesQuery, contactDisplayName, saveContact,
  getGroups, type Contact, type ContactGroup,
} from '@/services/contacts';
import { color, text, inter, space, radius, font, createStyles } from '@/constants/theme';
import { hapticLight } from '@/services/platform';

/** Leading avatar/icon diameter — row hairlines inset past it so they align
    under the text (Apple-Wallet style). */
const ROW_AVATAR = 40;

export function ContactPicker({ visible, onClose, onSelect, onSelectGroup, onScan, onAddContact, myAddress }: {
  visible: boolean;
  onClose: () => void;
  onSelect: (address: string, name?: string) => void;
  /** Pick a whole group as recipients — enables the Groups section. The host
      (SendScreen) seeds split mode with one row per member. */
  onSelectGroup?: (addresses: string[], name: string) => void;
  /** Optional QR-scan entry, shown as a quiet row at the top of the list.
      Tapping it closes the picker and hands off to the scanner. */
  onScan?: () => void;
  /** Optional "add a contact" escape hatch for the empty state. */
  onAddContact?: () => void;
  myAddress?: string;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [contacts, setContacts] = useState<Contact[] | null>(null);
  const [groups, setGroups] = useState<ContactGroup[]>([]);

  useEffect(() => {
    if (!visible) return;
    setQuery('');
    let cancelled = false;
    getAllContacts(myAddress)
      .then((list) => { if (!cancelled) setContacts(sortContacts(list)); })
      .catch(() => { if (!cancelled) setContacts([]); });
    if (onSelectGroup) {
      getGroups()
        .then((g) => { if (!cancelled) setGroups(g.filter((x) => x.members.length > 0)); })
        .catch(() => { if (!cancelled) setGroups([]); });
    }
    return () => { cancelled = true; };
  }, [visible, myAddress, onSelectGroup]);

  const filtered = useMemo(
    () => (contacts ?? []).filter((c) => matchesQuery(c, query)),
    [contacts, query],
  );
  const favorites = filtered.filter((c) => c.favorite);
  const rest = filtered.filter((c) => !c.favorite);

  // A pasted/typed fresh address that isn't already a contact → offer it directly.
  const typedAddr = isAddress(query.trim()) ? query.trim().toLowerCase() : null;
  const typedIsKnown = !!typedAddr && (contacts ?? []).some((c) => c.address === typedAddr);

  const pick = (address: string, name?: string) => {
    hapticLight();
    onSelect(address, name);
    onClose();
  };

  const pickGroup = (g: ContactGroup) => {
    hapticLight();
    onSelectGroup?.(g.members, g.name);
    onClose();
  };

  const showGroups = !!onSelectGroup && groups.length > 0 && !query;
  const isEmpty = contacts !== null && filtered.length === 0 && !typedAddr;

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.container}>
        <View style={styles.header}>
          <Text style={styles.title}>{t('contacts.pickerTitle')}</Text>
          <Pressable onPress={onClose} hitSlop={12} accessibilityRole="button" accessibilityLabel={t('contacts.cancel')}>
            <X size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </View>

        <View style={styles.searchWrap}>
          <Search size={16} color={color.fg.subtle} strokeWidth={2} />
          <TextInput
            style={styles.searchInput}
            placeholder={t('contacts.searchPlaceholder')}
            placeholderTextColor={color.fg.subtle}
            value={query}
            onChangeText={setQuery}
            autoCapitalize="none"
            autoCorrect={false}
          />
          {query.length > 0 && (
            <Pressable onPress={() => setQuery('')} hitSlop={8} accessibilityRole="button" accessibilityLabel={t('contacts.cancel')}>
              <X size={15} color={color.fg.subtle} strokeWidth={2} />
            </Pressable>
          )}
        </View>

        <ScrollView style={styles.scroll} keyboardShouldPersistTaps="handled" showsVerticalScrollIndicator={false}>
          {/* Scan — a plain row, not a promoted slab; the icon rhymes with the avatars. */}
          {onScan && !query && (
            <>
              <Pressable
                style={styles.row}
                onPress={() => { hapticLight(); onClose(); onScan(); }}
                accessibilityRole="button"
                accessibilityLabel={t('contacts.scanToAdd')}
              >
                <View style={styles.iconCircle}>
                  <ScanLine size={19} color={color.fg.muted} strokeWidth={2} />
                </View>
                <Text style={styles.rowName}>{t('contacts.scanToAdd')}</Text>
              </Pressable>
              <RowDivider />
            </>
          )}

          {/* Use a freshly typed address right away */}
          {typedAddr && !typedIsKnown && (
            <Pressable
              style={styles.row}
              onPress={() => pick(typedAddr)}
              accessibilityRole="button"
              accessibilityLabel={t('contacts.useTyped')}
            >
              <ContactAvatar name="" address={typedAddr} size={ROW_AVATAR} />
              <View style={styles.rowInfo}>
                <Text style={styles.rowName}>{t('contacts.useTyped')}</Text>
                <Text style={styles.rowAddr}>{shortAddr(typedAddr)}</Text>
              </View>
              <Pressable
                hitSlop={12}
                onPress={(e) => { e.stopPropagation?.(); saveContact({ address: typedAddr }).then(() => pick(typedAddr)); }}
                accessibilityRole="button"
                accessibilityLabel={t('contacts.saveToContacts')}
              >
                <Text style={styles.saveText}>{t('contacts.save')}</Text>
              </Pressable>
            </Pressable>
          )}

          {contacts === null ? (
            <View style={styles.loading}><ActivityIndicator size="small" color={color.fg.muted} /></View>
          ) : isEmpty ? (
            <View style={styles.empty}>
              <Text style={styles.emptyTitle}>
                {query ? t('contacts.noResults', { query }) : t('contacts.emptyPicker')}
              </Text>
              {!query && <Text style={styles.emptyHint}>{t('contacts.pickerEmptyHint')}</Text>}
              {!query && onAddContact && (
                <Pressable
                  style={styles.addContactBtn}
                  onPress={() => { hapticLight(); onAddContact(); }}
                  accessibilityRole="button"
                  accessibilityLabel={t('contacts.addContact')}
                >
                  <Text style={styles.addContactText}>{t('contacts.addContact')}</Text>
                </Pressable>
              )}
            </View>
          ) : (
            <>
              {showGroups && (
                <Section title={t('contacts.sectionGroups')}>
                  {groups.map((g, i) => (
                    <React.Fragment key={g.id}>
                      {i > 0 && <RowDivider />}
                      <GroupRow g={g} onPick={pickGroup} />
                    </React.Fragment>
                  ))}
                </Section>
              )}
              {favorites.length > 0 && (
                <Section title={t('contacts.sectionFavorites')}>
                  {favorites.map((c, i) => (
                    <React.Fragment key={c.address}>
                      {i > 0 && <RowDivider />}
                      <Row c={c} onPick={pick} />
                    </React.Fragment>
                  ))}
                </Section>
              )}
              {rest.length > 0 && (
                <Section title={favorites.length > 0 ? t('contacts.sectionRecent') : t('contacts.title')}>
                  {rest.map((c, i) => (
                    <React.Fragment key={c.address}>
                      {i > 0 && <RowDivider />}
                      <Row c={c} onPick={pick} />
                    </React.Fragment>
                  ))}
                </Section>
              )}
            </>
          )}
        </ScrollView>
      </View>
    </AppModal>
  );
}

/** Hairline inset past the leading avatar/icon so it aligns under the row text. */
function RowDivider() {
  return <View style={styles.rowDividerWrap}><Divider /></View>;
}

function Section({ title, children }: { title?: string; children: React.ReactNode }) {
  return (
    <View style={styles.section}>
      {title && <SectionLabel style={styles.sectionLabel}>{title}</SectionLabel>}
      {children}
    </View>
  );
}

function GroupRow({ g, onPick }: { g: ContactGroup; onPick: (g: ContactGroup) => void }) {
  const { t } = useTranslation();
  const members = t('contacts.groupMembers', { count: g.members.length });
  return (
    <Pressable
      style={styles.row}
      onPress={() => onPick(g)}
      testID="group-row"
      accessibilityRole="button"
      accessibilityLabel={`${g.name}, ${members}`}
    >
      <View style={styles.iconCircle}>
        <Users size={19} color={color.fg.muted} strokeWidth={2} />
      </View>
      <View style={styles.rowInfo}>
        <Text style={styles.rowName} numberOfLines={1}>{g.name}</Text>
        <Text style={styles.rowAddr}>{members}</Text>
      </View>
      <ChevronRight size={18} color={color.fg.subtle} strokeWidth={2} />
    </Pressable>
  );
}

function Row({ c, onPick }: {
  c: Contact;
  onPick: (address: string, name?: string) => void;
}) {
  const { t } = useTranslation();
  const name = contactDisplayName(c);
  return (
    <Pressable
      style={styles.row}
      onPress={() => onPick(c.address, name || undefined)}
      accessibilityRole="button"
      accessibilityLabel={name || shortAddr(c.address)}
    >
      <ContactAvatar name={name} address={c.address} kind={c.kind} size={ROW_AVATAR} />
      <View style={styles.rowInfo}>
        <View style={styles.rowNameLine}>
          <Text style={styles.rowName} numberOfLines={1}>{name || shortAddr(c.address)}</Text>
          {c.favorite && <Star size={12} color={color.warning.base} strokeWidth={2} fill={color.warning.base} />}
        </View>
        <Text style={styles.rowAddr} numberOfLines={1}>
          {name ? shortAddr(c.address) : (c.kind === 'account' ? t('contacts.kindAccount') : ' ')}
          {c.txCount > 0 ? `  ·  ${t('contacts.sends', { count: c.txCount })}` : ''}
        </Text>
      </View>
    </Pressable>
  );
}

const styles = createStyles(() => ({
  container: { paddingHorizontal: space['2xl'], paddingTop: space.lg, flex: 1 },
  header: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    marginBottom: space.xl,
  },
  title: { fontSize: text['2xl'], ...inter.bold, color: color.fg.base },

  searchWrap: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    backgroundColor: color.bg.sunken, borderRadius: radius.xl,
    paddingHorizontal: space.xl, paddingVertical: space.lg, marginBottom: space.sm,
  },
  searchInput: { flex: 1, fontSize: text.lg, ...inter.regular, color: color.fg.base, padding: 0 },

  scroll: { flex: 1 },
  section: { marginBottom: space.md },
  sectionLabel: { marginLeft: space.sm },

  // De-boxed rows: single shared left inset, hairline dividers between them.
  row: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.md, paddingHorizontal: space.sm, minHeight: 56,
  },
  rowDividerWrap: { marginLeft: space.sm + ROW_AVATAR + space.lg },
  iconCircle: {
    width: ROW_AVATAR, height: ROW_AVATAR, borderRadius: ROW_AVATAR / 2,
    alignItems: 'center', justifyContent: 'center', backgroundColor: color.bg.sunken,
  },
  rowInfo: { flex: 1, gap: 2 },
  rowNameLine: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  rowName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base, flexShrink: 1 },
  rowAddr: { fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.muted },
  saveText: { fontSize: text.base, ...inter.semibold, color: color.fg.muted, paddingHorizontal: space.sm },

  loading: { paddingVertical: space['4xl'], alignItems: 'center' },
  empty: { paddingVertical: space['4xl'], alignItems: 'center', paddingHorizontal: space.xl, gap: space.md },
  emptyTitle: { fontSize: text.lg, ...inter.semibold, color: color.fg.base, textAlign: 'center' },
  emptyHint: { fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center', lineHeight: 20 },
  addContactBtn: { paddingVertical: space.sm, paddingHorizontal: space.lg, marginTop: space.xs },
  addContactText: { fontSize: text.base, ...inter.semibold, color: color.accent.base },
}));
