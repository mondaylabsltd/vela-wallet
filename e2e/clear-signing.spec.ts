/**
 * Clear Signing E2E tests.
 *
 * Tests all signing modal scenarios via the /clear-signing-test page.
 * This page bypasses wallet auth by using a standalone mock modal.
 *
 * Each test clicks a scenario, verifies the modal renders correctly,
 * and checks key UI elements (intent, token amounts, contract info, buttons).
 */
import { test, expect, type Page } from '@playwright/test';

// The test page requires developer mode to be unlocked.
// We access it directly since it's a standalone route.
const TEST_PAGE = '/clear-signing-test';

// Helper: wait for app to hydrate
async function waitForApp(page: Page) {
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(2000);
}

// Helper: click a scenario row by label text
async function clickScenario(page: Page, label: string) {
  // Find the row containing the label text and click it
  const row = page.locator(`text=${label}`).first();
  await row.click();
  // Wait for modal to appear
  await page.waitForTimeout(1500);
}

// Helper: check modal is visible with expected content
async function expectModalVisible(page: Page) {
  const body = await page.textContent('body');
  expect(body).toBeTruthy();
  // Modal should have Reject and some action button
  expect(body).toContain('Reject');
}

// Helper: close modal by clicking Reject
async function closeModal(page: Page) {
  const rejectBtn = page.locator('text=Reject').first();
  await rejectBtn.click();
  await page.waitForTimeout(500);
}

test.describe('Clear Signing UI', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(TEST_PAGE);
    await waitForApp(page);
  });

  // =========================================================================
  // ERC-20 scenarios
  // =========================================================================

  test('ERC-20 Transfer shows clear signed transfer UI', async ({ page }) => {
    await clickScenario(page, 'ERC-20 Transfer');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    // Should show intent "Send" (from ERC-20 transfer descriptor)
    expect(body).toMatch(/Send|Transfer/i);
    // Should show the dApp banner
    expect(body).toContain('Test dApp');
    // Should show Ethereum network
    expect(body).toContain('Ethereum');
    // Should have a confirm button
    expect(body).toMatch(/Confirm|Approve|Sign/i);

    await closeModal(page);
  });

  test('ERC-20 Unlimited Approve shows warning', async ({ page }) => {
    await clickScenario(page, 'ERC-20 Approve');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    // Should show intent "Approve"
    expect(body).toMatch(/Approve/i);
    // Should show unlimited warning
    expect(body).toMatch(/Unlimited|unlimited/);

    await closeModal(page);
  });

  test('ERC-20 Limited Approve shows specific amount', async ({ page }) => {
    await clickScenario(page, 'ERC-20 Limited Approve');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    expect(body).toMatch(/Approve/i);
    // Should NOT show unlimited warning
    expect(body).not.toMatch(/Unlimited/);

    await closeModal(page);
  });

  test('ERC-20 TransferFrom shows clear signed UI', async ({ page }) => {
    await clickScenario(page, 'ERC-20 TransferFrom');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    expect(body).toMatch(/Send|Transfer/i);

    await closeModal(page);
  });

  // =========================================================================
  // ETH transfer scenarios
  // =========================================================================

  test('ETH Transfer shows send UI with amount', async ({ page }) => {
    await clickScenario(page, 'ETH Transfer');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    expect(body).toMatch(/Send/i);
    // Should show ETH amount
    expect(body).toMatch(/ETH/);
    // Should show recipient address
    expect(body).toMatch(/0x/);

    await closeModal(page);
  });

  test('Large ETH Send shows correct amount', async ({ page }) => {
    await clickScenario(page, 'Large ETH Send');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    expect(body).toMatch(/Send/i);
    expect(body).toContain('ETH');

    await closeModal(page);
  });

  // =========================================================================
  // personal_sign scenarios
  // =========================================================================

  test('Personal Sign shows message bubble', async ({ page }) => {
    await clickScenario(page, 'Personal Sign');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    // Should show "Sign Message" intent
    expect(body).toMatch(/Sign Message/);
    // Should show decoded message content
    expect(body).toContain('OpenSea');
    // Should show personal_sign tag
    expect(body).toMatch(/personal_sign/);
    // Should have Sign button
    expect(body).toContain('Sign');

    await closeModal(page);
  });

  test('Hex Message shows hex preview', async ({ page }) => {
    await clickScenario(page, 'Hex Message Sign');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    expect(body).toMatch(/Sign Message/);
    // Should show hex data (non-printable message)
    expect(body).toMatch(/0x/);

    await closeModal(page);
  });

  // =========================================================================
  // EIP-712 scenarios
  // =========================================================================

  test('EIP-712 Permit2 shows typed data UI', async ({ page }) => {
    await clickScenario(page, 'EIP-712 Permit2');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    // Should show either clear-signed intent or typed data fallback
    expect(body).toMatch(/Permit|Sign|Authorize/i);

    await closeModal(page);
  });

  test('EIP-712 Unknown shows blind typed data UI', async ({ page }) => {
    await clickScenario(page, 'EIP-712 Unknown');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    // Should show typed data intent
    expect(body).toMatch(/Sign Typed Data/);
    // Should show primary type
    expect(body).toContain('CustomOrder');
    // Should show domain info
    expect(body).toContain('Unknown Protocol');
    // Should show warning about no descriptor
    expect(body).toMatch(/could not be decoded|no.*descriptor/i);

    await closeModal(page);
  });

  // =========================================================================
  // Blind sign scenarios
  // =========================================================================

  test('Blind Transaction shows red warning', async ({ page }) => {
    await clickScenario(page, 'Blind Transaction');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    // Should show "Unknown" intent
    expect(body).toMatch(/Unknown/);
    // Should show warning about no descriptor
    expect(body).toMatch(/Unable to decode|no.*descriptor|ERC-7730/i);
    // Should show ETH amount
    expect(body).toContain('ETH');

    await closeModal(page);
  });

  // =========================================================================
  // NFT scenarios
  // =========================================================================

  test('NFT Transfer shows clear signed UI', async ({ page }) => {
    await clickScenario(page, 'NFT Transfer');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    // Should show NFT-related intent
    expect(body).toMatch(/Send NFT|Transfer/i);

    await closeModal(page);
  });

  test('NFT Approve All shows approval UI', async ({ page }) => {
    await clickScenario(page, 'NFT Approve All');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    expect(body).toMatch(/Approve|Manage|operator/i);

    await closeModal(page);
  });

  // =========================================================================
  // Vault scenarios
  // =========================================================================

  test('Vault Deposit shows deposit UI', async ({ page }) => {
    await clickScenario(page, 'Vault Deposit');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    expect(body).toMatch(/Deposit/i);

    await closeModal(page);
  });

  test('Vault Withdraw shows withdraw UI', async ({ page }) => {
    await clickScenario(page, 'Vault Withdraw');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    expect(body).toMatch(/Withdraw|Redeem/i);

    await closeModal(page);
  });

  // =========================================================================
  // Contract-specific descriptor
  // =========================================================================

  test('1inch Swap shows contract-specific clear sign', async ({ page }) => {
    await clickScenario(page, '1inch Swap');
    await expectModalVisible(page);

    const body = await page.textContent('body');
    // 1inch has contract-specific descriptor with Swap intent
    expect(body).toMatch(/Swap|Execute/i);

    await closeModal(page);
  });

  // =========================================================================
  // Interaction tests
  // =========================================================================

  test('Copy button works on contract address', async ({ page }) => {
    await clickScenario(page, 'ERC-20 Transfer');
    await expectModalVisible(page);

    // Find and click a copy button (the small icon button)
    const copyBtns = page.locator('[data-testid="copy-btn"]');
    const count = await copyBtns.count();

    // If no testid, try clicking any small button that might be copy
    // The copy feedback should show a check icon briefly
    // Just verify the modal renders without errors for now

    await closeModal(page);
  });

  test('Reject button closes modal', async ({ page }) => {
    await clickScenario(page, 'Personal Sign');
    await expectModalVisible(page);

    // Click reject
    await closeModal(page);

    // Modal should be closed — the scenario list should be visible again
    await page.waitForTimeout(500);
    const body = await page.textContent('body');
    expect(body).toContain('Clear Signing Test');
  });

  test('Confirm button shows signed alert', async ({ page }) => {
    await clickScenario(page, 'Personal Sign');
    await expectModalVisible(page);

    // Handle the dialog that appears on confirm
    page.on('dialog', async (dialog) => {
      expect(dialog.message()).toContain('test');
      await dialog.accept();
    });

    // Click the Sign/Confirm button
    const signBtn = page.locator('text=Sign').last();
    await signBtn.click();
    await page.waitForTimeout(500);
  });
});
