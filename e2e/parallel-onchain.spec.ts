/**
 * dApp connection — REAL on-chain settlement on Gnosis (opt-in). [@onchain]
 *
 * The one test that proves the whole stack works together on a live chain: the fixture
 * passkey signs a real ERC-4337 UserOp, the vela bundler submits it, the RIP-7212 P256
 * precompile + EntryPoint + Safe4337Module settle it on Gnosis (chain 100), and the dApp
 * gets a real transaction hash back.
 *
 * OPT-IN + costs real xDAI, so it is skipped unless RUN_ONCHAIN=1, and self-skips if the
 * fixture Safe (Parallel One) isn't funded. Excluded from default CI (@onchain).
 *
 *   1) Fund Parallel One:  0xD400866e00B055B20752a826CD5C89b811de130b  (a little xDAI)
 *   2) RUN_ONCHAIN=1 npx playwright test parallel-onchain.spec.ts
 */
import { test, expect, type Page } from '@playwright/test';
import { startRelay } from './support/relay';
import {
  RELAY_PORT, FIXTURE, type Relay,
  openWalletConnect, connectWallet, openTestDapp, request, confirmSheet,
  gnosisBalanceWei, bundlerGasAccount,
} from './support/parallel';

const MIN_WEI = 200_000_000_000_000n; // ~0.0002 xDAI: enough for the tiny transfer value
const MIN_GAS_WEI = 100_000_000_000_000n; // ~0.0001 xDAI in the bundler gas account

let relay: Relay;

test.describe('@onchain parallel-space · real Gnosis settlement', () => {
  test.skip(!process.env.RUN_ONCHAIN, 'set RUN_ONCHAIN=1 to run the real on-chain test');
  test.setTimeout(180_000); // real bundler submit + receipt wait

  test.beforeAll(async () => { relay = await startRelay({ port: RELAY_PORT + 1 }); });
  test.afterAll(async () => { await relay?.stop(); });

  test('signs and submits a real xDAI transfer, returning a tx hash', async ({ context }) => {
    const balance = await gnosisBalanceWei(FIXTURE.one);
    test.skip(
      balance < MIN_WEI,
      `Fund Parallel One (${FIXTURE.one}) with a little xDAI on Gnosis — balance ${balance} wei`,
    );

    // The vela-bundler pays gas from a per-Safe deposit address, not the Safe balance.
    const gas = await bundlerGasAccount(100, FIXTURE.one);
    test.skip(
      gas.spendableWei < MIN_GAS_WEI,
      `Fund the bundler GAS ACCOUNT for Parallel One with a little xDAI: ${gas.depositAddress} ` +
      `(spendable ${gas.spendableWei} wei). The Safe balance covers the transfer value; the deposit ` +
      `address covers gas. Alternatively use the in-app "request sponsorship" button.`,
    );

    const session = relay.newSession();
    const wallet: Page = await context.newPage();
    const dapp: Page = await openTestDapp(context, relay, session);

    // No network stub here — this hits real Gnosis + the real bundler.
    await openWalletConnect(wallet);
    await connectWallet(wallet, session.connectUrl);
    await expect(dapp.getByTestId('dapp-wallet-status')).toHaveText(/connected/i, { timeout: 20_000 });

    // Send a tiny amount (0.0001 xDAI) from the fixture Safe to Parallel Two.
    const resp = await request(dapp, 'eth_sendTransaction',
      [{ from: FIXTURE.one, to: FIXTURE.two, value: '0x5af3107a4000' }],
      async () => {
        // Wait out the real gas estimate, then confirm — signed by the fixture passkey.
        await expect(wallet.getByText('Reject', { exact: true })).toBeVisible({ timeout: 30_000 });
        // Give the gas estimate a moment so the confirm guard is satisfied.
        await wallet.waitForTimeout(4_000);
        await confirmSheet(wallet);
      });

    expect(resp.error).toBeUndefined();
    // eth_sendTransaction resolves with the on-chain transaction hash.
    expect(resp.result as string).toMatch(/^0x[0-9a-fA-F]{64}$/);
    // eslint-disable-next-line no-console
    console.log('on-chain tx:', `https://gnosisscan.io/tx/${resp.result}`);

    await dapp.close();
    await wallet.close();
  });
});
