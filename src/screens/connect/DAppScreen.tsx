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
  View, Text, TextInput, ScrollView, TouchableOpacity,
  StyleSheet, Alert, Platform,
} from 'react-native';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaButton } from '@/components/ui/VelaButton';
import { AppModal } from '@/components/ui/AppModal';
import { VelaColor, VelaFont, VelaRadius, VelaSpacing } from '@/constants/theme';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { shortAddr, type BLEIncomingRequest } from '@/models/types';
import { PasskeyErrorCode } from '@/modules/passkey';
import { handleDAppRequest, isSigningMethod, handleReadOnlyRPC } from '@/hooks/use-dapp-signing';
import { Bluetooth, Wifi, UserCircle, Check, ChevronRight } from 'lucide-react-native';

// BLE module — only imported on native
const BLE = Platform.OS !== 'web' ? require('@/modules/ble') : null;

type ConnectState = 'idle' | 'connecting' | 'advertising' | 'connected' | 'not-installed';

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
  const wsRef = useRef<WebSocket | null>(null); // web
  const bleUnsubsRef = useRef<(() => void)[]>([]); // native

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

    // Signing → show approval UI
    if (isSigningMethod(method)) {
      setIncomingRequest({ id, method, params, origin });
      return;
    }

    // Chain switch
    if (method === 'wallet_switchEthereumChain') {
      const cp = params?.[0] as { chainId?: string } | undefined;
      if (cp?.chainId) {
        const nc = parseInt(cp.chainId, 16);
        if (!isNaN(nc)) { chainIdRef.current = nc; setCurrentChainId(nc); }
      }
      sendResponse(id, null);
      return;
    }

    // Auto-reply
    handleReadOnlyRPC(method, params, addr, cid).then(res => {
      if (res.handled) sendResponse(id, res.result);
      else {
        sendResponse(id, undefined, { code: -32603, message: `RPC failed: ${method}` });
      }
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

  // --- Push wallet info (both transports) ---
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

  // Push wallet info when account/chain changes (NOT on connect — native layer handles that)
  const wasConnected = useRef(false);
  useEffect(() => {
    if (connectState === 'connected') {
      if (wasConnected.current) {
        // Already connected — push updated info
        pushWalletInfo();
      }
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

  // Disconnect (both)
  const disconnect = useCallback(() => {
    if (Platform.OS === 'web') { wsRef.current?.close(); wsRef.current = null; }
    else stopBLE();
    setConnectState('idle');
    setIncomingRequest(null);
  }, [stopBLE]);

  // Cleanup
  useEffect(() => () => { wsRef.current?.close(); }, []);

  // =========================================================================
  // Render
  // =========================================================================

  if (!state.hasWallet) {
    return (
      <ScreenContainer>
        <View style={styles.centered}><Text style={styles.emptyText}>Create a wallet first.</Text></View>
      </ScreenContainer>
    );
  }

  const isWeb = Platform.OS === 'web';
  const isNative = !isWeb;
  const isAdvertising = connectState === 'advertising';

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.scrollContent}>
        <Text style={styles.pageTitle}>dApps</Text>

        {/* Wallet card */}
        <TouchableOpacity activeOpacity={0.7} onPress={() => setShowAccountPicker(true)}>
          <VelaCard style={styles.walletCard}>
            <View style={styles.walletRow}>
              <View style={{ flex: 1 }}>
                <Text style={styles.walletName}>{accountName}</Text>
                <Text style={styles.walletAddr}>{shortAddress(address)}</Text>
              </View>
              <ChevronRight size={18} color={VelaColor.textTertiary} />
            </View>
          </VelaCard>
        </TouchableOpacity>

        {/* Connection section — platform specific */}
        {connectState === 'idle' && (
          <View style={styles.section}>
            <Text style={styles.sectionTitle}>
              {isWeb ? 'Connect to dApp Browser' : 'Connect via Bluetooth'}
            </Text>
            <Text style={styles.hint}>
              {isWeb
                ? 'Connect to the local dApp Browser to interact with dApps.'
                : 'Start advertising to pair with the Vela Connect browser extension.'}
            </Text>
            <VelaButton title="Connect" onPress={isWeb ? connectWS : startBLE} />
          </View>
        )}

        {connectState === 'connecting' && (
          <View style={styles.centered}><Text style={styles.statusText}>Connecting...</Text></View>
        )}

        {isAdvertising && isNative && (
          <View style={styles.section}>
            <VelaCard style={styles.statusCard}>
              <View style={styles.pulseRow}>
                <Bluetooth size={18} color={VelaColor.blue} />
                <Text style={styles.statusText}>Waiting for connection...</Text>
              </View>
            </VelaCard>
            <VelaButton title="Stop" onPress={stopBLE} variant="secondary" style={{ marginTop: 12 }} />
          </View>
        )}

        {connectState === 'not-installed' && isWeb && (
          <VelaCard style={styles.notInstalledCard}>
            <Text style={styles.notInstalledTitle}>dApp Browser not found</Text>
            <Text style={styles.hint}>Install the dApp Browser to connect.</Text>
            <VelaButton title="Download" onPress={() => {
              if (Platform.OS === 'web') window.open('https://getvela.app/dpp-browser', '_blank');
            }} variant="accent" style={{ marginTop: 12 }} />
            <VelaButton title="Try Again" onPress={connectWS} variant="secondary" style={{ marginTop: 8 }} />
          </VelaCard>
        )}

        {/* Connected state */}
        {connectState === 'connected' && !incomingRequest && (
          <View style={styles.section}>
            <VelaCard style={styles.connectedCard}>
              <View style={styles.connectedRow}>
                <View style={styles.dot} />
                <Text style={styles.connectedText}>Connected to {peerName}</Text>
              </View>
              <Text style={styles.connectedHint}>
                Signing requests from dApps will appear here.
              </Text>
            </VelaCard>
            <VelaButton title="Disconnect" onPress={disconnect} variant="secondary" style={{ marginTop: 12 }} />
          </View>
        )}

        {/* Signing request */}
        {incomingRequest && (
          <VelaCard style={styles.requestCard}>
            <Text style={styles.requestOrigin}>{incomingRequest.origin || peerName}</Text>
            <Text style={styles.requestMethod}>{methodLabel(incomingRequest.method)}</Text>

            {incomingRequest.method === 'eth_sendTransaction' && incomingRequest.params?.[0] && (
              <View style={styles.txDetails}>
                <DetailRow label="To" value={shortAddr(incomingRequest.params[0].to ?? '')} />
                <DetailRow label="Value" value={incomingRequest.params[0].value ?? '0x0'} />
              </View>
            )}

            {signError && <Text style={styles.errorText}>{signError}</Text>}

            <View style={styles.buttonRow}>
              <VelaButton
                title={isSigning ? 'Signing...' : 'Approve'}
                onPress={() => approveRequest(incomingRequest)}
                variant="accent" loading={isSigning} style={{ flex: 1 }}
              />
              <View style={{ width: 12 }} />
              <VelaButton
                title="Reject"
                onPress={() => rejectRequest(incomingRequest)}
                variant="secondary" disabled={isSigning} style={{ flex: 1 }}
              />
            </View>
          </VelaCard>
        )}
      </ScrollView>

      {/* Account picker modal */}
      <AppModal visible={showAccountPicker} onClose={() => setShowAccountPicker(false)}>
        <View style={styles.modalContainer}>
          <View style={styles.modalHeader}>
            <Text style={styles.modalTitle}>Accounts</Text>
            <TouchableOpacity onPress={() => setShowAccountPicker(false)}>
              <Text style={styles.modalClose}>Done</Text>
            </TouchableOpacity>
          </View>
          <ScrollView style={styles.modalScroll}>
            {state.accounts.map((account, index) => {
              const isActive = account.id === activeAccount?.id;
              return (
                <TouchableOpacity
                  key={account.id}
                  style={[styles.accountItem, isActive && styles.accountItemActive]}
                  onPress={() => { dispatch({ type: 'SWITCH_ACCOUNT', index }); setShowAccountPicker(false); }}
                  activeOpacity={0.7}
                >
                  <View>
                    <Text style={styles.accountItemName}>{account.name}</Text>
                    <Text style={styles.accountItemAddr}>{shortAddress(account.address)}</Text>
                  </View>
                  {isActive && <Check size={18} color={VelaColor.accent} />}
                </TouchableOpacity>
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

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = StyleSheet.create({
  scrollContent: { paddingBottom: 40 },
  pageTitle: { ...VelaFont.heading(28), color: VelaColor.textPrimary, marginTop: 16, marginBottom: 20 },

  // Wallet card
  walletCard: { padding: VelaSpacing.cardPadding, marginBottom: 20 },
  walletRow: { flexDirection: 'row', alignItems: 'center' },
  walletName: { ...VelaFont.title(16), color: VelaColor.textPrimary },
  walletAddr: { ...VelaFont.mono(13), color: VelaColor.textTertiary, marginTop: 2 },

  // Sections
  section: { marginBottom: 16 },
  sectionTitle: { ...VelaFont.title(18), color: VelaColor.textPrimary, marginBottom: 6 },
  hint: { ...VelaFont.body(14), color: VelaColor.textSecondary, lineHeight: 20, marginBottom: 14 },
  centered: { alignItems: 'center', paddingVertical: 40 },
  emptyText: { ...VelaFont.body(16), color: VelaColor.textSecondary },
  statusText: { ...VelaFont.title(15), color: VelaColor.blue },
  statusCard: { padding: VelaSpacing.cardPadding },
  pulseRow: { flexDirection: 'row', alignItems: 'center', gap: 10 },

  // Connected
  connectedCard: { padding: VelaSpacing.cardPadding, gap: 6 },
  connectedRow: { flexDirection: 'row', alignItems: 'center', gap: 10 },
  dot: { width: 10, height: 10, borderRadius: 5, backgroundColor: VelaColor.green },
  connectedText: { ...VelaFont.title(15), color: VelaColor.textPrimary },
  connectedHint: { ...VelaFont.body(13), color: VelaColor.textSecondary },

  // Not installed
  notInstalledCard: { padding: VelaSpacing.cardPadding, gap: 4 },
  notInstalledTitle: { ...VelaFont.title(17), color: VelaColor.textPrimary },

  // Request
  requestCard: { padding: VelaSpacing.cardPadding, gap: 12 },
  requestOrigin: { ...VelaFont.body(13), color: VelaColor.textSecondary },
  requestMethod: { ...VelaFont.heading(20), color: VelaColor.textPrimary },
  txDetails: { gap: 8, paddingVertical: 4 },
  detailRow: { flexDirection: 'row', justifyContent: 'space-between' },
  detailLabel: { ...VelaFont.body(14), color: VelaColor.textSecondary },
  detailValue: { ...VelaFont.mono(14), color: VelaColor.textPrimary, maxWidth: '60%' as any },
  errorText: { ...VelaFont.body(13), color: VelaColor.accent },
  buttonRow: { flexDirection: 'row', marginTop: 8 },

  // Modal
  modalContainer: { flex: 1, padding: VelaSpacing.cardPadding },
  modalHeader: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 },
  modalTitle: { ...VelaFont.title(18), color: VelaColor.textPrimary },
  modalClose: { ...VelaFont.title(16), color: VelaColor.accent },
  modalScroll: { flex: 1 },
  accountItem: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', padding: 14, borderRadius: VelaRadius.cardSmall, marginBottom: 8, borderWidth: 1, borderColor: VelaColor.border },
  accountItemActive: { borderColor: VelaColor.accent, backgroundColor: VelaColor.accentSoft },
  accountItemName: { ...VelaFont.title(15), color: VelaColor.textPrimary },
  accountItemAddr: { ...VelaFont.mono(12), color: VelaColor.textTertiary, marginTop: 2 },
});
