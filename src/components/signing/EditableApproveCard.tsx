/**
 * EditableApproveCard — the founder mandate made tangible.
 *
 * Replaces the passive "Unlimited ⚠" banner with an active control: the user
 * picks a FINITE spending cap (or revokes). There is intentionally no "Max" /
 * "Unlimited" preset anywhere, and for an unbounded incoming request the confirm
 * button stays disabled until the user makes a finite choice (reported as a
 * non-null `choice` to the parent).
 *
 * Pure presentational: all encoding/guarding lives in services/approval-guard.
 */
import React, { useEffect, useMemo, useState } from 'react';
import { View, Text, Pressable, TextInput } from 'react-native';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, ShieldCheck, Pencil } from 'lucide-react-native';
import { scaleFont, color, text, inter, space, radius, createStyles } from '@/constants/theme';
import { TokenLogo } from '@/components/TokenLogo';
import {
  type DetectedApproval,
  type ApprovalChoice,
  parseTokenAmount,
  formatTokenAmount,
  isUnboundedAmount,
} from '@/services/approval-guard';
import { useLocalePrefs, numberSeparators } from '@/services/locale-format';
import { useDisplayCurrency } from '@/hooks/use-display-currency';

interface Props {
  approval: DetectedApproval;
  symbol: string;
  decimals: number;
  decimalsVerified: boolean;
  /** Per-chain logo URLs (checksummed first, lowercase fallback). */
  logoUrls?: string[];
  /** Resolved spender name or short address, for the plain-language line. */
  spenderLabel: string;
  /** Token USD price for the ≈$ line (omit to hide it). */
  usdPrice?: number;
  choice: ApprovalChoice | null;
  onChange: (choice: ApprovalChoice | null) => void;
}

type Mode = 'requested' | 'custom' | 'revoke';

export function EditableApproveCard(props: Props) {
  if (props.approval.isBooleanGrant) return <BooleanGrantCard {...props} />;
  return <AmountCard {...props} />;
}

// ---------------------------------------------------------------------------
// Amount-bearing approvals (ERC-20 approve / increaseAllowance / ERC-2612 /
// Permit2 single). Decrease is rendered read-only-safe.
// ---------------------------------------------------------------------------

function AmountCard({
  approval, symbol, decimals, decimalsVerified, logoUrls, spenderLabel, usdPrice, onChange,
}: Props) {
  const { t } = useTranslation();
  useLocalePrefs();
  const sep = numberSeparators();
  const dc = useDisplayCurrency();
  const requested = approval.amountRaw ?? 0n;
  const requestedFinite = !approval.isUnbounded && requested > 0n;

  // Initial mode: a finite, reasonable request is pre-accepted; an unbounded
  // request forces a deliberate choice (custom).
  const [mode, setMode] = useState<Mode>(requestedFinite ? 'requested' : 'custom');
  const [customText, setCustomText] = useState<string>(
    requestedFinite ? formatTokenAmount(requested, decimals) : '',
  );

  // Derive the chosen amount + validity from mode/customText.
  const { choice, error, displayRaw } = useMemo(() => {
    if (mode === 'revoke') return { choice: { type: 'revoke' } as ApprovalChoice, error: null, displayRaw: 0n };
    if (mode === 'requested') return { choice: { type: 'amount', amountRaw: requested } as ApprovalChoice, error: null, displayRaw: requested };
    // custom
    const trimmed = customText.trim();
    if (trimmed === '') return { choice: null, error: null as string | null, displayRaw: null as bigint | null };
    const raw = parseTokenAmount(trimmed, decimals);
    if (raw === null) return { choice: null, error: t('componentsUi.signingApprove.invalidAmount'), displayRaw: null };
    if (isUnboundedAmount(raw, approval.amountBits ?? 256)) {
      return { choice: null, error: t('componentsUi.signingApprove.unlimitedDisabled'), displayRaw: raw };
    }
    return { choice: { type: 'amount', amountRaw: raw } as ApprovalChoice, error: null, displayRaw: raw };
  }, [mode, customText, requested, decimals, approval.amountBits, t]);

  useEffect(() => { onChange(choice); }, [choice]); // eslint-disable-line react-hooks/exhaustive-deps

  const isReducing = approval.kind === 'decreaseAllowance';
  const accent = isReducing ? color.success.base : color.accent.base;
  const usd = displayRaw != null && usdPrice ? (Number(displayRaw) / 10 ** decimals) * usdPrice : null;

  return (
    <View style={[styles.card, isReducing && styles.cardSafe]}>
      {/* Token header */}
      <View style={styles.header}>
        <TokenLogo symbol={symbol} logoUrls={logoUrls} size={28} />
        <Text style={styles.symbol}>{symbol}</Text>
        <Text style={styles.capLabel}>
          {isReducing ? t('componentsUi.signingApprove.reduceBy') : t('componentsUi.signingApprove.spendingCap')}
        </Text>
      </View>

      {/* The value — display or live custom input. The token symbol lives in the
          header (and the summary line), so the number stands alone here. */}
      {mode === 'custom' ? (
        <View style={styles.inputRow}>
          <TextInput
            style={[styles.amountInput, { color: error ? color.error.base : color.fg.base }]}
            value={customText}
            onChangeText={setCustomText}
            keyboardType="decimal-pad"
            inputMode="decimal"
            placeholder="0"
            placeholderTextColor={color.fg.subtle}
            autoFocus={!requestedFinite}
            selectionColor={accent}
          />
        </View>
      ) : (
        <Pressable style={styles.valueRow} onPress={() => setMode('custom')}>
          <Text style={[styles.amountValue, mode === 'revoke' && { color: color.success.base }]} numberOfLines={1}>
            {mode === 'revoke' ? t('componentsUi.signingApprove.revokeValue') : `${formatTokenAmount(displayRaw ?? 0n, decimals, 6, sep)} ${symbol}`}
          </Text>
          {mode !== 'revoke' && <Pencil size={15} color={color.fg.subtle} strokeWidth={2} />}
        </Pressable>
      )}

      {usd != null && mode !== 'revoke' && !error && (
        <Text style={styles.usd}>≈ {dc.fmt(usd)}</Text>
      )}

      {/* Presets */}
      <View style={styles.presets}>
        {requestedFinite && (
          <PresetChip
            label={t('componentsUi.signingApprove.requested')}
            active={mode === 'requested'}
            onPress={() => { setMode('requested'); setCustomText(formatTokenAmount(requested, decimals)); }}
          />
        )}
        <PresetChip
          label={t('componentsUi.signingApprove.custom')}
          active={mode === 'custom'}
          onPress={() => setMode('custom')}
        />
        <PresetChip
          label={t('componentsUi.signingApprove.revoke')}
          active={mode === 'revoke'}
          tone="safe"
          onPress={() => setMode('revoke')}
        />
      </View>

      {/* Inline error */}
      {error && (
        <View style={styles.errorRow}>
          <AlertTriangle size={13} color={color.error.base} strokeWidth={2} />
          <Text style={styles.errorText}>{error}</Text>
        </View>
      )}

      {/* Plain-language summary */}
      {!error && (
        <Text style={styles.summary}>
          {mode === 'revoke'
            ? t('componentsUi.signingApprove.revokeSummary', { spender: spenderLabel })
            : choice
              ? t('componentsUi.signingApprove.capSummary', { spender: spenderLabel, amount: `${formatTokenAmount((choice as any).amountRaw, decimals, 6, sep)} ${symbol}` })
              : t('componentsUi.signingApprove.choosePrompt')}
        </Text>
      )}

      {!decimalsVerified && (
        <Text style={styles.unverified}>{t('componentsUi.signingApprove.decimalsUnverified')}</Text>
      )}
    </View>
  );
}

// ---------------------------------------------------------------------------
// Boolean grants (setApprovalForAll / DAI permit). No amount — grant or revoke.
// ---------------------------------------------------------------------------

function BooleanGrantCard({ approval, spenderLabel, onChange }: Props) {
  const { t } = useTranslation();
  const incomingGrant = approval.isUnbounded; // true === a grant-all request
  // Default to the safe action: if the dApp asked to grant, we still default to a
  // conscious choice — preselect nothing for a grant (force deliberate tap), and
  // preselect revoke when the request is already a revoke.
  const [selected, setSelected] = useState<'grant' | 'revoke' | null>(incomingGrant ? null : 'revoke');

  useEffect(() => {
    onChange(selected === 'grant' ? { type: 'grant' } : selected === 'revoke' ? { type: 'revoke' } : null);
  }, [selected]); // eslint-disable-line react-hooks/exhaustive-deps

  const isNft = approval.kind === 'setApprovalForAll';

  return (
    <View style={[styles.card, styles.cardDanger]}>
      <View style={styles.header}>
        <AlertTriangle size={18} color={color.error.base} strokeWidth={2} />
        <Text style={[styles.symbol, { color: color.error.base }]}>
          {isNft ? t('componentsUi.signingApprove.allNfts') : t('componentsUi.signingApprove.fullBalance')}
        </Text>
      </View>

      <Text style={styles.booleanWarn}>
        {isNft
          ? t('componentsUi.signingApprove.setApprovalAllWarn', { operator: spenderLabel })
          : t('componentsUi.signingApprove.daiWarn', { spender: spenderLabel })}
      </Text>

      <Pressable
        style={[styles.boolBtn, styles.boolRevoke, selected === 'revoke' && styles.boolRevokeActive]}
        onPress={() => setSelected('revoke')}
      >
        <ShieldCheck size={16} color={color.success.base} strokeWidth={2} />
        <Text style={[styles.boolBtnText, { color: color.success.base }]}>{t('componentsUi.signingApprove.revokeAccess')}</Text>
      </Pressable>

      <Pressable
        style={[styles.boolBtn, styles.boolGrant, selected === 'grant' && styles.boolGrantActive]}
        onPress={() => setSelected('grant')}
      >
        <Text style={[styles.boolBtnText, { color: selected === 'grant' ? color.error.base : color.fg.muted }]}>
          {t('componentsUi.signingApprove.grantAllAnyway')}
        </Text>
      </Pressable>

      {selected === null && (
        <Text style={styles.summary}>{t('componentsUi.signingApprove.chooseAction')}</Text>
      )}
    </View>
  );
}

// ---------------------------------------------------------------------------
// Preset chip
// ---------------------------------------------------------------------------

function PresetChip({ label, active, onPress, tone }: {
  label: string; active: boolean; onPress: () => void; tone?: 'safe';
}) {
  return (
    <Pressable
      onPress={onPress}
      style={[
        styles.chip,
        active && (tone === 'safe' ? styles.chipActiveSafe : styles.chipActive),
      ]}
    >
      <Text style={[
        styles.chipText,
        active && (tone === 'safe' ? styles.chipTextActiveSafe : styles.chipTextActive),
      ]}>{label}</Text>
    </Pressable>
  );
}

const styles = createStyles(() => ({
  // De-containered (Wise / the mock): a routine bounded approve sits OPEN, aligned
  // to the sheet edge — no tinted box competing for attention. Only the genuinely
  // dangerous unbounded grant (cardDanger) gets a contained red alarm box.
  card: {
    paddingVertical: space.md,
    gap: space.md,
  },
  cardSafe: {},
  cardDanger: {
    backgroundColor: color.error.soft,
    borderWidth: 1, borderColor: color.error.base + '40',
    borderRadius: radius['2xl'],
    padding: space['2xl'],
    marginVertical: space.sm,
  },

  header: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  symbol: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  capLabel: {
    marginLeft: 'auto', fontSize: scaleFont(10), ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase', letterSpacing: 0.4,
  },

  valueRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  amountValue: { fontSize: text['3xl'], ...inter.bold, color: color.fg.base, letterSpacing: -0.5, flexShrink: 1 },
  inputRow: { flexDirection: 'row', alignItems: 'baseline', gap: space.sm },
  amountInput: {
    // minWidth:0 lets the flex input shrink below its intrinsic content width;
    // without it a long value overflows and horizontally scrolls the whole sheet,
    // clipping the detail rows (被授权方/代币) on the left edge.
    flex: 1, minWidth: 0, fontSize: text['3xl'], ...inter.bold, letterSpacing: -0.5, padding: 0,
  },
  usd: { fontSize: text.sm, ...inter.medium, color: color.fg.muted, marginTop: -space.xs },

  presets: { flexDirection: 'row', gap: space.sm, marginTop: space.xs },
  chip: {
    paddingHorizontal: space.lg, paddingVertical: space.sm, borderRadius: radius.full,
    backgroundColor: color.bg.raised, borderWidth: 1, borderColor: color.border.base,
  },
  chipActive: { backgroundColor: color.fg.base, borderColor: color.fg.base },
  chipActiveSafe: { backgroundColor: color.success.base, borderColor: color.success.base },
  chipText: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  chipTextActive: { color: color.fg.inverse },
  chipTextActiveSafe: { color: color.fg.inverse },

  errorRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  errorText: { fontSize: text.sm, ...inter.medium, color: color.error.base, flex: 1 },
  summary: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, lineHeight: 18 },
  unverified: { fontSize: text.xs, ...inter.regular, color: color.warning.base },

  // Restraint: red heading (the "All NFTs" symbol) carries the alarm; the body reads
  // in ink so the card isn't a wall of red (matches the eth_sign danger card).
  booleanWarn: { fontSize: text.sm, ...inter.medium, color: color.fg.base, lineHeight: 19 },
  boolBtn: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: space.sm,
    paddingVertical: space.lg, borderRadius: radius.lg, borderWidth: 1,
  },
  boolRevoke: { backgroundColor: color.success.soft, borderColor: color.success.base + '40' },
  boolRevokeActive: { borderColor: color.success.base, borderWidth: 2 },
  boolGrant: { backgroundColor: color.bg.raised, borderColor: color.border.base },
  boolGrantActive: { borderColor: color.error.base, borderWidth: 2, backgroundColor: color.error.soft },
  boolBtnText: { fontSize: text.base, ...inter.semibold },
}));
