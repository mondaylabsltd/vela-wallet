/**
 * Clear-signing test scenarios — the single source of truth for the dev harness.
 *
 * Pure data (no React/RN imports) so it's importable by both the test screen and
 * the unit test that runs every scenario through the resolver. The screen owns
 * presentation (icons); this file owns the request fixtures.
 */
import type { BLEIncomingRequest } from '@/models/types';

export interface ClearSigningScenario {
  id: string;
  labelKey: string;
  subtitleKey: string;
  request: BLEIncomingRequest;
}

export const CLEAR_SIGNING_SCENARIOS: ClearSigningScenario[] = [
  {
    id: 'erc20-transfer',
    labelKey: 'clearSigning.scenarioErc20Transfer',
    subtitleKey: 'clearSigning.scenarioErc20TransferSub',
    request: {
      id: 'test-erc20-transfer',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        // transfer(vitalik, 1000 USDC) — 1e9 @ 6 decimals
        data: '0xa9059cbb000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045000000000000000000000000000000000000000000000000000000003b9aca00',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'erc20-approve',
    labelKey: 'clearSigning.scenarioErc20Approve',
    subtitleKey: 'clearSigning.scenarioErc20ApproveSub',
    request: {
      id: 'test-erc20-approve',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        data: '0x095ea7b3000000000000000000000000111111125421ca6dc452d289314280a0f8842a65ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'eth-transfer',
    labelKey: 'clearSigning.scenarioEthTransfer',
    subtitleKey: 'clearSigning.scenarioEthTransferSub',
    request: {
      id: 'test-eth-transfer',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
        data: '0x',
        value: '0x2386f26fc10000', // 0.01 ETH
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'personal-sign',
    labelKey: 'clearSigning.scenarioPersonalSign',
    subtitleKey: 'clearSigning.scenarioPersonalSignSub',
    request: {
      id: 'test-personal-sign',
      method: 'personal_sign',
      params: [
        '0x' + Array.from(new TextEncoder().encode(
          'Welcome to OpenSea!\n\nClick to sign in and accept the OpenSea Terms of Service.\n\nThis request will not trigger a blockchain transaction or cost any gas fees.\n\nNonce: 8a3b9f2c'
        )).map(b => b.toString(16).padStart(2, '0')).join(''),
        '0x0000000000000000000000000000000000000000',
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'eip712-permit',
    labelKey: 'clearSigning.scenarioEip712Permit',
    subtitleKey: 'clearSigning.scenarioEip712PermitSub',
    request: {
      id: 'test-eip712-permit',
      method: 'eth_signTypedData_v4',
      params: [
        '0x0000000000000000000000000000000000000000',
        JSON.stringify({
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
            value: '1000000000', // 1000 USDC (6 decimals)
            nonce: '0',
            deadline: '1900000000', // far future — a valid (non-expired) permit demo
          },
        }),
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    // Uniswap's real flow: an UNLIMITED Permit2 PermitSingle signature. The wallet
    // can't cap it (the dApp redeems its own struct on-chain) → must show the risk
    // and sign verbatim, never the cap editor. Regression guard for "signed the
    // Permit2 but the swap reverts".
    id: 'permit2-single-unlimited',
    labelKey: 'clearSigning.scenarioPermit2Unlimited',
    subtitleKey: 'clearSigning.scenarioPermit2UnlimitedSub',
    request: {
      id: 'test-permit2-single-unlimited',
      method: 'eth_signTypedData_v4',
      params: [
        '0x0000000000000000000000000000000000000000',
        JSON.stringify({
          types: {
            EIP712Domain: [
              { name: 'name', type: 'string' },
              { name: 'chainId', type: 'uint256' },
              { name: 'verifyingContract', type: 'address' },
            ],
            PermitSingle: [
              { name: 'details', type: 'PermitDetails' },
              { name: 'spender', type: 'address' },
              { name: 'sigDeadline', type: 'uint256' },
            ],
            PermitDetails: [
              { name: 'token', type: 'address' },
              { name: 'amount', type: 'uint160' },
              { name: 'expiration', type: 'uint48' },
              { name: 'nonce', type: 'uint48' },
            ],
          },
          primaryType: 'PermitSingle',
          domain: {
            name: 'Permit2',
            chainId: 1,
            verifyingContract: '0x000000000022D473030F116dDEE9F6B43aC78BA3',
          },
          message: {
            details: {
              token: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
              amount: '1461501637330902918203684832716283019655932542975', // 2^160-1 (unlimited)
              expiration: '1790000000',
              nonce: '0',
            },
            spender: '0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD', // Universal Router
            sigDeadline: '1790000000',
          },
        }),
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'eip712-unknown',
    labelKey: 'clearSigning.scenarioEip712Unknown',
    subtitleKey: 'clearSigning.scenarioEip712UnknownSub',
    request: {
      id: 'test-eip712-unknown',
      method: 'eth_signTypedData_v4',
      params: [
        '0x0000000000000000000000000000000000000000',
        JSON.stringify({
          types: {
            EIP712Domain: [
              { name: 'name', type: 'string' },
              { name: 'verifyingContract', type: 'address' },
            ],
            CustomOrder: [
              { name: 'maker', type: 'address' },
              { name: 'amount', type: 'uint256' },
              { name: 'expiry', type: 'uint256' },
              { name: 'salt', type: 'bytes32' },
            ],
          },
          primaryType: 'CustomOrder',
          domain: {
            name: 'Unknown Protocol',
            verifyingContract: '0x1234567890abcdef1234567890abcdef12345678',
          },
          message: {
            maker: '0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1',
            amount: '5000000000000000000',
            expiry: '1750000000',
            salt: '0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890',
          },
        }),
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'blind-tx',
    labelKey: 'clearSigning.scenarioBlindTx',
    subtitleKey: 'clearSigning.scenarioBlindTxSub',
    request: {
      id: 'test-blind-tx',
      method: 'eth_sendTransaction',
      params: [{
        // An unknown contract + opaque selector: no local/contract descriptor, no
        // ERC fallback, and not in any 4-byte DB → genuinely blind. Exercises the
        // (most important) blind-sign warning surface, not a rich decode.
        to: '0x4e1dC6fd6f2EBa9bE43C1f0d54F8E9A5E4B6A9C1',
        // A REAL selector (mint(address,uint256)) on an undescribed contract — the
        // realistic blind case: 4-byte still names the function even with no descriptor.
        data: '0x40c10f1900000000000000000000000000000000000000000000000000000000deadbeef0000000000000000000000000000000000000000000000000de0b6b3a7640000',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: '1inch-swap',
    labelKey: 'clearSigning.scenario1inchSwap',
    subtitleKey: 'clearSigning.scenario1inchSwapSub',
    request: {
      id: 'test-1inch',
      method: 'eth_sendTransaction',
      params: [{
        to: '0x1111111254EEB25477B68fb85Ed929f73A960582', // 1inch Router V5
        data: '0x12aa3caf0000000000000000000000001111111254eeb25477b68fb85ed929f73a960582000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000001111111254eeb25477b68fb85ed929f73a960582000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045000000000000000000000000000000000000000000000000000000003b9aca000000000000000000000000000000000000000000000000000429d069189e000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000140000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'nft-transfer',
    labelKey: 'clearSigning.scenarioNftTransfer',
    subtitleKey: 'clearSigning.scenarioNftTransferSub',
    request: {
      id: 'test-nft-transfer',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D', // BAYC
        data: '0x23b872dd000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa960450000000000000000000000000000000000000000000000000000000000001981',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'nft-approve-all',
    labelKey: 'clearSigning.scenarioNftApproveAll',
    subtitleKey: 'clearSigning.scenarioNftApproveAllSub',
    request: {
      id: 'test-nft-approve-all',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D', // BAYC
        data: '0xa22cb4650000000000000000000000001e0049783f008a0085193e00003d00cd54003c710000000000000000000000000000000000000000000000000000000000000001',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'vault-deposit',
    labelKey: 'clearSigning.scenarioVaultDeposit',
    subtitleKey: 'clearSigning.scenarioVaultDepositSub',
    request: {
      id: 'test-vault-deposit',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xae78736Cd615f374D3085123A210448E74Fc6393', // rETH (example vault)
        data: '0x6e553f650000000000000000000000000000000000000000000000001bc16d674ec80000000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'vault-withdraw',
    labelKey: 'clearSigning.scenarioVaultWithdraw',
    subtitleKey: 'clearSigning.scenarioVaultWithdrawSub',
    request: {
      id: 'test-vault-withdraw',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xae78736Cd615f374D3085123A210448E74Fc6393',
        data: '0xb460af940000000000000000000000000000000000000000000000000de0b6b3a7640000000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'erc20-transferFrom',
    labelKey: 'clearSigning.scenarioErc20TransferFrom',
    subtitleKey: 'clearSigning.scenarioErc20TransferFromSub',
    request: {
      id: 'test-erc20-transferFrom',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xdAC17F958D2ee523a2206206994597C13D831ec7', // USDT
        data: '0x23b872dd000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa960450000000000000000000000000000000000000000000000000000000005f5e100',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'hex-message',
    labelKey: 'clearSigning.scenarioHexMessage',
    subtitleKey: 'clearSigning.scenarioHexMessageSub',
    request: {
      id: 'test-hex-msg',
      method: 'personal_sign',
      params: [
        '0xdeadbeefcafebabe0102030405060708091011121314151617181920212223242526272829303132',
        '0x0000000000000000000000000000000000000000',
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'large-eth-send',
    labelKey: 'clearSigning.scenarioLargeEthSend',
    subtitleKey: 'clearSigning.scenarioLargeEthSendSub',
    request: {
      id: 'test-large-eth',
      method: 'eth_sendTransaction',
      params: [{
        to: '0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D',
        data: '0x',
        value: '0x8ac7230489e80000', // 10 ETH
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'erc20-approve-limited',
    labelKey: 'clearSigning.scenarioErc20ApproveLimited',
    subtitleKey: 'clearSigning.scenarioErc20ApproveLimitedSub',
    request: {
      id: 'test-erc20-limited-approve',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        data: '0x095ea7b30000000000000000000000003fc91a3afd70395cd496c647d5a6cc9d4b2b7fad000000000000000000000000000000000000000000000000000000001dcd6500',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'eth-sign',
    labelKey: 'clearSigning.scenarioEthSign',
    subtitleKey: 'clearSigning.scenarioEthSignSub',
    request: {
      id: 'test-eth-sign',
      method: 'eth_sign',
      params: [
        '0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1',
        '0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658',
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'siwe-phish',
    labelKey: 'clearSigning.scenarioSiwePhish',
    subtitleKey: 'clearSigning.scenarioSiwePhishSub',
    request: {
      id: 'test-siwe-phish',
      method: 'personal_sign',
      params: [
        '0x' + Array.from(new TextEncoder().encode(
          'app.uniswap.org wants you to sign in with your Ethereum account:\n' +
          '0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1\n\n' +
          'Sign in to Uniswap\n\n' +
          'URI: https://app.uniswap.org\nVersion: 1\nChain ID: 1\nNonce: 8a3b9f2c\nIssued At: 2026-06-01T00:00:00.000Z'
        )).map(b => b.toString(16).padStart(2, '0')).join(''),
        '0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1',
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'increase-allowance',
    labelKey: 'clearSigning.scenarioIncreaseAllowance',
    subtitleKey: 'clearSigning.scenarioIncreaseAllowanceSub',
    request: {
      id: 'test-increase-allowance',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        data: '0x395093510000000000000000000000003fc91a3afd70395cd496c647d5a6cc9d4b2b7fad0000000000000000000000000000000000000000000000000000000005f5e100',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'batch-calls',
    labelKey: 'clearSigning.scenarioBatch',
    subtitleKey: 'clearSigning.scenarioBatchSub',
    request: {
      id: 'test-batch-calls',
      method: 'wallet_sendCalls',
      params: [{
        chainId: '0x1',
        from: '0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1',
        calls: [
          {
            to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
            data: '0x095ea7b30000000000000000000000003fc91a3afd70395cd496c647d5a6cc9d4b2b7fad000000000000000000000000000000000000000000000000000000001dcd6500',
            value: '0x0',
          },
          {
            to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
            data: '0xa9059cbb000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa960450000000000000000000000000000000000000000000000000000000005f5e100',
            value: '0x0',
          },
        ],
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'expired-swap',
    labelKey: 'clearSigning.scenarioExpiredSwap',
    subtitleKey: 'clearSigning.scenarioExpiredSwapSub',
    request: {
      id: 'test-expired-swap',
      method: 'eth_sendTransaction',
      params: [{
        to: '0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D', // Uniswap V2 Router
        // deadline = 1700000000 (2023-11-14, already past)
        data: '0x38ed1739000000000000000000000000000000000000000000000000000000003b9aca0000000000000000000000000000000000000000000000000006f05b59d3b2000000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045000000000000000000000000000000000000000000000000000000006553f1000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
];
