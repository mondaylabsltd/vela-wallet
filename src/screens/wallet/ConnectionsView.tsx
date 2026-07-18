/**
 * Connections tab — the single active dApp connection + its signing activity.
 *
 * Renders inline (never a pushed screen) so the user stays on the Connections
 * panel through the whole pairing flow. The signing-activity list also renders
 * in the disconnected branch (issue #88): browser/extension-signed dApp txs
 * leave conn.status as 'disconnected', so without it a just-signed tx would be
 * invisible with nothing to review.
 */
import { useRouter } from 'expo-router';
import { ArrowRight, ChevronRight, History, Plug, RefreshCw, Trash2 } from 'lucide-react-native';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Pressable, Text, TextInput, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import Animated, { Easing, useAnimatedStyle, useSharedValue, withRepeat, withTiming } from 'react-native-reanimated';

import { ConnectionFlowStates } from '@/components/ConnectionFlowStates';
import { BrowserHistorySheet } from '@/components/ui/BrowserHistorySheet';
import { VelaCard } from '@/components/ui/VelaCard';
import { color } from '@/constants/theme';
import { type ConnectionStatus } from '@/models/dapp-connection';
import { relativeTime, type ConnectionEvent } from '@/services/activity';
import { getBrowserHistory } from '@/services/browser-history';
import { hapticLight } from '@/services/platform';
import { type LocalTransaction } from '@/services/storage';

import { styles } from './HomeScreen.styles';

// "立即重连" — the manual reconnect tap. SDK reconnect is fire-and-forget with no
// status of its own, so the button owns its feedback: a haptic + pressed state on
// tap (you felt it register), a continuously spinning icon (work is happening),
// and a brief label flip to "重新连接中…" right after the press to acknowledge it.
function ReconnectButton({ onReconnect }: { onReconnect: () => void }) {
  const { t } = useTranslation();
  const spin = useSharedValue(0);
  const [tapped, setTapped] = useState(false);
  const tapTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    spin.value = withRepeat(withTiming(1, { duration: 900, easing: Easing.linear }), -1, false);
    return () => { if (tapTimer.current) clearTimeout(tapTimer.current); };
  }, [spin]);

  const spinStyle = useAnimatedStyle(() => ({ transform: [{ rotate: `${spin.value * 360}deg` }] }));

  const press = () => {
    hapticLight();
    setTapped(true);
    if (tapTimer.current) clearTimeout(tapTimer.current);
    tapTimer.current = setTimeout(() => setTapped(false), 1400);
    onReconnect();
  };

  return (
    <Pressable style={({ pressed }) => [styles.reconnectBtn, pressed && styles.reconnectBtnPressed]} onPress={press}>
      <Animated.View style={spinStyle}>
        <RefreshCw size={16} color={color.fg.inverse} strokeWidth={2.4} />
      </Animated.View>
      <Text style={styles.reconnectText}>
        {tapped ? t('connect.list.reconnecting') : t('home.connReconnectBtn')}
      </Text>
    </Pressable>
  );
}

export function ConnectionsView({
  status, reconnectStuck, dappName, dappUrl, events, onDisconnect, onReconnect, onConnect, onPasteConnect, onOpenEvent, onClearEvents, onDeleteEvent,
}: {
  status: ConnectionStatus;
  reconnectStuck: boolean;
  dappName: string | null;
  dappUrl: string | null;
  events: ConnectionEvent[];
  onDisconnect: () => void;
  onReconnect: () => void;
  onConnect: () => void;
  onPasteConnect: (uri: string) => void;
  onOpenEvent: (tx: LocalTransaction) => void;
  onClearEvents: () => void;
  onDeleteEvent: (id: string) => void;
}) {
  const { t } = useTranslation();
  const router = useRouter();
  const [linkInput, setLinkInput] = useState('');
  const submitPaste = () => {
    if (!linkInput.trim()) return;
    onPasteConnect(linkInput);
    setLinkInput('');
  };

  // Recently-opened dApps — hidden behind the clock icon (the icon only appears once
  // there's history, so a fresh install stays clean).
  const [showHistory, setShowHistory] = useState(false);
  const [historyCount, setHistoryCount] = useState(0);
  const refreshHistoryCount = useCallback(() => {
    void getBrowserHistory().then((h) => setHistoryCount(h.length));
  }, []);
  useEffect(() => { refreshHistoryCount(); }, [refreshHistoryCount]);

  // The persisted signing-activity list. Rendered in the connected branch AND —
  // crucially (issue #88) — in the disconnected branch, because dApp transactions
  // signed through the in-app browser / extension leave conn.status as
  // 'disconnected', so without this a just-signed tx would be invisible with
  // nothing to review. In `historyMode` (no live session) it renders only when
  // there are events, so a fresh install's empty state stays clean; the delete/
  // clear handlers act on stored ids and are safe regardless of status.
  const renderEvents = (historyMode = false) => {
    if (historyMode && events.length === 0) return null;
    return (
      <View style={historyMode ? styles.connHistorySection : undefined}>
        <View style={styles.connEventsHeadRow}>
          <Text style={styles.connEventsHead}>{t('home.connEventsHead', { count: events.length })}</Text>
          {events.length > 0 && (
            <Pressable style={styles.connClearBtn} onPress={onClearEvents} hitSlop={8}>
              <Trash2 size={13} color={color.fg.subtle} strokeWidth={2} />
              <Text style={styles.connClearText}>{t('home.connClear')}</Text>
            </Pressable>
          )}
        </View>
        {events.length === 0 ? (
          <Text style={styles.connNoEvents}>{t('home.connNoEvents')}</Text>
        ) : (
          events.map((e) => (
            <Swipeable
              key={e.id}
              overshootRight={false}
              renderRightActions={() => (
                <Pressable style={styles.eventDelete} onPress={() => onDeleteEvent(e.id)}>
                  <Trash2 size={18} color={color.fg.inverse} strokeWidth={2.2} />
                  <Text style={styles.eventDeleteText}>{t('home.connDelete')}</Text>
                </Pressable>
              )}
            >
              <Pressable style={styles.eventRow} onPress={() => onOpenEvent(e.tx)}>
                <View style={styles.eventInfo}>
                  <Text style={styles.eventLabel} numberOfLines={1}>{e.label}</Text>
                  <Text style={styles.eventSub} numberOfLines={1}>{e.subtitle}</Text>
                </View>
                {e.status !== 'confirmed' && (
                  <View style={[styles.eventPill, e.status === 'failed' ? styles.eventPillFailed : styles.eventPillPending]}>
                    <Text style={[styles.eventPillText, e.status === 'failed' ? styles.eventPillTextFailed : styles.eventPillTextPending]}>
                      {t(e.status === 'failed' ? 'home.connFailed' : 'home.connPending')}
                    </Text>
                  </View>
                )}
                <Text style={styles.eventTime}>{relativeTime(e.timestamp)}</Text>
                <ChevronRight size={16} color={color.fg.subtle} strokeWidth={2} />
              </Pressable>
            </Swipeable>
          ))
        )}
      </View>
    );
  };

  // Pairing in progress (fingerprint / waiting) or failed — shown inline so the
  // user never leaves the Connections panel while connecting.
  if (status === 'connecting' || status === 'error') {
    return <ConnectionFlowStates onScanAgain={onConnect} />;
  }

  if (status === 'disconnected') {
    return (
      <View>
      <View style={styles.connEmpty}>
        <View style={styles.connEmptyIcon}><Plug size={26} color={color.fg.subtle} strokeWidth={2} /></View>
        <Text style={styles.connEmptyTitle}>{t('home.connEmptyTitle')}</Text>
        {/* One line covers it all — scan (bottom FAB) or paste/type below, for a
            dApp or any site. Keeps the empty state calm instead of stacked text. */}
        <Text style={styles.connEmptySub}>{t('home.connEmptySub')}</Text>

        <View style={[styles.connPasteRow, styles.connPasteRowSpaced]}>
          <TextInput
            style={styles.connPasteInput}
            value={linkInput}
            onChangeText={setLinkInput}
            placeholder={t('connect.list.pastePlaceholder')}
            placeholderTextColor={color.fg.subtle}
            autoCapitalize="none"
            autoCorrect={false}
            // Multiline so a long walletpair link / URL wraps and stays fully visible
            // instead of scrolling out of a cramped one-line field. blurOnSubmit keeps
            // the return key a submit (not a newline) — the ArrowRight button is the
            // primary submit either way.
            multiline
            numberOfLines={3}
            textAlignVertical="top"
            blurOnSubmit
            returnKeyType="go"
            onSubmitEditing={submitPaste}
          />
          <Pressable
            style={[styles.connPasteBtn, !linkInput.trim() && styles.connPasteBtnDisabled]}
            onPress={submitPaste}
            disabled={!linkInput.trim()}
          >
            <ArrowRight size={18} color={!linkInput.trim() ? color.fg.subtle : color.fg.inverse} strokeWidth={2.5} />
          </Pressable>
        </View>

        {/* Recently-opened dApps — one tap to reveal, only shown once there's history. */}
        {historyCount > 0 ? (
          <Pressable
            style={styles.connHistoryBtn}
            onPress={() => setShowHistory(true)}
            accessibilityRole="button"
            accessibilityLabel={t('connect.browser.historyTitle')}
          >
            <History size={15} color={color.fg.muted} strokeWidth={2} />
            <Text style={styles.connHistoryText}>{t('connect.browser.historyOpen')}</Text>
          </Pressable>
        ) : null}

        <BrowserHistorySheet
          visible={showHistory}
          onClose={() => { setShowHistory(false); refreshHistoryCount(); }}
          onOpen={(url) => { setShowHistory(false); router.push({ pathname: '/browser', params: { url } }); }}
        />
      </View>
      {/* Signing history stays reviewable even with no live session — a browser/
          extension-signed dApp tx lands here (conn.status is 'disconnected'). */}
      {renderEvents(true)}
      </View>
    );
  }

  // Connected / reconnecting — active session + its signing activity.
  const reconnecting = status === 'reconnecting';
  return (
    <View>
      <VelaCard elevated style={styles.connCard}>
        <View style={styles.connTop}>
          <View style={styles.connDapp}><Text style={styles.connDappText}>{(dappName?.[0] ?? '?').toUpperCase()}</Text></View>
          <View style={styles.connInfo}>
            <Text style={styles.connName} numberOfLines={1}>{dappName ?? t('home.connDefaultName')}</Text>
            {dappUrl ? <Text style={styles.connUrl} numberOfLines={1}>{dappUrl}</Text> : null}
          </View>
          <View style={styles.connStatus}>
            <View style={[styles.connDot, reconnecting && styles.connDotReconnecting]} />
            <Text style={[styles.connStatusText, reconnecting && styles.connStatusTextReconnecting]}>
              {reconnecting ? t('connect.list.reconnecting') : t('home.connActive')}
            </Text>
          </View>
        </View>
        <Text style={[styles.connNote, reconnectStuck && styles.connNoteWarn]}>
          {reconnectStuck ? t('home.connReconnectStuck') : t('home.connNote')}
        </Text>
        {reconnecting && <ReconnectButton onReconnect={onReconnect} />}
        <Pressable style={styles.disconnectBtn} onPress={onDisconnect}>
          <Text style={styles.disconnectText}>{t('home.connDisconnect')}</Text>
        </Pressable>
      </VelaCard>

      {renderEvents()}
    </View>
  );
}
