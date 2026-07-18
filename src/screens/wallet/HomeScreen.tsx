/**
 * HomeScreen (layout A) — payment-first, activity-first single screen.
 *
 *   Header:   account selector · settings (gear)
 *   Balance:  "Total balance · CODE" label + the number (tap = hide/show)
 *   Content:  [ Activity | Assets | Connections ] toggle + Network filter
 *               · Activity    = value-transfer feed (received / sent)
 *               · Assets      = holdings list (HoldingsList → token detail)
 *               · Connections = single active dApp connection + its events
 *   Dock:     Receive · Scan · Send  (WaveDock, full-bleed)
 *
 * The hero is deliberately bare — the number is the only actor. Display
 * currency moved to Settings › Localization (N01 FR-1); balance privacy is the
 * number's own tap (persisted, masks the feed + holdings too, an EyeOff glyph
 * appears only in the masked state).
 *
 * This file is the view shell only. All state, effects, and handlers live in
 * `useHomeController`; the balance number, receipt toast, and connections panel
 * are their own components (BalanceDisplay / ReceiptToast / ConnectionsView).
 */
import { AlertTriangle, ChevronDown, ChevronRight, EyeOff, Inbox, Settings } from 'lucide-react-native';
import React from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { SafeAreaView } from 'react-native-safe-area-context';

import { QRScanner } from '@/components/QRScanner';
import { AccountSwitcherModal } from '@/components/ui/AccountSwitcherModal';
import { ActivityRow } from '@/components/ui/ActivityRow';
import { BalanceDetailSheet } from '@/components/ui/BalanceDetailSheet';
import { ConnectionEventDetailSheet } from '@/components/ui/ConnectionEventDetailSheet';
import { HoldingsList } from '@/components/ui/HoldingsList';
import { NetworkFilterButton, NetworkFilterSheet } from '@/components/ui/NetworkFilterSheet';
import { RpcFixModal, RpcTroubleBanner } from '@/components/ui/RpcTroubleBanner';
import { SegmentedToggle } from '@/components/ui/SegmentedToggle';
import { SigningReplaySheet } from '@/components/ui/SigningReplaySheet';
import { TransactionDetailSheet } from '@/components/ui/TransactionDetailSheet';
import { VelaRefresh } from '@/components/ui/VelaRefresh';
import { WaveDock } from '@/components/ui/WaveDock';
import { WalletAvatar } from '@/components/ui/WalletAvatar';

import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, space, text } from '@/constants/theme';
import { shortAddr, tokenLogoURLs } from '@/models/types';
import { shortAddress } from '@/models/wallet-state';

import { Balance, BalanceSkeleton } from './BalanceDisplay';
import { ConnectionsView } from './ConnectionsView';
import { ReceiptToast } from './ReceiptToast';
import { styles } from './HomeScreen.styles';
import { useHomeController, type FeedRow, type Tab } from './useHomeController';

export default function HomeScreen() {
  const c = useHomeController();
  const {
    t, router, conn, state, address, accountName, insets,
    tab, setTab, networks, selectedNetwork, selectedChainId, setSelectedChainId,
    showNetSheet, setShowNetSheet, connected, activity,
    dc, currency, hidden, toggleHidden, displayTotal, balancePartial, balanceUnknown,
    noticeAllowed, failedChainIds, rateLimitedChainIds, unpricedTokens,
    balanceScaleStyle, hasEntered,
    showBalanceDetail, setShowBalanceDetail, fixChainId, setFixChainId, setFailedChainIds,
    tokens, cachedTotal,
    activityFeed, aliasMap, newItemId, chainFor, openDetail,
    refreshing, onRefresh, refreshStatus, listContentStyle, loadData,
    receipt,
    connEvents, confirmDisconnect, onPasteConnect, clearConnEvents, deleteConnEvent, eventTx, setEventTx,
    showScanner, setShowScanner, onScan,
    openSwitcher, showSwitcher, setShowSwitcher, cachedBalances, switcherLoading,
    detailTx, detailBatch, detailAlias, setDetailTx, setDetailBatch,
  } = c;

  // --- renderers ---
  const renderHeader = () => (
    <Animated.View entering={hasEntered.current ? undefined : fadeInDown(60, 400)}>
      {/* Balance — the hero shows on every tab, Connections included: a constant
          anchor beats reclaiming its vertical room. */}
      <Animated.View style={balanceScaleStyle}>
        <View style={styles.balanceCard}>
          {/* The code in the label keeps the unit unambiguous ($ alone could be
              USD/CAD/AUD…) now that the currency control lives in Settings. */}
          <Text style={styles.balanceLabel}>{`${t('home.totalBalance')} · ${dc.code}`}</Text>
          {/* The number is the hero's only actor: tapping it toggles privacy
              mode (persisted). The EyeOff glyph appears only beside the masked
              value — chrome only when it has something to say. */}
          <Pressable
            style={styles.balanceTopRow}
            onPress={toggleHidden}
            disabled={balanceUnknown}
            hitSlop={8}
            accessibilityRole="button"
            accessibilityLabel={hidden ? t('home.a11yShowBalance') : dc.fmt(displayTotal)}
            accessibilityHint={hidden ? undefined : t('home.a11yHideBalance')}
          >
            {hidden ? (
              <View style={styles.balanceHiddenRow}>
                <View style={styles.balanceDots}>
                  {[0, 1, 2, 3, 4, 5].map((i) => (
                    <View key={i} style={styles.balanceDot} />
                  ))}
                </View>
                <EyeOff size={20} color={color.fg.subtle} strokeWidth={2} />
              </View>
            ) : balanceUnknown ? (
              <BalanceSkeleton />
            ) : (
              <Balance value={displayTotal * dc.rate} symbol={dc.symbol} code={dc.code} />
            )}
          </Pressable>
          {balancePartial && noticeAllowed && (
            // Tappable: the ChevronRight is the "there's more — see exactly what"
            // affordance. Opens a sheet enumerating the culprit networks + tokens.
            <Pressable
              style={({ pressed }) => [styles.balanceStaleRow, pressed && styles.balanceStalePressed]}
              onPress={() => setShowBalanceDetail(true)}
              hitSlop={8}
              accessibilityRole="button"
              accessibilityHint={t('home.balanceDetailViewHint')}
            >
              <AlertTriangle size={12} color={color.warning.base} strokeWidth={2.5} />
              {/* Failed chains are transient ("still updating" is honest — a retry
                  can fix it); a held token with no price source is not going to
                  resolve on its own, so promising an update would lie. */}
              <Text style={styles.balanceStaleText}>
                {t(failedChainIds.length > 0 ? 'home.balanceStale' : 'home.balanceUnpriced')}
              </Text>
              <ChevronRight size={14} color={color.warning.base} strokeWidth={2.5} />
            </Pressable>
          )}
        </View>
      </Animated.View>

      {/* RPC failure banner + fix flow. Rate-limited
          chains are excluded — that's transient and self-healing, so nagging the
          user to swap RPC would be wrong; their balance quietly stays on cache. */}
      <RpcTroubleBanner
        chainIds={failedChainIds.filter((id) => !rateLimitedChainIds.includes(id))}
        onFix={setFixChainId}
      />

      {/* Toggle + network filter */}
      <View style={styles.navRow}>
        <SegmentedToggle<Tab>
          options={[
            { key: 'activity', label: t('home.tabActivity') },
            { key: 'assets', label: t('home.tabAssets') },
            { key: 'connections', label: t('home.tabConnections'), badge: connected ? 1 : 0 },
          ]}
          value={tab}
          onChange={setTab}
        />
        <NetworkFilterButton
          networks={networks}
          selected={selectedNetwork}
          onPress={() => setShowNetSheet(true)}
          onClear={() => setSelectedChainId(null)}
        />
      </View>
    </Animated.View>
  );

  const renderActivityEmpty = () => (
    <View style={styles.empty}>
      <View style={styles.emptyIcon}>
        <Inbox size={28} color={color.fg.subtle} strokeWidth={2} />
      </View>
      <Text style={styles.emptyText}>
        {selectedChainId != null ? t('home.emptyNoActivityNetwork') : t('home.emptyNoActivity')}
      </Text>
      <Text style={styles.emptySub}>{t('home.emptySubtitle')}</Text>
    </View>
  );

  return (
    <View style={styles.root}>
      {/* Suppressed while balance privacy is on — an incoming toast would leak
          exactly the class of number the mask conceals. */}
      {receipt && !hidden && (
        <ReceiptToast amount={receipt.amount} token={receipt.token} top={insets.top + space.md} />
      )}
      <SafeAreaView style={styles.safe} edges={['top']}>
        {/* Header */}
        <Animated.View style={styles.header} entering={hasEntered.current ? undefined : fadeIn(0, 400)}>
          <Pressable
            style={styles.account}
            onPress={openSwitcher}
            accessibilityRole="button"
            accessibilityLabel={t('home.a11ySwitchAccount', { name: accountName })}
          >
            {/* Tapping the identicon itself enlarges it (handled inside
                WalletAvatar); the rest of this button opens the switcher. */}
            <WalletAvatar name={accountName} address={address} size={44} letterSize={text.lg} enlargeable />
            <View style={styles.accountInfo}>
              <View style={styles.accountNameRow}>
                <Text style={styles.accountName} numberOfLines={1}>{accountName}</Text>
                {state.accounts.length > 1 && (
                  <ChevronDown size={15} color={color.fg.subtle} strokeWidth={2.4} />
                )}
              </View>
              <Text style={styles.accountAddr} numberOfLines={1}>{shortAddr(address)}</Text>
            </View>
          </Pressable>
          <Pressable
            style={styles.iconBtn}
            onPress={() => router.navigate('/settings')}
            hitSlop={6}
            accessibilityRole="button"
            accessibilityLabel={t('home.a11yOpenSettings')}
          >
            <Settings size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </Animated.View>

        {/* Content — branded pull-to-refresh (gesture-driven, cross-platform) */}
        {tab === 'activity' ? (
          <VelaRefresh refreshing={refreshing} onRefresh={onRefresh} statusText={refreshStatus}>
            {(scrollProps) => (
              <Animated.FlatList
                {...scrollProps}
                data={activityFeed}
                keyExtractor={(row: FeedRow) => (row.kind === 'header' ? row.id : row.item.id)}
                ListHeaderComponent={renderHeader()}
                ListEmptyComponent={renderActivityEmpty()}
                renderItem={({ item: row, index }: { item: FeedRow; index: number }) => {
                  if (row.kind === 'header') {
                    return <Text style={styles.dayHeader}>{row.label}</Text>;
                  }
                  const item = row.item;
                  // Hairline only between consecutive item rows — never abutting a
                  // day header (the header's own spacing separates groups).
                  const prev = activityFeed[index - 1];
                  return (
                    <>
                      {prev && prev.kind === 'item' ? <View style={styles.sep} /> : null}
                      <ActivityRow
                        direction={item.direction}
                        title={t(item.direction === 'out' ? 'activity.sent' : 'activity.received')}
                        subtitle={
                          item.address
                            ? t(item.direction === 'out' ? 'activity.toAddr' : 'activity.fromAddr', {
                                // Prefer a resolved alias (ENS/.bnb/Vela/own-account), then the
                                // stored name, falling back to the short address. aliasMap is state,
                                // so the row re-renders to the name once it resolves.
                                addr: aliasMap.get(item.address.toLowerCase()) ?? item.alias ?? shortAddress(item.address),
                              })
                            : item.subtitle
                        }
                        amount={item.amount}
                        masked={hidden}
                        fiat={!hidden && item.usdValue > 0 ? dc.fmt(item.usdValue) : undefined}
                        chain={chainFor(item.chainId)}
                        index={index}
                        isNew={item.id === newItemId}
                        onPress={() => openDetail(item)}
                      />
                    </>
                  );
                }}
                contentContainerStyle={listContentStyle}
                showsVerticalScrollIndicator={false}
              />
            )}
          </VelaRefresh>
        ) : tab === 'assets' ? (
          // Keyed by address: an account switch resets the list's local state
          // (zero-balance superset, toggle, search) instead of leaking the
          // previous account's holdings while the new scan streams in.
          <HoldingsList
            key={address ?? 'none'}
            tokens={tokens}
            loading={tokens.length === 0 && (cachedTotal ?? 0) > 0}
            selectedChainId={selectedChainId}
            header={renderHeader()}
            refreshing={refreshing}
            onRefresh={onRefresh}
            refreshStatus={refreshStatus}
            contentContainerStyle={listContentStyle}
          />
        ) : (
          <VelaRefresh refreshing={refreshing} onRefresh={onRefresh} statusText={refreshStatus}>
            {(scrollProps) => (
              <Animated.ScrollView
                {...scrollProps}
                contentContainerStyle={listContentStyle}
                showsVerticalScrollIndicator={false}
              >
                {renderHeader()}
                <ConnectionsView
                  status={conn.status}
                  reconnectStuck={conn.reconnectStuck}
                  dappName={conn.dappInfo?.name ?? null}
                  dappUrl={conn.dappInfo?.url ?? null}
                  events={connEvents}
                  onDisconnect={confirmDisconnect}
                  onReconnect={conn.reconnect}
                  onConnect={() => setShowScanner(true)}
                  onPasteConnect={onPasteConnect}
                  onOpenEvent={setEventTx}
                  onClearEvents={clearConnEvents}
                  onDeleteEvent={deleteConnEvent}
                />
              </Animated.ScrollView>
            )}
          </VelaRefresh>
        )}
      </SafeAreaView>

      {/* Bottom dock (full-bleed) */}
      <WaveDock
        onReceive={() => router.push('/receive')}
        onScan={() => setShowScanner(true)}
        onSend={() => router.push('/send')}
      />

      {/* Network filter sheet */}
      <NetworkFilterSheet
        visible={showNetSheet}
        networks={networks}
        selectedChainId={selectedChainId}
        onSelect={setSelectedChainId}
        onClose={() => setShowNetSheet(false)}
        subtitleForChain={(n) => {
          const count = activity.filter((a) => a.chainId === n.chainId).length;
          return count > 0 ? `${count} event${count > 1 ? 's' : ''}` : undefined;
        }}
      />

      {/* Balance-detail sheet (opened by the tappable hero notice) + the single
          shared RPC-fix modal. The banner chips and the sheet's per-chain "Fix"
          rows both drive RpcFixModal, so there's one recovery form, not two. */}
      <BalanceDetailSheet
        visible={showBalanceDetail}
        onClose={() => setShowBalanceDetail(false)}
        failedChainIds={failedChainIds}
        rateLimitedChainIds={rateLimitedChainIds}
        unpricedTokens={unpricedTokens}
        onFixResolved={(chainId) => {
          setFailedChainIds((prev) => prev.filter((id) => id !== chainId));
          loadData();
        }}
        onRetry={() => loadData(true)}
        onTokenPress={(token) => {
          setShowBalanceDetail(false);
          router.push({
            pathname: '/token-detail',
            params: {
              symbol: token.symbol,
              name: token.name,
              network: token.network,
              balance: token.balance,
              decimals: String(token.decimals),
              logos: JSON.stringify(tokenLogoURLs(token)),
              tokenAddress: token.tokenAddress ?? '',
              priceUsd: String(token.priceUsd ?? 0),
              chainName: token.chainName,
            },
          });
        }}
      />
      <RpcFixModal
        chainId={fixChainId}
        onClose={() => setFixChainId(null)}
        onResolved={(chainId) => {
          setFailedChainIds((prev) => prev.filter((id) => id !== chainId));
          loadData();
        }}
      />

      {/* Transaction detail */}
      <TransactionDetailSheet
        visible={detailTx !== null || detailBatch !== null}
        tx={detailTx}
        batch={detailBatch}
        alias={detailAlias}
        rate={dc.rate}
        currency={currency}
        onResolved={() => loadData()}
        onClose={() => { setDetailTx(null); setDetailBatch(null); }}
      />

      {/* dApp signing-record detail. Records that captured their original request
          replay the FULL signing panel (read-only); older ones fall back to the
          metadata detail sheet. */}
      <SigningReplaySheet
        visible={eventTx !== null && !!eventTx?.signedRequest}
        tx={eventTx}
        onClose={() => setEventTx(null)}
      />
      <ConnectionEventDetailSheet
        visible={eventTx !== null && !eventTx?.signedRequest}
        tx={eventTx}
        onClose={() => setEventTx(null)}
      />

      {/* Account switcher (shared component) */}
      <AccountSwitcherModal
        visible={showSwitcher}
        onClose={() => setShowSwitcher(false)}
        title={t('home.switchAccountTitle')}
        formatSubtitle={(amount, count) => `${t('home.switcherAccountCount', { count })}${amount}`}
        balances={cachedBalances}
        loading={switcherLoading}
        showCreateActions
      />

      {showScanner && (
        <QRScanner visible={showScanner} onScan={onScan} onClose={() => setShowScanner(false)} />
      )}
    </View>
  );
}
