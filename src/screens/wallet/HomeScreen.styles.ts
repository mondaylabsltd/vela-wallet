import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';

// Shimmer placeholder geometry — shared by the skeleton style (below) and the
// <BalanceSkeleton> animation math in BalanceDisplay.
export const SKELETON_W = 208;
export const SKELETON_H = 46;
export const SKELETON_BAND_W = 96;

export const styles = createStyles(() => ({
  root: { flex: 1, backgroundColor: color.bg.base },
  safe: { flex: 1 },

  // Header
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingHorizontal: space['3xl'],
    paddingTop: space.md,
    paddingBottom: space.lg,
  },
  // De-boxed header (Wise): account + settings sit openly on the page, no card.
  account: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.xs,
  },
  accountInfo: { flex: 1, minWidth: 0 },
  accountNameRow: { flexDirection: 'row', alignItems: 'center', gap: space.xs },
  accountName: { fontSize: text.lg, ...inter.bold, color: color.fg.base, flexShrink: 1 },
  accountAddr: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle, fontFamily: font.mono },
  iconBtn: {
    width: 44, height: 44,
    alignItems: 'center', justifyContent: 'center',
  },
  iconBtnMuted: {},

  // Balance — OPEN hero (Wise): sits directly on the page, no card. Grouped by
  // space + a section label, not by a box. Premium via generous space + type.
  balanceCard: {
    paddingTop: space.lg,
    paddingBottom: space['2xl'],
  },
  balanceLabel: { fontSize: text.sm, ...inter.semibold, color: color.fg.subtle, letterSpacing: 0.6, textTransform: 'uppercase' },
  balanceTopRow: { flexDirection: 'row', alignItems: 'center', marginTop: space.sm },
  balanceFill: { flex: 1 },
  balanceInt: { fontSize: 52, ...inter.bold, fontFamily: font.display, color: color.fg.base, letterSpacing: -1.2 },
  balanceDec: { fontSize: 28, ...inter.bold, fontFamily: font.display, color: color.fg.subtle, letterSpacing: -0.5 },
  // Loading skeleton (sized to the balance line box): a sunken bar with a
  // sweeping raised band — reads as a highlight in both light and dark.
  balanceSkeleton: {
    width: SKELETON_W,
    height: SKELETON_H,
    marginVertical: (63 - SKELETON_H) / 2, // center within the ~63px balance line
    borderRadius: radius.md,
    backgroundColor: color.bg.sunken,
    overflow: 'hidden',
  },
  balanceSkeletonBand: {
    position: 'absolute',
    top: 0,
    bottom: 0,
    width: SKELETON_BAND_W,
    backgroundColor: color.bg.raised,
    opacity: 0.85,
  },
  // Masked state: fixed-size View dots (NOT bullet glyphs — those render wide and
  // wrap to a second line on Android) + the only chrome the hero ever shows (EyeOff
  // glyph). Row height is pinned to the ~63px balance line box so toggling privacy
  // doesn't shift the hero up/down.
  balanceHiddenRow: { flex: 1, flexDirection: 'row', alignItems: 'center', gap: space.md, height: 63 },
  balanceDots: { flexDirection: 'row', alignItems: 'center', gap: 12 },
  balanceDot: { width: 16, height: 16, borderRadius: 8, backgroundColor: color.fg.base },
  // Parity with the holdings view: when a chain read failed or a held token is
  // unpriced, the hero total is an estimate — say so, not a confident number.
  balanceStaleRow: { flexDirection: 'row', alignItems: 'center', gap: space.xs, marginTop: space.md, alignSelf: 'flex-start' },
  balanceStalePressed: { opacity: 0.6 },
  balanceStaleText: { fontSize: text.sm, ...inter.medium, color: color.warning.base },

  // Receipt toast
  toast: {
    position: 'absolute', alignSelf: 'center', zIndex: 50,
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    backgroundColor: color.success.base,
    paddingVertical: space.md, paddingHorizontal: space.xl,
    borderRadius: radius.full, ...shadow.lg,
  },
  toastDot: { width: 8, height: 8, borderRadius: 4, backgroundColor: color.fg.inverse },
  toastText: { fontSize: text.lg, ...inter.bold, color: color.fg.inverse },

  // Nav row — tabs are content-sized (scrollable), so push the network filter
  // to the right edge explicitly.
  navRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: space.md, marginBottom: space.lg },

  // List
  // paddingBottom (dock clearance) is inset-dependent — applied via listContentStyle.
  listContent: { paddingHorizontal: space['3xl'] },
  // Hairline divider between de-boxed rows, inset past the avatar (Apple-Wallet style)
  // so it aligns under the row's text, not the icon.
  sep: { height: 1, backgroundColor: color.border.base, marginLeft: 44 + space.lg + space.xs },
  // Date group header — quiet, uppercase-free date label above each day's rows.
  dayHeader: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.subtle,
    marginTop: space.xl,
    marginBottom: space.sm,
  },
  empty: { alignItems: 'center', paddingTop: space['5xl'], gap: space.md },
  emptyIcon: {
    width: 64, height: 64, borderRadius: 32,
    backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
    marginBottom: space.sm,
  },
  emptyText: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  emptySub: { fontSize: text.base, ...inter.regular, color: color.fg.subtle, textAlign: 'center', paddingHorizontal: space['3xl'], lineHeight: 20 },


  // Connections
  connCard: { padding: space.xl, marginBottom: space.xl },
  connTop: { flexDirection: 'row', alignItems: 'center', gap: space.lg },
  connDapp: { width: 44, height: 44, borderRadius: 13, backgroundColor: color.bg.sunken, alignItems: 'center', justifyContent: 'center' },
  connDappText: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  connInfo: { flex: 1, gap: 2 },
  connName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  connUrl: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  connStatus: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  connDot: { width: 7, height: 7, borderRadius: 4, backgroundColor: color.success.base },
  connDotReconnecting: { backgroundColor: color.warning.base, opacity: 0.8 },
  connStatusText: { fontSize: text.sm, ...inter.semibold, color: color.success.base },
  connStatusTextReconnecting: { color: color.warning.base },
  connNote: { fontSize: text.sm, ...inter.medium, color: color.fg.muted, marginTop: space.lg },
  connNoteWarn: { color: color.warning.base },
  reconnectBtn: {
    marginTop: space.lg, flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: space.sm,
    paddingVertical: space.lg, borderRadius: radius.lg, backgroundColor: color.accent.base, ...shadow.sm,
  },
  reconnectBtnPressed: { opacity: 0.82, transform: [{ scale: 0.985 }] },
  reconnectText: { fontSize: text.base, ...inter.semibold, color: color.fg.inverse },
  disconnectBtn: {
    marginTop: space.lg, alignItems: 'center',
    paddingVertical: space.lg, borderRadius: radius.lg,
    borderWidth: 1, borderColor: color.border.base, backgroundColor: color.bg.raised,
  },
  disconnectText: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  connEventsHeadRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: space.md },
  connEventsHead: { fontSize: text.sm, ...inter.semibold, color: color.fg.subtle, textTransform: 'uppercase', letterSpacing: 0.8 },
  connClearBtn: { flexDirection: 'row', alignItems: 'center', gap: space.xs, paddingVertical: space.xs, paddingHorizontal: space.sm },
  connClearText: { fontSize: text.sm, ...inter.semibold, color: color.fg.subtle },
  connNoEvents: { fontSize: text.base, ...inter.regular, color: color.fg.subtle },
  eventRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.lg, borderBottomWidth: 1, borderBottomColor: color.border.base,
    backgroundColor: color.bg.base,
  },
  eventInfo: { flex: 1, gap: 2 },
  eventLabel: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  eventSub: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  eventTime: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle },
  eventDelete: {
    backgroundColor: color.error.base, flexDirection: 'row', alignItems: 'center',
    justifyContent: 'center', gap: space.xs, paddingHorizontal: space.xl,
  },
  eventDeleteText: { fontSize: text.sm, ...inter.semibold, color: color.fg.inverse },
  eventPill: { paddingHorizontal: space.sm, paddingVertical: 2, borderRadius: radius.full },
  eventPillPending: { backgroundColor: color.info.soft },
  eventPillFailed: { backgroundColor: color.error.soft },
  eventPillText: { fontSize: text.xs, ...inter.semibold },
  eventPillTextPending: { color: color.info.base },
  eventPillTextFailed: { color: color.error.base },

  connHistorySection: { marginTop: space['3xl'] },
  connEmpty: { alignItems: 'center', paddingTop: space['4xl'], gap: space.md },
  connEmptyIcon: { width: 56, height: 56, borderRadius: 28, backgroundColor: color.bg.sunken, alignItems: 'center', justifyContent: 'center' },
  connEmptyTitle: { fontSize: text.xl, ...inter.semibold, color: color.fg.base },
  connEmptySub: { fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center', lineHeight: 20, paddingHorizontal: space.xl },
  connOrRow: { flexDirection: 'row', alignItems: 'center', gap: space.lg, alignSelf: 'stretch', paddingHorizontal: space.xl, marginTop: space.md },
  connOrLine: { flex: 1, height: 1, backgroundColor: color.border.base },
  connOrText: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  connPasteHint: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle, textAlign: 'center', marginBottom: space.sm },
  connPasteRow: { flexDirection: 'row', alignItems: 'flex-end', gap: space.md, alignSelf: 'stretch', paddingHorizontal: space.xl },
  connPasteRowSpaced: { marginTop: space.xl },
  connHistoryBtn: { flexDirection: 'row', alignItems: 'center', gap: space.xs, marginTop: space.xl, paddingVertical: space.xs },
  connHistoryText: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  connPasteInput: {
    // Taller, wider, larger type + multiline wrapping so a long link is readable.
    flex: 1, fontSize: text.base, fontWeight: '500', fontFamily: font.mono, lineHeight: 20,
    color: color.fg.base, paddingHorizontal: space.lg, paddingVertical: space.md,
    minHeight: 56, maxHeight: 108,
    backgroundColor: color.bg.sunken, borderRadius: radius.lg,
    borderWidth: 1, borderColor: color.border.base,
  },
  connPasteBtn: { width: 56, height: 56, borderRadius: radius.lg, backgroundColor: color.accent.base, alignItems: 'center', justifyContent: 'center' },
  connPasteBtnDisabled: { backgroundColor: color.bg.sunken },
}));
