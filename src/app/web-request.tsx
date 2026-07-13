import { useCallback, useEffect, useRef, useState } from 'react';
import { ActivityIndicator, Image, Pressable, StyleSheet, Text, View } from 'react-native';
import { useLocalSearchParams } from 'expo-router';
import { Link2, ShieldCheck, X } from 'lucide-react-native';
import { useWallet } from '@/models/wallet-state';
import { useDAppConnection } from '@/models/dapp-connection';
import OnboardingScreen from '@/screens/onboarding/OnboardingScreen';
import { getAllNetworksSync } from '@/models/network';
import { getGrant, resolveGranted, setGrant } from '@/services/dapp-permissions';
import { signAccountIndex } from '@/models/dapp-request-routing';
import { assertChainSupported } from '@/hooks/use-dapp-signing';
import { WebPopupTransport, isAllowedWebDAppOrigin, type WebPopupPeer } from '@/services/web-popup-transport';
import { color, font, inter, space, text as textSize } from '@/constants/theme';
import {
  VELA_WEB_CHANNEL,
  VELA_WEB_READY,
  VELA_WEB_RESPONSE,
  isVelaWebInit,
  type VelaWebReadyMessage,
  type VelaWebResponseMessage,
} from '../../packages/vela-sdk/src/protocol';

type Phase = 'waiting' | 'onboarding' | 'consent' | 'unsupported-chain' | 'processing' | 'done' | 'error';

interface UnsupportedNetwork {
  code: number;
  message: string;
  chainId: number;
}

const VELA_LOGO = require('../../assets/images/icon.png');

function hostOf(origin: string): string {
  try { return new URL(origin).host; } catch { return origin; }
}

/** Only load a dApp logo from the exact requesting origin. Metadata can suggest a
 * path, but it cannot turn the wallet into a third-party tracking-image client. */
function trustedDAppLogo(icon: string | undefined, origin: string | undefined): string | null {
  if (!icon || !origin) return null;
  try {
    const originUrl = new URL(origin);
    const iconUrl = new URL(icon, originUrl);
    const secure = iconUrl.protocol === 'https:' ||
      (iconUrl.protocol === 'http:' && (iconUrl.hostname === 'localhost' || iconUrl.hostname === '127.0.0.1'));
    return secure && iconUrl.origin === originUrl.origin ? iconUrl.href : null;
  } catch {
    return null;
  }
}

function closePopupSoon(): void {
  if (typeof window === 'undefined') return;
  window.setTimeout(() => window.close(), 250);
}

export default function WebRequestScreen(): React.ReactElement {
  const { session } = useLocalSearchParams<{ session?: string }>();
  const sessionId = typeof session === 'string' ? session : '';
  const { state, activeAccount, dispatch } = useWallet();
  const { beginExtensionSign } = useDAppConnection();
  const [phase, setPhase] = useState<Phase>('waiting');
  const [peer, setPeer] = useState<WebPopupPeer | null>(null);
  const [error, setError] = useState('');
  const [unsupportedNetwork, setUnsupportedNetwork] = useState<UnsupportedNetwork | null>(null);
  const [dappLogoFailed, setDappLogoFailed] = useState(false);
  const acceptedRef = useRef(false);
  const processedRef = useRef(false);
  const peerRef = useRef<WebPopupPeer | null>(null);

  const respond = useCallback((target: WebPopupPeer, result?: unknown, rpcError?: { code: number; message: string }) => {
    const message: VelaWebResponseMessage = {
      channel: VELA_WEB_CHANNEL,
      type: VELA_WEB_RESPONSE,
      sessionId: target.sessionId,
      id: target.request.id,
      ...(rpcError ? { error: rpcError } : { result: result ?? null }),
    };
    target.port.postMessage(message);
    target.port.close();
    peerRef.current = null;
    setPhase(rpcError ? 'error' : 'done');
    if (rpcError) setError(rpcError.message);
    closePopupSoon();
  }, []);

  useEffect(() => {
    if (typeof window === 'undefined' || !sessionId) {
      setPhase('error');
      setError('Invalid Vela request session.');
      return;
    }
    const opener = window.opener;
    if (!opener) {
      setPhase('error');
      setError('Open this page from a dApp using the Vela SDK.');
      return;
    }

    const ready: VelaWebReadyMessage = {
      channel: VELA_WEB_CHANNEL,
      type: VELA_WEB_READY,
      sessionId,
    };
    const announce = () => {
      if (!acceptedRef.current) opener.postMessage(ready, '*');
    };
    announce();
    const announceTimer = window.setInterval(announce, 300);

    const onMessage = (event: MessageEvent) => {
      if (acceptedRef.current || event.source !== opener || !isAllowedWebDAppOrigin(event.origin) ||
          !isVelaWebInit(event.data) || event.data.sessionId !== sessionId || !event.ports[0]) return;
      acceptedRef.current = true;
      window.clearInterval(announceTimer);

      const incoming: WebPopupPeer = {
        sessionId,
        origin: event.origin,
        dapp: {
          // Metadata is presentation-only. The security identity is always event.origin.
          name: event.data.dapp.name.trim().slice(0, 80) || hostOf(event.origin),
          url: event.origin,
          icon: event.data.dapp.icon,
        },
        request: event.data.request,
        port: event.ports[0],
      };
      incoming.port.start();
      peerRef.current = incoming;
      setPeer(incoming);
    };

    window.addEventListener('message', onMessage);
    return () => {
      window.clearInterval(announceTimer);
      window.removeEventListener('message', onMessage);
    };
  }, [sessionId]);

  // The SDK normally completes its handshake before AsyncStorage has finished
  // restoring the wallet. Hold the accepted capability port until wallet state is
  // ready; otherwise a cold popup would accept INIT and then strand it forever.
  useEffect(() => {
    if (!peer || state.isLoading || processedRef.current) return;
    processedRef.current = true;

    void (async () => {
      // Validate the chain before onboarding so a first-time user is never
      // asked to create a wallet for a request Vela cannot fulfill.
      try {
        assertChainSupported(peer.request.chainId);
      } catch (chainError: any) {
        setUnsupportedNetwork({
          code: typeof chainError?.code === 'number' ? chainError.code : 4902,
          message: chainError?.message ?? `Unsupported chain: ${peer.request.chainId}`,
          chainId: peer.request.chainId,
        });
        setPhase('unsupported-chain');
        return;
      }

      if (!state.hasWallet || !activeAccount) {
        // Keep the capability-bound MessagePort alive while the user creates or
        // recovers a wallet. Onboarding resumes this exact request on completion.
        setPhase('onboarding');
        return;
      }

      const grant = await getGrant(peer.origin);
      const currentAddresses = state.accounts.map((account) => account.address);
      const granted = resolveGranted(grant, currentAddresses);
      const isConnect = peer.request.method === 'eth_requestAccounts' || peer.request.method === 'wallet_requestPermissions';

      if (isConnect) {
        if (granted.length > 0) {
          const result = peer.request.method === 'wallet_requestPermissions'
            ? [{ parentCapability: 'eth_accounts' }]
            : granted;
          respond(peer, result);
        } else {
          setPhase('consent');
        }
        return;
      }

      if (granted.length === 0) {
        respond(peer, undefined, { code: 4100, message: 'Connect Vela Wallet to this site first' });
        return;
      }
      if (peer.request.address && peer.request.address.toLowerCase() !== granted[0].toLowerCase()) {
        respond(peer, undefined, { code: 4100, message: 'The requested account is no longer authorized' });
        return;
      }

      const transport = new WebPopupTransport(peer);
      transport.on('disconnected', () => {
        peerRef.current = null;
        setPhase('done');
        closePopupSoon();
      });
      const nextIndex = signAccountIndex(state.accounts, state.activeAccountIndex, granted[0]);
      if (nextIndex !== state.activeAccountIndex) dispatch({ type: 'SWITCH_ACCOUNT', index: nextIndex });
      beginExtensionSign(transport);
      setPhase('processing');
      // Let a reconciled account reach DAppConnectionProvider's active-account ref
      // before the approval sheet can be acted on.
      window.setTimeout(() => void transport.connect(), 0);
    })();
  }, [peer, state.isLoading, state.hasWallet, state.accounts, state.activeAccountIndex, activeAccount, beginExtensionSign, dispatch, respond]);

  useEffect(() => () => {
    const pending = peerRef.current;
    if (pending) respond(pending, undefined, { code: 4001, message: 'Vela request was closed' });
  }, [respond]);

  const approveConnection = async () => {
    if (!peer || !activeAccount) return;
    setPhase('processing');
    await setGrant({
      origin: peer.origin,
      address: activeAccount.address,
      chainId: peer.request.chainId,
      grantedAt: Date.now(),
    });
    const result = peer.request.method === 'wallet_requestPermissions'
      ? [{ parentCapability: 'eth_accounts' }]
      : [activeAccount.address];
    respond(peer, result);
  };

  const rejectConnection = () => {
    if (peer) respond(peer, undefined, { code: 4001, message: 'User rejected the connection' });
    else closePopupSoon();
  };

  const resumeAfterOnboarding = () => {
    processedRef.current = false;
    setPhase('waiting');
    // Trigger request evaluation even when React batches the wallet dispatch
    // performed immediately before this callback.
    setPeer((current) => current ? { ...current } : current);
  };

  const closeUnsupportedNetwork = () => {
    if (peer && unsupportedNetwork) {
      respond(peer, undefined, { code: unsupportedNetwork.code, message: unsupportedNetwork.message });
    } else {
      closePopupSoon();
    }
  };

  const dappName = peer?.dapp.name ?? 'dApp';
  const dappHost = peer ? hostOf(peer.origin) : '';
  const dappLogo = trustedDAppLogo(peer?.dapp.icon, peer?.origin);

  useEffect(() => setDappLogoFailed(false), [dappLogo]);

  const velaBrand = (
    <View style={styles.walletBrand}>
      <Image source={VELA_LOGO} style={styles.brandLogo} resizeMode="cover" />
      <Text style={styles.brandName}>Vela Wallet</Text>
    </View>
  );

  const identity = peer ? (
    <View style={styles.identityRow}>
      {velaBrand}
      <View style={styles.connectionMark}><Link2 size={19} color={color.accent.base} strokeWidth={2.4} /></View>
      <View style={styles.walletBrand}>
        {dappLogo && !dappLogoFailed ? (
          <Image source={{ uri: dappLogo }} style={styles.brandLogo} resizeMode="cover" onError={() => setDappLogoFailed(true)} />
        ) : (
          <View style={[styles.brandLogo, styles.dappLogoFallback]}>
            <Text style={styles.dappLogoText}>{dappName.slice(0, 3).toUpperCase()}</Text>
          </View>
        )}
        <Text style={styles.brandName} numberOfLines={1}>{dappName}</Text>
      </View>
    </View>
  ) : velaBrand;

  if (phase === 'onboarding') {
    return (
      <View style={styles.onboardingPage}>
        <View style={styles.onboardingContext}>
          {identity}
          <Text style={styles.onboardingTitle}>Set up Vela to continue</Text>
          <Text style={styles.note}>Create or recover your wallet. Your connection request from {dappName} will continue automatically.</Text>
        </View>
        <View style={styles.onboardingContent}>
          <OnboardingScreen onComplete={resumeAfterOnboarding} />
        </View>
      </View>
    );
  }

  return (
    <View style={styles.page}>
      <View style={styles.card}>
        {phase === 'consent' ? (
          <>
            {identity}
            <Text style={styles.title}>Connect with Vela</Text>
            <View style={styles.originPill}><ShieldCheck size={15} color={color.success.base} /><Text style={styles.origin}>{dappHost}</Text></View>
            <View style={styles.accountBox}>
              <Text style={styles.accountLabel}>Account</Text>
              <Text style={styles.accountName}>{activeAccount?.name ?? 'Wallet'}</Text>
              <Text style={styles.address} numberOfLines={1}>{activeAccount?.address}</Text>
            </View>
            <Text style={styles.note}>This site can view your wallet address and request signatures. Every signature still requires your approval.</Text>
            <Pressable style={styles.primaryButton} onPress={() => void approveConnection()}><Text style={styles.primaryText}>Connect</Text></Pressable>
            <Pressable style={styles.secondaryButton} onPress={rejectConnection}><Text style={styles.secondaryText}>Cancel</Text></Pressable>
          </>
        ) : phase === 'unsupported-chain' ? (
          <>
            {identity}
            <View style={styles.errorIcon}><X size={22} color={color.error.base} /></View>
            <Text style={styles.title}>Network not supported</Text>
            <Text style={styles.note}>{dappName} requested Chain ID {unsupportedNetwork?.chainId}. Vela cannot safely process this request.</Text>
            <View style={styles.networkBox}>
              <Text style={styles.accountLabel}>Networks available in Vela</Text>
              <Text style={styles.networkList}>{getAllNetworksSync().map((network) => `${network.displayName} (${network.chainId})`).join(' · ')}</Text>
            </View>
            <Pressable style={styles.secondaryButton} onPress={closeUnsupportedNetwork}><Text style={styles.secondaryText}>Close</Text></Pressable>
          </>
        ) : phase === 'error' ? (
          <>
            {velaBrand}
            <View style={styles.errorIcon}><X size={22} color={color.error.base} /></View>
            <Text style={styles.title}>Request unavailable</Text>
            <Text style={styles.note}>{error || 'Set up or recover Vela Wallet, then try again from the dApp.'}</Text>
            <Pressable style={styles.secondaryButton} onPress={closePopupSoon}><Text style={styles.secondaryText}>Close</Text></Pressable>
          </>
        ) : (
          <>
            {velaBrand}
            <ActivityIndicator size="small" color={color.accent.base} />
            <Text style={styles.title}>{phase === 'done' ? 'Done' : phase === 'processing' ? 'Confirm in Vela' : 'Connecting securely…'}</Text>
            <Text style={styles.note}>{phase === 'processing' ? 'Review the request in the Vela confirmation sheet.' : 'You can close this window after it finishes.'}</Text>
          </>
        )}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  onboardingPage: { flex: 1, minHeight: 640, alignItems: 'center', backgroundColor: color.bg.base },
  onboardingContext: { width: '100%', maxWidth: 480, alignItems: 'center', gap: space.sm, paddingHorizontal: 24, paddingTop: 24, paddingBottom: 8 },
  onboardingContent: { flex: 1, width: '100%', maxWidth: 480 },
  onboardingTitle: { color: color.fg.base, fontSize: textSize.lg, ...inter.bold, textAlign: 'center' },
  page: { flex: 1, minHeight: 560, alignItems: 'center', justifyContent: 'center', padding: 20, backgroundColor: color.bg.base },
  card: { width: '100%', maxWidth: 390, gap: space.md, alignItems: 'center', padding: 24, borderRadius: 24, backgroundColor: color.bg.raised, borderWidth: 1, borderColor: color.border.base },
  identityRow: { width: '100%', flexDirection: 'row', alignItems: 'flex-start', justifyContent: 'center', gap: 16 },
  walletBrand: { width: 96, alignItems: 'center', gap: 8 },
  brandLogo: { width: 68, height: 68, borderRadius: 19, borderWidth: 1, borderColor: color.border.base, backgroundColor: color.bg.sunken },
  brandName: { width: 96, color: color.fg.base, fontSize: textSize.sm, ...inter.semibold, textAlign: 'center' },
  connectionMark: { width: 38, height: 38, marginTop: 15, borderRadius: 19, alignItems: 'center', justifyContent: 'center', backgroundColor: color.accent.soft, borderWidth: 1, borderColor: color.border.base },
  dappLogoFallback: { alignItems: 'center', justifyContent: 'center', backgroundColor: '#0B0E0C' },
  dappLogoText: { color: '#99F6B7', fontSize: textSize.base, ...inter.bold },
  title: { color: color.fg.base, fontSize: textSize.xl, ...inter.bold, textAlign: 'center' },
  originPill: { flexDirection: 'row', alignItems: 'center', gap: 6, paddingHorizontal: 10, paddingVertical: 7, borderRadius: 999, backgroundColor: color.success.soft },
  origin: { color: color.success.base, fontSize: textSize.sm, ...inter.medium },
  accountBox: { width: '100%', padding: 16, gap: 4, borderRadius: 16, backgroundColor: color.bg.sunken },
  networkBox: { width: '100%', padding: 16, gap: 7, borderRadius: 16, backgroundColor: color.bg.sunken },
  networkList: { color: color.fg.muted, fontSize: textSize.xs, lineHeight: 18, textAlign: 'left' },
  accountLabel: { color: color.fg.subtle, fontSize: textSize.xs, ...inter.medium },
  accountName: { color: color.fg.base, fontSize: textSize.base, ...inter.semibold },
  address: { color: color.fg.muted, fontSize: textSize.sm, fontFamily: font.mono },
  note: { color: color.fg.muted, fontSize: textSize.sm, lineHeight: 20, textAlign: 'center' },
  primaryButton: { width: '100%', alignItems: 'center', paddingVertical: 14, borderRadius: 14, backgroundColor: color.accent.base },
  primaryText: { color: '#fff', fontSize: textSize.base, ...inter.semibold },
  secondaryButton: { width: '100%', alignItems: 'center', paddingVertical: 12 },
  secondaryText: { color: color.fg.muted, fontSize: textSize.base, ...inter.medium },
  errorIcon: { width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: color.error.soft },
});
