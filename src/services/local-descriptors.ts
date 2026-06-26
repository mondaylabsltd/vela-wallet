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
  '0xae7ab96520de3a18e5e111b5eaab095312d7fe84': { name: 'Lido (stETH)', owner: 'Lido' },
  '0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0': { name: 'Wrapped stETH', owner: 'Lido' },
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

// Seaport BasicOrderParameters — display the buy essentials (which NFT, price,
// seller, deadline). The deeply-nested fulfillOrder / fulfillAdvancedOrder
// (arrays of OfferItem/ConsiderationItem tuples) are intentionally left to the
// best-effort path; decoding them fully is deferred.
const seaportBasicFields = [
  { path: 'parameters.offerToken', label: 'NFT', format: 'addressName' },
  { path: 'parameters.offerIdentifier', label: 'Token ID', format: 'nftName' },
  { path: 'parameters.considerationAmount', label: 'Price', format: 'tokenAmount', params: { tokenPath: 'parameters.considerationToken', nativeCurrencyAddress: ['$.metadata.constants.native'] } },
  { path: 'parameters.offerer', label: 'Seller', format: 'addressName' },
  { path: 'parameters.endTime', label: 'Deadline', format: 'date' },
];
const SEAPORT_BASIC_TUPLE =
  '(address considerationToken,uint256 considerationIdentifier,uint256 considerationAmount,address offerer,address zone,address offerToken,uint256 offerIdentifier,uint256 offerAmount,uint8 basicOrderType,uint256 startTime,uint256 endTime,bytes32 zoneHash,uint256 salt,bytes32 offererConduitKey,bytes32 fulfillerConduitKey,uint256 totalOriginalAdditionalRecipients,(uint256 amount,address recipient)[] additionalRecipients,bytes signature) parameters';
const SEAPORT_DESCRIPTOR = {
  metadata: { contractName: 'Seaport', owner: 'OpenSea', constants: { native: '0x0000000000000000000000000000000000000000' } },
  display: {
    formats: {
      [`fulfillBasicOrder(${SEAPORT_BASIC_TUPLE})`]: { intent: 'Buy NFT', fields: seaportBasicFields },
      [`fulfillBasicOrder_efficient_6GL6yc(${SEAPORT_BASIC_TUPLE})`]: { intent: 'Buy NFT', fields: seaportBasicFields },
    },
  },
};

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

  // ---- Uniswap V3 SwapRouter02 (no deadline in structs) ----
  '0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45': {
    metadata: { contractName: 'Uniswap V3 Router', owner: 'Uniswap' },
    display: {
      formats: {
        'exactInputSingle((address tokenIn,address tokenOut,uint24 fee,address recipient,uint256 amountIn,uint256 amountOutMinimum,uint160 sqrtPriceLimitX96) params)': {
          intent: 'Swap',
          fields: [
            { path: 'params.amountIn', label: 'You pay', format: 'tokenAmount', params: { tokenPath: 'params.tokenIn' } },
            { path: 'params.amountOutMinimum', label: 'You receive (min)', format: 'tokenAmount', params: { tokenPath: 'params.tokenOut' } },
            { path: 'params.recipient', label: 'Recipient', format: 'addressName' },
          ],
        },
        'exactOutputSingle((address tokenIn,address tokenOut,uint24 fee,address recipient,uint256 amountOut,uint256 amountInMaximum,uint160 sqrtPriceLimitX96) params)': {
          intent: 'Swap',
          fields: [
            { path: 'params.amountInMaximum', label: 'You pay (max)', format: 'tokenAmount', params: { tokenPath: 'params.tokenIn' } },
            { path: 'params.amountOut', label: 'You receive', format: 'tokenAmount', params: { tokenPath: 'params.tokenOut' } },
            { path: 'params.recipient', label: 'Recipient', format: 'addressName' },
          ],
        },
        'exactInput((bytes path,address recipient,uint256 amountIn,uint256 amountOutMinimum) params)': {
          intent: 'Swap',
          fields: [
            { path: 'params.amountIn', label: 'You pay', format: 'tokenAmount', params: { tokenPath: 'params.path[0:20]' } },
            { path: 'params.amountOutMinimum', label: 'You receive (min)', format: 'tokenAmount', params: { tokenPath: 'params.path[-20:]' } },
            { path: 'params.recipient', label: 'Recipient', format: 'addressName' },
          ],
        },
        'exactOutput((bytes path,address recipient,uint256 amountOut,uint256 amountInMaximum) params)': {
          intent: 'Swap',
          // exactOutput path is reversed: first 20 bytes = output token, last 20 = input token.
          fields: [
            { path: 'params.amountInMaximum', label: 'You pay (max)', format: 'tokenAmount', params: { tokenPath: 'params.path[-20:]' } },
            { path: 'params.amountOut', label: 'You receive', format: 'tokenAmount', params: { tokenPath: 'params.path[0:20]' } },
            { path: 'params.recipient', label: 'Recipient', format: 'addressName' },
          ],
        },
      },
    },
  },

  // ---- Uniswap V3 SwapRouter (original — deadline inside each struct) ----
  '0xe592427a0aece92de3edee1f18e0157c05861564': {
    metadata: { contractName: 'Uniswap V3 Router', owner: 'Uniswap' },
    display: {
      formats: {
        'exactInputSingle((address tokenIn,address tokenOut,uint24 fee,address recipient,uint256 deadline,uint256 amountIn,uint256 amountOutMinimum,uint160 sqrtPriceLimitX96) params)': {
          intent: 'Swap',
          fields: [
            { path: 'params.amountIn', label: 'You pay', format: 'tokenAmount', params: { tokenPath: 'params.tokenIn' } },
            { path: 'params.amountOutMinimum', label: 'You receive (min)', format: 'tokenAmount', params: { tokenPath: 'params.tokenOut' } },
            { path: 'params.recipient', label: 'Recipient', format: 'addressName' },
            { path: 'params.deadline', label: 'Deadline', format: 'date' },
          ],
        },
        'exactOutputSingle((address tokenIn,address tokenOut,uint24 fee,address recipient,uint256 deadline,uint256 amountOut,uint256 amountInMaximum,uint160 sqrtPriceLimitX96) params)': {
          intent: 'Swap',
          fields: [
            { path: 'params.amountInMaximum', label: 'You pay (max)', format: 'tokenAmount', params: { tokenPath: 'params.tokenIn' } },
            { path: 'params.amountOut', label: 'You receive', format: 'tokenAmount', params: { tokenPath: 'params.tokenOut' } },
            { path: 'params.recipient', label: 'Recipient', format: 'addressName' },
            { path: 'params.deadline', label: 'Deadline', format: 'date' },
          ],
        },
        'exactInput((bytes path,address recipient,uint256 deadline,uint256 amountIn,uint256 amountOutMinimum) params)': {
          intent: 'Swap',
          fields: [
            { path: 'params.amountIn', label: 'You pay', format: 'tokenAmount', params: { tokenPath: 'params.path[0:20]' } },
            { path: 'params.amountOutMinimum', label: 'You receive (min)', format: 'tokenAmount', params: { tokenPath: 'params.path[-20:]' } },
            { path: 'params.recipient', label: 'Recipient', format: 'addressName' },
            { path: 'params.deadline', label: 'Deadline', format: 'date' },
          ],
        },
      },
    },
  },

  // ---- 1inch Aggregation Router V5 ----
  '0x1111111254eeb25477b68fb85ed929f73a960582': {
    metadata: { contractName: '1inch Router', owner: '1inch' },
    display: {
      formats: {
        'swap(address executor,(address srcToken,address dstToken,address srcReceiver,address dstReceiver,uint256 amount,uint256 minReturnAmount,uint256 flags) desc,bytes permit,bytes data)': {
          intent: 'Swap',
          fields: [
            { path: 'desc.amount', label: 'You pay', format: 'tokenAmount', params: { tokenPath: 'desc.srcToken' } },
            { path: 'desc.minReturnAmount', label: 'You receive (min)', format: 'tokenAmount', params: { tokenPath: 'desc.dstToken' } },
            { path: 'desc.dstReceiver', label: 'Recipient', format: 'addressName' },
          ],
        },
      },
    },
  },

  // ---- 1inch Aggregation Router V6 (swap drops the permit arg) ----
  '0x111111125421ca6dc452d289314280a0f8842a65': {
    metadata: { contractName: '1inch Router', owner: '1inch' },
    display: {
      formats: {
        'swap(address executor,(address srcToken,address dstToken,address srcReceiver,address dstReceiver,uint256 amount,uint256 minReturnAmount,uint256 flags) desc,bytes data)': {
          intent: 'Swap',
          fields: [
            { path: 'desc.amount', label: 'You pay', format: 'tokenAmount', params: { tokenPath: 'desc.srcToken' } },
            { path: 'desc.minReturnAmount', label: 'You receive (min)', format: 'tokenAmount', params: { tokenPath: 'desc.dstToken' } },
            { path: 'desc.dstReceiver', label: 'Recipient', format: 'addressName' },
          ],
        },
      },
    },
  },

  // ---- Lido — stETH staking ----
  '0xae7ab96520de3a18e5e111b5eaab095312d7fe84': {
    metadata: { contractName: 'Lido', owner: 'Lido' },
    display: {
      formats: {
        // Staked amount is msg.value, not a calldata arg.
        'submit(address _referral)': { intent: 'Stake', fields: [{ path: '@.value', label: 'Stake', format: 'amount' }] },
      },
    },
  },

  // ---- Lido — wstETH wrap/unwrap ----
  '0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0': {
    metadata: { contractName: 'Wrapped stETH', owner: 'Lido', constants: { steth: '0xae7ab96520de3a18e5e111b5eaab095312d7fe84' } },
    display: {
      formats: {
        'wrap(uint256 _stETHAmount)': {
          intent: 'Wrap',
          fields: [{ path: '_stETHAmount', label: 'Wrap', format: 'tokenAmount', params: { token: '$.metadata.constants.steth' } }],
        },
        'unwrap(uint256 _wstETHAmount)': {
          intent: 'Unwrap',
          fields: [{ path: '_wstETHAmount', label: 'Unwrap', format: 'tokenAmount', params: { tokenPath: '@.to' } }],
        },
      },
    },
  },

  // ---- OpenSea Seaport 1.5 / 1.6 — NFT marketplace (basic order only; the
  // deeply-nested fulfillOrder/fulfillAdvancedOrder variants are best-effort) ----
  '0x00000000000000adc04c56bf30ac9d3c0aaf14dc': SEAPORT_DESCRIPTOR,
  '0x0000000000000068f116a894984e2db1123eb395': SEAPORT_DESCRIPTOR,
};

export function localDescriptor(addr: string | undefined): any | undefined {
  return addr ? LOCAL_DESCRIPTORS[addr.toLowerCase()] : undefined;
}

// ---------------------------------------------------------------------------
// Generic, interface-level token descriptors (ERC-20 / ERC-721 / ERC-1155).
//
// The ethereum-data ERC fallbacks are incomplete (the ERC-20 descriptor has no
// `transferFrom`), and the `transferFrom` (0x23b872dd) / `approve` (0x095ea7b3)
// selectors collide between ERC-20 and ERC-721 — a token `transferFrom` would
// match the ERC-721 fallback and render an *amount* as a *tokenId* (or vice
// versa). So we resolve the standard selectors locally and pick the right
// descriptor by querying ERC-165 `supportsInterface` on-chain (see
// `detectTokenStandard` in clear-signing.ts). Token addresses are not known
// ahead of time, so these are keyed by standard, not by address.
// ---------------------------------------------------------------------------

/** ERC-165 interface ids (bytes4) used to disambiguate shared token selectors. */
export const INTERFACE_IDS = {
  erc721: '80ac58cd',
  erc1155: 'd9b67a26',
} as const;

export type TokenStandard = 'erc20' | 'erc721' | 'erc1155';

/** uint256 sentinel above which an approve amount reads as "Unlimited" (2^255). */
const UNLIMITED_THRESHOLD = '0x8000000000000000000000000000000000000000000000000000000000000000';

const ERC20_INTERFACE_DESCRIPTOR = {
  metadata: { standard: 'erc20' },
  display: {
    formats: {
      'transfer(address to,uint256 amount)': {
        intent: 'Send',
        fields: [
          { path: 'amount', label: 'Amount', format: 'tokenAmount', params: { tokenPath: '@.to' } },
          { path: 'to', label: 'To', format: 'addressName' },
        ],
      },
      'transferFrom(address from,address to,uint256 amount)': {
        intent: 'Transfer',
        fields: [
          { path: 'amount', label: 'Amount', format: 'tokenAmount', params: { tokenPath: '@.to' } },
          { path: 'from', label: 'From', format: 'addressName' },
          { path: 'to', label: 'To', format: 'addressName' },
        ],
      },
      'approve(address spender,uint256 amount)': {
        intent: 'Approve',
        fields: [
          { path: 'amount', label: 'Amount', format: 'tokenAmount', params: { tokenPath: '@.to', threshold: UNLIMITED_THRESHOLD } },
          { path: 'spender', label: 'Spender', format: 'addressName' },
        ],
      },
    },
  },
};

const ERC721_INTERFACE_DESCRIPTOR = {
  metadata: { standard: 'erc721' },
  display: {
    formats: {
      'transferFrom(address from,address to,uint256 tokenId)': {
        intent: 'Transfer NFT',
        fields: [
          { path: 'tokenId', label: 'Token ID', format: 'nftName' },
          { path: 'from', label: 'From', format: 'addressName' },
          { path: 'to', label: 'To', format: 'addressName' },
        ],
      },
      'safeTransferFrom(address from,address to,uint256 tokenId)': {
        intent: 'Transfer NFT',
        fields: [
          { path: 'tokenId', label: 'Token ID', format: 'nftName' },
          { path: 'from', label: 'From', format: 'addressName' },
          { path: 'to', label: 'To', format: 'addressName' },
        ],
      },
      'safeTransferFrom(address from,address to,uint256 tokenId,bytes data)': {
        intent: 'Transfer NFT',
        fields: [
          { path: 'tokenId', label: 'Token ID', format: 'nftName' },
          { path: 'from', label: 'From', format: 'addressName' },
          { path: 'to', label: 'To', format: 'addressName' },
        ],
      },
      'approve(address to,uint256 tokenId)': {
        intent: 'Approve NFT',
        fields: [
          { path: 'tokenId', label: 'Token ID', format: 'nftName' },
          { path: 'to', label: 'Approved', format: 'addressName' },
        ],
      },
      'setApprovalForAll(address operator,bool approved)': {
        intent: 'Approve all NFTs',
        fields: [
          { path: 'operator', label: 'Operator', format: 'addressName' },
          { path: 'approved', label: 'Approved', format: 'raw' },
        ],
      },
    },
  },
};

const ERC1155_INTERFACE_DESCRIPTOR = {
  metadata: { standard: 'erc1155' },
  display: {
    formats: {
      'safeTransferFrom(address from,address to,uint256 id,uint256 amount,bytes data)': {
        intent: 'Transfer NFT',
        fields: [
          { path: 'id', label: 'Token ID', format: 'nftName' },
          { path: 'amount', label: 'Quantity', format: 'raw' },
          { path: 'from', label: 'From', format: 'addressName' },
          { path: 'to', label: 'To', format: 'addressName' },
        ],
      },
      'safeBatchTransferFrom(address from,address to,uint256[] ids,uint256[] amounts,bytes data)': {
        intent: 'Transfer NFTs',
        fields: [
          { path: 'ids', label: 'Token IDs', format: 'raw' },
          { path: 'amounts', label: 'Quantities', format: 'raw' },
          { path: 'from', label: 'From', format: 'addressName' },
          { path: 'to', label: 'To', format: 'addressName' },
        ],
      },
      'setApprovalForAll(address operator,bool approved)': {
        intent: 'Approve all NFTs',
        fields: [
          { path: 'operator', label: 'Operator', format: 'addressName' },
          { path: 'approved', label: 'Approved', format: 'raw' },
        ],
      },
    },
  },
};

/** The interface-level descriptor for a detected token standard. */
export function interfaceDescriptor(standard: TokenStandard): any {
  return standard === 'erc721'
    ? ERC721_INTERFACE_DESCRIPTOR
    : standard === 'erc1155'
      ? ERC1155_INTERFACE_DESCRIPTOR
      : ERC20_INTERFACE_DESCRIPTOR;
}
