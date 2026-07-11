/**
 * recipient-risk — wallet vs contract classification, incl. EIP-7702.
 *
 * The signing sheet badges a recipient "Wallet" or "Contract" from eth_getCode.
 * A 7702-delegated EOA carries code `0xef0100 ++ impl` (23 bytes) yet is still a
 * PERSON'S WALLET — it must not be branded a contract (vitalik.eth already
 * delegates; this only grows). Regression guard for that classification.
 */
let mockCode: string | { error: unknown };
jest.mock('@/services/rpc-pool', () => ({
  poolRpcCall: jest.fn(async () =>
    typeof mockCode === 'string' ? { result: mockCode } : mockCode,
  ),
}));
jest.mock('@/services/storage', () => ({ loadTransactions: jest.fn(async () => []) }));

import { resolveRecipientRisk, clearRecipientRiskCache } from '@/services/recipient-risk';

const ADDR = '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045';

beforeEach(() => clearRecipientRiskCache());

describe('resolveRecipientRisk — isContract', () => {
  it('treats a 7702-delegated EOA (0xef0100 ++ impl) as a WALLET, not a contract', async () => {
    mockCode = '0xef01005a7fc11397e9f0b6ce408e0f813c5b1e8e3c2b9d'; // ef0100 + 20-byte impl (46 hex)
    const r = await resolveRecipientRisk(1, ADDR);
    expect(r.isContract).toBe(false);
  });

  it('treats a bare EOA (0x) as a wallet', async () => {
    mockCode = '0x';
    const r = await resolveRecipientRisk(1, ADDR);
    expect(r.isContract).toBe(false);
  });

  it('treats a real contract (arbitrary bytecode) as a contract', async () => {
    mockCode = '0x60806040523480156100...'; // ordinary bytecode
    const r = await resolveRecipientRisk(1, ADDR);
    expect(r.isContract).toBe(true);
  });

  it('reports unknown (null) when the chain is unreachable', async () => {
    mockCode = { error: { code: -32000, message: 'boom' } };
    const r = await resolveRecipientRisk(1, ADDR);
    expect(r.isContract).toBeNull();
  });
});
