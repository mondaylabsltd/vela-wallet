// src/app/sign.tsx — inbound target of velawallet://sign?rid=<uuid> from the
// Safari extension. Phase B: a HEADLESS controller (no more fake-sign). It waits
// until the wallet is signing-ready, constructs an ExtensionBridgeTransport for
// the rid, hands it to DAppConnectionProvider via beginExtensionSign(), and lets
// it connect() — which reads sign-req-<rid>.json and emits the request. The GLOBAL
// <SigningRequestModal> then renders the REAL sign UI (clear-signing, asset-sim,
// gas/funding, approval-guard, passkey, bundler) for free. On approve/reject the
// transport writes the frozen sign-result-<rid>.json the extension polls on
// return; we observe 'disconnected' and flip to a "Return to Safari" affordance.
//
// This screen renders only a thin background/status behind that modal — it never
// signs anything itself. All the fund-safety machinery (persist-at-submit, the
// 4001-only-on-explicit-reject / 4900-on-unknown mapping) lives in the transport
// + approveRequest, unchanged from the app's own signing pipeline.
import { useEffect, useRef, useState } from 'react';
import { Text, ScrollView } from 'react-native';
import { useLocalSearchParams } from 'expo-router';
import { useWallet } from '@/models/wallet-state';
import { useDAppConnection } from '@/models/dapp-connection';
import { ExtensionBridgeTransport } from '@/services/extension-bridge-transport';

type Phase = 'waiting-ready' | 'connecting' | 'signing' | 'done' | 'missing' | 'selftest';

// The extension's toolbar popup opens https://getvela.app/sign?rid=ul-selftest to
// bootstrap Universal-Link attestation (the applinks AASA only matches /sign, so
// the probe rides this route). Reaching here at all means the UL resolved — the
// <AccountFileWriter/> observer has already marked it verified; we just show a
// friendly confirmation instead of hunting for a sign-req that doesn't exist.
const UL_SELFTEST_RID = 'ul-selftest';

const V = (event: string, rid: string | undefined, extra = '') =>
  console.log(`VELAB APP ${event} rid=${rid ?? '(none)'} t=${Date.now()}${extra ? ' ' + extra : ''}`);

export default function SignScreen() {
  const { rid } = useLocalSearchParams<{ rid?: string }>();
  const { state, activeAccount } = useWallet();
  const { beginExtensionSign } = useDAppConnection();

  // Signing-ready gate (§12.1.5 cold-start guard): never render the sign against
  // an unready store — queue until the wallet is loaded + logged in.
  const ready = !state.isLoading && state.hasWallet && !!activeAccount;

  const [phase, setPhase] = useState<Phase>('waiting-ready');
  const [outcome, setOutcome] = useState<'submitted' | 'rejected' | 'unknown'>('unknown');
  const [origin, setOrigin] = useState<string | null>(null);
  // The rid we've already begun — so a re-render / ready-flip can't start it twice
  // (the modal instance is reused across deep-links; reset when the rid changes).
  const startedRef = useRef<string | null>(null);

  useEffect(() => {
    if (!rid) {
      setPhase('missing');
      return;
    }
    if (rid === UL_SELFTEST_RID) {
      // UL bootstrap probe — no sign to run; the observer marked attestation.
      setPhase('selftest');
      return;
    }
    if (!ready) {
      setPhase('waiting-ready');
      return; // re-runs when `ready` flips true
    }
    if (startedRef.current === rid) return; // already begun this rid
    startedRef.current = rid;

    V('begin', rid);
    setPhase('connecting');
    const transport = new ExtensionBridgeTransport(rid);
    // 'disconnected' fires AFTER the result file is written (or skipped) — the
    // signal that the sign has settled and the user can return to Safari.
    const unsub = transport.on('disconnected', () => {
      V('settled', rid, `outcome=${transport.outcome}`);
      setOutcome(transport.outcome);
      setPhase('done');
    });
    beginExtensionSign(transport);
    transport
      .connect()
      .then(() => {
        setOrigin(transport.requestOrigin ?? null);
        setPhase('signing'); // the global SigningRequestModal is now rendering
        V('connected', rid, `origin=${transport.requestOrigin}`);
      })
      .catch((e) => {
        V('connect-failed', rid, `err=${String(e)}`);
        setPhase('missing');
      });
    return () => unsub();
  }, [rid, ready, beginExtensionSign]);

  // NOTE: we deliberately do NOT offer a Linking.openURL(origin) "return" button.
  // On iOS openURL loads the URL fresh (new tab / reload) — that WIPES the dApp's
  // page state AND its pending request promise, so the sign result can never reach
  // it (§12.3: iOS can't programmatically re-focus the original tab). The correct,
  // state-preserving return is the OS "‹ Safari" back chip (top-left) or the app
  // switcher — the original tab stays alive and content.js's focus-poll delivers
  // the result to the still-pending promise.

  const statusText =
    phase === 'selftest'
      ? '一键签名已启用（Universal Link 生效）'
      : phase === 'waiting-ready'
        ? 'preparing wallet…'
        : phase === 'connecting'
          ? 'reading request…'
          : phase === 'signing'
            ? 'request received'
            : phase === 'done'
              ? outcome === 'submitted'
                ? 'Signed — return to Safari'
                : outcome === 'rejected'
                  ? 'Rejected — return to Safari'
                  : 'Unfinished — check Vela, return to Safari'
              : 'request not found / expired';

  return (
    <ScrollView contentContainerStyle={{ padding: 24, gap: 12, flexGrow: 1, justifyContent: 'center' }}>
      <Text style={{ fontSize: 20, fontWeight: '700' }}>Vela sign</Text>
      <Text selectable>rid: {rid ?? '(none)'}</Text>
      {/* Machine-readable line for the e2e harness (app_status reads "status:"). */}
      <Text selectable>status: {statusText}</Text>
      {origin && <Text selectable>from {origin}</Text>}

      {phase === 'selftest' && (
        <Text selectable style={{ color: '#16a34a', textAlign: 'center', marginTop: 4 }}>
          ✓ 已确认 Universal Link 关联生效。以后在 Safari 里签名会直接一键打开 Vela（无需「打开
          Vela？」弹窗）。点左上角「‹ Safari」返回即可。
        </Text>
      )}
      {(phase === 'done' || phase === 'missing') && (
        <Text selectable style={{ color: '#6b7280', textAlign: 'center', marginTop: 4 }}>
          点左上角「‹ Safari」返回此页面 — 页面不会刷新，签名结果会自动回传。
        </Text>
      )}
    </ScrollView>
  );
}
