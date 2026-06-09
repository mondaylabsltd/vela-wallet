/**
 * Clear Signing test screen — preview all signing modal scenarios.
 *
 * Triggers mock signing requests to test the SigningRequestModal UI
 * with different ERC-7730 scenarios.
 */
import React, { useState, useCallback } from 'react';
import { View, Text, ScrollView, Pressable } from 'react-native';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { useDAppConnection } from '@/models/dapp-connection';
import { color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';
import {
  ArrowRightLeft, Send, CheckCircle, Pen, FileText, ShieldAlert,
  Zap, ChevronLeft,
} from 'lucide-react-native';
import { useRouter } from 'expo-router';
import type { BLEIncomingRequest } from '@/models/types';

// ---------------------------------------------------------------------------
// Mock data for each scenario
// ---------------------------------------------------------------------------

const SCENARIOS: {
  id: string;
  label: string;
  subtitle: string;
  icon: React.ReactNode;
  iconBg: string;
  request: BLEIncomingRequest;
}[] = [
  {
    id: 'erc20-transfer',
    label: 'ERC-20 Transfer',
    subtitle: 'Clear sign — transfer(address, uint256)',
    icon: <Send size={18} color="#E8572A" strokeWidth={2} />,
    iconBg: '#FFF0EB',
    request: {
      id: 'test-erc20-transfer',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        data: '0xa9059cbb000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa9604500000000000000000000000000000000000000000000000000000000003b9aca00',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'erc20-approve',
    label: 'ERC-20 Approve (Unlimited)',
    subtitle: 'Clear sign — approve(address, uint256)',
    icon: <CheckCircle size={18} color="#d4890a" strokeWidth={2} />,
    iconBg: '#FFF8F0',
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
    label: 'ETH Transfer',
    subtitle: 'Plain transfer — no calldata',
    icon: <Send size={18} color="#E8572A" strokeWidth={2} />,
    iconBg: '#FFF0EB',
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
    label: 'Personal Sign',
    subtitle: 'Message signing — login/auth',
    icon: <Pen size={18} color="#6c5ce7" strokeWidth={2} />,
    iconBg: '#EEF0FF',
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
    label: 'EIP-712 Permit2',
    subtitle: 'Typed data — gasless token approval',
    icon: <FileText size={18} color="#6c5ce7" strokeWidth={2} />,
    iconBg: '#EEF0FF',
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
            deadline: '1750000000',
          },
        }),
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'eip712-unknown',
    label: 'EIP-712 Unknown',
    subtitle: 'Typed data — no descriptor (blind)',
    icon: <FileText size={18} color="#d4890a" strokeWidth={2} />,
    iconBg: '#FFF8F0',
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
    label: 'Blind Transaction',
    subtitle: 'Unknown contract — no descriptor',
    icon: <ShieldAlert size={18} color="#d43a2a" strokeWidth={2} />,
    iconBg: '#FEF2F2',
    request: {
      id: 'test-blind-tx',
      method: 'eth_sendTransaction',
      params: [{
        to: '0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D',
        data: '0x38ed173900000000000000000000000000000000000000000000000000000000003b9aca000000000000000000000000000000000000000000000000000de0b6b3a764000000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045000000000000000000000000000000000000000000000000000000006789abcd',
        value: '0x6f05b59d3b20000', // 0.5 ETH
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: '1inch-swap',
    label: '1inch Swap',
    subtitle: 'Clear sign — contract-specific descriptor',
    icon: <ArrowRightLeft size={18} color="#E8572A" strokeWidth={2} />,
    iconBg: '#FFF0EB',
    request: {
      id: 'test-1inch',
      method: 'eth_sendTransaction',
      params: [{
        to: '0x111111125421cA6dc452d289314280a0f8842A65',
        data: '0x12aa3caf000000000000000000000000111111125421ca6dc452d289314280a0f8842a65000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000003b9aca000000000000000000000000000000000000000000000000000de0b6b3a7640000',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },

  // --- ERC-721 NFT scenarios ---
  {
    id: 'nft-transfer',
    label: 'NFT Transfer',
    subtitle: 'ERC-721 — transferFrom(address,address,uint256)',
    icon: <Send size={18} color="#6c5ce7" strokeWidth={2} />,
    iconBg: '#EEF0FF',
    request: {
      id: 'test-nft-transfer',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D', // BAYC
        // transferFrom(address,address,uint256) = 0x23b872dd
        data: '0x23b872dd000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa960450000000000000000000000000000000000000000000000000000000000001981',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'nft-approve-all',
    label: 'NFT Approve All',
    subtitle: 'ERC-721 — setApprovalForAll(address,bool)',
    icon: <CheckCircle size={18} color="#d4890a" strokeWidth={2} />,
    iconBg: '#FFF8F0',
    request: {
      id: 'test-nft-approve-all',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D', // BAYC
        // setApprovalForAll(address,bool) = 0xa22cb465
        data: '0xa22cb4650000000000000000000000001e0049783f008a0085193e00003d00cd54003c710000000000000000000000000000000000000000000000000000000000000001',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },

  // --- ERC-4626 Vault scenarios ---
  {
    id: 'vault-deposit',
    label: 'Vault Deposit',
    subtitle: 'ERC-4626 — deposit(uint256,address)',
    icon: <Zap size={18} color="#22a456" strokeWidth={2} />,
    iconBg: '#EDFAF2',
    request: {
      id: 'test-vault-deposit',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xae78736Cd615f374D3085123A210448E74Fc6393', // rETH (used as example vault)
        // deposit(uint256,address) = 0x6e553f65
        data: '0x6e553f650000000000000000000000000000000000000000000000001bc16d674ec80000000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'vault-withdraw',
    label: 'Vault Withdraw',
    subtitle: 'ERC-4626 — withdraw(uint256,address,address)',
    icon: <Zap size={18} color="#6c5ce7" strokeWidth={2} />,
    iconBg: '#EEF0FF',
    request: {
      id: 'test-vault-withdraw',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xae78736Cd615f374D3085123A210448E74Fc6393',
        // withdraw(uint256,address,address) = 0xb460af94
        data: '0xb460af940000000000000000000000000000000000000000000000000de0b6b3a7640000000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },

  // --- ERC-20 transferFrom ---
  {
    id: 'erc20-transferFrom',
    label: 'ERC-20 TransferFrom',
    subtitle: 'Clear sign — transferFrom(address,address,uint256)',
    icon: <Send size={18} color="#E8572A" strokeWidth={2} />,
    iconBg: '#FFF0EB',
    request: {
      id: 'test-erc20-transferFrom',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xdAC17F958D2ee523a2206206994597C13D831ec7', // USDT
        // transferFrom(address,address,uint256) = 0x23b872dd
        data: '0x23b872dd000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa960450000000000000000000000000000000000000000000000000000000005f5e100',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },

  // --- Hex message (non-readable personal_sign) ---
  {
    id: 'hex-message',
    label: 'Hex Message Sign',
    subtitle: 'personal_sign — non-printable hex data',
    icon: <Pen size={18} color="#d4890a" strokeWidth={2} />,
    iconBg: '#FFF8F0',
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

  // --- Large ETH value send ---
  {
    id: 'large-eth-send',
    label: 'Large ETH Send',
    subtitle: 'Plain transfer — 10 ETH',
    icon: <Send size={18} color="#d43a2a" strokeWidth={2} />,
    iconBg: '#FEF2F2',
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

  // --- ERC-20 limited approve (not unlimited) ---
  {
    id: 'erc20-approve-limited',
    label: 'ERC-20 Limited Approve',
    subtitle: 'Clear sign — approve with specific amount',
    icon: <CheckCircle size={18} color="#22a456" strokeWidth={2} />,
    iconBg: '#EDFAF2',
    request: {
      id: 'test-erc20-limited-approve',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        // approve(address,uint256) with 500 USDC (500 * 1e6 = 0x1DCD6500)
        data: '0x095ea7b30000000000000000000000003fc91a3afd70395cd496c647d5a6cc9d4b2b7fad000000000000000000000000000000000000000000000000000000001dcd6500',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
];

// ---------------------------------------------------------------------------
// Main screen
// ---------------------------------------------------------------------------

export default function ClearSigningTestScreen() {
  const router = useRouter();
  const {
    incomingRequest,
  } = useDAppConnection();

  // We inject a mock request by re-using the context's state
  // Since we can't directly set incomingRequest, we use a local modal approach
  const [mockRequest, setMockRequest] = useState<BLEIncomingRequest | null>(null);

  const handleScenario = useCallback((scenario: typeof SCENARIOS[number]) => {
    setMockRequest(scenario.request);
  }, []);

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.scrollContent}>
        {/* Header */}
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.backBtn}>
            <ChevronLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <View>
            <Text style={styles.title}>Clear Signing Test</Text>
            <Text style={styles.subtitle}>ERC-7730 signing UI scenarios</Text>
          </View>
        </View>

        {/* Scenarios */}
        <VelaCard style={styles.card}>
          {SCENARIOS.map((scenario, i) => (
            <React.Fragment key={scenario.id}>
              <Pressable
                style={styles.row}
                onPress={() => handleScenario(scenario)}
              >
                <View style={[styles.iconWrap, { backgroundColor: scenario.iconBg }]}>
                  {scenario.icon}
                </View>
                <View style={styles.rowInfo}>
                  <Text style={styles.rowTitle}>{scenario.label}</Text>
                  <Text style={styles.rowSub}>{scenario.subtitle}</Text>
                </View>
              </Pressable>
              {i < SCENARIOS.length - 1 && <View style={styles.divider} />}
            </React.Fragment>
          ))}
        </VelaCard>

        <Text style={styles.hint}>
          Tap a scenario to open the signing modal with mock data.
          Clear signing descriptors are fetched live from the ERC-7730 registry.
        </Text>
      </ScrollView>

      {/* Mock signing modal — renders independently from DAppConnection */}
      {mockRequest && (
        <MockSigningModal
          request={mockRequest}
          onClose={() => setMockRequest(null)}
        />
      )}
    </ScreenContainer>
  );
}

// ---------------------------------------------------------------------------
// Mock Signing Modal — self-contained version for testing
// ---------------------------------------------------------------------------

import { AppModal } from '@/components/ui/AppModal';
import { VelaButton } from '@/components/ui/VelaButton';
import {
  resolveTransaction, resolveTypedData,
  type ClearSignResult, type ClearSignField,
} from '@/services/clear-signing';
import { chainName, nativeSymbol } from '@/models/network';
import { shortAddr } from '@/models/types';
import {
  AlertTriangle, Copy, Check, ChevronDown,
  ArrowDown, ShieldCheck, Shield,
} from 'lucide-react-native';
import { showAlert } from '@/services/platform';
import * as Clipboard from 'expo-clipboard';
import { useWallet } from '@/models/wallet-state';
import { TokenLogo } from '@/components/TokenLogo';
import { ChainLogo } from '@/components/ChainLogo';
import { DEFAULT_NETWORKS } from '@/models/network';

function MockSigningModal({ request, onClose }: {
  request: BLEIncomingRequest;
  onClose: () => void;
}) {
  const { activeAccount } = useWallet();
  const chainId = 1; // test on Ethereum mainnet
  const [clearSign, setClearSign] = useState<ClearSignResult | null>(null);
  const [resolving, setResolving] = useState(false);
  const [showRaw, setShowRaw] = useState(false);

  const { method, params } = request;
  const isPersonalSign = method === 'personal_sign';
  const isTypedData = method.includes('signTypedData');
  const isTx = method === 'eth_sendTransaction';

  // Resolve clear signing
  React.useEffect(() => {
    if (isTx && params?.[0]) {
      setResolving(true);
      resolveTransaction(params[0].to, params[0].data, params[0].value, chainId)
        .then(setClearSign)
        .catch(() => setClearSign(null))
        .finally(() => setResolving(false));
    } else if (isTypedData && params) {
      setResolving(true);
      const typedDataRaw = params[1] ?? params[0];
      try {
        const typedData = typeof typedDataRaw === 'string' ? JSON.parse(typedDataRaw) : typedDataRaw;
        resolveTypedData(typedData, chainId)
          .then(setClearSign)
          .catch(() => setClearSign(null))
          .finally(() => setResolving(false));
      } catch {
        setClearSign(null);
        setResolving(false);
      }
    } else {
      setClearSign(null);
    }
  }, [request]);

  // Mock dApp info
  const dappName = 'Test dApp';
  const accountName = activeAccount?.name ?? 'Wallet';

  // --- Helpers ---
  const RC = { safe: color.success.base, normal: color.accent.base, caution: color.warning.base, danger: color.error.base };
  const SIG_COLOR = color.info.base;

  function decodeMsg(hex: string): string {
    try {
      const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
      const bytes = new Uint8Array(clean.match(/.{1,2}/g)!.map(b => parseInt(b, 16)));
      const decoded = new TextDecoder().decode(bytes);
      if (/^[\x20-\x7E\n\r\t]+$/.test(decoded)) return decoded;
      return `0x${clean.slice(0, 64)}...`;
    } catch { return hex.slice(0, 66) + '...'; }
  }

  function fmtValue(v: string | undefined): string {
    const sym = nativeSymbol(chainId);
    if (!v || v === '0x0' || v === '0x') return `0 ${sym}`;
    try { return (Number(BigInt(v)) / 1e18).toFixed(4).replace(/\.?0+$/, '') + ' ' + sym; }
    catch { return v; }
  }

  const [copiedAddr, setCopiedAddr] = useState<string | null>(null);
  async function copyAddr(addr: string) {
    await Clipboard.setStringAsync(addr);
    setCopiedAddr(addr);
    setTimeout(() => setCopiedAddr(null), 1500);
  }

  // --- Render ---
  return (
    <AppModal visible={true} onClose={onClose}>
      <View style={ms.container}>
        <ScrollView showsVerticalScrollIndicator={false}>

          {/* dApp banner + context (merged, two-row) */}
          {(() => {
            const net = DEFAULT_NETWORKS.find(n => n.chainId === chainId);
            return (
              <View style={ms.dappBanner}>
                <View style={ms.dappRow1}>
                  <View style={ms.dappLogoFallback}>
                    <Text style={ms.dappLogoText}>T</Text>
                  </View>
                  <View style={ms.dappInfo}>
                    <Text style={ms.dappName}>{dappName}</Text>
                    <Text style={ms.dappDomain}>clear-signing-test</Text>
                  </View>
                  <View style={ms.dappChainRow}>
                    {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={16} />}
                    <Text style={ms.dappChainName}>{chainName(chainId)}</Text>
                  </View>
                </View>
                <Text style={ms.dappAccountLine} numberOfLines={1}>
                  {accountName}{activeAccount?.address ? `  ·  ${shortAddr(activeAccount.address)}` : ''}
                </Text>
              </View>
            );
          })()}

          {/* ---- Loading ---- */}
          {resolving ? (
            <View style={{ alignItems: 'center', padding: 40 }}>
              <Text style={ms.dappChainName}>Loading...</Text>
            </View>

          /* ---- Clear signed ---- */
          ) : clearSign ? (
            <>
              <View style={ms.intent}>
                <Text style={[ms.intentText, { color: RC[clearSign.risk] }]}>{clearSign.intent}</Text>
              </View>

              {/* Token cards by role */}
              {clearSign.fields.filter(f => f.role === 'send-amount').map((f, i) => (
                <View key={`s${i}`} style={[ms.tokenCard, { backgroundColor: clearSign.risk === 'caution' ? color.warning.soft : color.accent.soft }]}>
                  <TokenLogo
                    symbol={f.tokenAddress ? f.tokenAddress.slice(2, 6).toUpperCase() : '?'}
                    logoUrl={f.tokenAddress ? `https://ethereum-data.awesometools.dev/tokenlogos/${f.tokenAddress}.png` : undefined}
                    size={40}
                  />
                  <View style={ms.tokenInfo}>
                    <Text style={ms.tokenAmt} numberOfLines={1}>{f.value}</Text>
                    <Text style={ms.tokenLabel}>{f.label}{f.warning ? ' ⚠️' : ''}</Text>
                  </View>
                </View>
              ))}

              {clearSign.fields.some(f => f.role === 'receive-amount') && (
                <View style={ms.flowArrow}><View style={ms.flowCircle}><ArrowDown size={14} color={color.fg.subtle} strokeWidth={2.5} /></View></View>
              )}

              {clearSign.fields.filter(f => f.role === 'receive-amount').map((f, i) => (
                <View key={`r${i}`} style={[ms.tokenCard, { backgroundColor: color.info.soft }]}>
                  <TokenLogo
                    symbol={f.tokenAddress ? f.tokenAddress.slice(2, 6).toUpperCase() : '?'}
                    logoUrl={f.tokenAddress ? `https://ethereum-data.awesometools.dev/tokenlogos/${f.tokenAddress}.png` : undefined}
                    size={40}
                  />
                  <View style={ms.tokenInfo}>
                    <Text style={ms.tokenAmt} numberOfLines={1}>{f.value}</Text>
                    <Text style={ms.tokenLabel}>{f.label}</Text>
                  </View>
                </View>
              ))}

              {/* Spender/recipient */}
              {clearSign.fields.filter(f => f.role === 'spender' || f.role === 'recipient').map((f, i) => (
                <View key={`addr${i}`} style={ms.contractBar}>
                  <View style={ms.contractInfo}>
                    <Text style={ms.contractLabel}>{f.role === 'spender' ? 'SPENDER' : 'RECIPIENT'}</Text>
                    <Text style={ms.contractAddr}>{f.value}</Text>
                  </View>
                  <Pressable onPress={() => copyAddr(f.value)} style={[ms.copyBtn, copiedAddr === f.value && ms.copyBtnDone]}>
                    {copiedAddr === f.value ? <Check size={12} color={color.success.base} strokeWidth={2.5} /> : <Copy size={12} color={color.fg.muted} strokeWidth={2} />}
                  </Pressable>
                </View>
              ))}

              {/* Warning */}
              {clearSign.fields.some(f => f.warning) && (
                <View style={ms.warnDanger}>
                  <AlertTriangle size={14} color={RC.danger} strokeWidth={2} />
                  <Text style={ms.warnDangerText}>Unlimited — this contract can spend all your tokens</Text>
                </View>
              )}

              {/* Generic fields */}
              {clearSign.fields.filter(f => f.role === 'generic').map((f, i) => (
                <View key={`g${i}`} style={ms.genRow}>
                  <Text style={ms.genLabel}>{f.label}</Text>
                  <Text style={ms.genValue} numberOfLines={2}>{f.value}</Text>
                </View>
              ))}

              {/* Contract */}
              {clearSign.contractAddress && (
                <View style={ms.contractBar}>
                  <View style={ms.contractInfo}>
                    <Text style={ms.contractLabel}>INTERACTING WITH</Text>
                    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                      {clearSign.contractName && <Text style={[ms.contractAddr, { color: color.success.base }]}>{clearSign.contractName}</Text>}
                      <Text style={ms.contractAddr}>{shortAddr(clearSign.contractAddress)}</Text>
                    </View>
                  </View>
                  <Pressable onPress={() => copyAddr(clearSign.contractAddress!)} style={[ms.copyBtn, copiedAddr === clearSign.contractAddress && ms.copyBtnDone]}>
                    {copiedAddr === clearSign.contractAddress ? <Check size={12} color={color.success.base} strokeWidth={2.5} /> : <Copy size={12} color={color.fg.muted} strokeWidth={2} />}
                  </Pressable>
                  {clearSign.verified && <ShieldCheck size={14} color={color.success.base} strokeWidth={2} />}
                </View>
              )}

              {/* context already shown at top */}
            </>

          /* ---- personal_sign ---- */
          ) : isPersonalSign && params?.[0] ? (
            <>
              <View style={ms.intent}><Text style={[ms.intentText, { color: SIG_COLOR }]}>Sign Message</Text></View>
              <View style={ms.msgBubble}>
                <View style={ms.msgTag}><Text style={ms.msgTagText}>personal_sign · No gas fee</Text></View>
                <Text style={ms.msgText}>{decodeMsg(params[0])}</Text>
              </View>
              {/* context already shown at top */}
            </>

          /* ---- EIP-712 blind ---- */
          ) : isTypedData && params ? (
            <>
              <View style={ms.intent}><Text style={[ms.intentText, { color: '#d4890a' }]}>Sign Typed Data</Text></View>
              {(() => {
                const td = (() => { try { return typeof params[1] === 'string' ? JSON.parse(params[1]) : (params[1] ?? params[0]); } catch { return null; } })();
                if (!td) return null;
                return (
                  <>
                    {td.domain?.verifyingContract && (
                      <View style={ms.contractBar}>
                        <View style={ms.contractInfo}>
                          <Text style={ms.contractLabel}>SIGNING FOR</Text>
                          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                            {td.domain.name && <Text style={[ms.contractAddr, { color: color.fg.base, fontFamily: font.sans }]}>{td.domain.name}</Text>}
                            <Text style={ms.contractAddr}>{shortAddr(td.domain.verifyingContract)}</Text>
                          </View>
                        </View>
                        <Pressable onPress={() => copyAddr(td.domain.verifyingContract)} style={ms.copyBtn}>
                          <Copy size={12} color={color.fg.muted} strokeWidth={2} />
                        </Pressable>
                      </View>
                    )}
                    {td.primaryType && (
                      <View style={ms.genRow}>
                        <Text style={ms.genLabel}>Type</Text>
                        <Text style={ms.genValue}>{td.primaryType}</Text>
                      </View>
                    )}
                    {td.message && Object.entries(td.message).slice(0, 5).map(([k, v], i) => (
                      <View key={i} style={ms.genRow}>
                        <Text style={ms.genLabel}>{k}</Text>
                        <Text style={ms.genValue} numberOfLines={1}>{String(v).slice(0, 40)}</Text>
                      </View>
                    ))}
                    <View style={ms.warnCaution}>
                      <AlertTriangle size={14} color={color.warning.base} strokeWidth={2} />
                      <Text style={[ms.warnDangerText, { color: color.warning.base }]}>No descriptor found. Review carefully.</Text>
                    </View>
                  </>
                );
              })()}
              {/* context already shown at top */}
            </>

          /* ---- Blind transaction ---- */
          ) : isTx && params?.[0] ? (
            <>
              {(() => {
                const tx = params[0];
                const hasData = tx.data && tx.data !== '0x';
                return (
                  <>
                    <View style={ms.intent}><Text style={[ms.intentText, { color: hasData ? color.error.base : '#E8572A' }]}>{hasData ? 'Unknown' : 'Send'}</Text></View>
                    {fmtValue(tx.value) !== `0 ${nativeSymbol(chainId)}` && (
                      <View style={[ms.tokenCard, { backgroundColor: hasData ? color.error.soft : color.accent.soft }]}>
                        <View style={ms.tokenInfo}>
                          <Text style={ms.tokenAmt}>{fmtValue(tx.value)}</Text>
                          <Text style={ms.tokenLabel}>Value</Text>
                        </View>
                      </View>
                    )}
                    {(hasData || tx.to) && (
                      <View style={ms.flowArrow}><View style={[ms.flowCircle, hasData && { borderColor: color.error.base }]}><ArrowDown size={14} color={hasData ? color.error.base : color.fg.subtle} strokeWidth={2.5} /></View></View>
                    )}
                    <View style={[ms.contractBar, hasData && { borderWidth: 1, borderColor: color.error.base }]}>
                      <View style={ms.contractInfo}>
                        <Text style={ms.contractLabel}>{hasData ? 'UNVERIFIED CONTRACT' : 'RECIPIENT'}</Text>
                        <Text style={ms.contractAddr}>{shortAddr(tx.to ?? '')}</Text>
                      </View>
                      <Pressable onPress={() => copyAddr(tx.to ?? '')} style={ms.copyBtn}>
                        <Copy size={12} color={color.fg.muted} strokeWidth={2} />
                      </Pressable>
                    </View>
                    {hasData && (
                      <>
                        <View style={ms.warnDanger}>
                          <AlertTriangle size={14} color={color.error.base} strokeWidth={2} />
                          <Text style={ms.warnDangerText}>Unable to decode — no ERC-7730 descriptor ({Math.floor((tx.data.length - 2) / 2)} bytes)</Text>
                        </View>
                        <Pressable style={ms.detailsToggle} onPress={() => setShowRaw(!showRaw)}>
                          <Text style={ms.detailsToggleText}>Raw calldata</Text>
                          <ChevronDown size={12} color={color.fg.subtle} strokeWidth={2} style={showRaw ? { transform: [{ rotate: '180deg' }] } : undefined} />
                        </Pressable>
                        {showRaw && (
                          <View style={ms.rawBlock}>
                            <Text style={ms.rawText}>{tx.data.slice(0, 200)}{tx.data.length > 200 ? '...' : ''}</Text>
                          </View>
                        )}
                      </>
                    )}
                    {/* context already shown at top */}
                  </>
                );
              })()}
            </>

          ) : (
            <View style={{ alignItems: 'center', padding: 40 }}>
              <Shield size={28} color={color.fg.muted} strokeWidth={2} />
              <Text style={ms.dappChainName}>Signature request</Text>
            </View>
          )}

          {/* Resolving indicator */}
          {resolving && (
            <View style={{ alignItems: 'center', padding: space.lg }}>
              <Text style={ms.dappChainName}>Loading descriptor...</Text>
            </View>
          )}
        </ScrollView>

        {/* Buttons */}
        <View style={ms.btns}>
          <VelaButton title="Reject" onPress={onClose} variant="secondary" style={ms.btnFlex} />
          <VelaButton
            title={clearSign ? (clearSign.type === 'signature' ? 'Sign' : (clearSign.intent.length > 12 ? 'Confirm' : `Confirm ${clearSign.intent}`)) : (isPersonalSign || isTypedData ? 'Sign' : 'Approve')}
            onPress={() => { showAlert('Signed!', 'This is a test — no actual signature was created.'); onClose(); }}
            variant="accent"
            loading={resolving}
            style={ms.btnFlex}
          />
        </View>
      </View>
    </AppModal>
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = createStyles(() => ({
  scrollContent: { paddingBottom: space['5xl'] },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    marginTop: space.xl,
    marginBottom: space['2xl'],
  },
  backBtn: {
    width: 36, height: 36, borderRadius: 18,
    backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
  },
  title: { fontSize: text['2xl'], ...inter.bold, color: color.fg.base },
  subtitle: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, marginTop: 2 },
  card: { padding: 0, overflow: 'hidden' },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingVertical: space.xl,
    paddingHorizontal: space['2xl'],
  },
  iconWrap: {
    width: 40, height: 40, borderRadius: 12,
    alignItems: 'center', justifyContent: 'center',
  },
  rowInfo: { flex: 1, gap: 2 },
  rowTitle: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  rowSub: { fontSize: text.xs, ...inter.regular, color: color.fg.muted },
  divider: { height: 1, backgroundColor: color.border.base, marginHorizontal: space['2xl'] },
  hint: {
    fontSize: text.sm, ...inter.regular, color: color.fg.muted,
    textAlign: 'center', marginTop: space['2xl'], lineHeight: 18,
    paddingHorizontal: space.xl,
  },
}));

// Mock modal styles
const ms = createStyles(() => ({
  container: { flex: 1, padding: space['3xl'] },
  dappBanner: {
    paddingVertical: space.lg, paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken, borderRadius: radius.xl, marginBottom: space['2xl'],
    gap: space.md,
  },
  dappRow1: {
    flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.lg,
  },
  dappLogoFallback: {
    width: 36, height: 36, borderRadius: 10,
    backgroundColor: color.accent.soft, alignItems: 'center', justifyContent: 'center',
  },
  dappLogoText: { fontSize: text.lg, ...inter.bold, color: color.accent.base },
  dappInfo: { flex: 1, gap: 1 },
  dappName: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  dappDomain: { fontSize: text.xs, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.muted },
  dappChainRow: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.sm, marginLeft: 'auto' as const },
  dappChainName: { fontSize: text.xs, ...inter.semibold, color: color.fg.base },
  dappAccountLine: { fontSize: text.xs, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.muted, paddingLeft: space.sm },

  intent: { alignItems: 'center', paddingTop: space.lg, paddingBottom: space['2xl'] },
  intentText: { fontSize: text['5xl'], fontWeight: '800' as const, fontFamily: 'Inter-Bold', letterSpacing: -1 },

  tokenCard: {
    flexDirection: 'row', alignItems: 'center', gap: space.xl,
    paddingVertical: space['2xl'], paddingHorizontal: space['2xl'],
    borderRadius: radius['2xl'], marginVertical: space.sm,
  },
  tokenInfo: { flex: 1 },
  tokenAmt: { fontSize: text['3xl'], fontWeight: '800' as const, fontFamily: 'Inter-Bold', color: color.fg.base, letterSpacing: -0.5 },
  tokenLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.muted, marginTop: space.xs },

  flowArrow: { alignItems: 'center', marginVertical: -space.sm, zIndex: 1 },
  flowCircle: {
    width: 30, height: 30, borderRadius: 15,
    backgroundColor: color.bg.raised, borderWidth: 2, borderColor: color.border.base,
    alignItems: 'center', justifyContent: 'center', ...shadow.sm,
  },

  contractBar: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    paddingVertical: space.lg, paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken, borderRadius: radius.xl, marginVertical: space.md,
  },
  contractInfo: { flex: 1, gap: 2 },
  contractLabel: {
    fontSize: 10, ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase' as const, letterSpacing: 0.3,
  },
  contractAddr: { fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.muted },
  copyBtn: {
    width: 28, height: 28, borderRadius: radius.md,
    borderWidth: 1, borderColor: color.border.base, backgroundColor: color.bg.raised,
    alignItems: 'center', justifyContent: 'center',
  },
  copyBtnDone: {
    borderColor: color.success.base,
    backgroundColor: color.success.soft,
  },

  genRow: {
    flexDirection: 'row', justifyContent: 'space-between', alignItems: 'flex-start',
    paddingVertical: space.lg, paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken, borderRadius: radius.lg,
    marginVertical: space.xs, gap: space.lg,
  },
  genLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  genValue: {
    fontSize: text.sm, ...inter.semibold, color: color.fg.base,
    textAlign: 'right', flex: 1, fontFamily: font.mono, fontWeight: '500' as const,
  },

  warnDanger: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    paddingVertical: space.lg, paddingHorizontal: space.xl,
    backgroundColor: color.error.soft, borderWidth: 1, borderColor: color.error.base,
    borderRadius: radius.xl, marginVertical: space.md,
  },
  warnCaution: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    paddingVertical: space.lg, paddingHorizontal: space.xl,
    backgroundColor: color.warning.soft, borderWidth: 1, borderColor: color.warning.border,
    borderRadius: radius.xl, marginVertical: space.md,
  },
  warnDangerText: { fontSize: text.sm, ...inter.semibold, color: color.error.base, flex: 1, lineHeight: 18 },

  msgBubble: {
    backgroundColor: color.bg.sunken, borderRadius: radius['2xl'],
    padding: space['2xl'], marginVertical: space.md,
  },
  msgTag: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm, alignSelf: 'center',
    paddingVertical: space.xs, paddingHorizontal: space.lg,
    backgroundColor: color.border.base, borderRadius: radius.full, marginBottom: space.xl,
  },
  msgTagText: {
    fontSize: 10, ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase' as const, letterSpacing: 0.3,
  },
  msgText: { fontSize: text.base, ...inter.regular, color: color.fg.base, lineHeight: 22, textAlign: 'center' },

  detailsToggle: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'center',
    gap: space.sm, paddingVertical: space.md,
  },
  detailsToggleText: { fontSize: text.xs, ...inter.semibold, color: color.fg.subtle },
  rawBlock: {
    backgroundColor: color.bg.sunken, borderRadius: radius.lg,
    padding: space.lg, maxHeight: 80, marginBottom: space.lg,
  },
  rawText: { fontSize: 9, fontFamily: font.mono, fontWeight: '400' as const, color: color.fg.subtle, lineHeight: 14 },

  btns: { flexDirection: 'row', gap: space.lg, paddingTop: space.xl, borderTopWidth: 1, borderTopColor: color.border.base, marginTop: space.sm },
  btnFlex: { flex: 1 },
}));
