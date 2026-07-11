/**
 * Shared foundation for the signing UI (ERC-7730 Clear Signing).
 *
 * Leaf module: the signing-chain context, risk→color mapping, descriptor
 * label/intent localization, and the full StyleSheet. It imports only theme /
 * i18n / clear-signing types — never a signing component — so the view and
 * component modules can all depend on it without an import cycle.
 */
import React from 'react';
import i18n from '@/i18n';
import { type SigningRisk } from '@/services/clear-signing';
import { scaleFont, color, text, inter, space, radius, font, createStyles } from '@/constants/theme';

/** Chain id for the active signing sheet — lets leaf rows build explorer links. */
export const SigningChainContext = React.createContext<number>(1);

// ---------------------------------------------------------------------------
// Risk → color mapping
// ---------------------------------------------------------------------------

export function riskColors(): Record<SigningRisk, string> {
  return {
    safe: color.success.base,
    normal: color.accent.base,
    caution: color.warning.base,
    danger: color.error.base,
  };
}

// ---------------------------------------------------------------------------
// Descriptor label/intent localization.
//
// ERC-7730 descriptors carry English intents ("Send", "Swap") and field labels
// ("Amount", "To") by spec, so a descriptor-driven screen would render half-
// English inside a localized UI (the "确认Send" / "Amount" problem). Map the
// common canonical values to the user's language; anything unrecognized falls
// through to the raw descriptor string (an honest, if English, label).
// ---------------------------------------------------------------------------
const INTENT_L10N: Record<string, string> = {
  send: 'intentSend', transfer: 'intentSend',
  'transfer nft': 'intentTransferNft', 'send nft': 'intentTransferNft',
  approve: 'intentApprove', 'set allowance': 'intentApprove', 'increase allowance': 'intentApprove',
  swap: 'intentSwap', exchange: 'intentSwap', trade: 'intentSwap',
  deposit: 'intentDeposit', supply: 'intentDeposit',
  withdraw: 'intentWithdraw', redeem: 'intentWithdraw',
  mint: 'intentMint', burn: 'intentBurn',
  stake: 'intentStake', unstake: 'intentUnstake',
  claim: 'intentClaim', 'claim rewards': 'intentClaim',
  bridge: 'intentBridge', wrap: 'intentWrap', unwrap: 'intentUnwrap',
  borrow: 'intentBorrow', repay: 'intentRepay', revoke: 'intentRevoke',
};
const LABEL_L10N: Record<string, string> = {
  amount: 'labelAmount', value: 'labelAmount', assets: 'labelAmount',
  to: 'labelTo', recipient: 'labelTo', receiver: 'labelTo', beneficiary: 'labelTo', destination: 'labelTo',
  from: 'labelFrom', sender: 'labelFrom', owner: 'labelOwner',
  spender: 'labelSpender', operator: 'labelSpender',
  token: 'labelToken', 'token id': 'labelTokenId', tokenid: 'labelTokenId',
  deadline: 'labelDeadline',
  'min received': 'labelMinReceived', 'minimum received': 'labelMinReceived', 'min amount out': 'labelMinReceived',
  'you receive (min)': 'labelMinReceived', 'you receive (minimum)': 'labelMinReceived',
  'you pay': 'labelPay', pay: 'labelPay',
  'you receive': 'labelReceived', 'amount received': 'labelReceived', shares: 'labelShares',
  'deposit asset': 'labelAmount', 'withdraw asset': 'labelAmount',
  'mint shares': 'labelShares', 'redeem shares': 'labelShares',
  chain: 'labelChain', 'chain id': 'labelChain', nonce: 'labelNonce',
};
/** Localize a canonical ERC-7730 English intent; unknown → the raw string. */
export function localizeIntent(raw?: string): string {
  if (!raw) return '';
  const suffix = INTENT_L10N[raw.trim().toLowerCase()];
  return suffix ? String(i18n.t(('componentsUi.signing.' + suffix) as any, { defaultValue: raw })) : raw;
}
/** Localize a canonical ERC-7730 English field label; unknown → the raw string. */
export function localizeLabel(raw?: string): string {
  if (!raw) return '';
  const suffix = LABEL_L10N[raw.trim().toLowerCase()];
  return suffix ? String(i18n.t(('componentsUi.signing.' + suffix) as any, { defaultValue: raw })) : raw;
}

/**
 * Intent-header color. Restrained on purpose: color = meaning. Benign actions
 * (send, sign, deploy) read in neutral ink; only caution (amber) and danger
 * (red) get a hue, so a colored headline always signals real risk.
 */
export function intentColor(risk: SigningRisk): string {
  if (risk === 'danger') return color.error.base;
  if (risk === 'caution') return color.warning.base;
  return color.fg.base;
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

export const styles = createStyles(() => ({
  container: {
    flex: 1,
    padding: space['3xl'],
  },

  // ===== dApp Banner =====
  // De-containered (Wise): an open "who's asking" header, separated from the
  // action below by a hairline instead of a gray card.
  dappBanner: {
    paddingTop: space.sm,
    paddingBottom: space.xl,
    borderBottomWidth: 1,
    borderBottomColor: color.border.base,
    marginBottom: space.xl,
    gap: space.md,
  },
  dappRow1: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  dappLogo: {
    width: 36, height: 36, borderRadius: 10,
  },
  dappLogoFallback: {
    backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  dappLogoText: {
    fontSize: text.lg, ...inter.bold, color: color.accent.base,
  },
  dappInfo: { flex: 1, gap: 1 },
  dappName: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  dappDomain: {
    fontSize: text.xs, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.fg.muted,
  },
  dappChainRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    marginLeft: 'auto',
  },
  dappChainName: {
    fontSize: text.xs, ...inter.semibold, color: color.fg.base,
  },
  dappAccountRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  dappAccountLine: {
    fontSize: text.xs, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.fg.muted,
    flexShrink: 1,
  },

  // ===== Intent Header =====
  intentHeader: {
    alignItems: 'center',
    paddingTop: space.lg,
    paddingBottom: space['2xl'],
  },
  intentText: {
    fontSize: text['5xl'],
    ...inter.bold,
    textAlign: 'center',
    letterSpacing: -1,
  },
  // Eyebrow kicker — a small colored verb that cedes the headline to the asset flow.
  intentEyebrow: {
    alignSelf: 'center',
    paddingTop: space.md,
    paddingBottom: space.lg,
  },
  intentEyebrowText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.subtle,
    letterSpacing: 1.4,
    textTransform: 'uppercase',
  },

  // ===== Plain-language summary (the novice's one-sentence read) =====
  // Ink, not muted — it's the entry point, so it reads clearly; the key facts
  // (amount, counterparty) come in semibold for a scannable emphasis.
  summaryLine: {
    fontSize: scaleFont(15),
    lineHeight: 23,
    ...inter.medium,
    color: color.fg.base,
    paddingTop: space.sm,
    paddingBottom: space.lg,
  },
  summaryBold: { ...inter.semibold, color: color.fg.base },
  summaryCaution: { color: color.warning.base },
  summaryDanger: { color: color.error.base },

  // ===== Token Card =====
  // Open row (Wise de-container) for benign amounts — no card, just the number
  // breathing next to its logo, aligned to the sheet's content edge.
  tokenRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xl,
    paddingVertical: space.lg,
  },
  // Tinted card — caution/danger only, so a filled surface always means "attention".
  tokenCard: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xl,
    paddingVertical: space['2xl'],
    paddingHorizontal: space['2xl'],
    borderRadius: radius['2xl'],
    marginVertical: space.sm,
  },
  tokenInfo: { flex: 1, minWidth: 0 },
  tokenAmount: {
    // The hero of a benign transfer now that the verb is a mere eyebrow.
    // adjustsFontSizeToFit shrinks long amounts so the ticker never clips.
    fontSize: text['3xl'],
    ...inter.bold,
    color: color.fg.base,
    letterSpacing: -0.6,
  },
  tokenLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
    marginTop: space.xs,
  },
  tokenWarning: {
    width: 28, height: 28, borderRadius: 14,
    backgroundColor: color.error.soft,
    alignItems: 'center', justifyContent: 'center',
  },

  // ===== Flow Arrow =====
  flowArrow: {
    alignItems: 'center',
    marginVertical: -space.sm,
    zIndex: 1,
  },
  // Lightened: a quiet sunken dot, no border/shadow — it connects pay→receive on a
  // swap without becoming its own object (was a bordered, shadowed circle).
  flowCircle: {
    width: 24, height: 24, borderRadius: 12,
    backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
  },
  flowCircleDanger: {
    backgroundColor: color.error.soft,
  },

  // ===== Contract Bar =====
  // De-containered (Wise): an open recipient/contract row separated from the
  // asset flow above by a hairline, not a gray card.
  contractBar: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    borderTopWidth: 1,
    borderTopColor: color.border.base,
  },
  // A flagged recipient DOES get a tinted card back — danger should contain itself.
  contractBarWarning: {
    borderTopWidth: 0,
    paddingHorizontal: space.xl,
    backgroundColor: color.error.soft,
    borderWidth: 1,
    borderColor: color.error.base,
    borderRadius: radius.xl,
    marginVertical: space.md,
  },
  contractInfo: { flex: 1, gap: 2 },
  contractLabel: {
    fontSize: scaleFont(10), ...inter.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.3,
  },
  contractAddrRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    flexWrap: 'wrap',
  },
  contractName: {
    fontSize: text.sm, ...inter.semibold, color: color.success.base,
  },
  contractNameNeutral: { color: color.fg.base },
  sourceTag: {
    fontSize: scaleFont(9), ...inter.semibold, color: color.fg.subtle,
    backgroundColor: color.bg.sunken, overflow: 'hidden',
    paddingHorizontal: 5, paddingVertical: 1, borderRadius: radius.sm,
    textTransform: 'uppercase', letterSpacing: 0.3,
  },
  contractAddr: {
    fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.fg.muted,
  },
  copyBtn: {
    width: 28, height: 28, borderRadius: radius.md,
    borderWidth: 1, borderColor: color.border.base,
    backgroundColor: color.bg.raised,
    alignItems: 'center', justifyContent: 'center',
  },
  copyBtnDone: {
    borderColor: color.success.base,
    backgroundColor: color.success.soft,
  },
  verifiedBadge: {
    width: 24, height: 24,
    alignItems: 'center', justifyContent: 'center',
  },
  // A contract counterparty gets a neutral rounded-square glyph, NEVER a nimiq
  // identicon (which reads as a personal wallet). Same 36px footprint so the row
  // height matches a wallet row.
  contractGlyph: {
    width: 36, height: 36, borderRadius: radius.lg,
    backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
  },
  // Identity chips answer "who is this" at a glance — 钱包 / 合约 / 已验证. Small,
  // restrained; stacked at the row's trailing edge.
  idChips: { flexDirection: 'row', alignItems: 'center', gap: 6, flexWrap: 'wrap', justifyContent: 'flex-end', maxWidth: 128 },
  idChip: {
    flexDirection: 'row', alignItems: 'center', gap: 4,
    paddingHorizontal: 8, paddingVertical: 3, borderRadius: radius.full,
  },
  idChipText: { fontSize: scaleFont(10), ...inter.semibold, letterSpacing: 0.2 },
  idChipWallet: { backgroundColor: color.info.soft },
  idChipWalletText: { color: color.info.base },
  idChipContract: { backgroundColor: color.bg.sunken },
  idChipContractText: { color: color.fg.muted },
  idChipVerified: { backgroundColor: color.success.soft },
  idChipVerifiedText: { color: color.success.base },

  // ===== Warning Banner =====
  warnBanner: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    borderRadius: radius.xl,
    marginVertical: space.md,
  },
  warnCaution: {
    backgroundColor: color.warning.soft,
    borderWidth: 1, borderColor: color.warning.border,
  },
  warnDanger: {
    backgroundColor: color.error.soft,
    borderWidth: 1, borderColor: color.error.base,
  },
  warnText: {
    fontSize: text.sm, ...inter.semibold, flex: 1, lineHeight: 18,
  },

  // ===== Generic Fields =====
  genericFields: {
    marginVertical: space.md,
  },
  // De-containered (Wise): open rows split by hairlines, not stacked gray cards.
  genRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    paddingVertical: space.md,
    borderTopWidth: 1,
    borderTopColor: color.border.base,
    gap: space.lg,
  },
  genRowWarning: {
    marginHorizontal: -space.xl,
    paddingHorizontal: space.xl,
    borderTopWidth: 0,
    backgroundColor: color.warning.soft,
    borderRadius: radius.lg,
  },
  genLabel: {
    fontSize: text.sm, ...inter.medium, color: color.fg.muted,
    flexShrink: 0,
  },
  genValue: {
    fontSize: text.sm, ...inter.semibold, color: color.fg.base,
    // minWidth:0 lets a long unbreakable value (e.g. a raw address) wrap/truncate
    // within the row instead of overflowing off the right edge.
    textAlign: 'right', flex: 1, minWidth: 0,
    fontFamily: font.mono, fontWeight: '500' as const,
  },

  // ===== Message Bubble =====
  // De-containered (Wise): the signed message sits on an open block framed by
  // hairlines (top + bottom) — the payload boundary without a gray card.
  msgBubble: {
    paddingVertical: space.xl,
    borderTopWidth: 1,
    borderTopColor: color.border.base,
    borderBottomWidth: 1,
    borderBottomColor: color.border.base,
    marginVertical: space.md,
  },
  msgTag: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    alignSelf: 'center',
    paddingVertical: space.xs,
    paddingHorizontal: space.lg,
    backgroundColor: color.border.base,
    borderRadius: radius.full,
    marginBottom: space.xl,
  },
  msgTagText: {
    fontSize: scaleFont(10), ...inter.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.3,
  },
  msgText: {
    fontSize: text.base, ...inter.regular,
    color: color.fg.base,
    lineHeight: 22,
    textAlign: 'center',
  },

  // (context strip merged into dApp banner)

  // ===== Details Toggle =====
  detailsToggle: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    paddingVertical: space.md,
  },
  detailsToggleText: {
    fontSize: text.xs, ...inter.semibold, color: color.fg.subtle,
  },

  // ===== Raw Data =====
  rawBlock: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.lg,
    maxHeight: 160,
    marginBottom: space.lg,
  },
  rawText: {
    // Readable calldata: 9px + lowest-contrast ink made the raw viewer illegible.
    // 12px mono on fg.muted stays quiet without forcing a squint.
    fontSize: scaleFont(12), fontFamily: font.mono, fontWeight: '400' as const,
    color: color.fg.muted, lineHeight: 18,
  },

  // ===== Fallback =====
  fallback: {
    alignItems: 'center',
    paddingVertical: space['5xl'],
    gap: space.lg,
  },
  fallbackText: {
    fontSize: text.lg, ...inter.regular, color: color.fg.muted,
  },

  // ===== Error =====
  errorCard: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.error.soft,
    borderRadius: radius.lg,
    marginBottom: space.lg,
  },
  errorText: { fontSize: text.sm, ...inter.regular, color: color.error.base, flex: 1 },

  // ===== Pending (submitted, awaiting receipt) =====
  pendingCard: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.info.soft,
    borderRadius: radius.lg,
    marginBottom: space.lg,
  },
  pendingText: {
    fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.info.base, flex: 1,
  },

  // Read-only replay banner
  historyNote: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingVertical: space.md,
    paddingHorizontal: space.lg,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    marginBottom: space.lg,
  },
  historyNoteText: {
    fontSize: text.sm, ...inter.medium, color: color.fg.muted, flex: 1,
  },

  // ===== Buttons =====
  buttonRow: {
    flexDirection: 'row', gap: space.lg,
    paddingTop: space.xl,
    borderTopWidth: 1,
    borderTopColor: color.border.base,
    marginTop: space.sm,
  },
  buttonFlex: { flex: 1 },
  noFeeRow: { paddingTop: space.lg, alignItems: 'center' },
  noFeeLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle },
  // Danger signing stacks the full-width slide over a full-width Reject.
  dangerStack: { flex: 1, gap: space.md },

  // ===== Batch (EIP-5792) breakdown =====
  batchSub: {
    fontSize: text.sm, ...inter.regular, color: color.fg.muted,
    textAlign: 'center', marginTop: -space.md, marginBottom: space.lg,
  },
  batchRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.lg, paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken, borderRadius: radius.xl, marginVertical: space.sm,
  },
  batchRowDanger: { borderWidth: 1, borderColor: color.error.base },
  batchNum: {
    width: 24, height: 24, borderRadius: 12, backgroundColor: color.bg.raised,
    alignItems: 'center', justifyContent: 'center',
  },
  batchNumText: { fontSize: text.xs, ...inter.bold, color: color.fg.muted },
  batchInfo: { flex: 1, gap: 1 },
  batchTitle: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  batchDetail: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  batchAddr: { fontSize: text.xs, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.subtle },
  // Off-chain permit risk hint, under the permit card.
  permitHint: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, lineHeight: 18, marginTop: space.xs },
  // Editable approval leg: numbered header above the inline spending-cap editor.
  batchEditLeg: { marginVertical: space.sm },
  batchEditHead: { flexDirection: 'row', alignItems: 'center', gap: space.md, marginBottom: space.xs },
  batchEditTitle: { flex: 1, fontSize: text.base, ...inter.semibold, color: color.fg.base },

  // ===== Advanced panel =====
  advancedBody: { gap: space.sm, marginBottom: space.md },
  advancedRaw: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.lg,
    maxHeight: 260,
  },
  // Expert-layer address rows: full 0x with copy + explorer (the actions the calm
  // default view no longer shows inline). Boxed affordances are fine here — density
  // is welcome behind the 技术细节 disclosure.
  advAddrRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    paddingVertical: space.sm,
    borderBottomWidth: 1, borderBottomColor: color.border.base,
  },
  advAddrLabel: {
    fontSize: scaleFont(10), ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase', letterSpacing: 0.3, marginBottom: 2,
  },
  advAddrValue: {
    fontSize: scaleFont(12), fontFamily: font.mono, fontWeight: '400' as const,
    color: color.fg.base,
  },

  // ===== Token-card USD line =====
  tokenSubRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    marginTop: space.xs, flexWrap: 'wrap',
  },
  tokenUsd: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle },

  // ===== SIWE verified-domain confirmation row =====
  siweOkRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    paddingVertical: space.sm, paddingHorizontal: space.sm, marginBottom: space.xs,
  },
  siweOkText: { fontSize: text.xs, ...inter.medium, color: color.success.base },

  // ===== Recipient-risk tags (first-time / contract) =====
  // One restrained caution line (warning ink, no chip) — informs without shouting.
  riskNote: {
    fontSize: text.xs, ...inter.medium, color: color.warning.base, marginTop: 1,
  },

  // ===== increaseAllowance resulting total =====
  allowanceTotalRow: {
    paddingVertical: space.md, paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken, borderRadius: radius.lg,
    marginVertical: space.sm, gap: 2,
  },
  allowanceTotalLabel: {
    fontSize: scaleFont(10), ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase' as const, letterSpacing: 0.3,
  },
  allowanceTotalValue: {
    fontSize: text.sm, ...inter.semibold, color: color.fg.base, fontFamily: font.mono,
  },
  allowanceTotalUnknown: {
    fontSize: text.sm, ...inter.medium, color: color.warning.base, lineHeight: 18,
  },

  // ===== eth_sign danger surface =====
  ethSignCard: {
    backgroundColor: color.error.soft, borderRadius: radius['2xl'],
    padding: space['2xl'], marginVertical: space.md, gap: space.md,
    borderWidth: 1, borderColor: color.error.base + '40',
  },
  ethSignHeader: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  ethSignTitle: { fontSize: text.base, ...inter.bold, color: color.error.base },
  ethSignBody: { fontSize: text.sm, ...inter.regular, color: color.fg.base, lineHeight: 19 },
  ethSignHash: {
    fontSize: scaleFont(11), fontFamily: font.mono, fontWeight: '400' as const,
    color: color.fg.muted, backgroundColor: color.bg.sunken,
    padding: space.md, borderRadius: radius.md,
  },
}));
