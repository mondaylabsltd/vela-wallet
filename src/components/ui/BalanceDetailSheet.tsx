/**
 * BalanceDetailSheet — the "why is my total an estimate?" explainer.
 *
 * The hero shows a single warning line ("Some balances are still updating" /
 * "Some tokens couldn't be priced") whenever the live total is incomplete. That
 * line is now tappable, and this is what it opens: the concrete list of what's
 * behind the warning, so the user can see EXACTLY which networks and tokens are
 * responsible instead of a vague blanket notice.
 *
 * Two sections, each shown only when it has data (both can co-exist — a failed
 * chain suppresses the "unpriced" wording on the hero, but here we surface both):
 *   · Networks still updating — chains whose RPC read failed. Rate-limited ones
 *     are labeled transient (self-healing, no action); genuinely-failed ones get
 *     a "Fix" that hands off to the shared RPC-fix flow (via onFixChain).
 *   · Tokens without a price — held tokens the price feed couldn't value. Their
 *     balance is correct; they're just not counted in the total. Spam-filtered.
 *
 * Built on AppModal, mirroring ConnectionEventDetailSheet's header + open,
 * hairline-separated rows (no card pile), per the app's de-boxed design language.
 */
import React, { useEffect, useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { RefreshCw, X } from 'lucide-react-native';

import { AppModal } from '@/components/ui/AppModal';
import { RpcFixForm } from '@/components/ui/RpcTroubleBanner';
import { SectionLabel } from '@/components/ui/SectionLabel';
import { TokenRow } from '@/components/ui/TokenRow';
import { ChainLogo } from '@/components/ChainLogo';
import { chainName, networkForChainId, tokenBadgeNetwork } from '@/models/network';
import {
  tokenBalanceDouble, tokenChainId, tokenId, tokenLogoURLs, type APIToken,
} from '@/models/types';
import { formatTokenAmount, useLocalePrefs } from '@/services/locale-format';
import { useBalancePrivacy } from '@/hooks/use-balance-privacy';
import { color, createStyles, inter, space, text } from '@/constants/theme';

interface Props {
  visible: boolean;
  onClose: () => void;
  /** Chains whose RPC read failed (the FULL set — includes rate-limited ones). */
  failedChainIds: number[];
  /** Subset of failedChainIds that failed due to rate-limiting (transient). */
  rateLimitedChainIds: number[];
  /** Held tokens with a balance but no price (already spam-filtered by the caller). */
  unpricedTokens: APIToken[];
  /** Called after a failed chain's RPC is fixed from within the sheet, so the caller
      can drop it from the failed set + re-fetch. */
  onFixResolved: (chainId: number) => void;
  /** Re-run the balance fetch (helps rate-limited chains that self-heal). */
  onRetry: () => void;
  /** Tap a token row → its detail screen (caller closes the sheet + navigates). */
  onTokenPress: (token: APIToken) => void;
}

export function BalanceDetailSheet({
  visible, onClose, failedChainIds, rateLimitedChainIds, unpricedTokens, onFixResolved, onRetry, onTokenPress,
}: Props) {
  const { t } = useTranslation();
  useLocalePrefs(); // re-render when number format changes
  const { hidden } = useBalancePrivacy();

  // The RPC-fix form is shown IN PLACE (a sub-view of this same modal), never as a
  // second modal — presenting a sibling AppModal while this one dismisses is dropped
  // by UIKit on iOS. null = show the list; a chainId = show the fix form for it.
  const [fixChainId, setFixChainId] = useState<number | null>(null);
  useEffect(() => { if (!visible) setFixChainId(null); }, [visible]);

  const hasNetworks = failedChainIds.length > 0;
  const hasTokens = unpricedTokens.length > 0;

  // Once everything the sheet was explaining has recovered (e.g. a Retry lifted the
  // rate limit, or a background refresh repriced the tokens), there's nothing left to
  // show — close rather than stranding the user on the empty state. Guarded so it
  // never fires mid-fix.
  useEffect(() => {
    if (visible && fixChainId === null && !hasNetworks && !hasTokens) onClose();
  }, [visible, fixChainId, hasNetworks, hasTokens, onClose]);

  return (
    <AppModal visible={visible} onClose={onClose}>
      {fixChainId !== null ? (
        <RpcFixForm
          chainId={fixChainId}
          onClose={() => setFixChainId(null)}
          onResolved={onFixResolved}
        />
      ) : (
      <View style={styles.sheet}>
        <View style={styles.head}>
          <View style={styles.headSpacer} />
          <Text style={styles.headTitle} numberOfLines={1}>{t('home.balanceDetailTitle')}</Text>
          <Pressable onPress={onClose} hitSlop={8} style={styles.closeBtn} accessibilityRole="button">
            <X size={20} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </View>

        <ScrollView contentContainerStyle={styles.body} showsVerticalScrollIndicator={false}>
          {!hasNetworks && !hasTokens ? (
            <Text style={styles.empty}>{t('home.balanceDetailEmpty')}</Text>
          ) : null}

          {/* Networks still updating */}
          {hasNetworks ? (
            <>
              <SectionLabel style={styles.firstLabel}>{t('home.balanceDetailNetworksLabel')}</SectionLabel>
              <Text style={styles.note}>{t('home.balanceDetailNetworksNote')}</Text>
              {failedChainIds.map((id, i) => {
                const net = networkForChainId(id);
                const isRateLimited = rateLimitedChainIds.includes(id);
                return (
                  <View key={id}>
                    {i > 0 ? <View style={styles.netSep} /> : null}
                    <View style={styles.netRow}>
                      {net ? (
                        <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={36} />
                      ) : (
                        <View style={styles.netLogoFallback} />
                      )}
                      <View style={styles.netInfo}>
                        <Text style={styles.netName} numberOfLines={1}>
                          {net?.displayName ?? t('assets.chainFallback', { chainId: id })}
                        </Text>
                        <Text style={[styles.netStatus, isRateLimited ? styles.netStatusRetrying : styles.netStatusFailed]}>
                          {isRateLimited ? t('home.balanceDetailStatusRetrying') : t('home.balanceDetailStatusFailed')}
                        </Text>
                      </View>
                      {/* Rate-limited chains self-heal — swapping RPC is the wrong
                          fix, so only genuinely-failed chains offer it. */}
                      {!isRateLimited ? (
                        <Pressable onPress={() => setFixChainId(id)} hitSlop={8} style={styles.netFixBtn} accessibilityRole="button">
                          <Text style={styles.netFix}>{t('assets.rpcFix')}</Text>
                        </Pressable>
                      ) : null}
                    </View>
                  </View>
                );
              })}
              <Pressable onPress={onRetry} hitSlop={8} style={styles.retryRow} accessibilityRole="button">
                <RefreshCw size={14} color={color.accent.base} strokeWidth={2.5} />
                <Text style={styles.retryText}>{t('home.balanceDetailRetry')}</Text>
              </Pressable>
            </>
          ) : null}

          {/* Tokens without a price */}
          {hasTokens ? (
            <>
              <SectionLabel style={hasNetworks ? undefined : styles.firstLabel}>
                {t('home.balanceDetailUnpricedLabel')}
              </SectionLabel>
              <Text style={styles.note}>{t('home.balanceDetailUnpricedNote')}</Text>
              {unpricedTokens.map((tk, i) => (
                <View key={tokenId(tk)}>
                  {i > 0 ? <View style={styles.tokenSep} /> : null}
                  <TokenRow
                    symbol={tk.symbol}
                    chainLabel={chainName(tokenChainId(tk))}
                    logoUrls={tokenLogoURLs(tk)}
                    chain={tokenBadgeNetwork(tk)}
                    balance={hidden ? '••••' : formatTokenAmount(tokenBalanceDouble(tk), { compact: true })}
                    usdValue={t('home.balanceDetailNoPrice')}
                    onPress={() => onTokenPress(tk)}
                    index={i}
                  />
                </View>
              ))}
            </>
          ) : null}
        </ScrollView>
      </View>
      )}
    </AppModal>
  );
}

const styles = createStyles(() => ({
  sheet: { flex: 1, backgroundColor: color.bg.base },
  head: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    paddingHorizontal: space['2xl'], paddingVertical: space.md,
  },
  headSpacer: { width: 34 },
  headTitle: { flex: 1, textAlign: 'center', fontSize: text.xl, ...inter.bold, color: color.fg.base, paddingHorizontal: space.sm },
  closeBtn: { width: 34, height: 34, borderRadius: 17, alignItems: 'center', justifyContent: 'center' },
  body: { paddingHorizontal: space['2xl'], paddingBottom: space['4xl'] },

  // Trim the section label's default top gap for whichever section renders first
  // (the header already provides breathing room above it).
  firstLabel: { marginTop: space.lg },
  note: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, lineHeight: 18, marginTop: -space.sm, marginBottom: space.sm },

  // Network rows — open, hairline-separated (no card).
  netRow: { flexDirection: 'row', alignItems: 'center', gap: space.lg, paddingVertical: space.lg },
  netLogoFallback: { width: 36, height: 36, borderRadius: 18, backgroundColor: color.bg.sunken },
  netInfo: { flex: 1, gap: 3 },
  netName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  netStatus: { fontSize: text.sm, ...inter.medium },
  netStatusFailed: { color: color.warning.base },
  netStatusRetrying: { color: color.fg.muted },
  netFixBtn: { paddingVertical: space.xs, paddingHorizontal: space.sm },
  netFix: { fontSize: text.sm, ...inter.semibold, color: color.accent.base },
  // Inset under the network name, past the 36px logo + gap (Apple-Wallet style).
  netSep: { height: 1, backgroundColor: color.border.base, marginLeft: 36 + space.lg },

  retryRow: { flexDirection: 'row', alignItems: 'center', alignSelf: 'flex-start', gap: space.sm, paddingVertical: space.md, marginTop: space.xs },
  retryText: { fontSize: text.sm, ...inter.semibold, color: color.accent.base },

  // Token rows reuse TokenRow; separator matches HoldingsList (past the 40px logo).
  tokenSep: { height: 1, backgroundColor: color.border.base, marginLeft: space.md + 40 + space.lg },

  empty: { fontSize: text.base, ...inter.regular, color: color.fg.subtle, textAlign: 'center', paddingVertical: space['4xl'] },
}));
