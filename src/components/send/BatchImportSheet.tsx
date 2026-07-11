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
 *
 * Rate invariant: the rate string in the input IS the applied rate — display and
 * conversion never diverge (the old toFixed(2) mirror showed "0" for sub-cent
 * prices while converting at the true value, and a touch then zeroed every row).
 */
import React, { useEffect, useMemo, useState } from 'react';
import { View, Text, ScrollView, Pressable, TextInput } from 'react-native';
import { useTranslation } from 'react-i18next';
import { X, FileUp, Download, Check, ArrowRight, AlertCircle, ChevronRight } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { CurrencySheet } from '@/components/ui/CurrencySheet';
import { Divider } from '@/components/ui/DetailRow';
import { SectionLabel } from '@/components/ui/SectionLabel';
import { SegmentedToggle } from '@/components/ui/SegmentedToggle';
import { VelaButton } from '@/components/ui/VelaButton';
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
type RateStatus = 'loading' | 'ok' | 'failed';

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
  const [templateSaved, setTemplateSaved] = useState(false);
  const [usdFiatRate, setUsdFiatRate] = useState<number | null>(null);
  const [rateStatus, setRateStatus] = useState<RateStatus>('loading');
  const [rateInput, setRateInput] = useState('');
  const [rateEdited, setRateEdited] = useState(false);
  const [rateInputWidth, setRateInputWidth] = useState(0);
  const [showCurrency, setShowCurrency] = useState(false);

  // Reset per-open so the sheet never reopens with a stale paste/rate.
  useEffect(() => {
    if (!visible) return;
    setUnit(priced ? 'fiat' : 'token');
    setFiatCode(currencyCode);
    setRawText('');
    setFileParsed(null);
    setFileName(null);
    setTemplateSaved(false);
    setRateEdited(false);
  }, [visible, priced, currencyCode]);

  // USD→fiat rate for the chosen currency (auto rate). Re-fetch when it changes;
  // status drives the loading/failure hints so a dead "0" is never unexplained.
  useEffect(() => {
    if (!visible) return;
    let cancelled = false;
    setRateStatus('loading');
    getRate(fiatCode)
      .then((r) => { if (!cancelled) { setUsdFiatRate(r); setRateStatus('ok'); } })
      .catch(() => { if (!cancelled) { setUsdFiatRate(null); setRateStatus('failed'); } });
    return () => { cancelled = true; };
  }, [visible, fiatCode]);

  const autoPricePerToken = tokenPriceInFiat(token.priceUsd, usdFiatRate ?? 0); // fiat per 1 token
  // Keep the editable rate field mirroring the auto rate until the user overrides
  // it. Significant-digit formatting: a positive rate never mirrors as "0".
  useEffect(() => {
    if (rateEdited) return;
    setRateInput(autoPricePerToken > 0 ? formatRate(autoPricePerToken) : '');
  }, [autoPricePerToken, rateEdited]);

  // The displayed string is the single source of the applied rate.
  const effPricePerToken = parseFloat(rateInput) || 0;
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

  const onTemplate = async () => {
    hapticLight();
    try {
      await saveTextFile('vela-payroll-template.csv', TEMPLATE_CSV, 'text/csv');
      setTemplateSaved(true);
    } catch {
      // Share sheet dismissed / unavailable — silently keep the plain label.
    }
  };

  const apply = () => {
    const recipients: RecipientDraft[] = capped.map((r) => ({ id: makeRecipientId(), address: r.address, amount: r.tokenAmount, name: r.name }));
    hapticSuccess();
    onApply(recipients);
  };

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.container}>
        <View style={styles.header}>
          <Text style={styles.title}>{t('send.batchTitle', { defaultValue: 'Import recipients' })}</Text>
          <Pressable
            onPress={onClose}
            hitSlop={12}
            testID="batch-close"
            accessibilityRole="button"
            accessibilityLabel={t('send.batchClose', { defaultValue: 'Close' })}
          >
            <X size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </View>

        <ScrollView style={styles.scroll} keyboardShouldPersistTaps="handled" showsVerticalScrollIndicator={false}>
          {/* Amount unit FIRST — it defines what the numbers pasted below mean.
              Fiat works even for an unpriced token — the company can pin its own
              rate (e.g. "1 USDT = 7.2 CNY"), which is the whole payroll point. */}
          <View style={styles.toggleRow}>
            <SegmentedToggle<Unit>
              options={[
                { key: 'fiat', label: t('send.batchUnitFiat', { defaultValue: 'In {{code}}', code: fiatCode }), testID: 'batch-unit-fiat' },
                { key: 'token', label: t('send.batchUnitToken', { defaultValue: 'In {{sym}}', sym: token.symbol }), testID: 'batch-unit-token' },
              ]}
              value={unit}
              onChange={setUnit}
            />
          </View>

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
            <Pressable
              style={styles.sourceBtn}
              onPress={onPickFile}
              disabled={busy}
              testID="batch-file"
              accessibilityRole="button"
              accessibilityLabel={t('send.batchImportFile', { defaultValue: 'Import file' })}
              accessibilityState={{ disabled: busy }}
            >
              <FileUp size={16} color={color.fg.muted} strokeWidth={2} />
              <Text style={styles.sourceBtnText}>{busy ? t('send.batchReading', { defaultValue: 'Reading…' }) : t('send.batchImportFile', { defaultValue: 'Import file' })}</Text>
            </Pressable>
            <Pressable
              style={styles.sourceBtn}
              onPress={onTemplate}
              accessibilityRole="button"
              accessibilityLabel={t('send.batchTemplate', { defaultValue: 'Get template' })}
            >
              {templateSaved
                ? <Check size={16} color={color.fg.muted} strokeWidth={2} />
                : <Download size={16} color={color.fg.muted} strokeWidth={2} />}
              <Text style={styles.sourceBtnText}>
                {templateSaved ? t('send.batchTemplateSaved', { defaultValue: 'Template saved' }) : t('send.batchTemplate', { defaultValue: 'Get template' })}
              </Text>
            </Pressable>
          </View>
          {fileName && <Text style={styles.fileName}>{fileName}</Text>}

          {/* Settlement currency (same picker as the home balance) + editable rate.
              De-containered: SectionLabel + open rows + hairline, no nested cards. */}
          {unit === 'fiat' && (
            <View>
              <SectionLabel>{t('send.batchRateSection', { defaultValue: 'Rate' })}</SectionLabel>
              <Pressable
                style={styles.currencyRow}
                onPress={() => { hapticLight(); setShowCurrency(true); }}
                testID="batch-currency"
                accessibilityRole="button"
                accessibilityLabel={`${t('send.batchCurrencyLabel', { defaultValue: 'Priced in' })}: ${fiatCode}`}
              >
                <Text style={styles.rowLabel}>{t('send.batchCurrencyLabel', { defaultValue: 'Priced in' })}</Text>
                <View style={styles.currencyValue}>
                  <Text style={styles.currencyCode}>{fiatCode}</Text>
                  <ChevronRight size={16} color={color.fg.muted} strokeWidth={2} />
                </View>
              </Pressable>
              <Divider />
              {/* One continuous sentence: "1 USDT = 7.16 CNY", the editable span
                  underlined; "Auto" (reset) pushed to the row end. A hidden mirror
                  sizes the input to its text so the sentence never breaks apart. */}
              <View style={styles.rateRow}>
                <Text style={styles.rowLabel}>{t('send.batchRateLabel', { defaultValue: '1 {{sym}} =', sym: token.symbol })}</Text>
                <Text
                  style={styles.rateMirror}
                  onLayout={(e) => setRateInputWidth(e.nativeEvent.layout.width)}
                  accessibilityElementsHidden
                  importantForAccessibility="no-hide-descendants"
                >
                  {rateInput || '0'}
                </Text>
                <TextInput
                  testID="batch-rate"
                  style={[styles.rateInput, { width: Math.max(28, rateInputWidth + 6) }]}
                  value={rateInput}
                  onChangeText={(v) => { setRateInput(v.replace(/[^0-9.]/g, '')); setRateEdited(true); }}
                  keyboardType="decimal-pad"
                  placeholder="0"
                  placeholderTextColor={color.fg.subtle}
                />
                <Text style={styles.rowLabel}>{fiatCode}</Text>
                {rateEdited && (
                  <Pressable
                    onPress={() => setRateEdited(false)}
                    hitSlop={{ top: 14, bottom: 14, left: 12, right: 12 }}
                    style={styles.rateResetBtn}
                    accessibilityRole="button"
                    accessibilityLabel={t('send.batchRateReset', { defaultValue: 'Auto' })}
                  >
                    <Text style={styles.rateReset}>{t('send.batchRateReset', { defaultValue: 'Auto' })}</Text>
                  </Pressable>
                )}
              </View>
              {!priced && <Text style={styles.hintText}>{t('send.batchNoPrice', { defaultValue: 'No market price — set your own rate above.' })}</Text>}
              {priced && !rateEdited && rateStatus === 'loading' && (
                <Text style={styles.hintText}>{t('send.batchRateLoading', { defaultValue: 'Fetching rate…' })}</Text>
              )}
              {priced && !rateEdited && rateStatus === 'failed' && (
                <Text style={styles.hintText}>{t('send.batchRateFailed', { defaultValue: 'Rate unavailable — enter one manually.' })}</Text>
              )}
            </View>
          )}

          {/* Preview */}
          {preview.length > 0 && (
            <View style={styles.preview}>
              {preview.map((r, i) => (
                <View key={`${r.address}-${i}`} style={[styles.pRow, !r.ok && styles.pRowBad]} testID={r.ok ? 'batch-row-ok' : 'batch-row-bad'}>
                  <View style={styles.pInfo}>
                    <Text style={styles.pName} numberOfLines={1}>{r.name || shortAddr(r.address)}</Text>
                    {/* Second line only when it adds information: a status, or the
                        address under a NAME — never the address twice. */}
                    {(!r.valid || r.dup || !!r.name) && (
                      <Text style={styles.pAddr} numberOfLines={1}>
                        {!r.valid ? t('send.batchBadAddress', { defaultValue: 'Invalid address' }) : r.dup ? t('send.batchDup', { defaultValue: 'Duplicate — skipped' }) : shortAddr(r.address)}
                      </Text>
                    )}
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

          {/* Both notices can be true at once — never hide one behind the other. */}
          {overCap && (
            <View style={styles.noticeRow}>
              <AlertCircle size={13} color={color.warning.base} strokeWidth={2} />
              <Text style={styles.noticeText}>{t('send.batchOverCap', { defaultValue: 'Only the first {{n}} recipients will be sent.', n: maxRecipients })}</Text>
            </View>
          )}
          {rejected > 0 && (
            <View style={styles.noticeRow}>
              <AlertCircle size={13} color={color.warning.base} strokeWidth={2} />
              <Text style={styles.noticeText}>{t('send.batchRejected', { count: rejected, n: rejected })}</Text>
            </View>
          )}
        </ScrollView>

        {/* Summary + apply. Empty state renders NO totals and a count-free CTA —
            never "Import 0 recipients" over a row of zeros. */}
        <View style={styles.footer}>
          {capped.length > 0 && (
            <View style={styles.totalRow}>
              <Text style={styles.totalLabel}>{t('send.recipientCount', { count: capped.length, n: capped.length })}</Text>
              <View style={styles.totalRight}>
                <Text style={[styles.totalToken, overBalance && styles.totalOver]}>{totalTokenHuman} {token.symbol}</Text>
                {totalFiat > 0 && <Text style={styles.totalFiat}>≈ {fiatSymbol}{trimNum(totalFiat)} {fiatCode}</Text>}
              </View>
            </View>
          )}
          {overBalance && <Text style={styles.warnText}>{t('send.batchOverBalance', { defaultValue: 'Total exceeds your {{sym}} balance.', sym: token.symbol })}</Text>}
          <View testID="batch-apply">
            <VelaButton
              title={capped.length > 0
                ? t('send.batchApply', { count: capped.length, n: capped.length })
                : t('send.batchApplyEmpty', { defaultValue: 'Import recipients' })}
              onPress={apply}
              disabled={!canApply}
              variant={canApply ? 'accent' : 'secondary'}
              // Disabled = quiet sunken slab (secondary ink), not a washed-out accent.
              style={!canApply ? styles.applyDisabled : undefined}
            />
          </View>
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

/** Trim a float to a compact, trailing-zero-free string (fiat totals only). */
function trimNum(n: number): string {
  if (!Number.isFinite(n)) return '0';
  return n.toFixed(2).replace(/\.?0+$/, '');
}

const RATE_SIG_DIGITS = 4;
/**
 * Rate → plain-decimal string with 4 significant digits, trailing zeros trimmed.
 * Never returns "0" for a positive rate (that string, once touched, zeroed every
 * row via parseFloat) — and the returned string IS the applied rate, so what the
 * user reads is exactly what the conversion uses.
 */
function formatRate(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return '';
  const exp = Math.floor(Math.log10(n));
  const decimals = Math.min(Math.max(RATE_SIG_DIGITS - 1 - exp, 0), 18);
  const fixed = n.toFixed(decimals);
  const s = fixed.includes('.') ? fixed.replace(/0+$/, '').replace(/\.$/, '') : fixed;
  return parseFloat(s) > 0 ? s : '';
}

const styles = createStyles(() => ({
  container: { paddingHorizontal: space['2xl'], paddingTop: space.lg, flex: 1 },
  header: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: space.lg },
  title: { fontSize: text['2xl'], ...inter.bold, color: color.fg.base },
  scroll: { flex: 1 },

  toggleRow: { flexDirection: 'row', marginBottom: space.md },
  paste: {
    minHeight: 84, maxHeight: 140, backgroundColor: color.bg.sunken, borderRadius: radius.xl,
    padding: space.lg, fontSize: text.sm, fontFamily: font.mono, color: color.fg.base,
    textAlignVertical: 'top', outlineStyle: 'none',
  } as any,
  sourceRow: { flexDirection: 'row', gap: space['2xl'], marginTop: space.xs },
  // Plain text-buttons — no card/border boxes (design language: light controls).
  sourceBtn: { flexDirection: 'row', alignItems: 'center', gap: space.sm, minHeight: 44, paddingHorizontal: space.xs },
  sourceBtnText: { fontSize: text.sm, ...inter.semibold, color: color.fg.base },
  fileName: { fontSize: text.xs, ...inter.regular, color: color.fg.muted, marginTop: space.sm, marginLeft: space.xs, fontFamily: font.mono },

  currencyRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', minHeight: 44 },
  rowLabel: { fontSize: text.base, ...inter.medium, color: color.fg.muted },
  currencyValue: { flexDirection: 'row', alignItems: 'center', gap: space.xs },
  currencyCode: { fontSize: text.base, ...inter.semibold, color: color.fg.base },

  rateRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm, minHeight: 44 },
  // Hidden width-mirror for the content-sized rate input; must match rateInput's font.
  rateMirror: { position: 'absolute', opacity: 0, pointerEvents: 'none', fontSize: text.base, ...inter.semibold } as any,
  rateInput: {
    fontSize: text.base, ...inter.semibold, color: color.fg.base, paddingVertical: space.xs,
    borderBottomWidth: 1, borderBottomColor: color.border.strong, textAlign: 'center', outlineStyle: 'none',
  } as any,
  rateResetBtn: { marginLeft: 'auto' },
  rateReset: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  hintText: { fontSize: text.xs, ...inter.regular, color: color.fg.muted, marginTop: space.sm },

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

  applyDisabled: { backgroundColor: color.bg.sunken, borderWidth: 0 },
}));
