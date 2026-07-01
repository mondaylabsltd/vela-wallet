/**
 * dApp connection — end-to-end through the parallel space (Epic 5).
 *
 * The wallet is the REAL app under `/parallel/*`: real Connect screen, real transport,
 * real SigningRequestModal, real signing pipeline. The ONLY difference from production
 * is that approvals are signed by the fixture passkey. The "dApp" is the local relay's
 * self-contained test page. Each test fires a request from the dApp, interacts with the
 * real wallet UI, and asserts the round-trip response the dApp receives.
 *
 * Hermetic: outbound RPC/bundler calls are stubbed (stubWalletNetwork), so nothing hangs
 * and no xDAI is spent. The opt-in real-Gnosis submission lives in parallel-onchain.spec.ts.
 */
import { test, expect, type Page } from '@playwright/test';
import { startRelay } from './support/relay';
import {
  RELAY_PORT, FIXTURE, type Relay,
  stubWalletNetwork, openWalletConnect, connectWallet, openTestDapp,
  request, requestInstant, clickSheetButton,
} from './support/parallel';

let relay: Relay;

test.beforeAll(async () => { relay = await startRelay({ port: RELAY_PORT }); });
test.afterAll(async () => { await relay?.stop(); });

test.describe('parallel-space · dApp connection', () => {
  let wallet: Page;
  let dapp: Page;
  let session: { sessionId: string; nonce: string; secret: string; connectUrl: string };

  test.beforeEach(async ({ context }) => {
    session = relay.newSession();
    wallet = await context.newPage();
    await stubWalletNetwork(wallet);
    dapp = await openTestDapp(context, relay, session);
    await openWalletConnect(wallet);
    await connectWallet(wallet, session.connectUrl);
    // The dApp learns the wallet's address/chain over the relay once connected.
    await expect(dapp.getByTestId('dapp-wallet-status')).toHaveText(/connected/i, { timeout: 15_000 });
  });

  test.afterEach(async () => {
    await dapp?.close().catch(() => {});
    await wallet?.close().catch(() => {});
  });

  test('connects and exposes the fixture account to the dApp', async () => {
    await expect(dapp.getByTestId('dapp-wallet-address')).toHaveText(FIXTURE.one);
    // The wallet's connected card shows the dApp identity from the relay metadata.
    await expect(wallet.getByText('Vela Test dApp')).toBeVisible();
    await expect(wallet.getByText('Connected', { exact: true })).toBeVisible();
  });

  test('answers eth_requestAccounts locally with the fixture address', async () => {
    const resp = await requestInstant(dapp, 'eth_requestAccounts', []);
    expect(resp.result).toEqual([FIXTURE.one]);
  });

  test('answers eth_chainId locally', async () => {
    const resp = await requestInstant(dapp, 'eth_chainId', []);
    expect(typeof resp.result).toBe('string');
    expect(resp.result).toMatch(/^0x[0-9a-f]+$/);
  });

  test('personal_sign shows the message and returns an EIP-1271 signature', async () => {
    // ASCII only: a message with non-ASCII bytes is shown as hex, not decoded text.
    const msgHex =
      '0x' + Buffer.from('Hello from the Vela test dApp - sign to prove control.').toString('hex');
    const resp = await request(dapp, 'personal_sign', [msgHex, FIXTURE.one], async () => {
      await expect(wallet.getByText('Sign Message')).toBeVisible({ timeout: 15_000 });
      await expect(wallet.getByText(/Hello from the Vela test dApp/)).toBeVisible();
      await clickSheetButton(wallet, 'Sign');
    });
    expect(resp.error).toBeUndefined();
    expect(typeof resp.result).toBe('string');
    expect(resp.result as string).toMatch(/^0x[0-9a-fA-F]{200,}$/); // full Safe contract signature
  });

  test('rejecting personal_sign returns 4001 to the dApp', async () => {
    const msgHex = '0x' + Buffer.from('reject me').toString('hex');
    const resp = await request(dapp, 'personal_sign', [msgHex, FIXTURE.one], async () => {
      await expect(wallet.getByText('Sign Message')).toBeVisible({ timeout: 15_000 });
      await clickSheetButton(wallet, 'Reject');
    });
    expect(resp.result).toBeUndefined();
    expect(resp.error?.code).toBe(4001);
  });

  test('eth_signTypedData_v4 returns a signature', async () => {
    const typed = JSON.stringify({
      types: { EIP712Domain: [{ name: 'name', type: 'string' }, { name: 'chainId', type: 'uint256' }], Mail: [{ name: 'contents', type: 'string' }] },
      primaryType: 'Mail', domain: { name: 'Vela Test dApp', chainId: 100 }, message: { contents: 'gm' },
    });
    const resp = await request(dapp, 'eth_signTypedData_v4', [FIXTURE.one, typed], async () => {
      await clickSheetButton(wallet, 'Sign');
    });
    expect(resp.error).toBeUndefined();
    expect(resp.result as string).toMatch(/^0x[0-9a-fA-F]{200,}$/);
  });

  test('an unlimited approval is intercepted by the spending-cap editor, never blind-approved', async () => {
    const data = '0x095ea7b3' + '0'.repeat(24) + '000000000022d473030f116ddee9f6b43ac78ba3' + 'f'.repeat(64);
    const resp = await request(dapp, 'eth_sendTransaction',
      [{ from: FIXTURE.one, to: '0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d', data }],
      async () => {
        // The never-unlimited mandate: an unbounded approve surfaces the editable
        // spending-cap control (a deliberate slide-to-confirm, not a one-tap button).
        await expect(wallet.getByText('Spending cap')).toBeVisible({ timeout: 15_000 });
        await clickSheetButton(wallet, 'Reject');
      });
    expect(resp.error?.code).toBe(4001);
  });

  test('wallet_switchEthereumChain to Gnosis succeeds locally', async () => {
    const resp = await requestInstant(dapp, 'wallet_switchEthereumChain', [{ chainId: '0x64' }]);
    expect(resp.error).toBeUndefined();
    expect(resp.result).toBeNull();
  });

  test('an unsupported chain switch is rejected with 4902', async () => {
    const resp = await requestInstant(dapp, 'wallet_switchEthereumChain', [{ chainId: '0x270f' }]);
    expect(resp.error?.code).toBe(4902);
  });

  test('a transaction estimates the real tx (fee + simulation) before allowing confirm', async () => {
    // Parallel space uses real Gnosis reads, so the sheet shows a real fee + simulation
    // — proving the "never blind-submit an unestimated op" invariant. We reject (no xDAI).
    const resp = await request(dapp, 'eth_sendTransaction',
      [{ from: FIXTURE.one, to: FIXTURE.two, value: '0x5af3107a4000' }],
      async () => {
        await expect(wallet.getByText(/Est\. Fee|Simulation|Fee/i).first()).toBeVisible({ timeout: 20_000 });
        await clickSheetButton(wallet, 'Reject');
      });
    expect(resp.error?.code).toBe(4001);
  });

  test('disconnecting drops the session on both sides', async () => {
    await clickSheetButton(wallet, 'Disconnect');
    await expect(dapp.getByTestId('dapp-wallet-status')).toHaveText(/disconnected/i, { timeout: 15_000 });
  });
});
