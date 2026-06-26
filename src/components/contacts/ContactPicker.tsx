/**
 * ContactPicker — the unified recipient chooser.
 *
 * One sheet for: favorites, recent recipients (from send history), saved
 * contacts, and live search by name or address. Picking a row fills the caller's
 * recipient field. If you type/paste a fresh valid address, it offers to use it
 * directly (and save it) — so the picker never gets in the way of a one-off send.
 */
import React, { useEffect, useMemo, useState } from 'react';
import { View, Text, ScrollView, Pressable, TextInput, ActivityIndicator } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Search, X, Star, BookmarkPlus } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { ContactAvatar } from '@/components/contacts/ContactAvatar';
import { shortAddr } from '@/models/types';
import {
  getAllContacts, sortContacts, matchesQuery, contactDisplayName, saveContact,
  type Contact,
} from '@/services/contacts';
import { color, text, inter, space, radius, font, createStyles } from '@/constants/theme';
import { hapticLight } from '@/services/platform';

const ADDR_RE = /^0x[0-9a-fA-F]{40}$/;

export function ContactPicker({ visible, onClose, onSelect, myAddress }: {
  visible: boolean;
  onClose: () => void;
  onSelect: (address: string, name?: string) => void;
  myAddress?: string;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [contacts, setContacts] = useState<Contact[] | null>(null);

  useEffect(() => {
    if (!visible) return;
    setQuery('');
    let cancelled = false;
    getAllContacts(myAddress)
      .then((list) => { if (!cancelled) setContacts(sortContacts(list)); })
      .catch(() => { if (!cancelled) setContacts([]); });
    return () => { cancelled = true; };
  }, [visible, myAddress]);

  const filtered = useMemo(
    () => (contacts ?? []).filter((c) => matchesQuery(c, query)),
    [contacts, query],
  );
  const favorites = filtered.filter((c) => c.favorite);
  const rest = filtered.filter((c) => !c.favorite);

  // A pasted/typed fresh address that isn't already a contact → offer it directly.
  const typedAddr = ADDR_RE.test(query.trim()) ? query.trim().toLowerCase() : null;
  const typedIsKnown = !!typedAddr && (contacts ?? []).some((c) => c.address === typedAddr);

  const pick = (address: string, name?: string) => {
    hapticLight();
    onSelect(address, name);
    onClose();
  };

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.container}>
        <View style={styles.header}>
          <Text style={styles.title}>{t('contacts.pickerTitle')}</Text>
          <Pressable onPress={onClose} hitSlop={8}>
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
            <Pressable onPress={() => setQuery('')} hitSlop={8}>
              <X size={15} color={color.fg.subtle} strokeWidth={2} />
            </Pressable>
          )}
        </View>

        <ScrollView style={styles.scroll} keyboardShouldPersistTaps="handled" showsVerticalScrollIndicator={false}>
          {/* Use a freshly typed address right away */}
          {typedAddr && !typedIsKnown && (
            <Pressable style={styles.useRow} onPress={() => pick(typedAddr)}>
              <ContactAvatar name="" address={typedAddr} size={40} />
              <View style={styles.rowInfo}>
                <Text style={styles.rowName}>{t('contacts.useTyped', { defaultValue: 'Use this address' })}</Text>
                <Text style={styles.rowAddr}>{shortAddr(typedAddr)}</Text>
              </View>
              <Pressable
                hitSlop={8}
                style={styles.saveBtn}
                onPress={(e) => { e.stopPropagation?.(); saveContact({ address: typedAddr }).then(() => pick(typedAddr)); }}
              >
                <BookmarkPlus size={18} color={color.accent.base} strokeWidth={2} />
              </Pressable>
            </Pressable>
          )}

          {contacts === null ? (
            <View style={styles.loading}><ActivityIndicator size="small" color={color.fg.muted} /></View>
          ) : filtered.length === 0 && !typedAddr ? (
            <View style={styles.empty}>
              <Text style={styles.emptyText}>
                {query ? t('contacts.noResults', { query }) : t('contacts.emptyPicker')}
              </Text>
            </View>
          ) : (
            <>
              {favorites.length > 0 && (
                <Section title={t('contacts.sectionFavorites')}>
                  {favorites.map((c) => <Row key={c.address} c={c} onPick={pick} />)}
                </Section>
              )}
              {rest.length > 0 && (
                <Section title={favorites.length > 0 ? t('contacts.sectionRecent') : undefined}>
                  {rest.map((c) => <Row key={c.address} c={c} onPick={pick} />)}
                </Section>
              )}
            </>
          )}
        </ScrollView>
      </View>
    </AppModal>
  );
}

function Section({ title, children }: { title?: string; children: React.ReactNode }) {
  return (
    <View style={styles.section}>
      {title && <Text style={styles.sectionTitle}>{title}</Text>}
      {children}
    </View>
  );
}

function Row({ c, onPick }: {
  c: Contact;
  onPick: (address: string, name?: string) => void;
}) {
  const { t } = useTranslation();
  const name = contactDisplayName(c);
  return (
    <Pressable style={styles.row} onPress={() => onPick(c.address, name || undefined)}>
      <ContactAvatar name={name} address={c.address} kind={c.kind} size={40} />
      <View style={styles.rowInfo}>
        <View style={styles.rowNameLine}>
          <Text style={styles.rowName} numberOfLines={1}>{name || shortAddr(c.address)}</Text>
          {c.favorite && <Star size={12} color={color.warning.base} strokeWidth={2} fill={color.warning.base} />}
        </View>
        <Text style={styles.rowAddr} numberOfLines={1}>
          {name ? shortAddr(c.address) : (c.kind === 'account' ? t('contacts.kindAccount') : ' ')}
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
    paddingHorizontal: space.xl, paddingVertical: space.lg, marginBottom: space.lg,
  },
  searchInput: { flex: 1, fontSize: text.lg, ...inter.regular, color: color.fg.base, padding: 0 },

  scroll: { flex: 1 },
  section: { marginBottom: space.xl },
  sectionTitle: {
    fontSize: 10, ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase', letterSpacing: 0.3, marginBottom: space.sm, marginLeft: space.sm,
  },

  row: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.md, paddingHorizontal: space.sm,
  },
  useRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.md, paddingHorizontal: space.lg,
    backgroundColor: color.accent.soft, borderRadius: radius.xl, marginBottom: space.lg,
  },
  rowInfo: { flex: 1, gap: 2 },
  rowNameLine: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  rowName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base, flexShrink: 1 },
  rowAddr: { fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.muted },
  saveBtn: {
    width: 36, height: 36, borderRadius: radius.lg,
    alignItems: 'center', justifyContent: 'center',
    backgroundColor: color.bg.raised,
  },

  loading: { paddingVertical: space['4xl'], alignItems: 'center' },
  empty: { paddingVertical: space['4xl'], alignItems: 'center', paddingHorizontal: space.xl },
  emptyText: { fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center' },
}));
