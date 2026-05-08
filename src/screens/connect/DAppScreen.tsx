/**
 * Unified dApp Connect screen.
 *
 * Shared UI for account switching, signing requests, approve/reject.
 * Connection method differs by platform:
 *   - iOS/Android: BLE (Bluetooth peripheral)
 *   - Web: WebSocket to local dApp Browser
 */
import React, { useState, useEffect, useCallback, useRef } from 'react';
import {
  View, Text, ScrollView, Pressable,
  Alert, Platform,
} from 'react-native';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withRepeat,
  withSequence,
  withTiming,
  FadeIn,
  FadeInDown,
} from 'react-native-reanimated';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaButton } from '@/components/ui/VelaButton';
import { AppModal } from '@/components/ui/AppModal';
import { color, text, weight, space, radius, font, shadow, createStyles } from '@/constants/theme';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { shortAddr, type BLEIncomingRequest } from '@/models/types';
import { PasskeyErrorCode } from '@/modules/passkey';
import { handleDAppRequest, isSigningMethod, handleReadOnlyRPC } from '@/hooks/use-dapp-signing';
import {
  Bluetooth, Wifi, ChevronRight, Check,
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
      if (cp?.chainId) {
        const nc = parseInt(cp.chainId, 16);
        if (!isNaN(nc)) { chainIdRef.current = nc; setCurrentChainId(nc); }
      }
      sendResponse(id, null);
      return;
    }

    handleReadOnlyRPC(method, params, addr, cid).then(res => {
      if (res.handled) sendResponse(id, res.result);
      else sendResponse(id, undefined, { code: -32603, message: `RPC failed: ${method}` });
    });
  }, [sendResponse]);

  // --- Approve / Reject ---
  const approveRequest = useCallback(async (request: BLEIncomingRequest) => {
    if (!activeAccount) return;
    setIsSigning(true);
    setSignError(null);
    try {
      const result = await handleDAppRequest(request, activeAccount, state.address, chainIdRef.current);
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
  }, [activeAccount, state.address, sendResponse]);

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
    if (!granted) { Alert.alert('Permission Required', 'Bluetooth permission is needed.'); return; }
    await BLE.startAdvertising({
      walletAddress: address, accountName, chainId: currentChainId,
      accounts: state.accounts.map(a => ({ name: a.name, address: a.address })),
    });
    setConnectState('advertising');
  }, [address, accountName, currentChainId, state.accounts]);

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
          <Text style={styles.emptyText}>Create a wallet first</Text>
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
        <Animated.View entering={FadeIn.duration(300)}>
          <Text style={styles.pageTitle}>dApps</Text>
        </Animated.View>

        {/* Wallet card */}
        <Animated.View entering={FadeInDown.delay(50).duration(300)}>
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
          <Animated.View style={styles.section} entering={FadeInDown.delay(100).duration(300)}>
            <View style={styles.connectPrompt}>
              <View style={styles.connectIconWrap}>
                <ConnectIcon size={24} color={color.info.base} />
              </View>
              <Text style={styles.sectionTitle}>
                {isWeb ? 'Connect to dApp Browser' : 'Connect via Bluetooth'}
              </Text>
              <Text style={styles.hint}>
                {isWeb
                  ? 'Connect to the local dApp Browser to interact with dApps.'
                  : 'Start advertising to pair with the Vela Connect browser extension.'}
              </Text>
            </View>
            <VelaButton title="Connect" onPress={isWeb ? connectWS : startBLE} />
          </Animated.View>
        )}

        {connectState === 'connecting' && (
          <View style={styles.centered}>
            <Radio size={24} color={color.info.base} />
            <Text style={styles.statusText}>Connecting...</Text>
          </View>
        )}

        {isAdvertising && isNative && (
          <Animated.View style={styles.section} entering={FadeIn.duration(300)}>
            <VelaCard style={styles.statusCard}>
              <View style={styles.pulseRow}>
                <PulsingBluetooth />
                <Text style={styles.statusText}>Waiting for connection...</Text>
              </View>
            </VelaCard>
            <VelaButton title="Stop" onPress={stopBLE} variant="secondary" style={styles.sectionBtn} />
          </Animated.View>
        )}

        {connectState === 'not-installed' && isWeb && (
          <Animated.View entering={FadeInDown.duration(300)}>
            <VelaCard style={styles.notInstalledCard}>
              <AlertTriangle size={24} color={color.fg.muted} />
              <Text style={styles.notInstalledTitle}>dApp Browser not found</Text>
              <Text style={styles.hint}>Install the dApp Browser to connect.</Text>
              <VelaButton title="Download" onPress={() => {
                if (Platform.OS === 'web') window.open('https://getvela.app/dpp-browser', '_blank');
              }} variant="accent" style={styles.sectionBtn} />
              <VelaButton title="Try Again" onPress={connectWS} variant="secondary" style={styles.retryBtn} />
            </VelaCard>
          </Animated.View>
        )}

        {/* Connected state */}
        {connectState === 'connected' && !incomingRequest && (
          <Animated.View style={styles.section} entering={FadeIn.duration(300)}>
            <VelaCard style={styles.connectedCard}>
              <View style={styles.connectedRow}>
                <View style={styles.connectedDot} />
                <Text style={styles.connectedText}>Connected to {peerName}</Text>
              </View>
              <Text style={styles.connectedHint}>
                Signing requests from dApps will appear here.
              </Text>
            </VelaCard>
            <VelaButton title="Disconnect" onPress={disconnect} variant="secondary" style={styles.sectionBtn} />
          </Animated.View>
        )}

        {/* Signing request */}
        {incomingRequest && (
          <Animated.View entering={FadeInDown.duration(300)}>
            <VelaCard elevated style={styles.requestCard}>
              <View style={styles.requestHeader}>
                {methodIcon(incomingRequest.method)}
                <View style={styles.requestHeaderText}>
                  <Text style={styles.requestMethod}>{methodLabel(incomingRequest.method)}</Text>
                  <Text style={styles.requestOrigin}>{incomingRequest.origin || peerName}</Text>
                </View>
              </View>

              {incomingRequest.method === 'eth_sendTransaction' && incomingRequest.params?.[0] && (
                <View style={styles.txDetails}>
                  <DetailRow label="To" value={shortAddr(incomingRequest.params[0].to ?? '')} />
                  <DetailRow label="Value" value={incomingRequest.params[0].value ?? '0x0'} />
                </View>
              )}

              {signError && (
                <View style={styles.errorRow}>
                  <AlertTriangle size={14} color={color.accent.base} />
                  <Text style={styles.errorText}>{signError}</Text>
                </View>
              )}

              <View style={styles.buttonRow}>
                <VelaButton
                  title={isSigning ? 'Signing...' : 'Approve'}
                  onPress={() => approveRequest(incomingRequest)}
                  variant="accent" loading={isSigning} style={styles.buttonFlex}
                />
                <VelaButton
                  title="Reject"
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
            <Text style={styles.modalTitle}>Accounts</Text>
            <Pressable onPress={() => setShowAccountPicker(false)} hitSlop={8}>
              <Text style={styles.modalClose}>Done</Text>
            </Pressable>
          </View>
          <ScrollView style={styles.modalScroll}>
            {state.accounts.map((account, index) => {
              const isActive = account.id === activeAccount?.id;
              return (
                <Pressable
                  key={account.id}
                  style={[styles.accountItem, isActive && styles.accountItemActive]}
                  onPress={() => { dispatch({ type: 'SWITCH_ACCOUNT', index }); setShowAccountPicker(false); }}
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

function methodLabel(m: string): string {
  if (m === 'eth_sendTransaction') return 'Send Transaction';
  if (m === 'personal_sign') return 'Sign Message';
  if (m.includes('signTypedData')) return 'Sign Typed Data';
  return m;
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
    fontWeight: weight.bold,
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
  walletAvatarText: { fontSize: text.lg, fontWeight: weight.bold, color: color.accent.base },
  walletInfo: { flex: 1, gap: 2 },
  walletName: { fontSize: text.lg, fontWeight: weight.semibold, color: color.fg.base },
  walletAddr: { fontSize: text.sm, fontWeight: weight.medium, fontFamily: font.mono, color: color.fg.subtle },

  // Sections
  section: { marginBottom: space.xl },
  sectionTitle: { fontSize: text.xl, fontWeight: weight.bold, color: color.fg.base, marginBottom: space.sm },
  hint: { fontSize: text.base, fontWeight: weight.regular, color: color.fg.muted, lineHeight: 20, marginBottom: space.xl },
  sectionBtn: { marginTop: space.lg },
  retryBtn: { marginTop: space.md },
  centered: { alignItems: 'center', paddingVertical: space['5xl'], gap: space.lg },
  emptyText: { fontSize: text.lg, fontWeight: weight.regular, color: color.fg.muted },
  statusText: { fontSize: text.lg, fontWeight: weight.semibold, color: color.info.base },
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
  connectedText: { fontSize: text.lg, fontWeight: weight.semibold, color: color.fg.base },
  connectedHint: { fontSize: text.base, fontWeight: weight.regular, color: color.fg.muted },

  // Not installed
  notInstalledCard: { padding: space['2xl'], alignItems: 'center', gap: space.md },
  notInstalledTitle: { fontSize: text.xl, fontWeight: weight.bold, color: color.fg.base },

  // Request
  requestCard: { padding: space['2xl'], gap: space.xl },
  requestHeader: { flexDirection: 'row', alignItems: 'center', gap: space.lg },
  requestHeaderText: { flex: 1, gap: 2 },
  requestOrigin: { fontSize: text.sm, fontWeight: weight.regular, color: color.fg.muted },
  requestMethod: { fontSize: text.xl, fontWeight: weight.bold, color: color.fg.base },
  txDetails: {
    gap: space.md, paddingVertical: space.lg,
    borderTopWidth: 1, borderBottomWidth: 1, borderColor: color.border.base,
  },
  detailRow: { flexDirection: 'row', justifyContent: 'space-between' },
  detailLabel: { fontSize: text.base, fontWeight: weight.regular, color: color.fg.muted },
  detailValue: { fontSize: text.base, fontWeight: weight.medium, fontFamily: font.mono, color: color.fg.base, maxWidth: '60%' as any },
  errorRow: { flexDirection: 'row', alignItems: 'center', gap: space.md },
  errorText: { fontSize: text.base, fontWeight: weight.regular, color: color.accent.base, flex: 1 },
  buttonRow: { flexDirection: 'row', gap: space.lg },
  buttonFlex: { flex: 1 },

  // Modal
  modalContainer: { flex: 1, padding: space['3xl'] },
  modalHeader: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: space['2xl'] },
  modalTitle: { fontSize: text.xl, fontWeight: weight.bold, color: color.fg.base },
  modalClose: { fontSize: text.lg, fontWeight: weight.semibold, color: color.accent.base },
  modalScroll: { flex: 1 },
  accountItem: {
    flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center',
    padding: space.xl, borderRadius: radius.xl, marginBottom: space.md,
    borderWidth: 1, borderColor: color.border.base,
    ...shadow.sm,
  },
  accountItemActive: { borderColor: color.accent.base, backgroundColor: color.accent.soft },
  accountItemInfo: { gap: 2 },
  accountItemName: { fontSize: text.lg, fontWeight: weight.semibold, color: color.fg.base },
  accountItemAddr: { fontSize: text.sm, fontWeight: weight.medium, fontFamily: font.mono, color: color.fg.subtle },
}));
