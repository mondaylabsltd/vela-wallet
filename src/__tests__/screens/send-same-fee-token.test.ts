/** Regression guard: the confirmation page must not allow a same-token fee send
 * that already exceeds the wallet balance. The controller is not renderable in
 * this Jest environment, so pin the two boundaries (calculation + recovery UI). */
import { readFileSync } from 'fs';
import { resolve } from 'path';

const root = resolve(__dirname, '../../..');
const controller = readFileSync(resolve(root, 'src/screens/wallet/useSendController.ts'), 'utf8');
const confirm = readFileSync(resolve(root, 'src/screens/wallet/ConfirmStep.tsx'), 'utf8');

describe('same fee-token transfer guard', () => {
  test('calculates a final same-asset fee ceiling and refuses to submit above it', () => {
    expect(controller).toContain('const sameAssetFeeIssue =');
    expect(controller).toContain('sameAssetFeeLimit(');
    expect(controller).toContain('if (sameAssetFeeIssue)');
    expect(controller).toContain('handleEditAmount();');
  });

  test('shows the exact recovery and replaces the send slide with an edit action', () => {
    expect(confirm).toContain("t('send.sameFeeTokenMax'");
    expect(confirm).toContain("title={t('send.sameFeeTokenEdit')}");
    expect(confirm).toContain('onPress={handleEditAmount}');
  });
});
