// <ExtensionSignController> — mounted ONCE at the root (next to SigningRequestModal),
// ABOVE the navigator, so everything it renders overlays whatever screen the user is
// currently on — home, a token detail, the in-app browser, anywhere. It is not tied
// to any route.
//
// It owns the Safari-extension sign hand-off so the WHOLE flow renders as overlays
// over the CURRENT screen, never a standalone page:
//   • The inbound velawallet://sign?rid deep link hits /sign, which is a trampoline:
//     it hands the rid here (extension-sign-bus) and immediately returns to wherever
//     the user was (router.back; the wallet on a cold launch that has no prior screen).
//   • Here we wait until the wallet is signing-ready, build an ExtensionBridgeTransport,
//     hand it to DAppConnectionProvider via beginExtensionSign(), and connect() — which
//     reads sign-req-<rid>.json and emits the request. The GLOBAL <SigningRequestModal>
//     (the real sign sheet: clear-signing, asset-sim, gas/funding, approval-guard,
//     passkey, bundler) presents over the current screen.
//   • On approve/reject the transport writes sign-result-<rid>.json (what the extension
//     polls on return); we observe 'disconnected' and show a small CONFIRMATION sheet —
//     a plain overlay View over a dimmed wallet (NOT an RN Modal, so it can never
//     collide with the just-dismissed sign modal — the app's modal-over-modal weakness).
//
// It NEVER signs anything and shows NO raw request data. All fund-safety logic
// (persist-at-submit, 4001-only-on-explicit-reject / 4900-on-unknown, the
// anti-double-submit result-replay, the §12.1.6 account reconcile) lives in the
// transport + approveRequest + dapp-request-routing.
import { useEffect, useRef, useState } from 'react';
import { View, Text, Pressable, StyleSheet } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useTranslation } from 'react-i18next';
import { useWallet } from '@/models/wallet-state';
import { useDAppConnection } from '@/models/dapp-connection';
import { color, space, text, inter } from '@/constants/theme';
import { signAccountIndex } from '@/models/dapp-request-routing';
import { onExtensionSign } from '@/services/extension-sign-bus';
import { ExtensionBridgeTransport } from '@/services/extension-bridge-transport';

type Phase = 'idle' | 'connecting' | 'signing' | 'done' | 'missing' | 'selftest';
type Outcome = 'submitted' | 'rejected' | 'unknown';

// The extension's toolbar popup opens https://getvela.app/sign?rid=ul-selftest to
// bootstrap Universal-Link attestation. Reaching here means the UL resolved — the
// <AccountFileWriter/> observer already marked it; we just confirm, no sign to run.
const UL_SELFTEST_RID = 'ul-selftest';

const V = (event: string, rid: string | undefined, extra = '') =>
  console.log(`VELAB APP ${event} rid=${rid ?? '(none)'} t=${Date.now()}${extra ? ' ' + extra : ''}`);

// Machine-readable status for the e2e harness (lib.py app_status reads "status:").
// STABLE ASCII tokens, independent of localized copy, rendered strictly offscreen.
function machineStatus(phase: Phase, outcome: Outcome): string {
  switch (phase) {
    case 'idle': return 'idle';
    case 'connecting': return 'reading request…';
    case 'signing': return 'request received';
    case 'selftest': return 'UL verified';
    case 'missing': return 'request not found / expired';
    case 'done':
      return outcome === 'submitted' ? 'Signed'
        : outcome === 'rejected' ? 'Rejected'
        : 'Unfinished — check Vela';
  }
}

export function ExtensionSignController(): React.ReactElement {
  const { state, activeAccount, dispatch } = useWallet();
  const { beginExtensionSign } = useDAppConnection();
  const { t } = useTranslation();
  const insets = useSafeAreaInsets();

  const [rid, setRid] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>('idle');
  const [outcome, setOutcome] = useState<Outcome>('unknown');
  const startedRef = useRef<string | null>(null);

  // Fresh account snapshot for the §12.1.6 reconcile inside the connect callback.
  const accountsRef = useRef(state.accounts);
  const activeIdxRef = useRef(state.activeAccountIndex);
  accountsRef.current = state.accounts;
  activeIdxRef.current = state.activeAccountIndex;

  const ready = !state.isLoading && state.hasWallet && !!activeAccount;

  // Subscribe once: a new rid from the /sign trampoline starts a fresh flow.
  useEffect(() => {
    return onExtensionSign((next) => {
      startedRef.current = null; // allow a brand-new rid to begin
      setOutcome('unknown');
      setRid(next);
      setPhase(next === UL_SELFTEST_RID ? 'selftest' : 'connecting');
    });
  }, []);

  useEffect(() => {
    if (!rid || rid === UL_SELFTEST_RID) return;
    if (!ready) return; // wait for the store; re-runs when `ready` flips
    if (startedRef.current === rid) return;
    startedRef.current = rid;

    V('begin', rid);
    setPhase('connecting');
    const transport = new ExtensionBridgeTransport(rid);
    const unsub = transport.on('disconnected', () => {
      V('settled', rid, `outcome=${transport.outcome}`);
      setOutcome(transport.outcome);
      setPhase('done');
    });
    beginExtensionSign(transport);
    transport
      .connect()
      .then(() => {
        // Anti-double-submit: a rid already signed replays its outcome (no re-emit,
        // no second modal) — go straight to the settled sheet.
        if (transport.alreadySettled) {
          V('replay', rid, `outcome=${transport.outcome}`);
          setOutcome(transport.outcome);
          setPhase('done');
          return;
        }
        // §12.1.6 reconcile: sign from the account the origin was GRANTED, not
        // whatever is active. Switch to it before the user approves.
        const idx = signAccountIndex(accountsRef.current, activeIdxRef.current, transport.requestAddress);
        if (idx !== activeIdxRef.current) {
          V('reconcile', rid, `switch ${activeIdxRef.current}->${idx}`);
          dispatch({ type: 'SWITCH_ACCOUNT', index: idx });
        }
        setPhase('signing'); // the global SigningRequestModal is now rendering over home
        V('connected', rid, `origin=${transport.requestOrigin}`);
      })
      .catch((e) => {
        V('connect-failed', rid, `err=${String(e)}`);
        setPhase('missing');
      });
    return () => unsub();
  }, [rid, ready, beginExtensionSign, dispatch]);

  const close = () => { setPhase('idle'); setRid(null); startedRef.current = null; };

  // Offscreen machine-readable line for the harness — always present, never seen.
  const harnessLine = (
    <Text
      style={styles.offscreen}
      accessible
      accessibilityLabel={`status: ${machineStatus(phase, outcome)}`}
    >
      status: {machineStatus(phase, outcome)} rid:{rid ?? '(none)'}
    </Text>
  );

  // Idle / connecting / signing → render only the (non-blocking, offscreen) harness
  // line. The wallet home is fully interactive; the SigningRequestModal is the sign UI.
  const settled = phase === 'done' || phase === 'missing' || phase === 'selftest';
  if (!settled) {
    return <View style={styles.hostIdle} pointerEvents="none">{harnessLine}</View>;
  }

  // Settled → a bottom-sheet CONFIRMATION over the dimmed wallet. Color grammar
  // (§12.3): success = submitted, neutral = user-chosen reject, amber = ambiguous.
  let glyph = '', glyphColor: string = color.fg.subtle, glyphBg: string = color.bg.sunken;
  let title = '', hint = '';
  if (phase === 'selftest') {
    glyph = '✓'; glyphColor = color.success.base; glyphBg = color.success.soft;
    title = t('signHandoff.oneTapTitle'); hint = t('signHandoff.oneTapHint');
  } else if (phase === 'missing') {
    glyph = '!'; glyphColor = color.fg.subtle; glyphBg = color.bg.sunken;
    title = t('signHandoff.expired'); hint = t('signHandoff.returnHint');
  } else if (outcome === 'submitted') {
    glyph = '✓'; glyphColor = color.success.base; glyphBg = color.success.soft;
    title = t('signHandoff.signed'); hint = t('signHandoff.returnHint');
  } else if (outcome === 'rejected') {
    glyph = '✕'; glyphColor = color.fg.subtle; glyphBg = color.bg.sunken;
    title = t('signHandoff.rejected'); hint = t('signHandoff.returnHint');
  } else {
    glyph = '!'; glyphColor = color.warning.base; glyphBg = color.warning.soft;
    title = t('signHandoff.pending'); hint = t('signHandoff.returnHint');
  }

  return (
    <View style={styles.hostOverlay}>
      {harnessLine}
      {/* Tap the dimmed wallet behind to dismiss the confirmation. */}
      <Pressable style={StyleSheet.absoluteFill} onPress={close} accessibilityLabel={t('signHandoff.done')} />
      <View style={[styles.sheet, { paddingBottom: space['3xl'] + insets.bottom }]}>
        <View style={styles.grab} />
        <View style={[styles.badge, { backgroundColor: glyphBg }]}>
          <Text style={[styles.glyph, { color: glyphColor }]}>{glyph}</Text>
        </View>
        <Text style={styles.title}>{title}</Text>
        {!!hint && <Text style={styles.hint}>{hint}</Text>}
        <Pressable
          style={({ pressed }) => [styles.doneBtn, pressed && styles.doneBtnPressed]}
          onPress={close}
          accessibilityRole="button"
        >
          <Text style={styles.doneLabel}>{t('signHandoff.done')}</Text>
        </Pressable>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  // Non-blocking host while idle: zero-size, lets all wallet touches through.
  hostIdle: { position: 'absolute', width: 0, height: 0 },
  // Full-screen dimmed overlay OVER the wallet home for the confirmation.
  hostOverlay: { ...StyleSheet.absoluteFillObject, justifyContent: 'flex-end', backgroundColor: 'rgba(20,20,18,0.32)' },
  sheet: {
    backgroundColor: color.bg.raised,
    borderTopLeftRadius: 24,
    borderTopRightRadius: 24,
    paddingTop: space.md,
    paddingHorizontal: space['4xl'],
    alignItems: 'center',
    shadowColor: '#000', shadowOffset: { width: 0, height: -8 }, shadowOpacity: 0.14, shadowRadius: 24, elevation: 16,
  },
  grab: { width: 36, height: 5, borderRadius: 3, backgroundColor: color.border.strong, opacity: 0.8, marginBottom: space['2xl'] },
  badge: { width: 56, height: 56, borderRadius: 28, alignItems: 'center', justifyContent: 'center', marginBottom: space['xl'] },
  glyph: { fontSize: 28, lineHeight: 34, ...inter.bold },
  title: { fontSize: text.xl, color: color.fg.base, textAlign: 'center', ...inter.bold },
  hint: { fontSize: text.base, color: color.fg.muted, textAlign: 'center', marginTop: space.md, lineHeight: 20, maxWidth: 300, ...inter.regular },
  doneBtn: { alignSelf: 'stretch', marginTop: space['3xl'], paddingVertical: space.xl, borderRadius: 15, backgroundColor: color.accent.base, alignItems: 'center' },
  doneBtnPressed: { opacity: 0.92 },
  doneLabel: { fontSize: text.lg, color: '#fff', ...inter.semibold },
  offscreen: { position: 'absolute', left: -9999, top: -9999, width: 1, height: 1, opacity: 0 },
});
