// Recently-opened dApps — a bottom sheet reachable from the Connections tab.
//
// Hidden behind a clock icon (the list is not shown until asked for). Tap a row to
// reopen the dApp, swipe-free per-row delete, or clear all. Deliberately minimal:
// favicon + host, newest-first, hairline dividers — the app's design language.
import { useCallback, useEffect, useState } from 'react';
import { Image, Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Globe, X } from 'lucide-react-native';

import { AppModal } from '@/components/ui/AppModal';
import {
  clearBrowserHistory,
  deleteBrowserHistory,
  getBrowserHistory,
  type BrowserHistoryEntry,
} from '@/services/browser-history';
import { showAlert } from '@/services/platform';
import { color, space, text as textScale, createStyles } from '@/constants/theme';

interface Props {
  visible: boolean;
  onClose: () => void;
  /** Reopen a dApp — the caller navigates to /browser and closes the sheet. */
  onOpen: (url: string) => void;
}

/** Favicon with a graceful Globe fallback (broken/missing icon). */
function HistoryIcon({ favicon }: { favicon: string }) {
  const [broken, setBroken] = useState(false);
  if (!favicon || broken) {
    return (
      <View style={styles.iconFallback}>
        <Globe size={16} color={color.fg.subtle} />
      </View>
    );
  }
  return <Image source={{ uri: favicon }} style={styles.icon} onError={() => setBroken(true)} />;
}

export function BrowserHistorySheet({ visible, onClose, onOpen }: Props) {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<BrowserHistoryEntry[]>([]);

  const refresh = useCallback(() => {
    void getBrowserHistory().then(setEntries);
  }, []);

  useEffect(() => {
    if (visible) refresh();
  }, [visible, refresh]);

  const remove = useCallback(
    (origin: string) => {
      void deleteBrowserHistory(origin).then(refresh);
    },
    [refresh],
  );

  const confirmClear = useCallback(() => {
    showAlert(t('connect.browser.clearAllTitle'), t('connect.browser.clearAllBody'), [
      { text: t('connect.browser.cancel'), style: 'cancel' },
      {
        text: t('connect.browser.clearAll'),
        style: 'destructive',
        onPress: () => void clearBrowserHistory().then(refresh),
      },
    ]);
  }, [t, refresh]);

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.sheet}>
        <View style={styles.header}>
          <Text style={styles.title}>{t('connect.browser.historyTitle')}</Text>
          {entries.length > 0 ? (
            <Pressable
              hitSlop={8}
              onPress={confirmClear}
              accessibilityRole="button"
              accessibilityLabel={t('connect.browser.clearAll')}
            >
              <Text style={styles.clear}>{t('connect.browser.clearAll')}</Text>
            </Pressable>
          ) : null}
        </View>

        {entries.length === 0 ? (
          <Text style={styles.empty}>{t('connect.browser.historyEmpty')}</Text>
        ) : (
          <ScrollView style={styles.list} showsVerticalScrollIndicator={false}>
            {entries.map((e, i) => (
              <View key={e.origin} style={[styles.row, i > 0 && styles.rowDivider]}>
                <Pressable
                  style={styles.rowMain}
                  onPress={() => onOpen(e.url)}
                  accessibilityRole="button"
                  accessibilityLabel={e.host}
                >
                  <HistoryIcon favicon={e.favicon} />
                  <View style={styles.rowText}>
                    <Text style={styles.host} numberOfLines={1}>{e.host}</Text>
                    {e.title && e.title !== e.host ? (
                      <Text style={styles.sub} numberOfLines={1}>{e.title}</Text>
                    ) : null}
                  </View>
                </Pressable>
                <Pressable
                  hitSlop={10}
                  onPress={() => remove(e.origin)}
                  style={styles.del}
                  accessibilityRole="button"
                  accessibilityLabel={t('connect.browser.a11yDeleteHistory')}
                >
                  <X size={16} color={color.fg.subtle} />
                </Pressable>
              </View>
            ))}
          </ScrollView>
        )}
      </View>
    </AppModal>
  );
}

const styles = createStyles(() => ({
  sheet: { paddingHorizontal: space.xl, paddingTop: space.sm, paddingBottom: space.sm, maxHeight: 460 },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: space.md,
  },
  title: { color: color.fg.base, fontSize: textScale.lg, fontWeight: '700' },
  clear: { color: color.error.base, fontSize: textScale.sm, fontWeight: '600' },
  empty: {
    color: color.fg.muted,
    fontSize: textScale.sm,
    textAlign: 'center',
    paddingVertical: space['2xl'],
  },
  list: { alignSelf: 'stretch' },
  row: { flexDirection: 'row', alignItems: 'center', paddingVertical: space.sm },
  rowDivider: { borderTopWidth: StyleSheet.hairlineWidth, borderTopColor: color.border.base },
  rowMain: { flex: 1, flexDirection: 'row', alignItems: 'center', gap: space.md },
  icon: { width: 28, height: 28, borderRadius: 8 },
  iconFallback: {
    width: 28,
    height: 28,
    borderRadius: 8,
    backgroundColor: color.bg.sunken,
    alignItems: 'center',
    justifyContent: 'center',
  },
  rowText: { flex: 1 },
  host: { color: color.fg.base, fontSize: textScale.sm, fontWeight: '600' },
  sub: { color: color.fg.muted, fontSize: 12, marginTop: 1 },
  del: { padding: space.sm },
}));
