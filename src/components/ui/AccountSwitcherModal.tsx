/**
 * AccountSwitcherModal — the multi-account picker that was built three times,
 * near-identically, in Home, Assets and Settings (avatar + balance-sorted list +
 * SWITCH_ACCOUNT dispatch + header total). This is the one copy.
 *
 * Balance sourcing is flexible so a screen that already holds a cached-balance map
 * doesn't pay for a second fetch:
 *   - pass `balances` (+ optional `loading`) to render from the screen's own state, or
 *   - omit it and the modal loads balances itself when it becomes visible.
 *
 * The header subtitle stays at the call site (each screen has its own i18n copy):
 *   formatSubtitle={(amount) => t('assets.switcherTotal', { amount })}
 */
import React, { useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, ScrollView, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { useRouter } from 'expo-router';
import { Check, X } from 'lucide-react-native';

import { AppModal } from '@/components/ui/AppModal';
import { IdenticonViewerProvider } from '@/components/ui/IdenticonViewerProvider';
import { SectionLabel } from '@/components/ui/SectionLabel';
import { VelaButton } from '@/components/ui/VelaButton';
import { WalletAvatar } from '@/components/ui/WalletAvatar';
import type { Account } from '@/models/types';
import { shortAddress, useWallet } from '@/models/wallet-state';
import { useBalancePrivacy } from '@/hooks/use-balance-privacy';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { getAccountBalances } from '@/services/balance-cache';
import { sortAccountsByBalance, totalAccountBalance } from '@/services/accounts';
import { hapticSuccess } from '@/services/platform';
import { color, createStyles, font, inter, space, text } from '@/constants/theme';

type Props = {
  visible: boolean;
  onClose: () => void;
  /** Header title — each screen passes its own i18n string. */
  title: string;
  /**
   * Render the header subtitle from the formatted total + account count, e.g.
   * `(amount) => t('assets.switcherTotal', { amount })`. Omit for no subtitle.
   */
  formatSubtitle?: (amount: string, count: number) => string;
  /**
   * Cached USD balances by address. Pass the screen's own map to avoid a second
   * fetch; omit and the modal loads them itself once visible.
   */
  balances?: Map<string, number>;
  /** Parent's loading flag — only meaningful when `balances` is supplied. */
  loading?: boolean;
  /** Show the "create / sign in" actions (Settings only). */
  showCreateActions?: boolean;
  /**
   * Fired alongside the SWITCH_ACCOUNT dispatch with the NEWLY-selected account, so
   * a caller can react to the switch (e.g. the in-app browser re-points its dApp
   * grant + emits accountsChanged). Receives the account directly so it needn't wait
   * for the async wallet-state update.
   */
  onSwitch?: (index: number, account: Account) => void;
  /** Extra content rendered below the account list (e.g. a "close page" row). */
  footer?: React.ReactNode;
};

export function AccountSwitcherModal({
  visible, onClose, title, formatSubtitle, balances, loading, showCreateActions, onSwitch, footer,
}: Props) {
  const { t } = useTranslation();
  const { state, dispatch } = useWallet();
  const router = useRouter();
  const dc = useDisplayCurrency();
  // Balance privacy: the switcher lists every account's fiat balance — exactly
  // the numbers the masked hero conceals — so it masks with the same store.
  const { hidden } = useBalancePrivacy();
  const fmtBal = (usd: number) => (hidden ? '••••' : dc.fmt(usd));

  // Self-load balances only when the parent doesn't supply them.
  const selfLoad = balances === undefined;
  const [loaded, setLoaded] = useState<Map<string, number>>(new Map());
  const [selfLoading, setSelfLoading] = useState(false);
  useEffect(() => {
    if (!selfLoad || !visible) return;
    setSelfLoading(true);
    getAccountBalances(state.accounts.map((a) => a.address))
      .then(setLoaded)
      .finally(() => setSelfLoading(false));
  }, [selfLoad, visible, state.accounts]);

  const bals = balances ?? loaded;
  const isLoading = balances ? !!loading : selfLoading;
  const ordered = sortAccountsByBalance(state.accounts, bals);

  return (
    <AppModal visible={visible} onClose={onClose}>
      {/* Host the identicon viewer INSIDE this modal: on iOS a modal opened from
          the root VC while this pageSheet is up deadlocks — presenting it from
          this modal's own VC does not. */}
      <IdenticonViewerProvider>
      <View style={styles.container}>
        <View style={styles.header}>
          <View style={styles.headerInfo}>
            <Text style={styles.title}>{title}</Text>
            {bals.size > 0 && formatSubtitle && (
              <Text style={styles.subtitle}>{formatSubtitle(fmtBal(totalAccountBalance(bals)), state.accounts.length)}</Text>
            )}
          </View>
          {isLoading && <ActivityIndicator size="small" color={color.fg.subtle} style={styles.spinner} />}
          <Pressable onPress={onClose} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
        </View>

        <ScrollView contentContainerStyle={styles.list}>
          <SectionLabel style={styles.sectionLabel}>{title}</SectionLabel>
          {ordered.map(({ account, index }, pos) => {
            const isActive = index === state.activeAccountIndex;
            const bal = bals.get(account.address);
            return (
              <React.Fragment key={account.id}>
                {pos > 0 && <View style={styles.sep} />}
                <Pressable
                  style={styles.item}
                  onPress={() => { dispatch({ type: 'SWITCH_ACCOUNT', index }); hapticSuccess(); onSwitch?.(index, account); onClose(); }}
                >
                  <WalletAvatar name={account.name} address={account.address} size={40} letterSize={text.base} enlargeable />
                  <View style={styles.info}>
                    <Text style={[styles.name, isActive && styles.nameActive]} numberOfLines={1}>{account.name}</Text>
                    <Text style={styles.addr}>{shortAddress(account.address)}</Text>
                  </View>
                  <View style={styles.right}>
                    {bal != null
                      ? <Text style={styles.bal}>{fmtBal(bal)}</Text>
                      : isLoading ? <ActivityIndicator size="small" color={color.fg.subtle} /> : null}
                    {isActive && <Check size={18} color={color.accent.base} />}
                  </View>
                </Pressable>
              </React.Fragment>
            );
          })}

          {showCreateActions && (
            <View style={styles.actions}>
              <VelaButton title={t('settingsModals.account.createNew')} onPress={() => { onClose(); router.push('/onboarding'); }} />
              <VelaButton title={t('settingsModals.account.signInExisting')} variant="secondary" onPress={() => { onClose(); router.push('/onboarding'); }} />
            </View>
          )}

          {footer}
        </ScrollView>
      </View>
      </IdenticonViewerProvider>
    </AppModal>
  );
}

const styles = createStyles(() => ({
  container: { flex: 1, backgroundColor: color.bg.base },
  header: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    gap: space.md, paddingHorizontal: space.xl, paddingVertical: space.lg,
  },
  headerInfo: { flex: 1, gap: space.xs },
  title: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  subtitle: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle },
  spinner: { marginRight: space.sm },
  list: { paddingHorizontal: space.xl, paddingBottom: space['2xl'] },
  sectionLabel: { marginTop: space.sm },
  // Accounts sit as open rows directly on the page, grouped under a SectionLabel
  // and separated by hairline dividers (inset past the avatar). The active account
  // is signalled by an accent name + the trailing Check — not by a boxed card.
  item: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    paddingVertical: space.lg,
  },
  // Hairline divider inset past the 40px avatar + its space.md gap so it aligns
  // under the account name (Apple-Wallet style).
  sep: { height: 1, backgroundColor: color.border.base, marginLeft: 40 + space.md },
  info: { flex: 1, gap: space.xs },
  name: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  nameActive: { color: color.accent.base },
  addr: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle, fontFamily: font.mono },
  right: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  bal: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  actions: { gap: space.md, paddingTop: space['4xl'], paddingBottom: space.lg, paddingHorizontal: space.sm },
}));
