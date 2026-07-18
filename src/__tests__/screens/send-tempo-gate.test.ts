/**
 * Issue #91 (FIX A) — a new/undeployed Tempo account's send was routed into the
 * bundler gas-account funding gate + silent-sponsorship probe, which is wrong for
 * Tempo (gas is settled in-band from the user's own pathUSD by sendUserOpTempo).
 * The gate's ~20s grant wait dead-ended with no feedback. The pre-check must skip
 * the funding gate for Tempo while still estimating the fee. Source-level guard
 * (the component isn't render-testable in this runner).
 */
import { readFileSync } from 'fs';
import { resolve } from 'path';

// The funding-gate pre-check moved from SendScreen.tsx into the extracted controller
// (refactor: split SendScreen into controller + view). The guard now lives here.
const src = readFileSync(resolve(__dirname, '../../..', 'src/screens/wallet/useSendController.ts'), 'utf8');

describe('SendScreen skips the bundler funding gate on Tempo (issue #91)', () => {
  it('the pre-check short-circuits to null for Tempo before checkBundlerFunding', () => {
    // The Tempo skip must appear shortly before the checkBundlerFunding call,
    // so the funding gate + sponsorship probe never run for a Tempo send. The
    // in-band skip (generic chains, same settlement model — the per-safe EOA is
    // operator float, never a user deposit bucket) sits between them.
    expect(src).toMatch(/if \(isTempoChain\(chainId\)\) return null;[\s\S]{0,800}?checkBundlerFunding\(/);
  });

  it('the pre-check also short-circuits for generic in-band chains before checkBundlerFunding', () => {
    expect(src).toMatch(/fee\?\.inBand \|\| await isInBandChain\(chainId,[\s\S]{0,400}?checkBundlerFunding\(/);
  });

  it('still estimates the transaction fee (kept out of the skipped block)', () => {
    expect(src).toContain('estimateTransactionFee(');
    expect(src).toContain('setFeeEstimate(');
  });
});
