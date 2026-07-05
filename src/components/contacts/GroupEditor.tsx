/**
 * GroupEditor — create or edit a contact group (e.g. "Payroll"). A group is a
 * named set of saved contacts that can be picked wholesale as the recipients of a
 * split send. Membership is a simple multi-select over the saved address book.
 */
import React, { useEffect, useMemo, useState } from 'react';
import { View, Text, ScrollView, Pressable, TextInput, ActivityIndicator } from 'react-native';
import { useTranslation } from 'react-i18next';
import { ChevronLeft, Trash2, Check, Search } from 'lucide-react-native';
import { VelaButton } from '@/components/ui/VelaButton';
import { ContactAvatar } from '@/components/contacts/ContactAvatar';
import { shortAddr } from '@/models/types';
import { hapticSuccess, hapticLight } from '@/services/platform';
import {
  getSavedContacts, saveGroup, deleteGroup, contactDisplayName, matchesQuery,
  type Contact, type ContactGroup,
} from '@/services/contacts';
import { scaleFont, color, text, inter, space, radius, font, createStyles } from '@/constants/theme';

export function GroupEditor({ editing, onBack, onSaved }: {
  editing: ContactGroup | null;
  onBack: () => void;
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(editing?.name ?? '');
  const [selected, setSelected] = useState<Set<string>>(new Set(editing?.members ?? []));
  const [contacts, setContacts] = useState<Contact[] | null>(null);
  const [query, setQuery] = useState('');
  const [saving, setSaving] = useState(false);
  const isEdit = !!editing;

  useEffect(() => {
    let cancelled = false;
    getSavedContacts()
      .then((l) => { if (!cancelled) setContacts(l); })
      .catch(() => { if (!cancelled) setContacts([]); });
    return () => { cancelled = true; };
  }, []);

  const filtered = useMemo(
    () => (contacts ?? []).filter((c) => matchesQuery(c, query)),
    [contacts, query],
  );

  const toggle = (address: string) => {
    hapticLight();
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(address)) next.delete(address);
      else next.add(address);
      return next;
    });
  };

  const canSave = name.trim().length > 0 && !saving;

  const onSave = async () => {
    if (!canSave) return;
    setSaving(true);
    await saveGroup({ id: editing?.id, name: name.trim(), members: [...selected] });
    hapticSuccess();
    onSaved();
  };

  const onDelete = async () => {
    if (!editing) return;
    setSaving(true);
    await deleteGroup(editing.id);
    hapticSuccess();
    onSaved();
  };

  return (
    <>
      <View style={styles.header}>
        <Pressable onPress={onBack} hitSlop={8} style={styles.backBtn}>
          <ChevronLeft size={22} color={color.fg.base} strokeWidth={2} />
        </Pressable>
        <Text style={styles.title}>{isEdit ? t('contacts.groupEdit', { defaultValue: 'Edit group' }) : t('contacts.groupNew', { defaultValue: 'New group' })}</Text>
        <View style={{ width: 22 }} />
      </View>

      <Text style={styles.fieldLabel}>{t('contacts.groupNameLabel', { defaultValue: 'Group name' })}</Text>
      <TextInput
        testID="group-name"
        style={styles.field}
        placeholder={t('contacts.groupNamePlaceholder', { defaultValue: 'e.g. Payroll' })}
        placeholderTextColor={color.fg.subtle}
        value={name}
        onChangeText={setName}
        autoCapitalize="words"
      />

      <View style={styles.membersHead}>
        <Text style={styles.fieldLabel}>{t('contacts.groupMembersLabel', { defaultValue: 'Members' })}</Text>
        <Text style={styles.count}>{selected.size}</Text>
      </View>

      <View style={styles.searchWrap}>
        <Search size={15} color={color.fg.subtle} strokeWidth={2} />
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
          <View style={styles.empty}><Text style={styles.emptyText}>{t('contacts.groupNoContacts', { defaultValue: 'Save some contacts first, then group them here.' })}</Text></View>
        ) : (
          filtered.map((c) => {
            const on = selected.has(c.address);
            const dn = contactDisplayName(c);
            return (
              <Pressable key={c.address} style={styles.memberRow} onPress={() => toggle(c.address)} testID="group-member">
                <ContactAvatar name={dn} address={c.address} kind={c.kind} size={38} />
                <View style={styles.memberInfo}>
                  <Text style={styles.memberName} numberOfLines={1}>{dn || shortAddr(c.address)}</Text>
                  <Text style={styles.memberAddr} numberOfLines={1}>{shortAddr(c.address)}</Text>
                </View>
                <View style={[styles.check, on && styles.checkOn]}>
                  {on && <Check size={14} color={color.fg.inverse} strokeWidth={3} />}
                </View>
              </Pressable>
            );
          })
        )}
      </ScrollView>

      <View style={styles.actions}>
        <VelaButton title={t('contacts.save')} onPress={onSave} variant="accent" loading={saving} disabled={!canSave} />
        {isEdit && (
          <Pressable style={styles.deleteRow} onPress={onDelete} hitSlop={8}>
            <Trash2 size={16} color={color.error.base} strokeWidth={2} />
            <Text style={styles.deleteText}>{t('contacts.groupDelete', { defaultValue: 'Delete group' })}</Text>
          </Pressable>
        )}
      </View>
    </>
  );
}

const styles = createStyles(() => ({
  header: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: space.xl, minHeight: 40 },
  backBtn: { width: 32, height: 32, justifyContent: 'center' },
  title: { fontSize: text['2xl'], ...inter.bold, color: color.fg.base },
  fieldLabel: {
    fontSize: scaleFont(10), ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase', letterSpacing: 0.3, marginBottom: space.sm, marginLeft: space.sm,
  },
  field: {
    backgroundColor: color.bg.sunken, borderRadius: radius.xl,
    paddingHorizontal: space.xl, paddingVertical: space.lg,
    fontSize: text.lg, ...inter.regular, color: color.fg.base,
  },
  membersHead: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginTop: space.xl },
  count: { fontSize: text.sm, ...inter.bold, color: color.accent.base, marginRight: space.sm },
  searchWrap: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    backgroundColor: color.bg.sunken, borderRadius: radius.lg,
    paddingHorizontal: space.lg, paddingVertical: space.md, marginBottom: space.md,
  },
  searchInput: { flex: 1, fontSize: text.base, ...inter.regular, color: color.fg.base, padding: 0 },
  scroll: { flex: 1 },
  memberRow: { flexDirection: 'row', alignItems: 'center', gap: space.lg, paddingVertical: space.sm },
  memberInfo: { flex: 1, gap: 1 },
  memberName: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  memberAddr: { fontSize: text.xs, fontFamily: font.mono, color: color.fg.muted },
  check: {
    width: 24, height: 24, borderRadius: 12, borderWidth: 2, borderColor: color.border.strong,
    alignItems: 'center', justifyContent: 'center',
  },
  checkOn: { backgroundColor: color.accent.base, borderColor: color.accent.base },
  loading: { paddingVertical: space['4xl'], alignItems: 'center' },
  empty: { paddingVertical: space['3xl'], alignItems: 'center', paddingHorizontal: space.xl },
  emptyText: { fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center', lineHeight: 20 },
  actions: { paddingTop: space.lg, gap: space.lg },
  deleteRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: space.sm, paddingVertical: space.md },
  deleteText: { fontSize: text.base, ...inter.semibold, color: color.error.base },
}));
