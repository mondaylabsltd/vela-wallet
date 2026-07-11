/**
 * Global signing request modal — ERC-7730 Clear Signing UI.
 *
 * Renders signing requests with intent-driven, human-readable layouts:
 *   - Clear signed transactions/signatures (descriptor found)
 *   - Plain message signing (personal_sign)
 *   - Blind sign fallback (no descriptor)
 *
 * Design principles:
 *   L1 — Intent: large colored action word (Swap, Send, Approve, Sign)
 *   L2 — Substance: token cards with amounts, recipients, flow arrows
 *   L3 — Context: contract info, chain, details (collapsed)
 *
 * The implementation lives under ./signing/*. This module is a thin barrel that
 * preserves the public API (SigningRequestModal + SigningSheet/SigningSheetProps).
 */
export { SigningRequestModal } from './signing/SigningRequestModal';
export { SigningSheet, type SigningSheetProps } from './signing/SigningSheet';
