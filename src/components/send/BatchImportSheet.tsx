/**
 * BatchImportSheet — the payroll batch importer (feature: 表格批量发薪).
 *
 * A user pastes or uploads a table of `(address, amount)` rows; the amount is a
 * fiat figure (the display currency, e.g. CNY) that we convert to a token amount
 * (e.g. USDT) at a shown, editable exchange rate — the exact "priced in RMB, paid
 * in USDT" flow. On apply it hands back `RecipientDraft[]` (token amounts) that
 * SendScreen drops into the existing split editor, so submission is the ordinary
 * single-UserOp `buildSplitCalls → sendBatchCalls` path — nothing new to sign.
 *
 * The heavy Excel parser (SheetJS) is only reached through file-io/recipient-table
 * lazy imports; the paste path is pure text and is what the E2E drives.
 */
import React, { useEffect, useMemo, useState } from 'react';
import { View, Text, ScrollView, Pressable, TextInput } from 'react-native';
import { useTranslation } from 'react-i18next';
import { X, FileUp, Download, ArrowRight, AlertCircle, ChevronDown } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { CurrencySheet } from '@/components/ui/CurrencySheet';
import { color, text, inter, space, radius, font, createStyles } from '@/constants/theme';
import { type APIToken, isAddress, shortAddr } from '@/models/types';
import { toBaseUnits, fromBaseUnits } from '@/services/eip681';
import { fiatToTokenAmount, tokenPriceInFiat } from '@/services/fiat-convert';
import { parseRecipientTableText, parseRecipientTable, type ParseResult } from '@/services/recipient-table';
import { getRate } from '@/services/currency';
import { currencyMeta } from '@/services/currency-catalog';
import { pickTable, saveTextFile } from '@/services/file-io';
import { makeRecipientId, type RecipientDraft } from '@/components/send/MultiRecipientEditor';
import { formatTokenAmount } from '@/services/locale-format';
import { showAlert, hapticSuccess, hapticLight } from '@/services/platform';

const TEMPLATE_CSV =
  'name,address,amount\n' +
  'Alice,0x1111111111111111111111111111111111111111,5000\n' +
  'Bob,0x2222222222222222222222222222222222222222,8000\n' +
  'Carol,0x3333333333333333333333333333333333333333,6500\n';

type Unit = 'fiat' | 'token';

interface Props {
  visible: boolean;
  onClose: () => void;
  token: APIToken;
  /** Display-currency code + symbol (the default fiat the amounts are read as). */
  currencyCode: string;
  currencySymbol: string;
  onApply: (recipients: RecipientDraft[]) => void;
  maxRecipients: number;
}

export function BatchImportSheet({ visible, onClose, token, currencyCode, currencySymbol, onApply, maxRecipients }: Props) {
  const { t } = useTranslation();
  const priced = !!token.priceUsd && token.priceUsd > 0;

  const [unit, setUnit] = useState<Unit>(priced ? 'fiat' : 'token');
  const [fiatCode, setFiatCode] = useState(currencyCode);
  const [rawText, setRawText] = useState('');
  const [fileParsed, setFileParsed] = useState<ParseResult | null>(null);
  const [fileName, setFileName] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [usdFiatRate, setUsdFiatRate] = useState<number | null>(null);
  const [rateInput, setRateInput] = useState('');
  const [rateEdited, setRateEdited] = useState(false);
  const [showCurrency, setShowCurrency] = useState(false);

  // Reset per-open so the sheet never reopens with a stale paste/rate.
  useEffect(() => {
    if (!visible) return;
    setUnit(priced ? 'fiat' : 'token');
    setFiatCode(currencyCode);
    setRawText('');
    setFileParsed(null);
    setFileName(null);
    setRateEdited(false);
  }, [visible, priced, currencyCode]);

  // USD→fiat rate for the chosen currency (auto rate). Re-fetch when it changes.
  useEffect(() => {
    if (!visible) return;
    let cancelled = false;
    getRate(fiatCode)
      .then((r) => { if (!cancelled) setUsdFiatRate(r); })
      .catch(() => { if (!cancelled) setUsdFiatRate(null); });
    return () => { cancelled = true; };
  }, [visible, fiatCode]);

  const autoPricePerToken = tokenPriceInFiat(token.priceUsd, usdFiatRate ?? 0); // fiat per 1 token
  // Keep the editable rate field mirroring the auto rate until the user overrides it.
  useEffect(() => {
    if (!rateEdited && autoPricePerToken > 0) setRateInput(trimNum(autoPricePerToken));
  }, [autoPricePerToken, rateEdited]);

  const effPricePerToken = rateEdited ? parseFloat(rateInput) || 0 : autoPricePerToken;
  const fiatSymbol = currencyMeta(fiatCode).symbol || currencySymbol;

  const parsed: ParseResult = useMemo(
    () => fileParsed ?? parseRecipientTableText(rawText),
    [fileParsed, rawText],
  );

  // Build the preview: validate, de-dupe by address, and convert fiat→token.
  const preview = useMemo(() => {
    const seen = new Set<string>();
    return parsed.rows.map((r) => {
      const address = r.address.trim();
      const valid = isAddress(address);
      const low = address.toLowerCase();
      const dup = valid && seen.has(low);
      if (valid) seen.add(low);
      const fiatNum = parseFloat(r.rawAmount) || 0;
      const tokenAmount =
        unit === 'fiat'
          ? effPricePerToken > 0
            ? fiatToTokenAmount(fiatNum, effPricePerToken, token.decimals)
            : ''
          : r.rawAmount;
      const ok = valid && !dup && parseFloat(tokenAmount) > 0;
      return { line: r.line, name: r.name, address, valid, dup, fiatNum, tokenAmount, ok };
    });
  }, [parsed, unit, effPricePerToken, token.decimals]);

  const okRows = preview.filter((r) => r.ok);
  const capped = okRows.slice(0, maxRecipients);
  const overCap = okRows.length > maxRecipients;

  const totalTokenBase = capped.reduce((s, r) => s + toBaseUnits(r.tokenAmount, token.decimals), 0n);
  const totalTokenHuman = fromBaseUnits(totalTokenBase, token.decimals);
  const totalFiat = unit === 'fiat' ? capped.reduce((s, r) => s + r.fiatNum, 0) : 0;
  const balBase = toBaseUnits(token.balance || '0', token.decimals);
  const overBalance = totalTokenBase > balBase;

  const rejected = preview.length - okRows.length + parsed.errors.length;
  const canApply = capped.length > 0 && !overBalance && (unit === 'token' || effPricePerToken > 0);

  const onPickFile = async () => {
    setBusy(true);
    try {
      const picked = await pickTable();
      if (!picked) return;
      setFileName(picked.name);
      if (picked.text != null) {
        setRawText(picked.text);
        setFileParsed(null);
      } else if (picked.bytes) {
        setRawText('');
        setFileParsed(await parseRecipientTable(picked.bytes, picked.name));
      }
    } catch {
      showAlert(t('send.batchImportFailedTitle', { defaultValue: 'Could not read file' }), t('send.batchImportFailedBody', { defaultValue: 'Please use a CSV, TSV, TXT, or Excel file.' }));
    } finally {
      setBusy(false);
    }
  };

  const apply = () => {
    const recipients: RecipientDraft[] = capped.map((r) => ({ id: makeRecipientId(), address: r.address, amount: r.tokenAmount }));
    hapticSuccess();
    onApply(recipients);
  };

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.container}>
        <View style={styles.header}>
          <Text style={styles.title}>{t('send.batchTitle', { defaultValue: 'Import recipients' })}</Text>
          <Pressable onPress={onClose} hitSlop={8} testID="batch-close">
            <X size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </View>

        <ScrollView style={styles.scroll} keyboardShouldPersistTaps="handled" showsVerticalScrollIndicator={false}>
          {/* Source: paste or file */}
          <TextInput
            testID="batch-paste"
            style={styles.paste}
            value={rawText}
            onChangeText={(v) => { setRawText(v); setFileParsed(null); setFileName(null); }}
            placeholder={t('send.batchPastePlaceholder', { defaultValue: '0xabc… , 5000\n0xdef… , 8000' })}
            placeholderTextColor={color.fg.subtle}
            multiline
            autoCapitalize="none"
            autoCorrect={false}
          />
          <View style={styles.sourceRow}>
            <Pressable style={styles.sourceBtn} onPress={onPickFile} disabled={busy} testID="batch-file">
              <FileUp size={16} color={color.accent.base} strokeWidth={2} />
              <Text style={styles.sourceBtnText}>{busy ? t('send.batchReading', { defaultValue: 'Reading…' }) : t('send.batchImportFile', { defaultValue: 'Import file' })}</Text>
            </Pressable>
            <Pressable style={styles.sourceBtn} onPress={() => saveTextFile('vela-payroll-template.csv', TEMPLATE_CSV, 'text/csv')}>
              <Download size={16} color={color.fg.muted} strokeWidth={2} />
              <Text style={[styles.sourceBtnText, { color: color.fg.muted }]}>{t('send.batchTemplate', { defaultValue: 'Template' })}</Text>
            </Pressable>
          </View>
          {fileName && <Text style={styles.fileName}>{fileName}</Text>}

          {/* Amount unit: fiat (converted at the shown/edited rate) vs raw token.
              Fiat works even for an unpriced token — the company can pin its own
              rate (e.g. "1 USDT = 7.2 CNY"), which is the whole payroll point. */}
          <View style={styles.segmentRow}>
            <Pressable
              style={[styles.segment, unit === 'fiat' && styles.segmentActive]}
              onPress={() => setUnit('fiat')}
              testID="batch-unit-fiat"
            >
              <Text style={[styles.segmentText, unit === 'fiat' && styles.segmentTextActive]}>{t('send.batchModeFiat', { defaultValue: 'By fiat value' })}</Text>
            </Pressable>
            <Pressable style={[styles.segment, unit === 'token' && styles.segmentActive]} onPress={() => setUnit('token')} testID="batch-unit-token">
              <Text style={[styles.segmentText, unit === 'token' && styles.segmentTextActive]}>{t('send.batchModeToken', { defaultValue: 'By {{sym}} amount', sym: token.symbol })}</Text>
            </Pressable>
          </View>

          {/* Settlement currency (same picker as the home balance) + editable rate */}
          {unit === 'fiat' && (
            <View style={styles.rateCard}>
              <Pressable style={styles.currencyRow} onPress={() => { hapticLight(); setShowCurrency(true); }} testID="batch-currency">
                <Text style={styles.currencyLabel}>{t('send.batchCurrencyLabel', { defaultValue: 'Priced in' })}</Text>
                <View style={styles.currencyPill}>
                  <View style={styles.currencySym}><Text style={styles.currencySymText}>{fiatSymbol}</Text></View>
                  <Text style={styles.currencyCode}>{fiatCode}</Text>
                  <ChevronDown size={15} color={color.fg.muted} strokeWidth={2.4} />
                </View>
              </Pressable>
              <View style={styles.divider} />
              <View style={styles.rateRow}>
                <Text style={styles.rateLabel}>{t('send.batchRateLabel', { defaultValue: '1 {{sym}} =', sym: token.symbol })}</Text>
                <TextInput
                  testID="batch-rate"
                  style={styles.rateInput}
                  value={rateInput}
                  onChangeText={(v) => { setRateInput(v.replace(/[^0-9.]/g, '')); setRateEdited(true); }}
                  keyboardType="decimal-pad"
                  placeholder="0"
                  placeholderTextColor={color.fg.subtle}
                />
                <Text style={styles.rateCode}>{fiatCode}</Text>
                {rateEdited && (
                  <Pressable onPress={() => setRateEdited(false)} hitSlop={6}>
                    <Text style={styles.rateReset}>{t('send.batchRateReset', { defaultValue: 'Auto' })}</Text>
                  </Pressable>
                )}
              </View>
              {!priced && <Text style={styles.hintText}>{t('send.batchNoPrice', { defaultValue: 'No market price — set your own rate above.' })}</Text>}
            </View>
          )}

          {/* Preview */}
          {preview.length > 0 && (
            <View style={styles.preview}>
              {preview.map((r, i) => (
                <View key={`${r.address}-${i}`} style={[styles.pRow, !r.ok && styles.pRowBad]} testID={r.ok ? 'batch-row-ok' : 'batch-row-bad'}>
                  <View style={styles.pInfo}>
                    <Text style={styles.pName} numberOfLines={1}>{r.name || shortAddr(r.address)}</Text>
                    <Text style={styles.pAddr} numberOfLines={1}>
                      {!r.valid ? t('send.batchBadAddress', { defaultValue: 'Invalid address' }) : r.dup ? t('send.batchDup', { defaultValue: 'Duplicate — skipped' }) : shortAddr(r.address)}
                    </Text>
                  </View>
                  <View style={styles.pAmt}>
                    {unit === 'fiat' && <Text style={styles.pFiat}>{fiatSymbol}{trimNum(r.fiatNum)}</Text>}
                    {r.ok ? (
                      <View style={styles.pTokenRow}>
                        <ArrowRight size={11} color={color.fg.subtle} strokeWidth={2} />
                        <Text style={styles.pToken}>{formatTokenAmount(parseFloat(r.tokenAmount))} {token.symbol}</Text>
                      </View>
                    ) : (
                      <Text style={styles.pSkip}>—</Text>
                    )}
                  </View>
                </View>
              ))}
            </View>
          )}

          {(rejected > 0 || overCap) && (
            <View style={styles.noticeRow}>
              <AlertCircle size={13} color={color.warning.base} strokeWidth={2} />
              <Text style={styles.noticeText}>
                {overCap
                  ? t('send.batchOverCap', { defaultValue: 'Only the first {{n}} recipients will be sent.', n: maxRecipients })
                  : t('send.batchRejected', { defaultValue: '{{n}} row(s) skipped (invalid or duplicate).', n: rejected })}
              </Text>
            </View>
          )}
        </ScrollView>

        {/* Summary + apply */}
        <View style={styles.footer}>
          <View style={styles.totalRow}>
            <Text style={styles.totalLabel}>{t('send.recipientCount', { n: capped.length, defaultValue: `${capped.length} recipients` })}</Text>
            <View style={styles.totalRight}>
              <Text style={[styles.totalToken, overBalance && styles.totalOver]}>{totalTokenHuman} {token.symbol}</Text>
              {totalFiat > 0 && <Text style={styles.totalFiat}>≈ {fiatSymbol}{trimNum(totalFiat)} {fiatCode}</Text>}
            </View>
          </View>
          {overBalance && <Text style={styles.warnText}>{t('send.alertInsufficientBalanceTitle')}</Text>}
          <Pressable
            style={[styles.applyBtn, !canApply && styles.applyBtnDisabled]}
            onPress={apply}
            disabled={!canApply}
            testID="batch-apply"
          >
            <Text style={styles.applyText}>{t('send.batchApply', { defaultValue: 'Import {{n}} recipients', n: capped.length })}</Text>
          </Pressable>
        </View>
      </View>

      {/* Same searchable, provider-driven currency list as the home balance. */}
      <CurrencySheet
        visible={showCurrency}
        selected={fiatCode}
        onSelect={(code) => { setFiatCode(code); setRateEdited(false); }}
        onClose={() => setShowCurrency(false)}
      />
    </AppModal>
  );
}

/** Trim a float to a compact, trailing-zero-free string. */
function trimNum(n: number): string {
  if (!Number.isFinite(n)) return '0';
  return n.toFixed(2).replace(/\.?0+$/, '');
}

const styles = createStyles(() => ({
  container: { paddingHorizontal: space['2xl'], paddingTop: space.lg, flex: 1 },
  header: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: space.lg },
  title: { fontSize: text['2xl'], ...inter.bold, color: color.fg.base },
  scroll: { flex: 1 },

  paste: {
    minHeight: 84, maxHeight: 140, backgroundColor: color.bg.sunken, borderRadius: radius.xl,
    padding: space.lg, fontSize: text.sm, fontFamily: font.mono, color: color.fg.base,
    textAlignVertical: 'top', outlineStyle: 'none',
  } as any,
  sourceRow: { flexDirection: 'row', gap: space.md, marginTop: space.md },
  sourceBtn: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm, paddingVertical: space.md,
    paddingHorizontal: space.lg, borderRadius: radius.lg, backgroundColor: color.bg.raised,
    borderWidth: 1, borderColor: color.border.base,
  },
  sourceBtnText: { fontSize: text.sm, ...inter.semibold, color: color.accent.base },
  fileName: { fontSize: text.xs, ...inter.regular, color: color.fg.muted, marginTop: space.sm, marginLeft: space.xs, fontFamily: font.mono },

  segmentRow: { flexDirection: 'row', gap: space.xs, backgroundColor: color.bg.sunken, borderRadius: radius.xl, padding: 3, marginTop: space.lg },
  segment: { flex: 1, alignItems: 'center', justifyContent: 'center', paddingVertical: space.md, borderRadius: radius.lg },
  segmentActive: { backgroundColor: color.bg.raised },
  segmentText: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  segmentTextActive: { color: color.fg.base },

  rateCard: { backgroundColor: color.bg.sunken, borderRadius: radius.xl, padding: space.lg, marginTop: space.md, gap: space.md },
  currencyRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' },
  currencyLabel: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  currencyPill: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    backgroundColor: color.bg.raised, borderRadius: radius.full,
    paddingVertical: space.xs, paddingHorizontal: space.md,
  },
  currencySym: {
    width: 22, height: 22, borderRadius: 11, backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
  },
  currencySymText: { fontSize: text.xs, ...inter.bold, color: color.fg.base },
  currencyCode: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  divider: { height: 1, backgroundColor: color.border.base },
  rateRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  rateLabel: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  rateInput: { flex: 1, fontSize: text.lg, ...inter.bold, color: color.fg.base, paddingVertical: space.xs, outlineStyle: 'none' } as any,
  rateCode: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  rateReset: { fontSize: text.xs, ...inter.semibold, color: color.accent.base, marginLeft: space.sm },

  preview: { marginTop: space.lg, gap: 2 },
  pRow: { flexDirection: 'row', alignItems: 'center', gap: space.md, paddingVertical: space.sm, paddingHorizontal: space.sm, borderRadius: radius.md },
  pRowBad: { opacity: 0.5 },
  pInfo: { flex: 1, gap: 1 },
  pName: { fontSize: text.sm, ...inter.semibold, color: color.fg.base },
  pAddr: { fontSize: text.xs, fontFamily: font.mono, color: color.fg.muted },
  pAmt: { alignItems: 'flex-end', gap: 1 },
  pFiat: { fontSize: text.xs, ...inter.regular, color: color.fg.muted },
  pTokenRow: { flexDirection: 'row', alignItems: 'center', gap: 3 },
  pToken: { fontSize: text.sm, ...inter.semibold, color: color.fg.base },
  pSkip: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle },

  noticeRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm, marginTop: space.md, paddingHorizontal: space.sm },
  noticeText: { flex: 1, fontSize: text.xs, ...inter.medium, color: color.warning.base },

  footer: { paddingTop: space.md, gap: space.sm, borderTopWidth: 1, borderTopColor: color.border.base },
  totalRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', paddingHorizontal: space.sm },
  totalLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  totalRight: { alignItems: 'flex-end' },
  totalToken: { fontSize: text.lg, ...inter.bold, color: color.fg.base },
  totalOver: { color: color.error.base },
  totalFiat: { fontSize: text.xs, ...inter.regular, color: color.fg.muted },
  warnText: { fontSize: text.xs, ...inter.medium, color: color.error.base, paddingHorizontal: space.sm },
  hintText: { fontSize: text.xs, ...inter.regular, color: color.fg.muted },

  applyBtn: { backgroundColor: color.accent.base, borderRadius: radius.xl, paddingVertical: space.lg, alignItems: 'center', marginTop: space.xs },
  applyBtnDisabled: { opacity: 0.4 },
  applyText: { fontSize: text.lg, ...inter.bold, color: color.fg.inverse },
}));
