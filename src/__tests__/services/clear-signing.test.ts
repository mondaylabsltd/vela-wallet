/**
 * ERC-7730 Clear Signing service tests.
 *
 * Tests field resolution, risk assessment, format types, and role inference.
 * Does NOT test network fetching (mocked) — only the resolution logic.
 */

// Mock the fetch-based descriptor loading
const mockDescriptorCache = new Map<string, any>();
jest.mock('@/services/storage', () => ({
  getEthereumDataURL: () => 'https://ethereum-data.awesometools.dev',
}));

// Mock fetch globally
const originalFetch = global.fetch;
beforeAll(() => {
  global.fetch = jest.fn(async (url: string) => {
    const path = url.replace('https://ethereum-data.awesometools.dev', '');
    const cached = mockDescriptorCache.get(path);
    if (cached) {
      return { ok: true, json: async () => cached } as Response;
    }
    return { ok: false } as Response;
  }) as any;
});
afterAll(() => {
  global.fetch = originalFetch;
});

import {
  resolveTransaction,
  resolveTypedData,
  clearDescriptorCache,
  type ClearSignResult,
  type SigningRisk,
} from '@/services/clear-signing';

// ---------------------------------------------------------------------------
// Helper: load a mock ERC-20 descriptor
// ---------------------------------------------------------------------------

function loadERC20Descriptor() {
  mockDescriptorCache.set('/erc7730/ercs/calldata-erc20-tokens.json', {
    context: { contract: {} },
    display: {
      formats: {
        'transfer(address _to, uint256 _value)': {
          intent: 'Send',
          fields: [
            { path: '_value', label: 'Amount', format: 'tokenAmount', params: { tokenPath: '@.to' }, visible: 'always' },
            { path: '_to', label: 'To', format: 'addressName', visible: 'always' },
          ],
        },
        'approve(address _spender, uint256 _value)': {
          intent: 'Approve',
          fields: [
            { path: '_spender', label: 'Spender', format: 'addressName', visible: 'always' },
            {
              path: '_value', label: 'Amount', format: 'tokenAmount',
              params: { tokenPath: '@.to', threshold: '0x8000000000000000000000000000000000000000000000000000000000000000' },
              visible: 'always',
            },
          ],
        },
      },
    },
  });
}

function loadERC2612Descriptor() {
  mockDescriptorCache.set('/erc7730/ercs/eip712-erc2612-permit.json', {
    context: { eip712: {} },
    display: {
      formats: {
        'Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)': {
          intent: 'Authorize spending of tokens',
          fields: [
            { path: 'spender', label: 'Spender', format: 'raw', visible: 'always' },
            { path: 'value', label: 'Max spending amount', format: 'tokenAmount', params: { tokenPath: '@.to' }, visible: 'always' },
            { path: 'deadline', label: 'Valid until', format: 'date', params: { encoding: 'timestamp' } },
            { path: 'owner', label: 'Owner', visible: 'never' },
            { path: 'nonce', label: 'Nonce', visible: 'never' },
          ],
        },
      },
    },
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('ERC-7730 Clear Signing', () => {
  beforeEach(() => {
    mockDescriptorCache.clear();
    clearDescriptorCache();
  });

  describe('resolveTransaction', () => {
    it('returns null for plain ETH transfer (no calldata)', async () => {
      const result = await resolveTransaction(
        '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
        '0x',
        '0xde0b6b3a7640000',
        1,
      );
      expect(result).toBeNull();
    });

    it('returns null for empty data', async () => {
      const result = await resolveTransaction(
        '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
        '',
        '0x0',
        1,
      );
      expect(result).toBeNull();
    });

    it('returns null when no descriptor matches', async () => {
      const result = await resolveTransaction(
        '0x1234567890abcdef1234567890abcdef12345678',
        '0xdeadbeef00000000000000000000000000000000000000000000000000000000',
        '0x0',
        1,
      );
      expect(result).toBeNull();
    });

    it('resolves ERC-20 transfer with correct intent and fields', async () => {
      loadERC20Descriptor();

      // transfer(address, uint256) = 0xa9059cbb
      const calldata = '0xa9059cbb' +
        '000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045' + // to
        '00000000000000000000000000000000000000000000000000000000003b9aca00'; // value

      const result = await resolveTransaction(
        '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        calldata,
        '0x0',
        1,
      );

      expect(result).not.toBeNull();
      expect(result!.intent).toBe('Send');
      expect(result!.type).toBe('transaction');
      expect(result!.verified).toBe(false); // ERC fallback, not contract-specific
      expect(result!.fields.length).toBeGreaterThan(0);
    });

    it('resolves ERC-20 unlimited approve with warning', async () => {
      loadERC20Descriptor();

      // approve(address, uint256) = 0x095ea7b3, max uint256
      const calldata = '0x095ea7b3' +
        '000000000000000000000000111111125421ca6dc452d289314280a0f8842a65' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff';

      const result = await resolveTransaction(
        '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
        calldata,
        '0x0',
        1,
      );

      expect(result).not.toBeNull();
      expect(result!.intent).toBe('Approve');
      expect(result!.risk).toBe('danger'); // unlimited approval
      expect(result!.fields.some(f => f.warning)).toBe(true);
    });

    it('resolves ERC-20 limited approve without warning', async () => {
      loadERC20Descriptor();

      // approve with 500 USDC (0x1DCD6500)
      const calldata = '0x095ea7b3' +
        '0000000000000000000000003fc91a3afd70395cd496c647d5a6cc9d4b2b7fad' +
        '000000000000000000000000000000000000000000000000000000001dcd6500';

      const result = await resolveTransaction(
        '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
        calldata,
        '0x0',
        1,
      );

      expect(result).not.toBeNull();
      expect(result!.intent).toBe('Approve');
      expect(result!.risk).toBe('caution'); // limited approval, no warning field
      expect(result!.fields.some(f => f.warning)).toBe(false);
    });

    it('sets correct field roles for transfer', async () => {
      loadERC20Descriptor();

      const calldata = '0xa9059cbb' +
        '000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045' +
        '00000000000000000000000000000000000000000000000000000000003b9aca00';

      const result = await resolveTransaction(
        '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
        calldata,
        '0x0',
        1,
      );

      expect(result).not.toBeNull();
      const sendFields = result!.fields.filter(f => f.role === 'send-amount');
      const recipientFields = result!.fields.filter(f => f.role === 'recipient');

      expect(sendFields.length).toBeGreaterThan(0);
      expect(recipientFields.length).toBeGreaterThan(0);
    });

    it('includes contractAddress in result', async () => {
      loadERC20Descriptor();

      const calldata = '0xa9059cbb' +
        '000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045' +
        '00000000000000000000000000000000000000000000000000000000003b9aca00';

      const result = await resolveTransaction(
        '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
        calldata,
        '0x0',
        1,
      );

      expect(result!.contractAddress).toBe('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48');
    });
  });

  describe('resolveTypedData', () => {
    it('returns null when no descriptor matches', async () => {
      const typedData = {
        types: {
          EIP712Domain: [{ name: 'name', type: 'string' }],
          Unknown: [{ name: 'value', type: 'uint256' }],
        },
        primaryType: 'Unknown',
        domain: { name: 'Test', verifyingContract: '0x1234567890abcdef1234567890abcdef12345678' },
        message: { value: '100' },
      };

      const result = await resolveTypedData(typedData, 1);
      expect(result).toBeNull();
    });

    it('resolves ERC-2612 Permit with correct fields', async () => {
      loadERC2612Descriptor();

      const typedData = {
        types: {
          EIP712Domain: [
            { name: 'name', type: 'string' },
            { name: 'chainId', type: 'uint256' },
            { name: 'verifyingContract', type: 'address' },
          ],
          Permit: [
            { name: 'owner', type: 'address' },
            { name: 'spender', type: 'address' },
            { name: 'value', type: 'uint256' },
            { name: 'nonce', type: 'uint256' },
            { name: 'deadline', type: 'uint256' },
          ],
        },
        primaryType: 'Permit',
        domain: {
          name: 'USD Coin',
          chainId: 1,
          verifyingContract: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
        },
        message: {
          owner: '0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1',
          spender: '0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD',
          value: '1000000000',
          nonce: '0',
          deadline: '1750000000',
        },
      };

      const result = await resolveTypedData(typedData, 1);

      expect(result).not.toBeNull();
      expect(result!.intent).toBe('Authorize spending of tokens');
      expect(result!.type).toBe('signature');
      // Hidden fields (owner, nonce) should be filtered out
      const visibleLabels = result!.fields.map(f => f.label);
      expect(visibleLabels).not.toContain('Owner');
      expect(visibleLabels).not.toContain('Nonce');
      // Visible fields should be present
      expect(visibleLabels).toContain('Spender');
    });

    it('returns null when verifyingContract is missing', async () => {
      const typedData = {
        types: { EIP712Domain: [], Test: [{ name: 'x', type: 'uint256' }] },
        primaryType: 'Test',
        domain: {},
        message: { x: '1' },
      };
      const result = await resolveTypedData(typedData, 1);
      expect(result).toBeNull();
    });
  });

  describe('risk assessment', () => {
    it('approve intent without warning = caution', async () => {
      loadERC20Descriptor();

      const calldata = '0x095ea7b3' +
        '0000000000000000000000003fc91a3afd70395cd496c647d5a6cc9d4b2b7fad' +
        '000000000000000000000000000000000000000000000000000000001dcd6500';

      const result = await resolveTransaction('0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', calldata, '0x0', 1);
      expect(result!.risk).toBe('caution');
    });

    it('approve intent with warning field = danger', async () => {
      loadERC20Descriptor();

      const calldata = '0x095ea7b3' +
        '000000000000000000000000111111125421ca6dc452d289314280a0f8842a65' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff';

      const result = await resolveTransaction('0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', calldata, '0x0', 1);
      expect(result!.risk).toBe('danger');
    });

    it('send intent = normal', async () => {
      loadERC20Descriptor();

      const calldata = '0xa9059cbb' +
        '000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045' +
        '00000000000000000000000000000000000000000000000000000000003b9aca00';

      const result = await resolveTransaction('0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', calldata, '0x0', 1);
      expect(result!.risk).toBe('normal');
    });
  });

  describe('field resolution safety check', () => {
    it('falls back to null when too many fields fail to resolve', async () => {
      // Create a descriptor with 4 visible fields but only 1 can resolve
      mockDescriptorCache.set('/erc7730/ercs/calldata-erc20-tokens.json', {
        context: { contract: {} },
        display: {
          formats: {
            'transfer(address _to, uint256 _value)': {
              intent: 'Send',
              fields: [
                { path: '_value', label: 'Amount', format: 'tokenAmount', visible: 'always' },
                { path: '_to', label: 'To', format: 'addressName', visible: 'always' },
                { path: 'nonexistent1', label: 'Field 3', format: 'raw', visible: 'always' },
                { path: 'nonexistent2', label: 'Field 4', format: 'raw', visible: 'always' },
                { path: 'nonexistent3', label: 'Field 5', format: 'raw', visible: 'always' },
                { path: 'nonexistent4', label: 'Field 6', format: 'raw', visible: 'always' },
              ],
            },
          },
        },
      });

      const calldata = '0xa9059cbb' +
        '000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045' +
        '00000000000000000000000000000000000000000000000000000000003b9aca00';

      // Only 2 of 6 visible fields resolve → less than ceil(6/2)=3 → should return null
      const result = await resolveTransaction('0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', calldata, '0x0', 1);
      expect(result).toBeNull();
    });

    it('allows clear sign when enough fields resolve', async () => {
      loadERC20Descriptor();

      const calldata = '0xa9059cbb' +
        '000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045' +
        '00000000000000000000000000000000000000000000000000000000003b9aca00';

      // 2 of 2 visible fields resolve → 2 >= ceil(2/2)=1 → should succeed
      const result = await resolveTransaction('0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', calldata, '0x0', 1);
      expect(result).not.toBeNull();
    });
  });
});
