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

const src = readFileSync(resolve(__dirname, '../../..', 'src/screens/wallet/SendScreen.tsx'), 'utf8');

describe('SendScreen skips the bundler funding gate on Tempo (issue #91)', () => {
  it('the pre-check short-circuits to null for Tempo before checkBundlerFunding', () => {
    // The Tempo skip must appear immediately before the checkBundlerFunding call,
    // so the funding gate + sponsorship probe never run for a Tempo send.
    expect(src).toMatch(/if \(isTempoChain\(chainId\)\) return null;[\s\S]{0,200}?checkBundlerFunding\(/);
  });

  it('still estimates the transaction fee (kept out of the skipped block)', () => {
    expect(src).toContain('estimateTransactionFee(');
    expect(src).toContain('setFeeEstimate(');
  });
});
