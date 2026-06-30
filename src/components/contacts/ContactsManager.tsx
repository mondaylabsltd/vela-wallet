/**
 * ContactsManager — the address-book management sheet (opened from Settings).
 *
 * Two views in one modal: a searchable list, and an add/edit form. Saved and
 * recent contacts share the list; favouriting is one tap; editing/deleting lives
 * in the form. Manual adds resolve an identity name (ENS/Basename/passkey) as you
 * type a valid address, so a contact rarely needs a hand-typed name.
 */
import React, { useEffect, useMemo, useState } from 'react';
import { View, Text, ScrollView, Pressable, TextInput, ActivityIndicator } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Search, X, Star, Plus, Trash2, ChevronLeft } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { AutoGrowTextInput } from '@/components/ui/AutoGrowTextInput';
import { VelaButton } from '@/components/ui/VelaButton';
import { ContactAvatar } from '@/components/contacts/ContactAvatar';
import { shortAddr } from '@/models/types';
import { showAlert, hapticLight, hapticSuccess } from '@/services/platform';
import {
  getAllContacts, sortContacts, matchesQuery, contactDisplayName,
  saveContact, deleteContact, toggleFavorite, enrichContactIdentity,
  type Contact,
} from '@/services/contacts';
import { color, text, inter, space, radius, font, createStyles } from '@/constants/theme';

const ADDR_RE = /^0x[0-9a-fA-F]{40}$/;

export function ContactsManager({ visible, onClose }: { visible: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const [contacts, setContacts] = useState<Contact[] | null>(null);
  const [query, setQuery] = useState('');
  const [view, setView] = useState<'list' | 'form'>('list');
  const [editing, setEditing] = useState<Contact | null>(null);

  const reload = React.useCallback(() => {
    getAllContacts().then((l) => setContacts(sortContacts(l))).catch(() => setContacts([]));
  }, []);

  useEffect(() => {
    if (!visible) return;
    setView('list');
    setQuery('');
    reload();
  }, [visible, reload]);

  const filtered = useMemo(
    () => (contacts ?? []).filter((c) => matchesQuery(c, query)),
    [contacts, query],
  );

  const openAdd = () => { setEditing(null); setView('form'); };
  const openEdit = (c: Contact) => { setEditing(c); setView('form'); };

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

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.container}>
        {view === 'list' ? (
          <>
            <View style={styles.header}>
              <Text style={styles.title}>{t('contacts.title')}</Text>
              <View style={styles.headerActions}>
                <Pressable onPress={openAdd} hitSlop={8} style={styles.addBtn}>
                  <Plus size={18} color={color.accent.base} strokeWidth={2.5} />
                </Pressable>
                <Pressable onPress={onClose} hitSlop={8}>
                  <X size={22} color={color.fg.base} strokeWidth={2} />
                </Pressable>
              </View>
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
            </View>

            <ScrollView style={styles.scroll} keyboardShouldPersistTaps="handled" showsVerticalScrollIndicator={false}>
              {contacts === null ? (
                <View style={styles.loading}><ActivityIndicator size="small" color={color.fg.muted} /></View>
              ) : filtered.length === 0 ? (
                <View style={styles.empty}>
                  <Text style={styles.emptyTitle}>{query ? t('contacts.noResults', { query }) : t('contacts.empty')}</Text>
                  {!query && <Text style={styles.emptyHint}>{t('contacts.emptyHint')}</Text>}
                </View>
              ) : (
                filtered.map((c) => {
                  const name = contactDisplayName(c);
                  return (
                    <Pressable key={c.address} style={styles.row} onPress={() => openEdit(c)}>
                      <ContactAvatar name={name} address={c.address} kind={c.kind} size={42} />
                      <View style={styles.rowInfo}>
                        <Text style={styles.rowName} numberOfLines={1}>{name || shortAddr(c.address)}</Text>
                        <Text style={styles.rowAddr} numberOfLines={1}>
                          {name ? shortAddr(c.address) : (c.kind === 'account' ? t('contacts.kindAccount') : ' ')}
                          {c.source === 'manual' ? `  ·  ${t('contacts.savedTag')}` : ''}
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
                })
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
  const addrValid = ADDR_RE.test(address.trim());

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
    marginBottom: space.xl,
  },
  headerActions: { flexDirection: 'row', alignItems: 'center', gap: space.xl },
  title: { fontSize: text['2xl'], ...inter.bold, color: color.fg.base },
  addBtn: {
    width: 32, height: 32, borderRadius: radius.lg, backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  backBtn: { width: 32, height: 32, justifyContent: 'center' },

  searchWrap: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    backgroundColor: color.bg.sunken, borderRadius: radius.xl,
    paddingHorizontal: space.xl, paddingVertical: space.lg, marginBottom: space.lg,
  },
  searchInput: { flex: 1, fontSize: text.lg, ...inter.regular, color: color.fg.base, padding: 0 },

  scroll: { flex: 1 },
  row: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.md, paddingHorizontal: space.sm,
  },
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
