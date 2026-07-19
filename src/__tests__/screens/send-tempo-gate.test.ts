/** Source-level guard for the send-page gas flow (the controller is not
 * render-testable in this runner). */
import { readFileSync } from 'fs';
import { resolve } from 'path';

const src = readFileSync(resolve(__dirname, '../../..', 'src/screens/wallet/useSendController.ts'), 'utf8');
const screenSrc = readFileSync(resolve(__dirname, '../../..', 'src/screens/wallet/SendScreen.tsx'), 'utf8');

describe('SendScreen checks only the relayer treasury', () => {
  it('opens the treasury bootstrap sheet from the amount screen before confirm', () => {
    const preCheck = src.slice(src.indexOf('const preCheck = async'), src.indexOf('const timeout ='));
    expect(preCheck).toContain('getTreasuryBootstrap(chainId)');
    expect(src).toMatch(/const bootstrapStatus = await Promise\.race\([\s\S]{0,250}?if \(bootstrapStatus && mountedRef\.current\)[\s\S]{0,250}?setTreasuryBootstrap\(bootstrapStatus\)[\s\S]{0,250}?return;/);
  });

  it('does not inspect or fund the user gas account', () => {
    expect(src).not.toContain('checkBundlerFunding(');
    expect(src).not.toContain('fetchBundlerAccountInfo(');
    expect(src).not.toContain('attemptSilentSponsorship(');
    expect(src).not.toContain('BundlerFundingModal');
  });

  it('removes the personal funding sheet from the send screen', () => {
    expect(screenSrc).not.toContain('BundlerFundingModal');
    expect(screenSrc).toContain('TreasuryBootstrapSheet');
    expect(screenSrc).toMatch(/if \(step === 'enter-details'\)[\s\S]{0,100}?c\.handleContinue\(\)/);
  });

  it('still estimates the transaction fee', () => {
    expect(src).toContain('estimateTransactionFee(');
    expect(src).toContain('setFeeEstimate(');
  });
});
