/**
 * Built-in clear-signing descriptors for high-usage protocols that aren't (yet)
 * in the ERC-7730 registry.
 *
 * Two layers:
 *   - KNOWN_CONTRACTS: address → human protocol name, used to label "Interacting
 *     with: Uniswap V2 Router" even for a best-effort 4-byte decode.
 *   - LOCAL_DESCRIPTORS: address → an ERC-7730-shaped descriptor the resolver
 *     already understands, so top protocols render richly (intent + token amounts
 *     + roles) with zero new rendering code. Add a protocol = add an entry here.
 *
 * Addresses are lowercased. The big routers (Uniswap, 1inch, Permit2, Seaport)
 * deploy to the same address across most EVM chains via CREATE2, so a single
 * chain-agnostic map covers them; chain-specific ones (WETH) match their canon.
 */

export interface ContractInfo {
  name: string;
  owner?: string;
}

/** Address → protocol name (chain-agnostic for CREATE2-deterministic contracts). */
export const KNOWN_CONTRACTS: Record<string, ContractInfo> = {
  '0x7a250d5630b4cf539739df2c5dacb4c659f2488d': { name: 'Uniswap V2 Router', owner: 'Uniswap' },
  '0xe592427a0aece92de3edee1f18e0157c05861564': { name: 'Uniswap V3 Router', owner: 'Uniswap' },
  '0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45': { name: 'Uniswap V3 Router 2', owner: 'Uniswap' },
  '0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad': { name: 'Uniswap Universal Router', owner: 'Uniswap' },
  '0x66a9893cc07d91d95644aedd05d03f95e1dba8af': { name: 'Uniswap Universal Router', owner: 'Uniswap' },
  '0x1111111254eeb25477b68fb85ed929f73a960582': { name: '1inch Router (V5)', owner: '1inch' },
  '0x111111125421ca6dc452d289314280a0f8842a65': { name: '1inch Router (V6)', owner: '1inch' },
  '0x000000000022d473030f116ddee9f6b43ac78ba3': { name: 'Permit2', owner: 'Uniswap' },
  '0x00000000000000adc04c56bf30ac9d3c0aaf14dc': { name: 'Seaport 1.5', owner: 'OpenSea' },
  '0x0000000000000068f116a894984e2db1123eb395': { name: 'Seaport 1.6', owner: 'OpenSea' },
  '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2': { name: 'Wrapped Ether', owner: 'WETH' },
  '0x4200000000000000000000000000000000000006': { name: 'Wrapped Ether', owner: 'WETH' }, // OP-stack WETH
  '0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2': { name: 'Aave V3 Pool', owner: 'Aave' },
  '0xdef1c0ded9bec7f1a1670819833240f027b25eff': { name: '0x Exchange Proxy', owner: '0x' },
  '0x9008d19f58aabd9ed0d60971565aa8510560ab41': { name: 'CoW Protocol', owner: 'CoW' },
  '0xe66b31678d6c16e9ebf358268a790b763c133750': { name: 'Coinbase Smart Wallet', owner: 'Coinbase' },
};

export function knownContract(addr: string | undefined): ContractInfo | undefined {
  return addr ? KNOWN_CONTRACTS[addr.toLowerCase()] : undefined;
}

// Shared field fragments for swaps.
const recipientField = { path: 'to', label: 'Recipient', format: 'addressName' };
const deadlineField = { path: 'deadline', label: 'Deadline', format: 'date' };

/** Address → ERC-7730-shaped descriptor (display.formats keyed by named signature). */
export const LOCAL_DESCRIPTORS: Record<string, any> = {
  // ---- Uniswap V2 Router ----
  '0x7a250d5630b4cf539739df2c5dacb4c659f2488d': {
    metadata: { contractName: 'Uniswap V2 Router', owner: 'Uniswap' },
    display: {
      formats: {
        'swapExactTokensForTokens(uint256 amountIn,uint256 amountOutMin,address[] path,address to,uint256 deadline)': {
          intent: 'Swap',
          fields: [
            { path: 'amountIn', label: 'You pay', format: 'tokenAmount', params: { tokenPath: 'path.0' } },
            { path: 'amountOutMin', label: 'You receive (min)', format: 'tokenAmount', params: { tokenPath: 'path.-1' } },
            recipientField, deadlineField,
          ],
        },
        'swapTokensForExactTokens(uint256 amountOut,uint256 amountInMax,address[] path,address to,uint256 deadline)': {
          intent: 'Swap',
          fields: [
            { path: 'amountInMax', label: 'You pay (max)', format: 'tokenAmount', params: { tokenPath: 'path.0' } },
            { path: 'amountOut', label: 'You receive', format: 'tokenAmount', params: { tokenPath: 'path.-1' } },
            recipientField, deadlineField,
          ],
        },
        'swapExactETHForTokens(uint256 amountOutMin,address[] path,address to,uint256 deadline)': {
          intent: 'Swap',
          fields: [
            { path: '@.value', label: 'You pay', format: 'amount' },
            { path: 'amountOutMin', label: 'You receive (min)', format: 'tokenAmount', params: { tokenPath: 'path.-1' } },
            recipientField, deadlineField,
          ],
        },
        'swapExactTokensForETH(uint256 amountIn,uint256 amountOutMin,address[] path,address to,uint256 deadline)': {
          intent: 'Swap',
          fields: [
            { path: 'amountIn', label: 'You pay', format: 'tokenAmount', params: { tokenPath: 'path.0' } },
            { path: 'amountOutMin', label: 'You receive (min)', format: 'amount' },
            recipientField, deadlineField,
          ],
        },
      },
    },
  },

  // ---- Wrapped Ether ----
  '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2': {
    metadata: { contractName: 'Wrapped Ether', owner: 'WETH' },
    display: {
      formats: {
        'deposit()': { intent: 'Wrap ETH', fields: [{ path: '@.value', label: 'Amount', format: 'amount' }] },
        'withdraw(uint256 wad)': { intent: 'Unwrap WETH', fields: [{ path: 'wad', label: 'Amount', format: 'amount' }] },
      },
    },
  },

  // ---- Aave V3 Pool ----
  '0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2': {
    metadata: { contractName: 'Aave V3 Pool', owner: 'Aave' },
    display: {
      formats: {
        'supply(address asset,uint256 amount,address onBehalfOf,uint16 referralCode)': {
          intent: 'Supply',
          fields: [
            { path: 'amount', label: 'Supply', format: 'tokenAmount', params: { tokenPath: 'asset' } },
            { path: 'onBehalfOf', label: 'On behalf of', format: 'addressName' },
          ],
        },
        'withdraw(address asset,uint256 amount,address to)': {
          intent: 'Withdraw',
          fields: [
            { path: 'amount', label: 'Withdraw', format: 'tokenAmount', params: { tokenPath: 'asset' } },
            { path: 'to', label: 'Recipient', format: 'addressName' },
          ],
        },
        'borrow(address asset,uint256 amount,uint256 interestRateMode,uint16 referralCode,address onBehalfOf)': {
          intent: 'Borrow',
          fields: [
            { path: 'amount', label: 'Borrow', format: 'tokenAmount', params: { tokenPath: 'asset' } },
            { path: 'onBehalfOf', label: 'On behalf of', format: 'addressName' },
          ],
        },
        'repay(address asset,uint256 amount,uint256 interestRateMode,address onBehalfOf)': {
          intent: 'Repay',
          fields: [
            { path: 'amount', label: 'Repay', format: 'tokenAmount', params: { tokenPath: 'asset' } },
            { path: 'onBehalfOf', label: 'On behalf of', format: 'addressName' },
          ],
        },
      },
    },
  },
};

export function localDescriptor(addr: string | undefined): any | undefined {
  return addr ? LOCAL_DESCRIPTORS[addr.toLowerCase()] : undefined;
}
