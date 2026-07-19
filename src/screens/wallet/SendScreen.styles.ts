import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';

export const styles = createStyles(() => ({
  navBar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.lg,
  },
  navBtn: {
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
  },
  navSpacer: { minWidth: 60 },

  // Step content
  stepContainer: {
    flex: 1,
  },
  stepTitle: {
    fontSize: text['3xl'],
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space['2xl'],
  },
  loadingText: {
    fontSize: text.lg,
    ...inter.regular,
    color: color.fg.muted,
    textAlign: 'center',
    marginTop: space['5xl'],
  },
  emptyContainer: {
    alignItems: 'center',
    marginTop: space['5xl'],
  },
  emptyText: {
    fontSize: text.xl,
    ...inter.semibold,
    color: color.fg.muted,
  },

  // Search
  searchWrap: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    paddingHorizontal: space.lg,
    paddingVertical: space.md,
    marginBottom: space.md,
  },
  searchInput: {
    flex: 1,
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.base,
    padding: 0,
    outlineStyle: 'none',
  } as any,

  // Category + network filters
  filterRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    marginBottom: space.xl,
  },
  chipScroll: {
    flex: 1,
  },
  chipRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingRight: space.sm,
  },
  chip: {
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    borderRadius: radius.full,
    backgroundColor: color.bg.sunken,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  chipActive: {
    backgroundColor: color.accent.soft,
    borderColor: color.accent.base,
  },
  chipText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
  },
  chipTextActive: {
    color: color.accent.base,
  },

  // Count + total summary above the list
  summaryRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: space.sm,
    marginBottom: space.md,
  },
  summaryCount: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  summaryTotal: {
    fontSize: text.sm,
    ...inter.semibold,
    fontFamily: font.numeric,
    color: color.fg.base,
  },

  // Add-token affordance (footer + empty state)
  addTokenRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    alignSelf: 'stretch', // full width in both the list footer and the centered empty state
    gap: space.sm,
    paddingVertical: space.xl,
    marginTop: space.lg,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    borderStyle: 'dashed',
    backgroundColor: color.bg.raised,
  },
  addTokenText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
  },

  // Token hero — open on the page (de-boxed), grouped by space + a hairline
  heroBlock: {
    marginBottom: space['3xl'],
  },
  heroDivider: {
    height: 1,
    backgroundColor: color.border.base,
    marginTop: space.lg,
    // inset past the 44px logo + its gap so the line aligns under the text
    marginLeft: 44 + space.lg,
  },
  heroRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  heroIdentity: {
    flex: 1,
    gap: 2,
  },
  heroSymbol: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.fg.base,
  },
  heroChain: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
  },
  // ERC-20 contract address (tap to copy) — open row under the hairline, inset to
  // align under the hero's text
  contractRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginTop: space.lg,
    marginLeft: 44 + space.lg,
  },
  contractLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
  },
  contractAddr: {
    flex: 1,
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.muted,
    textAlign: 'right',
  },
  heroBalance: {
    alignItems: 'flex-end',
    gap: 2,
    flexShrink: 1,
    maxWidth: '58%', // keep the token symbol readable; huge balances shrink
  },
  heroAmountBox: {
    alignSelf: 'stretch',
  },
  heroAmount: {
    fontSize: text.xl,
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
    textAlign: 'right',
  },
  heroUsd: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },

  // Form fields — full width, icons inside
  fieldLabelRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: space.md,
  },
  fieldLabel: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.8,
  },
  addrActionBtn: {
    width: 40,
    height: 40,
    borderRadius: radius.md,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: color.bg.base,
  },
  // "+ 添加收款人" / "导入表格" entries → split mode (side by side)
  splitEntryRow: {
    flexDirection: 'row',
    gap: space.md,
    marginTop: space.lg,
  },
  addRecipientEntry: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    paddingVertical: space.md,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    borderStyle: 'dashed',
  },
  addRecipientEntryText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
  },
  // ② multi-token send (multi-select → one recipient) — open rows on the page,
  // grouped by a summary line + hairline dividers (de-boxed, no card).
  multiBlock: { marginBottom: space.lg },
  mtSummary: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingTop: space.md,
    paddingBottom: space.md,
  },
  mtSummaryTitle: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted, flex: 1 },
  mtSummaryUsd: { fontSize: text.lg, ...inter.bold, fontFamily: font.numeric, color: color.fg.base },
  mtRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingVertical: space.lg,
  },
  // Hairline between token rows, inset past the 32px logo + gap
  mtSep: { height: 1, backgroundColor: color.border.base, marginLeft: 32 + space.lg },
  mtInfo: { flex: 1, gap: 1 },
  mtSym: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  mtChain: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle },
  mtVals: { alignItems: 'flex-end' },
  mtBal: { fontSize: text.base, ...inter.semibold, fontFamily: font.numeric, color: color.fg.base },
  mtUsd: { fontSize: text.sm, ...inter.regular, fontFamily: font.numeric, color: color.fg.muted },
  mtConfirmList: { flex: 1, gap: space.md },
  inputWrap: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    marginBottom: space.sm,
  },
  input: {
    flex: 1,
    paddingHorizontal: space.xl,
    paddingVertical: space.lg,
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.base,
    maxHeight: 100,
    outlineStyle: 'none',
  } as any,
  inputIcons: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingRight: space.lg,
  },
  // Recent contacts
  contactsCard: {
    marginBottom: space.lg,
    padding: space.sm,
  },
  contactRow: {
    paddingVertical: space.md,
    paddingHorizontal: space.lg,
  },
  contactAddr: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
  },
  contactSep: {
    height: 1,
    backgroundColor: color.border.base,
    marginHorizontal: space.lg,
  },

  // Amount hero — open on the page (no box); the big number leads
  amountWrap: {
    paddingVertical: space.md,
    marginBottom: space.lg,
  },
  amountTopRow: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  amountInputWrap: {
    flex: 1,
    overflow: 'hidden',
  },
  amountInput: {
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
    padding: 0,
    outlineStyle: 'none',
  } as any,
  unitLabel: {
    ...inter.medium,
    color: color.fg.subtle,
    marginLeft: space.sm,
    flexShrink: 0,
  },
  // MAX — a soft, borderless light chip (matches the filter-chip language)
  maxBtn: {
    paddingVertical: space.sm,
    paddingHorizontal: space.lg,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
  },
  maxBtnText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
  },
  conversionRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginTop: space.md,
  },
  conversionText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  amountWarning: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.error.base,
    marginTop: space.sm,
    marginBottom: space.sm,
    paddingHorizontal: space.xs,
  },
  continueBtn: {
    marginTop: space.lg,
  },
  // Confirm — transfer review, open on the page (de-boxed)
  confirmBlock: {
    marginBottom: space.lg,
  },
  // From → To flow (simple 1→1 transfer) — money follows the person (− / +).
  party: {
    flexDirection: 'row',
    alignItems: 'flex-start',
    gap: space.md,
  },
  partyWho: {
    flex: 1,
    minWidth: 0,
    gap: 2,
  },
  partyName: {
    fontSize: text.base,
    ...inter.bold,
    color: color.fg.base,
  },
  partyAmt: {
    alignItems: 'flex-end',
    gap: 1,
    flexShrink: 0,
  },
  amtOut: {
    fontSize: text.base,
    ...inter.bold,
    fontFamily: font.numeric,
    color: color.fg.base,
  },
  amtIn: {
    fontSize: text.base,
    ...inter.bold,
    fontFamily: font.numeric,
    color: color.success.base,
  },
  // A taller connector so From → To reads as a real flow (aligned under the avatar
  // column: width == avatar, centred). A short line above and below the arrow.
  flowConnector: {
    width: 38,
    alignItems: 'center',
    gap: 4,
    paddingVertical: space.sm,
  },
  nameRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
  },
  recipientIdentityRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginTop: space.md,
  },
  // Split mode: recipients as a fixed-height, internally-scrolling list (≤5 visible).
  recipientListLabel: {
    marginBottom: space.sm,
  },
  recipientList: {
    maxHeight: 320,
  },
  recipientRow: {
    alignItems: 'center',
    paddingVertical: space.md,
  },
  recipientIndex: {
    minWidth: 16,
    textAlign: 'center' as const,
    fontSize: text.xs,
    ...inter.semibold,
    fontFamily: font.numeric,
    color: color.fg.subtle,
  },
  assetChip: {
    flexDirection: 'row',
    alignItems: 'center',
    alignSelf: 'flex-start',
    gap: space.sm,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
    paddingVertical: space.sm,
    paddingLeft: space.sm,
    paddingRight: space.lg,
    marginTop: space.lg,
  },
  assetChipText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
  },
  transferEndpoint: {
    gap: 2,
  },
  transferLabel: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.5,
  },
  transferAddr: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.muted,
  },
  transferMiddle: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space['3xl'],
    marginVertical: space['2xl'],
  },
  transferLineCol: {
    alignSelf: 'stretch',
    alignItems: 'center',
    width: space.lg,
    paddingLeft:space['2xl']
  },
  transferLine: {
    width: 1,
    flex: 1,
    backgroundColor: color.border.base,
  },
  // Token line in the confirm review — open row (de-boxed, no sunken panel)
  transferTokenRow: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.md,
  },
  transferTokenIdentity: {
    flex: 1,
    gap: 1,
  },
  transferTokenSymbol: {
    fontSize: text.base,
    ...inter.bold,
    color: color.fg.base,
  },
  transferTokenChain: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
  },
  transferTokenValues: {
    alignItems: 'flex-end' as const,
    gap: 1,
  },
  transferTokenAmount: {
    fontSize: text.base,
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
  },
  transferTokenSub: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
  },
  confirmBtn: {
    marginTop: space.md,
    // Keep the slide clear of the iPhone home-indicator band: a horizontal drag
    // that starts inside it is the SYSTEM app-switch gesture, not ours. The
    // screen has no bottom safe-area edge, so the clearance lives here.
    marginBottom: space['5xl'],
  },
  sameAssetFeeWarning: {
    flexDirection: 'row',
    alignItems: 'flex-start',
    gap: space.md,
    marginTop: space.sm,
    marginBottom: space.md,
    padding: space.lg,
    borderRadius: radius.lg,
    backgroundColor: color.error.soft,
    borderWidth: 1,
    borderColor: color.error.base,
  },
  sameAssetFeeWarningCopy: {
    flex: 1,
    gap: space.xs,
  },
  sameAssetFeeWarningTitle: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.error.base,
  },
  sameAssetFeeWarningBody: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 20,
  },
  sameAssetFeeWarningMax: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    marginTop: 2,
  },

  // Inline tx status
  txStatusWrap: {
    marginTop: space.xl,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.xl,
    padding: space.xl,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  txStatusRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  txSpinner: {
    width: 20,
    height: 20,
  },
  txStatusText: {
    fontSize: text.base,
    ...inter.medium,
    color: color.fg.muted,
    flex: 1,
  },
  txStatusSuccess: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.success.base,
  },
  txStatusHash: {
    fontSize: text.xs,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.accent.base,
    marginTop: 2,
    textDecorationLine: 'underline',
  },
  txStatusError: {
    fontSize: text.base,
    ...inter.medium,
    color: color.error.base,
    flex: 1,
  },
  txStatusActions: {
    marginTop: space.xl,
  },
  txDoneBtn: {
    backgroundColor: color.accent.base,
    borderRadius: radius.xl,
    paddingVertical: space.lg,
    alignItems: 'center',
  },
  txDoneBtnText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.inverse,
  },
  txRetryBtn: {
    backgroundColor: color.bg.base,
    borderRadius: radius.xl,
    paddingVertical: space.lg,
    alignItems: 'center',
    borderWidth: 1,
    borderColor: color.border.base,
  },
  txRetryBtnText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
  },
  txConfirmTime: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.accent.base,
    marginTop: 2,
  },
  txProgressTrack: {
    height: 4,
    backgroundColor: color.border.base,
    borderRadius: 2,
    marginTop: space.lg,
    overflow: 'hidden' as const,
  },
  txProgressFill: {
    height: 4,
    backgroundColor: color.accent.base,
    borderRadius: 2,
  },
  txProgressFillSlow: {
    backgroundColor: color.warning.base,
  },
  txConfirmHint: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    marginTop: space.sm,
  },

  // EIP-681 lock states
  lockLoading: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
  },
  lockErrorWrap: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: space['3xl'],
    gap: space.lg,
  },
  lockErrorIcon: {
    width: 64,
    height: 64,
    borderRadius: 32,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.sm,
  },
  lockErrorTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    textAlign: 'center',
  },
  lockErrorBody: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: 22,
  },
  lockErrorMsg: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.error.base,
    textAlign: 'center',
  },
  lockErrorBtn: {
    alignSelf: 'stretch',
    marginTop: space.md,
  },
  lockErrorCancel: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.muted,
    padding: space.md,
  },
}));
