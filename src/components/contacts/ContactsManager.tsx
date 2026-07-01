/**
 * ContactsManager — the address-book management sheet (opened from Settings).
 *
 * Two views in one modal: a searchable list, and an add/edit form. The list is
 * calm by default — search is tucked behind a header icon, and a segmented
 * [All | Favorites] filter (with counts) sits above the rows so the book never
 * reads as one flat noisy wall. Favouriting is one tap; editing/deleting lives
 * in the form. Manual adds resolve an identity name (ENS/Basename/passkey) as
 * you type a valid address, so a contact rarely needs a hand-typed name.
 */
import React, { useEffect, useMemo, useState } from 'react';
import { View, Text, ScrollView, Pressable, TextInput, ActivityIndicator } from 'react-native';
import Animated from 'react-native-reanimated';
import { useTranslation } from 'react-i18next';
import { Search, X, Star, Plus, Trash2, ChevronLeft } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { AutoGrowTextInput } from '@/components/ui/AutoGrowTextInput';
import { VelaButton } from '@/components/ui/VelaButton';
import { ContactAvatar } from '@/components/contacts/ContactAvatar';
import { shortAddr, isAddress } from '@/models/types';
import { showAlert, hapticLight, hapticSuccess } from '@/services/platform';
import { fadeIn } from '@/constants/entering';
import {
  getAllContacts, sortContacts, matchesQuery, contactDisplayName,
  saveContact, deleteContact, toggleFavorite, enrichContactIdentity,
  type Contact,
} from '@/services/contacts';
import { color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';

type Filter = 'all' | 'starred';

export function ContactsManager({ visible, onClose }: { visible: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const [contacts, setContacts] = useState<Contact[] | null>(null);
  const [query, setQuery] = useState('');
  const [searching, setSearching] = useState(false);
  const [filter, setFilter] = useState<Filter>('all');
  const [view, setView] = useState<'list' | 'form'>('list');
  const [editing, setEditing] = useState<Contact | null>(null);

  const reload = React.useCallback(() => {
    getAllContacts().then((l) => setContacts(sortContacts(l))).catch(() => setContacts([]));
  }, []);

  useEffect(() => {
    if (!visible) return;
    setView('list');
    setQuery('');
    setSearching(false);
    setFilter('all');
    reload();
  }, [visible, reload]);

  const favorites = useMemo(() => (contacts ?? []).filter((c) => c.favorite), [contacts]);
  const others = useMemo(() => (contacts ?? []).filter((c) => !c.favorite), [contacts]);
  const searchResults = useMemo(
    () => (contacts ?? []).filter((c) => matchesQuery(c, query)),
    [contacts, query],
  );

  const total = contacts?.length ?? 0;
  // The filter only earns its place once there's something to separate.
  const showSegment = !searching && total > 0 && favorites.length > 0;

  const openAdd = () => { setEditing(null); setView('form'); };
  const openEdit = (c: Contact) => { setEditing(c); setView('form'); };

  const openSearch = () => { hapticLight(); setSearching(true); };
  const closeSearch = () => { hapticLight(); setQuery(''); setSearching(false); };

  const onToggleFav = async (c: Contact) => {
    hapticLight();
    await toggleFavorite(c.address);
    reload();
  };

  const onDelete = (c: Contact) => {
    const name = contactDisplayName(c) || shortAddr(c.address);
    showAlert(t('contacts.deleteTitle'), t('contacts.deleteBody', { name }), [
      { text: t('contacts.cancel'), style: 'cancel' },
      {
        text: t('contacts.delete'),
        style: 'destructive',
        onPress: async () => { await deleteContact(c.address); setView('list'); reload(); },
      },
    ]);
  };

  const setFilterTo = (f: Filter) => { if (f !== filter) { hapticLight(); setFilter(f); } };

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.container}>
        {view === 'list' ? (
          <>
            {searching ? (
              <Animated.View style={styles.searchHeader} entering={fadeIn(0, 160)}>
                <Search size={16} color={color.fg.subtle} strokeWidth={2} />
                <TextInput
                  style={styles.searchInput}
                  placeholder={t('contacts.searchPlaceholder')}
                  placeholderTextColor={color.fg.subtle}
                  value={query}
                  onChangeText={setQuery}
                  autoFocus
                  autoCapitalize="none"
                  autoCorrect={false}
                  returnKeyType="search"
                />
                <Pressable onPress={closeSearch} hitSlop={8} style={styles.searchClose}>
                  <X size={18} color={color.fg.muted} strokeWidth={2} />
                </Pressable>
              </Animated.View>
            ) : (
              <View style={styles.header}>
                <Text style={styles.title}>{t('contacts.title')}</Text>
                <View style={styles.headerActions}>
                  <Pressable onPress={openSearch} hitSlop={8} style={styles.iconBtn}>
                    <Search size={20} color={color.fg.muted} strokeWidth={2} />
                  </Pressable>
                  <Pressable onPress={openAdd} hitSlop={8} style={styles.addBtn}>
                    <Plus size={18} color={color.accent.base} strokeWidth={2.5} />
                  </Pressable>
                  <Pressable onPress={onClose} hitSlop={8} style={styles.iconBtn}>
                    <X size={22} color={color.fg.base} strokeWidth={2} />
                  </Pressable>
                </View>
              </View>
            )}

            {showSegment && (
              <View style={styles.segmentRow}>
                <Segment
                  label={t('contacts.filterAll')}
                  count={total}
                  active={filter === 'all'}
                  onPress={() => setFilterTo('all')}
                />
                <Segment
                  label={t('contacts.sectionFavorites')}
                  count={favorites.length}
                  active={filter === 'starred'}
                  onPress={() => setFilterTo('starred')}
                  starred
                />
              </View>
            )}

            <ScrollView style={styles.scroll} keyboardShouldPersistTaps="handled" showsVerticalScrollIndicator={false}>
              {contacts === null ? (
                <View style={styles.loading}><ActivityIndicator size="small" color={color.fg.muted} /></View>
              ) : searching ? (
                searchResults.length === 0 ? (
                  <View style={styles.empty}>
                    <Text style={styles.emptyTitle}>
                      {query ? t('contacts.noResults', { query }) : t('contacts.empty')}
                    </Text>
                  </View>
                ) : (
                  searchResults.map((c) => (
                    <ContactRow key={c.address} c={c} onOpen={openEdit} onToggleFav={onToggleFav} />
                  ))
                )
              ) : total === 0 ? (
                <View style={styles.empty}>
                  <Text style={styles.emptyTitle}>{t('contacts.empty')}</Text>
                  <Text style={styles.emptyHint}>{t('contacts.emptyHint')}</Text>
                </View>
              ) : filter === 'starred' ? (
                favorites.map((c) => (
                  <ContactRow key={c.address} c={c} onOpen={openEdit} onToggleFav={onToggleFav} />
                ))
              ) : (
                <>
                  {favorites.length > 0 && (
                    <Section title={others.length > 0 ? t('contacts.sectionFavorites') : undefined}>
                      {favorites.map((c) => (
                        <ContactRow key={c.address} c={c} onOpen={openEdit} onToggleFav={onToggleFav} />
                      ))}
                    </Section>
                  )}
                  {others.length > 0 && (
                    <Section title={favorites.length > 0 ? t('contacts.sectionRecent') : undefined}>
                      {others.map((c) => (
                        <ContactRow key={c.address} c={c} onOpen={openEdit} onToggleFav={onToggleFav} />
                      ))}
                    </Section>
                  )}
                </>
              )}
            </ScrollView>
          </>
        ) : (
          <ContactForm
            editing={editing}
            onBack={() => setView('list')}
            onSaved={() => { setView('list'); reload(); }}
            onDelete={editing ? () => onDelete(editing) : undefined}
          />
        )}
      </View>
    </AppModal>
  );
}

function Segment({ label, count, active, onPress, starred }: {
  label: string;
  count: number;
  active: boolean;
  onPress: () => void;
  starred?: boolean;
}) {
  return (
    <Pressable style={[styles.segment, active && styles.segmentActive]} onPress={onPress}>
      {starred && (
        <Star
          size={13}
          color={active ? color.warning.base : color.fg.subtle}
          strokeWidth={2}
          fill={active ? color.warning.base : 'none'}
        />
      )}
      <Text style={[styles.segmentLabel, active && styles.segmentLabelActive]} numberOfLines={1}>
        {label}
      </Text>
      <Text style={[styles.segmentCount, active && styles.segmentCountActive]}>{count}</Text>
    </Pressable>
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

function ContactRow({ c, onOpen, onToggleFav }: {
  c: Contact;
  onOpen: (c: Contact) => void;
  onToggleFav: (c: Contact) => void;
}) {
  const { t } = useTranslation();
  const name = contactDisplayName(c);
  return (
    <Pressable
      style={({ pressed }) => [styles.row, pressed && styles.rowPressed]}
      onPress={() => onOpen(c)}
    >
      <ContactAvatar name={name} address={c.address} kind={c.kind} size={42} />
      <View style={styles.rowInfo}>
        <Text style={styles.rowName} numberOfLines={1}>{name || shortAddr(c.address)}</Text>
        <Text style={styles.rowAddr} numberOfLines={1}>
          {name ? shortAddr(c.address) : (c.kind === 'account' ? t('contacts.kindAccount') : shortAddr(c.address))}
        </Text>
      </View>
      <Pressable hitSlop={10} onPress={() => onToggleFav(c)} style={styles.starBtn}>
        <Star
          size={18}
          color={c.favorite ? color.warning.base : color.fg.subtle}
          strokeWidth={2}
          fill={c.favorite ? color.warning.base : 'none'}
        />
      </Pressable>
    </Pressable>
  );
}

function ContactForm({ editing, onBack, onSaved, onDelete }: {
  editing: Contact | null;
  onBack: () => void;
  onSaved: () => void;
  onDelete?: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(editing?.name ?? '');
  const [address, setAddress] = useState(editing?.address ?? '');
  const [resolved, setResolved] = useState<string | undefined>(editing?.resolvedName);
  const [saving, setSaving] = useState(false);
  const isEdit = !!editing;
  const addrValid = isAddress(address.trim());

  // Auto-resolve an identity name for a freshly entered address (add mode).
  useEffect(() => {
    if (isEdit || !addrValid) { setResolved(undefined); return; }
    let cancelled = false;
    enrichContactIdentity(address.trim()).then((id) => { if (!cancelled) setResolved(id.name); });
    return () => { cancelled = true; };
  }, [address, addrValid, isEdit]);

  const canSave = addrValid && !saving;

  const onSave = async () => {
    if (!canSave) return;
    setSaving(true);
    await saveContact({
      address: address.trim(),
      name: name.trim() || undefined,
      resolvedName: resolved,
    });
    hapticSuccess();
    onSaved();
  };

  return (
    <>
      <View style={styles.header}>
        <Pressable onPress={onBack} hitSlop={8} style={styles.backBtn}>
          <ChevronLeft size={22} color={color.fg.base} strokeWidth={2} />
        </Pressable>
        <Text style={styles.title}>{isEdit ? t('contacts.editTitle') : t('contacts.addTitle')}</Text>
        <View style={{ width: 22 }} />
      </View>

      <ScrollView style={styles.scroll} keyboardShouldPersistTaps="handled" showsVerticalScrollIndicator={false}>
        <View style={styles.formAvatar}>
          <ContactAvatar name={name || resolved || ''} address={address || '0x'} kind={editing?.kind} size={64} />
        </View>

        <Text style={styles.fieldLabel}>{t('contacts.nameLabel')}</Text>
        <TextInput
          style={styles.field}
          placeholder={resolved ?? t('contacts.namePlaceholder')}
          placeholderTextColor={color.fg.subtle}
          value={name}
          onChangeText={setName}
          autoCapitalize="words"
        />

        <Text style={[styles.fieldLabel, { marginTop: space.xl }]}>{t('contacts.addressLabel')}</Text>
        <AutoGrowTextInput
          style={[styles.field, styles.fieldMono, isEdit && styles.fieldDisabled]}
          minHeight={52}
          placeholder={t('contacts.addressPlaceholder')}
          placeholderTextColor={color.fg.subtle}
          value={address}
          onChangeText={setAddress}
          editable={!isEdit}
          autoCapitalize="none"
          autoCorrect={false}
        />
        {address.length > 0 && !addrValid && (
          <Text style={styles.fieldError}>{t('contacts.invalidAddress')}</Text>
        )}

        <View style={styles.formActions}>
          <VelaButton title={t('contacts.save')} onPress={onSave} variant="accent" loading={saving} disabled={!canSave} />
          {isEdit && onDelete && (
            <Pressable style={styles.deleteRow} onPress={onDelete} hitSlop={8}>
              <Trash2 size={16} color={color.error.base} strokeWidth={2} />
              <Text style={styles.deleteText}>{t('contacts.delete')}</Text>
            </Pressable>
          )}
        </View>
      </ScrollView>
    </>
  );
}

const styles = createStyles(() => ({
  container: { paddingHorizontal: space['2xl'], paddingTop: space.lg, flex: 1 },
  header: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    marginBottom: space.xl, minHeight: 40,
  },
  headerActions: { flexDirection: 'row', alignItems: 'center', gap: space.lg },
  title: { fontSize: text['2xl'], ...inter.bold, color: color.fg.base },
  iconBtn: { width: 32, height: 32, alignItems: 'center', justifyContent: 'center' },
  addBtn: {
    width: 32, height: 32, borderRadius: radius.lg, backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  backBtn: { width: 32, height: 32, justifyContent: 'center' },

  // Search — revealed from the header icon, replaces the title row.
  searchHeader: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    backgroundColor: color.bg.sunken, borderRadius: radius.xl,
    paddingHorizontal: space.xl, paddingVertical: space.lg,
    marginBottom: space.xl, minHeight: 40,
  },
  searchInput: { flex: 1, fontSize: text.lg, ...inter.regular, color: color.fg.base, padding: 0 },
  searchClose: { width: 24, height: 24, alignItems: 'center', justifyContent: 'center' },

  // Segmented [All | Favorites] filter with counts.
  segmentRow: {
    flexDirection: 'row', gap: space.xs,
    backgroundColor: color.bg.sunken, borderRadius: radius.xl,
    padding: 3, marginBottom: space.lg,
  },
  segment: {
    flex: 1, flexDirection: 'row', alignItems: 'center', justifyContent: 'center',
    gap: space.sm, paddingVertical: space.md, borderRadius: radius.lg,
  },
  segmentActive: { backgroundColor: color.bg.raised, ...shadow.sm },
  segmentLabel: { fontSize: text.base, ...inter.semibold, color: color.fg.muted },
  segmentLabelActive: { color: color.fg.base },
  segmentCount: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle },
  segmentCountActive: { color: color.fg.muted },

  scroll: { flex: 1 },
  section: { marginBottom: space.lg },
  sectionTitle: {
    fontSize: 10, ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase', letterSpacing: 0.3, marginBottom: space.sm, marginLeft: space.sm,
  },

  row: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.lg, paddingHorizontal: space.sm, borderRadius: radius.lg,
  },
  rowPressed: { backgroundColor: color.bg.sunken },
  rowInfo: { flex: 1, gap: 2 },
  rowName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  rowAddr: { fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.muted },
  starBtn: { width: 36, height: 36, alignItems: 'center', justifyContent: 'center' },

  loading: { paddingVertical: space['4xl'], alignItems: 'center' },
  empty: { paddingVertical: space['4xl'], alignItems: 'center', paddingHorizontal: space.xl, gap: space.md },
  emptyTitle: { fontSize: text.lg, ...inter.semibold, color: color.fg.base, textAlign: 'center' },
  emptyHint: { fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center', lineHeight: 20 },

  // ===== Form =====
  formAvatar: { alignItems: 'center', paddingVertical: space.xl },
  fieldLabel: {
    fontSize: 10, ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase', letterSpacing: 0.3, marginBottom: space.sm, marginLeft: space.sm,
  },
  field: {
    backgroundColor: color.bg.sunken, borderRadius: radius.xl,
    paddingHorizontal: space.xl, paddingVertical: space.lg,
    fontSize: text.lg, ...inter.regular, color: color.fg.base,
  },
  fieldMono: { fontFamily: font.mono, fontWeight: '500' as const, fontSize: text.base },
  fieldDisabled: { color: color.fg.muted },
  fieldError: { fontSize: text.sm, ...inter.medium, color: color.error.base, marginTop: space.sm, marginLeft: space.sm },
  formActions: { marginTop: space['2xl'], gap: space.xl },
  deleteRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: space.sm, paddingVertical: space.md },
  deleteText: { fontSize: text.base, ...inter.semibold, color: color.error.base },
}));
