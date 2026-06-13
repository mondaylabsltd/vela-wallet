/**
 * Unified dApp Connect screen.
 *
 * Shared UI for account switching, signing requests, approve/reject.
 * Connection method differs by platform:
 *   - iOS/Android: BLE (Bluetooth peripheral)
 *   - Web: WebSocket to local dApp Browser
 */
import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import {
  View, Text, ScrollView, Pressable,
  Platform,
} from 'react-native';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withRepeat,
  withSequence,
  withTiming,
} from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { showAlert, hapticSuccess } from '@/services/platform';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaButton } from '@/components/ui/VelaButton';
import { AppModal } from '@/components/ui/AppModal';
import { color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { shortAddr, type BLEIncomingRequest } from '@/models/types';
import { PasskeyErrorCode } from '@/modules/passkey';
import { handleDAppRequest, isSigningMethod, handleReadOnlyRPC, INSTANT_READONLY_METHODS, assertChainSupported } from '@/hooks/use-dapp-signing';
import { gateReadOnly, readOnlyKey } from '@/services/readonly-rpc-gate';
import {
  Bluetooth, Wifi, ChevronRight, Check, X,
  Radio, Unplug, Shield, AlertTriangle, Download,
  Send, FileSignature, FileText,
} from 'lucide-react-native';

// BLE module — only imported on native
const BLE = Platform.OS !== 'web' ? require('@/modules/ble') : null;

type ConnectState = 'idle' | 'connecting' | 'advertising' | 'connected' | 'not-installed';

// Pulsing dot for BLE advertising state
function PulsingBluetooth() {
  const opacity = useSharedValue(1);
  useEffect(() => {
    opacity.value = withRepeat(
      withSequence(
        withTiming(0.3, { duration: 800 }),
        withTiming(1, { duration: 800 }),
      ),
      -1,
      false,
    );
  }, [opacity]);
  const animatedStyle = useAnimatedStyle(() => ({ opacity: opacity.value }));
  return (
    <Animated.View style={animatedStyle}>
      <Bluetooth size={18} color={color.info.base} strokeWidth={2.5} />
    </Animated.View>
  );
}

// ---------------------------------------------------------------------------
// Main screen
// ---------------------------------------------------------------------------

export default function DAppScreen() {
  const { t } = useTranslation();
  const { state, dispatch, activeAccount } = useWallet();
  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';

  const [connectState, setConnectState] = useState<ConnectState>('idle');
  const [peerName, setPeerName] = useState('');
  const [incomingRequest, setIncomingRequest] = useState<BLEIncomingRequest | null>(null);
  const [isSigning, setIsSigning] = useState(false);
  const [signError, setSignError] = useState<string | null>(null);
  const [showAccountPicker, setShowAccountPicker] = useState(false);
  const [currentChainId, setCurrentChainId] = useState(137);

  // Refs for stable access in callbacks
  const addressRef = useRef(address);
  const chainIdRef = useRef(currentChainId);
  const accountNameRef = useRef(accountName);
  const accountsRef = useRef(state.accounts);
  useEffect(() => { addressRef.current = address; }, [address]);
  useEffect(() => { chainIdRef.current = currentChainId; }, [currentChainId]);
  useEffect(() => { accountNameRef.current = accountName; }, [accountName]);
  useEffect(() => { accountsRef.current = state.accounts; }, [state.accounts]);

  // --- Transport-specific refs ---
  const wsRef = useRef<WebSocket | null>(null);
  const bleUnsubsRef = useRef<(() => void)[]>([]);

  // --- Send response (works for both transports) ---
  const sendResponse = useCallback((id: string, result?: any, error?: { code: number; message: string }) => {
    if (Platform.OS === 'web') {
      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) return;
      const msg: any = { id };
      if (error) msg.error = error; else msg.result = result ?? null;
      ws.send(JSON.stringify(msg));
    } else {
      if (error) BLE.sendResponse(id, undefined, error).catch(() => {});
      else BLE.sendResponse(id, result).catch(() => {});
    }
  }, []);

  // --- Handle incoming request (shared) ---
  const handleIncoming = useCallback((id: string, method: string, params: any[], origin: string) => {
    const addr = addressRef.current;
    const cid = chainIdRef.current;

    if (isSigningMethod(method)) {
      setIncomingRequest({ id, method, params, origin });
      return;
    }

    if (method === 'wallet_switchEthereumChain') {
      const cp = params?.[0] as { chainId?: string } | undefined;
      const nc = cp?.chainId
        ? (cp.chainId.startsWith('0x') ? parseInt(cp.chainId, 16) : parseInt(cp.chainId, 10))
        : NaN;
      if (isNaN(nc)) {
        // Missing/malformed chainId — don't report a phantom success.
        sendResponse(id, undefined, { code: -32602, message: 'Invalid params: missing chainId' });
        return;
      }
      try {
        assertChainSupported(nc);
      } catch (err: any) {
        // Wallet doesn't support this chain — reject (EIP-3326 / 4902)
        // instead of silently switching to an unknown network.
        sendResponse(id, undefined, {
          code: err.code ?? 4902,
          message: err.message ?? `Unsupported chain: ${nc}`,
        });
        return;
      }
      chainIdRef.current = nc;
      setCurrentChainId(nc);
      sendResponse(id, null);
      return;
    }

    // Network-bound reads go through the dedupe + concurrency gate so a flood
    // can't starve the signing path; instant local methods bypass it.
    const dispatch = INSTANT_READONLY_METHODS.has(method)
      ? handleReadOnlyRPC(method, params, addr, cid)
      : gateReadOnly(readOnlyKey(cid, addr, method, params), () => handleReadOnlyRPC(method, params, addr, cid));
    dispatch.then(res => {
      if (res.handled) sendResponse(id, res.result);
      else sendResponse(id, undefined, { code: -32603, message: `RPC failed: ${method}` });
    }).catch((err: any) => {
      // Gate overflow (too many concurrent reads) — answer with a retryable error.
      sendResponse(id, undefined, { code: err?.code ?? -32603, message: err?.message ?? `RPC failed: ${method}` });
    });
  }, [sendResponse]);

  // --- Approve / Reject ---
  const approveRequest = useCallback(async (request: BLEIncomingRequest) => {
    if (!activeAccount) return;
    setIsSigning(true);
    setSignError(null);
    try {
      const result = await handleDAppRequest(request, activeAccount, activeAccount.address, chainIdRef.current);
      sendResponse(request.id, result);
      setIncomingRequest(null);
    } catch (err: any) {
      if (err?.code === PasskeyErrorCode.CANCELLED) { setIsSigning(false); return; }
      setSignError(err.message ?? 'Signing failed');
      sendResponse(request.id, undefined, { code: -32603, message: err.message });
      setIncomingRequest(null);
    } finally {
      setIsSigning(false);
    }
  }, [activeAccount, sendResponse]);

  const rejectRequest = useCallback((request: BLEIncomingRequest) => {
    sendResponse(request.id, undefined, { code: 4001, message: 'User rejected' });
    setIncomingRequest(null);
  }, [sendResponse]);

  // --- Push wallet info ---
  const pushWalletInfo = useCallback(() => {
    const info = {
      address: addressRef.current,
      chainId: chainIdRef.current,
      name: accountNameRef.current,
      accounts: accountsRef.current.map(a => ({ name: a.name, address: a.address })),
    };
    if (Platform.OS === 'web') {
      const ws = wsRef.current;
      if (ws?.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: 'wallet_info', ...info }));
    } else {
      BLE?.updateWalletInfo({ walletAddress: info.address, accountName: info.name, chainId: info.chainId, accounts: info.accounts }).catch(() => {});
    }
  }, []);

  const wasConnected = useRef(false);
  useEffect(() => {
    if (connectState === 'connected') {
      if (wasConnected.current) pushWalletInfo();
      wasConnected.current = true;
    } else {
      wasConnected.current = false;
    }
  }, [address, accountName, currentChainId, connectState, pushWalletInfo]);

  // =========================================================================
  // Web: WebSocket connection
  // =========================================================================
  const connectWS = useCallback(() => {
    if (wsRef.current) { wsRef.current.close(); wsRef.current = null; }
    setConnectState('connecting');
    let didConnect = false;
    const ws = new WebSocket('ws://localhost:9710');
    ws.onopen = () => {
      didConnect = true;
      setConnectState('connected');
      setPeerName('dApp Browser');
      ws.send(JSON.stringify({
        type: 'wallet_info',
        address: addressRef.current, chainId: chainIdRef.current,
        name: accountNameRef.current,
        accounts: accountsRef.current.map(a => ({ name: a.name, address: a.address })),
      }));
    };
    ws.onmessage = (e) => {
      try {
        const msg = JSON.parse(typeof e.data === 'string' ? e.data : '');
        if (msg.id && msg.method) handleIncoming(msg.id, msg.method, msg.params ?? [], msg.origin ?? '');
      } catch {}
    };
    ws.onclose = () => { wsRef.current = null; setIncomingRequest(null); if (didConnect) setConnectState('idle'); };
    ws.onerror = () => { if (!didConnect) { wsRef.current = null; setConnectState('not-installed'); } };
    wsRef.current = ws;
  }, [handleIncoming]);

  // =========================================================================
  // Native: BLE connection
  // =========================================================================
  const [bleAvailable, setBleAvailable] = useState<boolean | null>(null);

  useEffect(() => {
    if (Platform.OS === 'web') return;
    BLE.isSupported().then(setBleAvailable).catch(() => setBleAvailable(false));
  }, []);

  useEffect(() => {
    if (Platform.OS === 'web' || !bleAvailable) return;
    const unsubs: (() => void)[] = [];
    try {
      unsubs.push(BLE.addListener('centralConnected', () => { setConnectState('connected'); setPeerName('Vela Connect'); }));
      unsubs.push(BLE.addListener('centralDisconnected', () => { setConnectState('advertising'); setIncomingRequest(null); }));
      unsubs.push(BLE.addListener('requestReceived', (data: any) => {
        handleIncoming(data.id, data.method, data.params ?? [], data.origin ?? '');
      }));
      unsubs.push(BLE.addListener('advertisingStopped', () => { setConnectState('idle'); setIncomingRequest(null); }));
    } catch {}
    bleUnsubsRef.current = unsubs;
    return () => unsubs.forEach(fn => fn());
  }, [bleAvailable, handleIncoming]);

  const startBLE = useCallback(async () => {
    const granted = await BLE.requestPermissions();
    if (!granted) { showAlert(t('connect.dapp.blePermTitle'), t('connect.dapp.blePermBody')); return; }
    await BLE.startAdvertising({
      walletAddress: address, accountName, chainId: currentChainId,
      accounts: state.accounts.map(a => ({ name: a.name, address: a.address })),
    });
    setConnectState('advertising');
  }, [address, accountName, currentChainId, state.accounts, t]);

  const stopBLE = useCallback(async () => {
    try { await BLE.stopAdvertising(); } catch {}
    setConnectState('idle');
    setIncomingRequest(null);
  }, []);

  const disconnect = useCallback(() => {
    if (Platform.OS === 'web') { wsRef.current?.close(); wsRef.current = null; }
    else stopBLE();
    setConnectState('idle');
    setIncomingRequest(null);
  }, [stopBLE]);

  useEffect(() => () => { wsRef.current?.close(); }, []);

  // =========================================================================
  // Render
  // =========================================================================

  if (!state.hasWallet) {
    return (
      <ScreenContainer>
        <View style={styles.centered}>
          <Shield size={32} color={color.fg.subtle} />
          <Text style={styles.emptyText}>{t('connect.dapp.noWallet')}</Text>
        </View>
      </ScreenContainer>
    );
  }

  const isWeb = Platform.OS === 'web';
  const isNative = !isWeb;
  const isAdvertising = connectState === 'advertising';
  const ConnectIcon = isWeb ? Wifi : Bluetooth;

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.scrollContent}>
        <Animated.View entering={fadeIn(0, 300)}>
          <Text style={styles.pageTitle}>{t('connect.dapp.pageTitle')}</Text>
        </Animated.View>

        {/* Wallet card */}
        <Animated.View entering={fadeInDown(50, 300)}>
          <Pressable onPress={() => setShowAccountPicker(true)}>
            <VelaCard style={styles.walletCard}>
              <View style={styles.walletRow}>
                <View style={styles.walletAvatar}>
                  <Text style={styles.walletAvatarText}>
                    {(accountName[0] ?? 'V').toUpperCase()}
                  </Text>
                </View>
                <View style={styles.walletInfo}>
                  <Text style={styles.walletName}>{accountName}</Text>
                  <Text style={styles.walletAddr}>{shortAddress(address)}</Text>
                </View>
                <ChevronRight size={16} color={color.fg.subtle} />
              </View>
            </VelaCard>
          </Pressable>
        </Animated.View>

        {/* Connection section */}
        {connectState === 'idle' && (
          <Animated.View style={styles.section} entering={fadeInDown(100, 300)}>
            <View style={styles.connectPrompt}>
              <View style={styles.connectIconWrap}>
                <ConnectIcon size={24} color={color.info.base} />
              </View>
              <Text style={styles.sectionTitle}>
                {isWeb ? t('connect.dapp.connectWebTitle') : t('connect.dapp.connectBleTitle')}
              </Text>
              <Text style={styles.hint}>
                {isWeb
                  ? t('connect.dapp.connectWebHint')
                  : t('connect.dapp.connectBleHint')}
              </Text>
            </View>
            <VelaButton title={t('connect.dapp.connect')} onPress={isWeb ? connectWS : startBLE} />
          </Animated.View>
        )}

        {connectState === 'connecting' && (
          <View style={styles.centered}>
            <Radio size={24} color={color.info.base} />
            <Text style={styles.statusText}>{t('connect.dapp.connecting')}</Text>
          </View>
        )}

        {isAdvertising && isNative && (
          <Animated.View style={styles.section} entering={fadeIn(0, 300)}>
            <VelaCard style={styles.statusCard}>
              <View style={styles.pulseRow}>
                <PulsingBluetooth />
                <Text style={styles.statusText}>{t('connect.dapp.waiting')}</Text>
              </View>
            </VelaCard>
            <VelaButton title={t('connect.dapp.stop')} onPress={stopBLE} variant="secondary" style={styles.sectionBtn} />
          </Animated.View>
        )}

        {connectState === 'not-installed' && isWeb && (
          <Animated.View entering={fadeInDown(0, 300)}>
            <VelaCard style={styles.notInstalledCard}>
              <AlertTriangle size={24} color={color.fg.muted} />
              <Text style={styles.notInstalledTitle}>{t('connect.dapp.notInstalledTitle')}</Text>
              <Text style={styles.hint}>{t('connect.dapp.notInstalledHint')}</Text>
              <VelaButton title={t('connect.dapp.download')} onPress={() => {
                if (Platform.OS === 'web') window.open('https://getvela.app/dpp-browser', '_blank');
              }} variant="accent" style={styles.sectionBtn} />
              <VelaButton title={t('connect.dapp.tryAgain')} onPress={connectWS} variant="secondary" style={styles.retryBtn} />
            </VelaCard>
          </Animated.View>
        )}

        {/* Connected state */}
        {connectState === 'connected' && !incomingRequest && (
          <Animated.View style={styles.section} entering={fadeIn(0, 300)}>
            <VelaCard style={styles.connectedCard}>
              <View style={styles.connectedRow}>
                <View style={styles.connectedDot} />
                <Text style={styles.connectedText}>{t('connect.dapp.connectedTo', { peerName })}</Text>
              </View>
              <Text style={styles.connectedHint}>
                {t('connect.dapp.connectedHint')}
              </Text>
            </VelaCard>
            <VelaButton title={t('connect.dapp.disconnectBtn')} onPress={disconnect} variant="secondary" style={styles.sectionBtn} />
          </Animated.View>
        )}

        {/* Signing request */}
        {incomingRequest && (
          <Animated.View entering={fadeInDown(0, 300)}>
            <VelaCard elevated style={styles.requestCard}>
              <View style={styles.requestHeader}>
                {methodIcon(incomingRequest.method)}
                <View style={styles.requestHeaderText}>
                  <Text style={styles.requestMethod}>{t(methodLabelKey(incomingRequest.method) as any) || incomingRequest.method}</Text>
                  <Text style={styles.requestOrigin}>{incomingRequest.origin || peerName}</Text>
                </View>
              </View>

              {/* Structured request details */}
              <View style={styles.txDetails}>
                <Text style={styles.requestDescription}>{t(methodDescKey(incomingRequest.method) as any)}</Text>
                {incomingRequest.method === 'personal_sign' && incomingRequest.params?.[0] && (
                  <View style={styles.messagePreview}>
                    <Text style={styles.messagePreviewLabel}>MESSAGE</Text>
                    <Text style={styles.messagePreviewText} numberOfLines={6}>
                      {decodePersonalMessage(incomingRequest.params[0])}
                    </Text>
                  </View>
                )}
                {incomingRequest.method === 'eth_sendTransaction' && incomingRequest.params?.[0] && (
                  <>
                    <DetailRow label={t('connect.dapp.detailTo')} value={shortAddr(incomingRequest.params[0].to ?? '')} />
                    <DetailRow label={t('connect.dapp.detailValue')} value={formatTxValue(incomingRequest.params[0].value)} />
                    {incomingRequest.params[0].data && incomingRequest.params[0].data !== '0x' && (
                      <DetailRow label={t('connect.dapp.detailData')} value={`${incomingRequest.params[0].data.length / 2 - 1} bytes`} />
                    )}
                  </>
                )}
                {incomingRequest.method.includes('signTypedData') && incomingRequest.params && (
                  <View style={styles.messagePreview}>
                    <Text style={styles.messagePreviewLabel}>TYPED DATA</Text>
                    <Text style={styles.messagePreviewText} numberOfLines={4}>
                      {parseTypedDataSummary(incomingRequest.params, t('connect.dapp.structuredData'))}
                    </Text>
                  </View>
                )}
              </View>

              {signError && (
                <View style={styles.errorRow}>
                  <AlertTriangle size={14} color={color.accent.base} />
                  <Text style={styles.errorText}>{signError}</Text>
                </View>
              )}

              <View style={styles.buttonRow}>
                <VelaButton
                  title={isSigning ? t('connect.dapp.signing') : t('connect.dapp.approve')}
                  onPress={() => approveRequest(incomingRequest)}
                  variant="accent" loading={isSigning} style={styles.buttonFlex}
                />
                <VelaButton
                  title={t('connect.dapp.reject')}
                  onPress={() => rejectRequest(incomingRequest)}
                  variant="secondary" disabled={isSigning} style={styles.buttonFlex}
                />
              </View>
            </VelaCard>
          </Animated.View>
        )}
      </ScrollView>

      {/* Account picker modal */}
      <AppModal visible={showAccountPicker} onClose={() => setShowAccountPicker(false)}>
        <View style={styles.modalContainer}>
          <View style={styles.modalHeader}>
            <Text style={styles.modalTitle}>{t('connect.dapp.accounts')}</Text>
            <Pressable onPress={() => setShowAccountPicker(false)} hitSlop={8}>
              <X size={22} color={color.fg.base} strokeWidth={2} />
            </Pressable>
          </View>
          <ScrollView style={styles.modalScroll}>
            {state.accounts.map((account, index) => {
              const isActive = account.id === activeAccount?.id;
              return (
                <Pressable
                  key={account.id}
                  style={[styles.accountItem, isActive && styles.accountItemActive]}
                  onPress={() => { dispatch({ type: 'SWITCH_ACCOUNT', index }); hapticSuccess(); setShowAccountPicker(false); }}
                >
                  <View style={styles.accountItemInfo}>
                    <Text style={styles.accountItemName}>{account.name}</Text>
                    <Text style={styles.accountItemAddr}>{shortAddress(account.address)}</Text>
                  </View>
                  {isActive && <Check size={18} color={color.accent.base} strokeWidth={2.5} />}
                </Pressable>
              );
            })}
          </ScrollView>
        </View>
      </AppModal>
    </ScreenContainer>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function DetailRow({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.detailRow}>
      <Text style={styles.detailLabel}>{label}</Text>
      <Text style={styles.detailValue} numberOfLines={1}>{value}</Text>
    </View>
  );
}

function decodePersonalMessage(hexMsg: string): string {
  try {
    const clean = hexMsg.startsWith('0x') ? hexMsg.slice(2) : hexMsg;
    const bytes = new Uint8Array(clean.match(/.{1,2}/g)!.map(b => parseInt(b, 16)));
    const decoded = new TextDecoder().decode(bytes);
    // Check if it's printable text
    if (/^[\x20-\x7E\n\r\t]+$/.test(decoded)) return decoded;
    return `0x${clean.slice(0, 64)}${clean.length > 64 ? '...' : ''}`;
  } catch {
    return hexMsg.slice(0, 66) + (hexMsg.length > 66 ? '...' : '');
  }
}

function formatTxValue(value?: string): string {
  if (!value || value === '0x0' || value === '0x') return '0 ETH';
  try {
    const clean = value.startsWith('0x') ? value.slice(2) : value;
    const wei = BigInt('0x' + clean);
    const eth = Number(wei) / 1e18;
    if (eth === 0) return '0 ETH';
    if (eth < 0.0001) return '< 0.0001 ETH';
    return eth.toFixed(4).replace(/\.?0+$/, '') + ' ETH';
  } catch {
    return value;
  }
}

function parseTypedDataSummary(params: any[], fallback: string): string {
  try {
    // EIP-712: params[1] is the typed data JSON string
    const data = typeof params[1] === 'string' ? JSON.parse(params[1]) : params[1];
    if (data?.primaryType) {
      const msg = data.message;
      if (msg) {
        const fields = Object.entries(msg).slice(0, 3).map(([k, v]) => `${k}: ${String(v).slice(0, 40)}`).join('\n');
        return `${data.primaryType}\n${fields}`;
      }
      return data.primaryType;
    }
    return fallback;
  } catch {
    return fallback;
  }
}

function methodDescKey(m: string): string {
  if (m === 'eth_sendTransaction') return 'connect.dapp.sendTxDesc';
  if (m === 'personal_sign') return 'connect.dapp.signMsgDesc';
  if (m.includes('signTypedData')) return 'connect.dapp.signTypedDesc';
  return 'connect.dapp.signDesc';
}

function methodLabelKey(m: string): string {
  if (m === 'eth_sendTransaction') return 'connect.dapp.sendTx';
  if (m === 'personal_sign') return 'connect.dapp.signMsg';
  if (m.includes('signTypedData')) return 'connect.dapp.signTyped';
  return '';
}

function methodIcon(m: string): React.ReactNode {
  const size = 20;
  const strokeWidth = 2;
  if (m === 'eth_sendTransaction') return <Send size={size} color={color.accent.base} strokeWidth={strokeWidth} />;
  if (m === 'personal_sign') return <FileSignature size={size} color={color.info.base} strokeWidth={strokeWidth} />;
  if (m.includes('signTypedData')) return <FileText size={size} color={color.info.base} strokeWidth={strokeWidth} />;
  return <Shield size={size} color={color.fg.muted} strokeWidth={strokeWidth} />;
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = createStyles(() => ({
  scrollContent: { paddingBottom: space['5xl'] },
  pageTitle: {
    fontSize: text['2xl'],
    ...inter.bold,
    color: color.fg.base,
    marginTop: space.xl,
    marginBottom: space['2xl'],
  },

  // Wallet card
  walletCard: { padding: space['2xl'], marginBottom: space['2xl'] },
  walletRow: { flexDirection: 'row', alignItems: 'center', gap: space.lg },
  walletAvatar: {
    width: 40, height: 40, borderRadius: 20,
    backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  walletAvatarText: { fontSize: text.lg, ...inter.bold, color: color.accent.base },
  walletInfo: { flex: 1, gap: 2 },
  walletName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  walletAddr: { fontSize: text.sm, fontWeight: '500', fontFamily: font.mono, color: color.fg.subtle },

  // Sections
  section: { marginBottom: space.xl },
  sectionTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base, marginBottom: space.sm },
  hint: { fontSize: text.base, ...inter.regular, color: color.fg.muted, lineHeight: 20, marginBottom: space.xl },
  sectionBtn: { marginTop: space.lg },
  retryBtn: { marginTop: space.md },
  centered: { alignItems: 'center', paddingVertical: space['5xl'], gap: space.lg },
  emptyText: { fontSize: text.lg, ...inter.regular, color: color.fg.muted },
  statusText: { fontSize: text.lg, ...inter.semibold, color: color.info.base },
  statusCard: { padding: space['2xl'] },
  pulseRow: { flexDirection: 'row', alignItems: 'center', gap: space.lg },

  // Connect prompt
  connectPrompt: { alignItems: 'center', paddingVertical: space['2xl'] },
  connectIconWrap: {
    width: 56, height: 56, borderRadius: 28,
    backgroundColor: color.info.soft,
    alignItems: 'center', justifyContent: 'center',
    marginBottom: space.xl,
  },

  // Connected
  connectedCard: { padding: space['2xl'], gap: space.md },
  connectedRow: { flexDirection: 'row', alignItems: 'center', gap: space.lg },
  connectedDot: { width: 10, height: 10, borderRadius: 5, backgroundColor: color.success.base },
  connectedText: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  connectedHint: { fontSize: text.base, ...inter.regular, color: color.fg.muted },

  // Not installed
  notInstalledCard: { padding: space['2xl'], alignItems: 'center', gap: space.md },
  notInstalledTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },

  // Request
  requestCard: { padding: space['2xl'], gap: space.xl },
  requestHeader: { flexDirection: 'row', alignItems: 'center', gap: space.lg },
  requestHeaderText: { flex: 1, gap: 2 },
  requestOrigin: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  requestMethod: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  txDetails: {
    gap: space.md, paddingVertical: space.lg,
    borderTopWidth: 1, borderBottomWidth: 1, borderColor: color.border.base,
  },
  requestDescription: { fontSize: text.base, ...inter.regular, color: color.fg.muted, lineHeight: 20, marginBottom: space.md },
  messagePreview: { backgroundColor: color.bg.sunken, borderRadius: radius.lg, padding: space.lg, gap: space.sm },
  messagePreviewLabel: { fontSize: text.xs, ...inter.semibold, color: color.fg.subtle, letterSpacing: 1, textTransform: 'uppercase' as const },
  messagePreviewText: { fontSize: text.sm, fontWeight: '500', fontFamily: font.mono, color: color.fg.base, lineHeight: 18 },
  detailRow: { flexDirection: 'row', justifyContent: 'space-between' },
  detailLabel: { fontSize: text.base, ...inter.regular, color: color.fg.muted },
  detailValue: { fontSize: text.base, fontWeight: '500', fontFamily: font.mono, color: color.fg.base, maxWidth: '60%' as any },
  errorRow: { flexDirection: 'row', alignItems: 'center', gap: space.md },
  errorText: { fontSize: text.base, ...inter.regular, color: color.accent.base, flex: 1 },
  buttonRow: { flexDirection: 'row', gap: space.lg },
  buttonFlex: { flex: 1 },

  // Modal
  modalContainer: { flex: 1, padding: space['3xl'] },
  modalHeader: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: space['2xl'] },
  modalTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  modalClose: { fontSize: text.lg, ...inter.semibold, color: color.accent.base },
  modalScroll: { flex: 1 },
  accountItem: {
    flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center',
    padding: space.xl, borderRadius: radius.xl, marginBottom: space.md,
    borderWidth: 1, borderColor: color.border.base,
    ...shadow.sm,
  },
  accountItemActive: { borderColor: color.accent.base, backgroundColor: color.accent.soft },
  accountItemInfo: { gap: 2 },
  accountItemName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  accountItemAddr: { fontSize: text.sm, fontWeight: '500', fontFamily: font.mono, color: color.fg.subtle },
}));
