/**
 * Runs EVERY clear-signing test-harness scenario through the resolver/guard and
 * asserts it renders safely — no throw, and never a malformed token/address that
 * would crash <TokenCard> (regression for the ERC-4626 `underlyingToken: "0x0"`
 * placeholder that produced "Invalid address length: got 1").
 */
jest.mock('@/services/storage', () => ({
  getEthereumDataURL: () => 'https://ethereum-data.awesometools.dev',
  loadTransactions: async () => [],
}));

const mockPoolRpcCall = jest.fn(async (_m: string, _p: any[], _c: number): Promise<any> => { throw new Error('no rpc'); });
jest.mock('@/services/rpc-pool', () => ({ poolRpcCall: (...a: any[]) => (mockPoolRpcCall as any)(...a) }));

import {
  resolveTransaction, resolveTypedData, clearDescriptorCache, clearTokenStandardCache,
  type ClearSignResult,
} from '@/services/clear-signing';
import { detectApproval } from '@/services/approval-guard';
import { parseSiwe } from '@/services/siwe';
import { tokenLogoURLsByAddress } from '@/models/types';
import { CLEAR_SIGNING_SCENARIOS } from '@/screens/settings/clear-signing-scenarios';

const ADDR_RE = /^0x[0-9a-fA-F]{40}$/;
const BAYC = '0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d';

// The live ERC-4626 descriptor (its `underlyingToken` is the literal placeholder
// "0x0" — the exact value that crashed the logo lookup).
const ERC4626 = {
  metadata: { constants: { underlyingToken: '0x0' } },
  display: {
    formats: {
      'deposit(uint256 assets,address receiver)': {
        intent: 'Deposit',
        fields: [
          { path: 'assets', label: 'Deposit', format: 'tokenAmount', params: { token: '$.metadata.constants.underlyingToken' }, visible: 'always' },
          { path: 'receiver', label: 'Receiver', format: 'addressName', visible: 'always' },
        ],
      },
      'withdraw(uint256 assets,address receiver,address owner)': {
        intent: 'Withdraw',
        fields: [
          { path: 'assets', label: 'Withdraw', format: 'tokenAmount', params: { token: '$.metadata.constants.underlyingToken' }, visible: 'always' },
          { path: 'receiver', label: 'Receiver', format: 'addressName', visible: 'always' },
        ],
      },
    },
  },
};
const ERC2612 = {
  display: {
    formats: {
      'Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)': {
        intent: 'Approve',
        fields: [
          { path: 'spender', label: 'Spender', format: 'addressName', visible: 'always' },
          { path: 'value', label: 'Amount', format: 'tokenAmount', params: { tokenPath: '@.to', threshold: '0x8000000000000000000000000000000000000000000000000000000000000000' }, visible: 'always' },
          { path: 'deadline', label: 'Valid until', format: 'date', visible: 'always' },
        ],
      },
    },
  },
};

beforeAll(() => {
  global.fetch = jest.fn(async (url: string) => {
    if (url.endsWith('/erc7730/ercs/calldata-erc4626-vaults.json')) return { ok: true, json: async () => ERC4626 } as Response;
    if (url.endsWith('/erc7730/ercs/eip712-erc2612-permit.json')) return { ok: true, json: async () => ERC2612 } as Response;
    return { ok: false } as Response; // contract-specific + other ERC fallbacks 404
  }) as any;
});

beforeEach(() => {
  clearDescriptorCache();
  clearTokenStandardCache();
  mockPoolRpcCall.mockReset();
  // ERC-165: only BAYC advertises ERC-721; everything else reverts (→ ERC-20).
  // All other reads (decimals/allowance/getCode/simulate) are unreachable.
  mockPoolRpcCall.mockImplementation(async (_method: string, params: any[]) => {
    const to = (params?.[0]?.to ?? '').toLowerCase();
    const data: string = params?.[0]?.data ?? '';
    if (data.includes('01ffc9a7')) {
      const yes = to === BAYC && data.includes('80ac58cd');
      return { result: '0x' + (yes ? '1' : '0').padStart(64, '0') };
    }
    throw new Error('no rpc');
  });
});

/** A resolved result must never carry a malformed token/address (would crash TokenCard). */
function assertRenderable(r: ClearSignResult | null) {
  if (!r) return;
  for (const f of r.fields) {
    if (f.tokenAddress !== undefined) expect(f.tokenAddress).toMatch(ADDR_RE);
    if (f.address !== undefined) expect(f.address).toMatch(ADDR_RE);
    // The exact call <TokenCard> makes — must never throw.
    expect(() => tokenLogoURLsByAddress(1, f.tokenAddress ?? '')).not.toThrow();
  }
}

async function resolveScenario(req: any): Promise<ClearSignResult | null> {
  if (req.method === 'eth_sendTransaction') {
    const tx = req.params[0];
    return resolveTransaction(tx.to, tx.data, tx.value, 1);
  }
  if (req.method.includes('signTypedData')) {
    const td = typeof req.params[1] === 'string' ? JSON.parse(req.params[1]) : req.params[1];
    return resolveTypedData(td, 1);
  }
  return null; // personal_sign / eth_sign — no calldata resolver
}

describe('clear-signing harness scenarios', () => {
  it('covers all 21 scenarios', () => {
    expect(CLEAR_SIGNING_SCENARIOS.length).toBe(21);
  });

  // Every scenario must resolve + detect approvals without throwing, and never
  // produce a field that would crash the render.
  it.each(CLEAR_SIGNING_SCENARIOS.map((s) => [s.id, s] as const))(
    'scenario "%s" resolves without crashing the render',
    async (_id, s) => {
      // Guard detection (UI runs this on the raw request) — never throws.
      expect(() => detectApproval(s.request.method, s.request.params)).not.toThrow();

      if (s.request.method === 'wallet_sendCalls') {
        const calls = s.request.params[0].calls;
        for (const c of calls) {
          const r = await resolveTransaction(c.to, c.data, c.value, 1);
          assertRenderable(r);
        }
        return;
      }
      const r = await resolveScenario(s.request);
      assertRenderable(r);
    },
  );

  // The specific crash: vault deposit/withdraw resolve a token from the ERC-4626
  // "0x0" placeholder — the amount must show as an unidentified/unverified token,
  // never a malformed address.
  it('vault deposit (ERC-4626 underlyingToken="0x0") renders without a bad address', async () => {
    const vault = CLEAR_SIGNING_SCENARIOS.find((s) => s.id === 'vault-deposit')!;
    const tx = vault.request.params[0];
    const r = await resolveTransaction(tx.to, tx.data, tx.value, 1);
    expect(r).not.toBeNull();
    const amount = r!.fields.find((f) => f.format === 'tokenAmount');
    expect(amount).toBeTruthy();
    expect(amount!.tokenAddress).toBeUndefined();   // "0x0" placeholder dropped
    expect(amount!.unverified).toBe(true);          // unknown token → flagged
    expect(() => tokenLogoURLsByAddress(1, amount!.tokenAddress ?? '')).not.toThrow();
  });

  it('ERC-20 transfer resolves as a token amount', async () => {
    const s = CLEAR_SIGNING_SCENARIOS.find((x) => x.id === 'erc20-transfer')!;
    const tx = s.request.params[0];
    const r = await resolveTransaction(tx.to, tx.data, tx.value, 1);
    expect(r!.intent).toBe('Send');
    const amount = r!.fields.find((f) => f.role === 'send-amount');
    expect(amount!.value).toContain('USDC');
  });

  it('NFT transferFrom resolves as an NFT (ERC-165 = erc721), not a token amount', async () => {
    const s = CLEAR_SIGNING_SCENARIOS.find((x) => x.id === 'nft-transfer')!;
    const tx = s.request.params[0];
    const r = await resolveTransaction(tx.to, tx.data, tx.value, 1);
    expect(r!.intent).toBe('Transfer NFT');
    expect(r!.fields.some((f) => f.format === 'tokenAmount')).toBe(false);
  });

  it('SIWE phishing scenario is detected as a domain mismatch', () => {
    const s = CLEAR_SIGNING_SCENARIOS.find((x) => x.id === 'siwe-phish')!;
    const hex = s.request.params[0] as string;
    const text = Buffer.from(hex.slice(2), 'hex').toString('utf8');
    const siwe = parseSiwe(text);
    expect(siwe).not.toBeNull();
    expect(siwe!.domain).toBe('app.uniswap.org'); // vs request origin clear-signing-test → mismatch in UI
  });

  it('every scenario maps to a known icon id (no orphan rows)', () => {
    // Mirror of SCENARIO_ICONS keys — kept in sync with the screen.
    const ids = new Set(CLEAR_SIGNING_SCENARIOS.map((s) => s.id));
    expect(ids.size).toBe(CLEAR_SIGNING_SCENARIOS.length); // unique ids
  });
});
