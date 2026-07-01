/**
 * Receipt / batch-send harness (dev-only).
 *
 * Renders the REAL <TransactionReceipt> + <TransactionDetailSheet> with mock props
 * so the batch-send fixes can be verified WITHOUT passkey login, a wallet, or a
 * live bundler. Reachable at /receipt-harness under __DEV__ or the `dev_unlocked`
 * flag (same gate as the clear-signing test route).
 *
 * Covers:
 *   #1  batch breakdown in the detail sheet (split / multiSelect)
 *   #2  token logos in the shared canvas image (multiSelect rows) — "Preview
 *       share image" renders the exact PNG the Share button produces
 *   #3  status stamp transitions (submitted → confirmed → failed), incl. the
 *       "Simulate confirm" button that flips the status live
 */
import React, { useState } from 'react';
import { Image, Platform, Pressable, ScrollView, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { useRouter } from 'expo-router';

import { TransactionReceipt, renderReceiptToCanvas, buildCanvasLabels, type ReceiptTransfer } from '@/components/ui/TransactionReceipt';
import { TransactionDetailSheet } from '@/components/ui/TransactionDetailSheet';
import { tokenLogoURLsByAddress } from '@/models/types';
import type { ActivityBatch } from '@/services/activity';
import type { Currency } from '@/services/currency';
import { color, createStyles, inter, radius, space, text } from '@/constants/theme';

// BNB Chain (56) — real token addresses so logo URLs resolve exactly like prod.
const CHAIN_ID = 56;
const USDT = '0x55d398326f99059fF775485246999027B3197955';
const USDC = '0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d';
const usdtLogos = tokenLogoURLsByAddress(CHAIN_ID, USDT);
const usdcLogos = tokenLogoURLsByAddress(CHAIN_ID, USDC);

const FROM = '0x14fB1f3d2b7c9a1E5f4c0a8B2d6E9f0a1D1eA5c0';
const RECIPIENT = '0x600746aC1234567890abcDef1234567890f495f4';
const FAKE_HASH = '0xd41d8cd98f00b204e9800998ecf8427e1234567890abcdef1234567890a01243';
const FAKE_UOP = '0xa11ce0000000000000000000000000000000000000000000000000000000dead';

type Scenario = 'single' | 'split' | 'multiSelect';
type Status = 'submitted' | 'confirmed' | 'failed';

const USD: Currency = { code: 'USD', symbol: '$', name: 'US Dollar' };
const nowSec = Math.floor(Date.now() / 1000);

/** Per-scenario receipt breakdown (null for a plain single send). */
function transfersFor(scenario: Scenario): ReceiptTransfer[] | null {
  if (scenario === 'multiSelect') {
    return [
      { to: RECIPIENT, toName: 'Samuel', amount: '6', symbol: 'USDC', logoUrls: usdcLogos, usdValue: 6 },
      { to: RECIPIENT, toName: 'Samuel', amount: '5.7249', symbol: 'USDT', logoUrls: usdtLogos, usdValue: 5.72 },
    ];
  }
  if (scenario === 'split') {
    return [
      { to: '0xAAaA000000000000000000000000000000000001', toName: 'Alice', amount: '10', symbol: 'USDC', logoUrls: usdcLogos, usdValue: 10 },
      { to: '0xBBbB000000000000000000000000000000000002', toName: 'Bob', amount: '10', symbol: 'USDC', logoUrls: usdcLogos, usdValue: 10 },
      { to: RECIPIENT, toName: 'Samuel', amount: '10', symbol: 'USDC', logoUrls: usdcLogos, usdValue: 10 },
    ];
  }
  return null;
}

/** ActivityBatch for the detail sheet (mirrors what loadActivityItems builds). */
function batchFor(scenario: Scenario, status: Status, txHash: string): ActivityBatch | null {
  const transfers = transfersFor(scenario);
  if (!transfers) return null;
  const isSplit = scenario === 'split';
  return {
    kind: isSplit ? 'split' : 'multiSelect',
    count: transfers.length,
    totalUsd: transfers.reduce((s, x) => s + (x.usdValue ?? 0), 0),
    transfers: transfers.map((tr) => ({ to: tr.to, toName: tr.toName ?? undefined, value: tr.amount, symbol: tr.symbol, decimals: 18, usdValue: tr.usdValue ?? 0, logoUrls: tr.logoUrls })),
    ids: transfers.map((_, i) => `${FAKE_UOP}-${i}`),
    from: FROM,
    chainId: CHAIN_ID,
    timestamp: nowSec,
    status: status === 'confirmed' ? 'confirmed' : status === 'failed' ? 'failed' : 'pending',
    txHash,
    userOpHash: FAKE_UOP,
    symbol: isSplit ? 'USDC' : undefined,
    logoUrls: isSplit ? usdcLogos : undefined,
    to: isSplit ? undefined : RECIPIENT,
    toName: isSplit ? undefined : 'Samuel',
  };
}

function Chip({ label, active, onPress }: { label: string; active: boolean; onPress: () => void }) {
  return (
    <Pressable onPress={onPress} style={[styles.chip, active && styles.chipActive]}>
      <Text style={[styles.chipText, active && styles.chipTextActive]}>{label}</Text>
    </Pressable>
  );
}

export default function ReceiptHarnessScreen() {
  const { t } = useTranslation();
  const router = useRouter();
  const [scenario, setScenario] = useState<Scenario>('multiSelect');
  const [status, setStatus] = useState<Status>('submitted');
  const [preview, setPreview] = useState<string | null>(null);
  const [showDetail, setShowDetail] = useState(false);

  const transfers = transfersFor(scenario);
  const txHash = status === 'confirmed' ? FAKE_HASH : '';

  // Flip submitted → confirmed after a beat, so the status stamp / QR / hash row
  // can be seen updating live (the user-visible outcome of the self-poll).
  const simulateConfirm = () => {
    setStatus('submitted');
    setTimeout(() => setStatus('confirmed'), 2500);
  };

  const renderPreview = async () => {
    if (Platform.OS !== 'web') return;
    const props = {
      from: FROM, fromName: '大表哥', to: RECIPIENT, toName: 'Samuel',
      amount: '5.7249', symbol: 'USDT', chainId: CHAIN_ID, txHash: txHash || FAKE_HASH,
      logoUrls: usdtLogos, usdValue: 5.72, rate: 1, currencyCode: 'USD', currencySymbol: '$',
      timestamp: new Date(nowSec * 1000), transfers: transfers ?? undefined,
      batchKind: (scenario === 'single' ? undefined : scenario) as 'split' | 'multiSelect' | undefined,
      onDone: () => {},
    };
    try {
      const blob = await renderReceiptToCanvas(props, buildCanvasLabels(t, transfers?.length ?? 0));
      setPreview((prev) => { if (prev) URL.revokeObjectURL(prev); return URL.createObjectURL(blob); });
    } catch (e) {
      console.warn('[harness] canvas render failed', e);
    }
  };

  return (
    <View style={styles.root}>
      <View style={styles.bar}>
        <Pressable onPress={() => router.back()} hitSlop={8}><Text style={styles.back}>‹ Back</Text></Pressable>
        <Text style={styles.title}>Receipt harness</Text>
        <View style={{ width: 44 }} />
      </View>

      <ScrollView contentContainerStyle={styles.controls} horizontal={false}>
        <Text style={styles.group}>Scenario</Text>
        <View style={styles.row}>
          <Chip label="Single" active={scenario === 'single'} onPress={() => setScenario('single')} />
          <Chip label="Split (1→N)" active={scenario === 'split'} onPress={() => setScenario('split')} />
          <Chip label="MultiSelect (N→1)" active={scenario === 'multiSelect'} onPress={() => setScenario('multiSelect')} />
        </View>
        <Text style={styles.group}>Status</Text>
        <View style={styles.row}>
          <Chip label="Submitted" active={status === 'submitted'} onPress={() => setStatus('submitted')} />
          <Chip label="Confirmed" active={status === 'confirmed'} onPress={() => setStatus('confirmed')} />
          <Chip label="Failed" active={status === 'failed'} onPress={() => setStatus('failed')} />
        </View>
        <View style={styles.row}>
          <Chip label="▶ Simulate confirm (2.5s)" active={false} onPress={simulateConfirm} />
          {transfers ? <Chip label="Open detail sheet (#1)" active={false} onPress={() => setShowDetail(true)} /> : null}
          {Platform.OS === 'web' ? <Chip label="Preview share image (#2)" active={false} onPress={renderPreview} /> : null}
        </View>
        {preview ? (
          <View style={styles.previewWrap}>
            <Text style={styles.group}>Share image (canvas) — check per-token logos</Text>
            <Image source={{ uri: preview }} style={styles.previewImg} resizeMode="contain" />
          </View>
        ) : null}
      </ScrollView>

      <View style={styles.receiptWrap}>
        <TransactionReceipt
          key={`${scenario}-${status}`}
          from={FROM}
          fromName="大表哥"
          to={RECIPIENT}
          toName="Samuel"
          amount="5.7249"
          symbol="USDT"
          chainId={CHAIN_ID}
          txHash={txHash}
          logoUrls={usdtLogos}
          usdValue={5.72}
          rate={1}
          currencyCode="USD"
          currencySymbol="$"
          timestamp={new Date(nowSec * 1000)}
          transfers={transfers ?? undefined}
          batchKind={scenario === 'single' ? undefined : scenario}
          status={status}
          onDone={() => router.back()}
        />
      </View>

      <TransactionDetailSheet
        visible={showDetail}
        tx={null}
        batch={batchFor(scenario, status, txHash)}
        rate={1}
        currency={USD}
        onClose={() => setShowDetail(false)}
      />
    </View>
  );
}

const styles = createStyles(() => ({
  root: { flex: 1, backgroundColor: color.bg.base },
  bar: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', paddingHorizontal: space.lg, paddingTop: space['2xl'], paddingBottom: space.sm },
  back: { fontSize: text.base, ...inter.semibold, color: color.accent.base, width: 44 },
  title: { fontSize: text.lg, ...inter.bold, color: color.fg.base },
  controls: { paddingHorizontal: space.lg, gap: space.sm, paddingBottom: space.md },
  group: { fontSize: text.xs, ...inter.semibold, color: color.fg.muted, marginTop: space.sm, textTransform: 'uppercase' as const, letterSpacing: 0.5 },
  row: { flexDirection: 'row', flexWrap: 'wrap', gap: space.sm },
  chip: { paddingHorizontal: space.md, paddingVertical: space.sm, borderRadius: radius.full, backgroundColor: color.bg.raised, borderWidth: 1, borderColor: color.border.base },
  chipActive: { backgroundColor: color.accent.soft, borderColor: color.accent.base },
  chipText: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  chipTextActive: { color: color.accent.base },
  previewWrap: { marginTop: space.md, gap: space.sm },
  previewImg: { width: '100%', height: 520, backgroundColor: color.bg.sunken, borderRadius: radius.lg },
  receiptWrap: { flex: 1, borderTopWidth: 1, borderTopColor: color.border.base },
}));
