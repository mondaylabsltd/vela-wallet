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

// Mock the RPC pool so on-chain reads (ERC-165 supportsInterface, decimals()) are
// deterministic and never hit the network. Default: reject → callers fall back
// (decimals→18+unverified, token-standard→erc20). Individual tests override.
const mockPoolRpcCall = jest.fn(async (_method: string, _params: any[], _chainId: number): Promise<any> => {
  throw new Error('no rpc');
});
jest.mock('@/services/rpc-pool', () => ({
  poolRpcCall: (...args: any[]) => (mockPoolRpcCall as any)(...args),
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
  clearTokenStandardCache,
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
    clearTokenStandardCache();
    mockPoolRpcCall.mockReset();
    mockPoolRpcCall.mockRejectedValue(new Error('no rpc'));
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
    it('marks the result partial when too many fields fail to resolve', async () => {
      // Contract-specific descriptor (tried before the generic interface path) with
      // 6 visible fields but only 2 can resolve.
      mockDescriptorCache.set('/erc7730/calldata/eip155-1/0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48.json', {
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

      // Only 2 of 6 visible fields resolve → less than ceil(6/2)=3 → show what
      // decoded but flag `partial` (with elevated risk) instead of blind sign.
      const result = await resolveTransaction('0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', calldata, '0x0', 1);
      expect(result).not.toBeNull();
      expect(result!.partial).toBe(true);
      expect(result!.fields.length).toBeGreaterThan(0);
      expect(result!.risk).toBe('caution');
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

  describe('local protocol descriptors (no ERC-7730 server descriptor)', () => {
    const UNIV2 = '0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D';
    const pad = (h: string) => h.replace(/^0x/, '').toLowerCase().padStart(64, '0');
    // swapExactTokensForTokens(1000 USDC, ≥0.5 WETH, [USDC,WETH], to, deadline)
    const SWAP = '0x38ed1739'
      + pad((1000000000n).toString(16))          // amountIn 1e9 (1000 USDC, 6dp)
      + pad((500000000000000000n).toString(16))  // amountOutMin 0.5e18 (0.5 WETH)
      + pad('a0')                                 // path offset (5 head words)
      + pad('d8da6bf26964af9d7eed9e03e53415d37aa96045') // to
      + pad((1750000000).toString(16))           // deadline
      + pad('2')                                  // path length
      + pad('a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48') // USDC
      + pad('c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2');  // WETH

    it('renders a Uniswap V2 swap richly without a network descriptor', async () => {
      clearDescriptorCache();
      const r = await resolveTransaction(UNIV2, SWAP, '0x0', 1);
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Swap');
      expect(r!.contractName).toBe('Uniswap V2 Router');
      expect(r!.verified).toBe(true);
      const send = r!.fields.find((f) => f.role === 'send-amount');
      const recv = r!.fields.find((f) => f.role === 'receive-amount');
      expect(send!.value).toContain('USDC');
      expect(send!.value).toContain('1,000');
      expect(recv!.value).toContain('WETH');
      expect(recv!.value).toContain('0.5');
    });
  });

  describe('ERC-20/721/1155 selector disambiguation (ERC-165)', () => {
    const pad = (h: string) => h.replace(/^0x/, '').toLowerCase().padStart(64, '0');
    const FROM = '0xaf5e8917831ef08a64e18b2cde9f8f5d32c7b3e1';
    const TO = '0xd8da6bf26964af9d7eed9e03e53415d37aa96045';
    const USDT = '0xdAC17F958D2ee523a2206206994597C13D831ec7';
    const BAYC = '0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D';

    // transferFrom(from, to, value/tokenId) = 0x23b872dd
    const transferFrom = (third: bigint) =>
      '0x23b872dd' + pad(FROM) + pad(TO) + pad(third.toString(16));

    /** Make supportsInterface(0x<id>) answer true for the given interface id. */
    const supportsInterface = (trueId: string) => {
      mockPoolRpcCall.mockImplementation(async (_m: string, params: any[]) => {
        const data: string = params?.[0]?.data ?? '';
        if (data.includes('01ffc9a7')) {
          const yes = data.includes(trueId);
          return { result: '0x' + (yes ? '1' : '0').padStart(64, '0') };
        }
        throw new Error('no rpc'); // decimals() etc. → fall back
      });
    };

    it('renders ERC-20 transferFrom as a token amount (no ERC-165)', async () => {
      // USDT doesn't implement ERC-165 → default erc20. 100 USDT (6dp).
      const r = await resolveTransaction(USDT, transferFrom(100000000n), '0x0', 1);
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Transfer');
      const amount = r!.fields.find((f) => f.role === 'send-amount');
      expect(amount!.value).toContain('USDT');
      expect(amount!.value).toContain('100');
      // Stablecoin → cheap ≈$ valuation (peg $1), no price engine.
      expect(amount!.usd).toBe('$100.00');
    });

    it('renders ERC-721 transferFrom as an NFT token id (ERC-165 = 0x80ac58cd)', async () => {
      supportsInterface('80ac58cd');
      // tokenId 6529 (0x1981) must render as "#6,529", NOT a token amount.
      const r = await resolveTransaction(BAYC, transferFrom(6529n), '0x0', 1);
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Transfer NFT');
      const tokenId = r!.fields.find((f) => f.label === 'Token ID');
      expect(tokenId!.value).toBe('#6,529');
      // Must NOT be misread as a fungible amount.
      expect(r!.fields.some((f) => f.role === 'send-amount')).toBe(false);
    });

    it('renders ERC-721 safeTransferFrom(...,bytes) as NFT without an ERC-165 probe', async () => {
      // 0xb88d4fde is ERC-721-only — no on-chain call needed.
      const data = '0xb88d4fde' + pad(FROM) + pad(TO) + pad((42n).toString(16)) + pad('a0') + pad('0');
      const r = await resolveTransaction(BAYC, data, '0x0', 1);
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Transfer NFT');
      expect(mockPoolRpcCall).not.toHaveBeenCalled();
    });

    it('renders ERC-1155 safeTransferFrom with quantity', async () => {
      // safeTransferFrom(from,to,id,amount,data) = 0xf242432a (1155-only selector).
      const data = '0xf242432a' + pad(FROM) + pad(TO) + pad((7n).toString(16)) +
        pad((3n).toString(16)) + pad('a0') + pad('0');
      const r = await resolveTransaction(BAYC, data, '0x0', 1);
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Transfer NFT');
      const id = r!.fields.find((f) => f.label === 'Token ID');
      const qty = r!.fields.find((f) => f.label === 'Quantity');
      expect(id!.value).toBe('#7');
      expect(qty!.value).toBe('3');
    });
  });

  describe('Wave 2 protocol descriptors (local, no server descriptor)', () => {
    const V3_ROUTER02 = '0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45';
    const V3_EXACT_INPUT_SINGLE = '0x04e45aaf000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000001f4000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045000000000000000000000000000000000000000000000000000000003b9aca000000000000000000000000000000000000000000000000000429d069189e00000000000000000000000000000000000000000000000000000000000000000000';
    const V3_EXACT_INPUT = '0xb858183f00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000080000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045000000000000000000000000000000000000000000000000000000003b9aca000000000000000000000000000000000000000000000000000429d069189e0000000000000000000000000000000000000000000000000000000000000000002ba0b86991c6218b36c1d19d4a2e9eb0ce3606eb480001f4c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000';

    it('Uniswap V3 exactInputSingle (static tuple)', async () => {
      const r = await resolveTransaction(V3_ROUTER02, V3_EXACT_INPUT_SINGLE, '0x0', 1);
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Swap');
      expect(r!.contractName).toBe('Uniswap V3 Router');
      const send = r!.fields.find((f) => f.role === 'send-amount');
      const recv = r!.fields.find((f) => f.role === 'receive-amount');
      expect(send!.value).toContain('1,000');
      expect(send!.value).toContain('USDC');
      expect(recv!.value).toContain('0.3');
      expect(recv!.value).toContain('WETH');
    });

    it('Uniswap V3 exactInput (tokens encoded in bytes path)', async () => {
      const r = await resolveTransaction(V3_ROUTER02, V3_EXACT_INPUT, '0x0', 1);
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Swap');
      const send = r!.fields.find((f) => f.role === 'send-amount');
      const recv = r!.fields.find((f) => f.role === 'receive-amount');
      // tokenIn = path[0:20] = USDC; tokenOut = path[-20:] = WETH
      expect(send!.value).toContain('USDC');
      expect(recv!.value).toContain('WETH');
    });

    it('Lido stake (submit — amount is msg.value)', async () => {
      const r = await resolveTransaction(
        '0xae7ab96520de3a18e5e111b5eaab095312d7fe84',
        '0xa1903eab0000000000000000000000000000000000000000000000000000000000000000',
        '0xde0b6b3a7640000', // 1 ETH
        1,
      );
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Stake');
      expect(r!.fields[0].value).toContain('1');
    });

    it('wstETH wrap (token = stETH via metadata constant)', async () => {
      const r = await resolveTransaction(
        '0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0',
        '0xea598cb00000000000000000000000000000000000000000000000000de0b6b3a7640000',
        '0x0',
        1,
      );
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Wrap');
      expect(r!.fields[0].value).toContain('stETH');
      expect(r!.fields[0].value).toContain('1');
    });

    it('1inch V5 swap (SwapDescription tuple)', async () => {
      const data = '0x12aa3caf0000000000000000000000001111111254eeb25477b68fb85ed929f73a960582000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000001111111254eeb25477b68fb85ed929f73a960582000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045000000000000000000000000000000000000000000000000000000003b9aca000000000000000000000000000000000000000000000000000429d069189e000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000140000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000';
      const r = await resolveTransaction('0x1111111254eeb25477b68fb85ed929f73a960582', data, '0x0', 1);
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Swap');
      expect(r!.contractName).toBe('1inch Router');
      const send = r!.fields.find((f) => f.role === 'send-amount');
      expect(send!.value).toContain('1,000');
      expect(send!.value).toContain('USDC');
    });

    it('Seaport fulfillBasicOrder (NFT buy)', async () => {
      const data = '0xfb0f3ee10000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000de0b6b3a764000000000000000000000000000022222222222222222222222222222222222222220000000000000000000000000000000000000000000000000000000000000000000000000000000000000000bc4ca0eda7647a8ab7c2061c2e118a18a936f13d000000000000000000000000000000000000000000000000000000000000002a00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000713fb300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000240000000000000000000000000000000000000000000000000000000000000026000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000';
      const r = await resolveTransaction('0x00000000000000adc04c56bf30ac9d3c0aaf14dc', data, '0x0', 1);
      expect(r).not.toBeNull();
      expect(r!.intent).toBe('Buy NFT');
      expect(r!.contractName).toBe('Seaport');
      const tokenId = r!.fields.find((f) => f.label === 'Token ID');
      expect(tokenId!.value).toBe('#42');
    });
  });
});
