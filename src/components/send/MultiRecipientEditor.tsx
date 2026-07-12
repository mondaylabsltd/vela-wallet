/**
 * MultiRecipientEditor — the ① "一币多人" (split) editor: send ONE token to many
 * recipients, each with its own amount. Self-contained and presentational — the
 * host (SendScreen) owns the recipient list, the contact picker, and submission
 * (one UserOp via `sendBatchCalls`, built from `buildSplitCalls`).
 *
 * Designed to mount only in multi-recipient mode; single sends keep their
 * existing big-amount-hero UI untouched. Each row is uniform (address + amount);
 * the remove control hides when only one recipient remains. Totals and the
 * over-balance check run in base units via the batch-send helpers so they never
 * drift from what actually gets submitted.
 */
import React from 'react';
import { View, Text, Pressable, TextInput } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Plus, X, BookUser, FileUp } from 'lucide-react-native';
import { AutoGrowTextInput } from '@/components/ui/AutoGrowTextInput';
import { RecipientTrust } from '@/components/contacts/RecipientTrust';
import { color, text, inter, space, radius, createStyles } from '@/constants/theme';
import { sumSplitBaseUnits } from '@/services/batch-send';
import { toBaseUnits, fromBaseUnits } from '@/services/eip681';
import { useLocalePrefs, numberSeparators, parseLocaleNumber } from '@/services/locale-format';

export interface RecipientDraft {
  id: string;
  address: string;
  amount: string; // human decimal string
  /** Optional label carried from the payroll importer's name column, so a split
   *  send records the recipient's name in the address book (not just the address). */
  name?: string;
}

const ADDR_RE = /^0x[0-9a-fA-F]{40}$/;

// Monotonic, collision-free ids for recipient rows (host seeds the first row
// with this too). No Date.now/random needed — a process counter is enough.
let _seq = 0;
export function makeRecipientId(): string {
  _seq += 1;
  return `rcpt_${_seq}`;
}

/** Clamp free text to a valid token amount (digits + a single dot, capped decimals). */
function sanitizeAmount(input: string, maxDecimals: number): string {
  const cleaned = input.replace(/[^0-9.]/g, '');
  const firstDot = cleaned.indexOf('.');
  const single = firstDot === -1 ? cleaned : cleaned.slice(0, firstDot + 1) + cleaned.slice(firstDot + 1).replace(/\./g, '');
  const [intPart, frac] = single.split('.');
  if (frac !== undefined && frac.length > maxDecimals) return `${intPart}.${frac.slice(0, maxDecimals)}`;
  return single;
}

/** Every row has a valid address and a positive amount, and there's at least one. */
export function recipientsAreValid(recipients: RecipientDraft[]): boolean {
  return recipients.length > 0 && recipients.every(
    (r) => ADDR_RE.test(r.address.trim()) && parseFloat(r.amount) > 0,
  );
}

interface Props {
  recipients: RecipientDraft[];
  onChange: (next: RecipientDraft[]) => void;
  tokenSymbol: string;
  decimals: number;
  priceUsd?: number | null;
  /** Token balance (human string) for the over-balance check. */
  balance: string;
  formatUsd: (n: number) => string;
  /** Open the host's contact picker to fill the row with this id. */
  onPickContact: (id: string) => void;
  /** Open the payroll batch importer (paste/file → converted rows). */
  onImport?: () => void;
  maxRecipients?: number;
}

export function MultiRecipientEditor({
  recipients, onChange, tokenSymbol, decimals, priceUsd, balance, formatUsd, onPickContact, onImport, maxRecipients = 20,
}: Props) {
  const { t } = useTranslation();
  useLocalePrefs(); // re-render on number-format change

  const patch = (id: string, change: Partial<RecipientDraft>) =>
    onChange(recipients.map((r) => (r.id === id ? { ...r, ...change } : r)));
  const remove = (id: string) => onChange(recipients.filter((r) => r.id !== id));
  const add = () => {
    if (recipients.length >= maxRecipients) return;
    onChange([...recipients, { id: makeRecipientId(), address: '', amount: '' }]);
  };

  const totalBase = sumSplitBaseUnits(recipients, decimals);
  const balanceBase = toBaseUnits(balance || '0', decimals);
  const overBalance = totalBase > balanceBase;
  const totalHuman = fromBaseUnits(totalBase, decimals);
  const totalUsd = priceUsd ? parseFloat(totalHuman) * priceUsd : 0;

  return (
    <View style={styles.wrap}>
      {recipients.map((r, i) => {
        const addrInvalid = r.address.trim().length > 0 && !ADDR_RE.test(r.address.trim());
        const amtUsd = priceUsd && parseFloat(r.amount) > 0 ? parseFloat(r.amount) * priceUsd : 0;
        return (
          <View key={r.id} style={styles.card}>
            <View style={styles.cardHead}>
              <Text style={styles.cardIndex}>{t('send.recipientN', { n: i + 1, defaultValue: `Recipient ${i + 1}` })}</Text>
              {recipients.length > 1 && (
                <Pressable onPress={() => remove(r.id)} hitSlop={8} style={styles.removeBtn} accessibilityLabel={t('send.removeRecipient', { defaultValue: 'Remove' })}>
                  <X size={16} color={color.fg.muted} strokeWidth={2} />
                </Pressable>
              )}
            </View>

            {/* Address */}
            <View style={styles.addrWrap}>
              <AutoGrowTextInput
                style={styles.addrInput}
                minHeight={44}
                maxHeight={96}
                value={r.address}
                onChangeText={(v) => patch(r.id, { address: v })}
                placeholder={t('send.recipientPlaceholder')}
                placeholderTextColor={color.fg.subtle}
                autoCapitalize="none"
                autoCorrect={false}
              />
              <Pressable onPress={() => onPickContact(r.id)} hitSlop={8} style={styles.addrBookBtn}>
                <BookUser size={20} color={color.fg.muted} strokeWidth={2} />
              </Pressable>
            </View>
            {addrInvalid
              ? <Text style={styles.rowError}>{t('send.alertInvalidAddressTitle')}</Text>
              : <View style={styles.badgeWrap}><RecipientTrust address={r.address} compact /></View>}

            {/* Amount */}
            <View style={styles.amountRow}>
              <TextInput
                style={styles.amountInput}
                value={r.amount.replace('.', numberSeparators().decimal)}
                onChangeText={(v) => patch(r.id, { amount: sanitizeAmount(parseLocaleNumber(v), decimals) })}
                placeholder="0"
                placeholderTextColor={color.fg.subtle}
                keyboardType="decimal-pad"
              />
              <Text style={styles.amountSym}>{tokenSymbol}</Text>
            </View>
            {amtUsd > 0 && <Text style={styles.amountUsd}>≈ {formatUsd(amtUsd)}</Text>}
          </View>
        );
      })}

      <View style={styles.addBtnRow}>
        <Pressable
          onPress={add}
          disabled={recipients.length >= maxRecipients}
          style={[styles.addRow, recipients.length >= maxRecipients && styles.addRowDisabled]}
        >
          <Plus size={18} color={color.accent.base} strokeWidth={2.5} />
          <Text style={styles.addText}>{t('send.addRecipient', { defaultValue: 'Add recipient' })}</Text>
        </Pressable>
        {onImport && (
          <Pressable onPress={onImport} style={styles.addRow} testID="editor-batch-import">
            <FileUp size={18} color={color.accent.base} strokeWidth={2.5} />
            <Text style={styles.addText}>{t('send.batchImport', { defaultValue: 'Import list' })}</Text>
          </Pressable>
        )}
      </View>

      {/* Total */}
      <View style={styles.totalRow}>
        <Text style={styles.totalLabel}>
          {/* `count` drives the en/zh plural forms; `n` still feeds the
              not-yet-pluralized bare key in the other locales. */}
          {t('send.recipientCount', { count: recipients.length, n: recipients.length })}
        </Text>
        <View style={styles.totalRight}>
          <Text style={[styles.totalValue, overBalance && styles.totalOver]}>{totalHuman} {tokenSymbol}</Text>
          {totalUsd > 0 && <Text style={styles.totalUsd}>≈ {formatUsd(totalUsd)}</Text>}
        </View>
      </View>
      {overBalance && (
        <Text style={styles.overWarn}>{t('send.alertInsufficientBalanceTitle')}</Text>
      )}
    </View>
  );
}

const styles = createStyles(() => ({
  wrap: { gap: space.md },
  card: {
    backgroundColor: color.bg.sunken, borderRadius: radius.xl,
    borderWidth: 1, borderColor: color.border.base,
    padding: space.lg, gap: space.sm,
  },
  cardHead: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' },
  cardIndex: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  removeBtn: {
    width: 28, height: 28, borderRadius: radius.md,
    alignItems: 'center', justifyContent: 'center', backgroundColor: color.bg.base,
  },
  addrWrap: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  addrInput: {
    flex: 1, backgroundColor: color.bg.base, borderRadius: radius.lg,
    paddingHorizontal: space.lg, paddingVertical: space.md,
    fontSize: text.sm, ...inter.regular, color: color.fg.base,
    outlineStyle: 'none',
  } as any,
  addrBookBtn: {
    width: 40, height: 40, borderRadius: radius.md,
    alignItems: 'center', justifyContent: 'center', backgroundColor: color.bg.base,
  },
  badgeWrap: { paddingLeft: space.xs },
  rowError: { fontSize: text.xs, ...inter.medium, color: color.error.base, paddingLeft: space.xs },
  amountRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    backgroundColor: color.bg.base, borderRadius: radius.lg, paddingHorizontal: space.lg,
  },
  amountInput: {
    flex: 1, paddingVertical: space.md, fontSize: text.lg, ...inter.semibold, color: color.fg.base,
    outlineStyle: 'none',
  } as any,
  amountSym: { fontSize: text.base, ...inter.semibold, color: color.fg.muted },
  amountUsd: { fontSize: text.xs, ...inter.regular, color: color.fg.muted, paddingLeft: space.lg },
  addBtnRow: { flexDirection: 'row', gap: space.md },
  addRow: {
    flex: 1, flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: space.sm,
    paddingVertical: space.lg, borderRadius: radius.lg,
    borderWidth: 1, borderColor: color.border.base, borderStyle: 'dashed', backgroundColor: color.bg.raised,
  },
  addRowDisabled: { opacity: 0.4 },
  addText: { fontSize: text.base, ...inter.semibold, color: color.accent.base },
  totalRow: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    paddingHorizontal: space.sm, marginTop: space.xs,
  },
  totalLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  totalRight: { alignItems: 'flex-end' },
  totalValue: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  totalOver: { color: color.error.base },
  totalUsd: { fontSize: text.xs, ...inter.regular, color: color.fg.muted },
  overWarn: { fontSize: text.sm, ...inter.medium, color: color.error.base, paddingHorizontal: space.sm },
}));
